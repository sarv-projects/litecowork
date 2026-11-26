//! Pure validation for immutable, message-bound RichPresentation documents.
//!
//! This crate deliberately has no storage, provider, UI, or host-authority dependency. It
//! validates only a closed declarative document against the exact committed semantic text.

mod blocks;
mod calc;

#[cfg(test)]
mod tests;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

pub const MAX_DOCUMENT_BYTES: usize = 512 * 1024;
pub const MAX_SEMANTIC_BYTES: usize = 1024 * 1024;
const MAX_BLOCKS: usize = 200;
const MAX_JSON_VALUES: usize = 30_000;
const MAX_JSON_DEPTH: usize = 32;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// One exact, simple GFM pipe table discovered in committed semantic text.
/// Byte offsets are UTF-8 boundaries into the original, unnormalized string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticMarkdownTable {
    pub start_utf8_byte: usize,
    pub end_utf8_byte_exclusive: usize,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Extract only the bounded pipe-table subset already understood by the canonical
/// validator. This is a presentation convenience, never an authority or Markdown
/// renderer; every produced TABLE is still revalidated against the exact committed
/// message before publication.
pub fn extract_semantic_markdown_tables(semantic_text: &str) -> Vec<SemanticMarkdownTable> {
    blocks::extract_semantic_markdown_tables(semantic_text)
}

