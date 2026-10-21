#![cfg(target_os = "linux")]

//! Opt-in, native Linux Secret Service qualification for the production
//! workspace blob-key provider.
//!
//! Run only on a disposable test account/session with a running Secret Service:
//! `LITECOWORK_RUN_LINUX_SECRET_SERVICE_TEST=1 cargo test --locked -p storage-sqlite --test linux_secret_service -- --ignored --nocapture`
//!
//! `scripts/qualify-os-credential-store.sh` performs the environment probe and
//! runs exactly this test, recording the host and provider versions.
//!
//! The deterministic CODE tests inject a credential store and prove the
//! provider's rules. They do not prove that a real Secret Service accepts,
//! retains, and releases the same material. This qualification closes that gap
//! for the Linux platform without weakening any rule: a malformed active
//! pointer written into the live store must still fail closed.

use keyring::Entry;
use sha2::{Digest, Sha256};
use storage_core::BlobPurpose;
use storage_sqlite::{OsWorkspaceBlobKeyProvider, WorkspaceBlobKeyProvider};
use zeroize::Zeroize;

const SERVICE: &str = "com.litecowork.workspace-blob-key";
const OPT_IN_VARIABLE: &str = "LITECOWORK_RUN_LINUX_SECRET_SERVICE_TEST";

/// Builds the same persistent Secret Service credential the production
/// provider uses, so the qualification observes exactly the platform backend
/// under test rather than a different credential store.
fn secret_service_entry(account: &str) -> Result<Entry, keyring::Error> {
    let credential =
        keyring::secret_service::SsCredential::new_with_target(None, SERVICE, account)?;
    Ok(Entry::new_with_credential(Box::new(credential)))
}

struct CreatedCredentials {
    accounts: Vec<String>,
}

impl CreatedCredentials {
    fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    fn track_created(&mut self, account: String) {
        if !self.accounts.contains(&account) {
            self.accounts.push(account);
        }
    }

    fn cleanup(&mut self) -> Result<(), ()> {
        for account in self.accounts.clone() {
            let entry = secret_service_entry(&account).map_err(|_| ())?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {
                    self.accounts.retain(|tracked| tracked != &account);
                }
                Err(_) => return Err(()),
            }
        }
        Ok(())
    }
}

impl Drop for CreatedCredentials {
    fn drop(&mut self) {
        // Delete only account names recorded after this test successfully caused
        // their creation. Never enumerate the Secret Service collection and never
        // delete by service or scope wildcard.
        let _ = self.cleanup();
    }
}

fn qualification_scope() -> String {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("OS random number generator is available");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "linux-secret-service-qualification-{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn account_for(scope: &str, item: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"litecowork-os-blob-key-v1\0");
    digest.update(scope.as_bytes());
    digest.update([0]);
    digest.update(BlobPurpose::Resource.as_str().as_bytes());
    format!("{}:{item}", hex::encode(digest.finalize()))
}

fn assert_absent(account: &str) {
    let entry = secret_service_entry(account).expect("construct exact Secret Service entry");
    match entry.get_password() {
        Err(keyring::Error::NoEntry) => {}
        Ok(mut password) => {
            password.zeroize();
            panic!("the random test credential unexpectedly already exists");
        }
        Err(error) => panic!("cannot safely establish credential absence: {error}"),
    }
}

fn exists(account: &str) -> bool {
    let entry = secret_service_entry(account).expect("construct exact Secret Service entry");
    match entry.get_password() {
        Ok(mut password) => {
            password.zeroize();
            true
        }
        Err(keyring::Error::NoEntry) => false,
        Err(error) => panic!("cannot safely inspect the exact test credential: {error}"),
    }
}

#[test]
#[ignore = "writes uniquely scoped entries to the current user's Secret Service"]
fn native_secret_service_provisions_rotates_reopens_and_rejects_a_malformed_pointer() {
    assert_eq!(
        std::env::var(OPT_IN_VARIABLE).as_deref(),
        Ok("1"),
        "set the explicit opt-in variable to run this native Secret Service test"
    );

    let scope = qualification_scope();
    let key_one_account = account_for(&scope, "key-1");
    let key_two_account = account_for(&scope, "key-2");
    let active_account = account_for(&scope, "active");

    // Preflight every exact entry before invoking production code. A collision or an
    // unavailable provider aborts without overwriting or deleting anything.
    for account in [&key_one_account, &key_two_account, &active_account] {
        assert_absent(account);
    }

    let mut created = CreatedCredentials::new();
    let provider = OsWorkspaceBlobKeyProvider;
    let first = match provider.current_key(&scope, BlobPurpose::Resource) {
        Ok(key) => key,
        Err(error) => {
            for account in [&key_one_account, &active_account] {
                if exists(account) {
                    created.track_created(account.clone());
                }
            }
            panic!("the Secret Service provisions the first key version: {error}");
        }
    };
    created.track_created(key_one_account);
    created.track_created(active_account.clone());
    assert_eq!(first.version, 1);

    let second = match provider.rotate_key(&scope, BlobPurpose::Resource) {
        Ok(key) => key,
        Err(error) => {
            if exists(&key_two_account) {
                created.track_created(key_two_account.clone());
            }
            panic!("the Secret Service persists the next key and active pointer: {error}");
        }
    };
    created.track_created(key_two_account);
    assert_eq!(second.version, 2);
    assert!(
        second.bytes.as_ref() != first.bytes.as_ref(),
        "rotation must activate different key material"
    );

    // Reconstruct the provider to exercise persisted Secret Service state rather
    // than relying on an in-memory test double or provider-local cache.
    let reopened = OsWorkspaceBlobKeyProvider;
    let active = reopened
        .current_key(&scope, BlobPurpose::Resource)
        .expect("the active key survives provider reconstruction");
    let retained = reopened
        .key_by_version(&scope, BlobPurpose::Resource, 1)
        .expect("the prior key remains available after rotation");
    assert_eq!(active.version, 2);
    assert!(
        active.bytes.as_ref() == second.bytes.as_ref(),
        "the active key changed across provider reconstruction"
    );
    assert!(
        retained.bytes.as_ref() == first.bytes.as_ref(),
        "the retained key changed across rotation"
    );

    // A malformed active pointer written straight into the live Secret Service
    // must fail closed instead of activating an unknown key.
    secret_service_entry(&active_account)
        .expect("the Secret Service is reachable")
        .set_password("not-a-version-pointer")
        .expect("the qualification may write a malformed pointer");
    assert!(matches!(
        reopened.current_key(&scope, BlobPurpose::Resource),
        Err(storage_core::StoreError::Integrity(_))
    ));
    assert!(matches!(
        reopened.rotate_key(&scope, BlobPurpose::Resource),
        Err(storage_core::StoreError::Integrity(_))
    ));

    created
        .cleanup()
        .expect("the test-created credentials are deleted by exact account name");
    drop(created);

    // The test's own exact entries are removed; verify that cleanup succeeded
    // without probing or touching any other Secret Service item.
    for account in [
        &account_for(&scope, "key-1"),
        &account_for(&scope, "key-2"),
        &active_account,
    ] {
        assert_absent(account);
    }
}
