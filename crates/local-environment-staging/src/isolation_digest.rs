//! Canonical digests that bind an Environment admission request to observed evidence.
//!
//! The Runtime and the provider must derive the same digest from the same logical
//! content, so every helper here is deterministic and independent of ordering, of
//! absolute filesystem paths, and of secret material. Nothing in this module returns
//! or embeds a filesystem path: attestation payloads carry digests only.

use crate::PinnedInput;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Digest an ordered list of labels. Duplicates are removed and the labels are sorted
/// so that two equivalent requests produce the same digest.
pub fn digest_of_labels<I, S>(labels: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut unique = BTreeSet::new();
    for label in labels {
        unique.insert(label.as_ref().to_owned());
    }
    let mut hasher = Sha256::new();
    for label in unique {
        hasher.update(label.as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Digest a set of root names.
///
/// Root names are relative, logical names such as `inputs` or `outputs`; absolute host
/// paths are never accepted here.
pub fn digest_root_set(entries: &[&str]) -> String {
    let mut normalized = BTreeSet::new();
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        normalized.insert(trimmed.trim_matches('/').to_owned());
    }
    digest_of_labels(normalized)
}

/// Digest the exact pinned input set.
///
/// The digest binds the Workspace, Resource, revision, content digest, and staged
/// relative path of every pinned input, so a changed pin set cannot reuse an
/// attestation produced for a different one.
pub fn digest_pinned_inputs(inputs: &[PinnedInput<'_>]) -> String {
    let mut labels = Vec::with_capacity(inputs.len());
    for input in inputs {
        labels.push(format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            input.workspace_id,
            input.resource_id,
            input.revision_id,
            input.content_digest,
            input.relative_path
        ));
    }
    digest_of_labels(labels)
}

/// Digest a JSON value using a canonical form: object keys are sorted, arrays keep
/// their order, and no insignificant whitespace is emitted.
pub fn digest_of_canonical_json(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hash_canonical(value, &mut hasher);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn hash_canonical(value: &Value, hasher: &mut Sha256) {
    match value {
        Value::Null => hasher.update(b"null"),
        Value::Bool(true) => hasher.update(b"true"),
        Value::Bool(false) => hasher.update(b"false"),
        Value::Number(number) => {
            hasher.update(b"n");
            hasher.update(number.to_string().as_bytes());
        }
        Value::String(text) => {
            hasher.update(b"s");
            hasher.update(text.as_bytes());
        }
        Value::Array(items) => {
            hasher.update(b"[");
            for item in items {
                hash_canonical(item, hasher);
                hasher.update(b",");
            }
            hasher.update(b"]");
        }
        Value::Object(entries) => {
            hasher.update(b"{");
            let mut keys = entries.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                hasher.update(b"k");
                hasher.update(key.as_bytes());
                hasher.update(b"v");
                hash_canonical(&entries[key], hasher);
                hasher.update(b",");
            }
            hasher.update(b"}");
        }
    }
}