/// Exact identity and source values bound by the storage owner.
#[derive(Clone, Copy, Debug)]
pub struct ValidationContext<'a> {
    pub schema_version: u32,
    pub renderer_contract_version: u32,
    pub presentation_id: &'a str,
    pub message_id: &'a str,
    pub semantic_content_digest: &'a str,
    pub document_digest: &'a str,
    pub document_size_bytes: u64,
    pub semantic_text: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    Invalid(&'static str),
    Integrity(&'static str),
    UnsupportedBlock,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Integrity(message) => f.write_str(message),
            Self::UnsupportedBlock => f.write_str("unsupported presentation block"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate the canonical immutable document and all source/provenance bindings.
pub fn validate_document(
    context: ValidationContext<'_>,
    bytes: &[u8],
) -> Result<(), ValidationError> {
    if context.schema_version != 1
        || !matches!(context.renderer_contract_version, 1 | 2)
        || context.presentation_id.trim().is_empty()
        || context.presentation_id.len() > 256
        || context.presentation_id.chars().any(char::is_control)
        || context.message_id.trim().is_empty()
        || context.message_id.len() > 256
        || context.message_id.chars().any(char::is_control)
        || !valid_digest(context.semantic_content_digest)
    {
        return Err(ValidationError::Invalid(
            "unsupported presentation identity/version",
        ));
    }
    if bytes.is_empty()
        || bytes.len() > MAX_DOCUMENT_BYTES
        || bytes.len() as u64 != context.document_size_bytes
        || digest(bytes) != context.document_digest
    {
        return Err(ValidationError::Integrity(
            "presentation document digest/length mismatch",
        ));
    }
    if context.semantic_text.len() > MAX_SEMANTIC_BYTES {
        return Err(ValidationError::Invalid(
            "semantic text exceeds presentation limit",
        ));
    }

    let document: Value = serde_json::from_slice(bytes)
        .map_err(|_| ValidationError::Invalid("invalid presentation JSON"))?;
    check_json_budget(&document)?;
    let canonical = serde_json_canonicalizer::to_vec(&document)
        .map_err(|_| ValidationError::Invalid("presentation is not canonical JSON"))?;
    if canonical != bytes {
        return Err(ValidationError::Invalid(
            "presentation bytes must use RFC 8785 canonical JSON",
        ));
    }

    blocks::closed_object(
        &document,
        &[
            "schema_version",
            "renderer_contract_version",
            "presentation_id",
            "message_id",
            "semantic_content_digest",
            "root_blocks",
            "block_provenance",
        ],
        &["citations", "actions", "accessibility_summary"],
    )?;
    if document["schema_version"] != 1
        || document["renderer_contract_version"] != context.renderer_contract_version
        || document["presentation_id"] != context.presentation_id
        || document["message_id"] != context.message_id
        || document["semantic_content_digest"] != context.semantic_content_digest
    {
        return Err(ValidationError::Invalid(
            "presentation document binding/version mismatch",
        ));
    }

    for key in ["citations", "actions"] {
        if let Some(value) = document.get(key)
            && !value.as_array().is_some_and(Vec::is_empty)
        {
            return Err(ValidationError::Invalid(
                "presentation source/action binders unavailable",
            ));
        }
    }
    if let Some(summary) = document.get("accessibility_summary") {
        let summary = summary
            .as_str()
            .ok_or(ValidationError::Invalid("invalid accessibility summary"))?;
        if !blocks::within_code_units(summary, 4_000)
            || !blocks::semantic_line_matches(context.semantic_text, summary)
        {
            return Err(ValidationError::Invalid(
                "accessibility summary must repeat a semantic line",
            ));
        }
    }

    let roots = document["root_blocks"]
        .as_array()
        .ok_or(ValidationError::Invalid("root blocks array required"))?;
    if roots.len() > MAX_BLOCKS {
        return Err(ValidationError::Invalid(
            "presentation block limit exceeded",
        ));
    }
    let mut paths = Vec::new();
    for (index, block) in roots.iter().enumerate() {
        blocks::validate_block(
            block,
            &format!("/{index}"),
            context.semantic_text,
            context.renderer_contract_version,
            0,
            &mut paths,
        )?;
    }
    validate_provenance(&document, &paths)
}

fn validate_provenance(document: &Value, paths: &[String]) -> Result<(), ValidationError> {
    let entries = document["block_provenance"]
        .as_array()
        .ok_or(ValidationError::Invalid("block provenance array required"))?;
    if entries.len() > MAX_BLOCKS || entries.len() != paths.len() {
        return Err(ValidationError::Invalid("invalid block provenance count"));
    }
    let mut seen = Vec::with_capacity(entries.len());
    for entry in entries {
        blocks::closed_object(
            entry,
            &[
                "block_path",
                "origin",
                "resource_refs",
                "artifact_refs",
                "evidence_refs",
                "verification_refs",
            ],
            &[],
        )?;
        let path = entry["block_path"]
            .as_str()
            .ok_or(ValidationError::Invalid("invalid block provenance path"))?;
        if !paths.iter().any(|expected| expected == path)
            || seen.iter().any(|previous| *previous == path)
            || !matches!(
                entry["origin"].as_str(),
                Some("SEMANTIC_MESSAGE" | "MODEL_INTENT")
            )
        {
            return Err(ValidationError::Invalid(
                "invalid/duplicate/untrusted block provenance",
            ));
        }
        for key in [
            "resource_refs",
            "artifact_refs",
            "evidence_refs",
            "verification_refs",
        ] {
            if !entry[key].as_array().is_some_and(Vec::is_empty) {
                return Err(ValidationError::Invalid(
                    "source-bearing provenance needs a qualified binder",
                ));
            }
        }
        seen.push(path);
    }
    Ok(())
}

fn check_json_budget(root: &Value) -> Result<(), ValidationError> {
    let mut pending = vec![(root, 0usize)];
    let mut count = 0usize;
    while let Some((value, depth)) = pending.pop() {
        count += 1;
        if count > MAX_JSON_VALUES || depth > MAX_JSON_DEPTH {
            return Err(ValidationError::Invalid(
                "presentation JSON complexity limit exceeded",
            ));
        }
        match value {
            Value::Number(number) => {
                let numeric = number.as_f64().filter(|value| value.is_finite()).ok_or(
                    ValidationError::Invalid("presentation number is not finite"),
                )?;
                if numeric.fract() == 0.0 && numeric.abs() > MAX_SAFE_INTEGER as f64 {
                    return Err(ValidationError::Invalid(
                        "integer exceeds the exact RFC 8785/I-JSON range",
                    ));
                }
            }
            Value::Array(items) => {
                pending.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(fields) => {
                pending.extend(fields.values().map(|item| (item, depth + 1)));
            }
            Value::Null | Value::Bool(_) | Value::String(_) => {}
        }
    }
    Ok(())
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(7 + digest.len() * 2);
    result.push_str("sha256:");
    for byte in digest {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
