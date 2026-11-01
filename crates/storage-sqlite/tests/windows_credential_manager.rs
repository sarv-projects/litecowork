#![cfg(windows)]

//! Opt-in, native Windows qualification for the production workspace blob-key provider.
//!
//! Run only on a disposable test account/session with:
//! `LITECOWORK_RUN_WINDOWS_CREDENTIAL_TEST=1 cargo test --locked -p storage-sqlite --test windows_credential_manager -- --ignored --nocapture`

use keyring::Entry;
use sha2::{Digest, Sha256};
use storage_core::BlobPurpose;
use storage_sqlite::{OsWorkspaceBlobKeyProvider, WorkspaceBlobKeyProvider};
use zeroize::Zeroize;

const SERVICE: &str = "com.litecowork.workspace-blob-key";

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
            let entry = Entry::new(SERVICE, &account).map_err(|_| ())?;
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
        // Delete only account names recorded after this test successfully caused their
        // creation. Never enumerate Credential Manager or delete by service/scope wildcard.
        let _ = self.cleanup();
    }
}

fn uuid_v4_scope() -> String {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("OS random number generator is available");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "windows-credential-qualification-{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
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
    let entry = Entry::new(SERVICE, account).expect("construct exact Credential Manager entry");
    match entry.get_password() {
        Err(keyring::Error::NoEntry) => {}
        Ok(mut password) => {
            password.zeroize();
            panic!("random test credential unexpectedly already exists");
        }
        Err(error) => panic!("cannot safely establish credential absence: {error}"),
    }
}

fn exists(account: &str) -> bool {
    let entry = Entry::new(SERVICE, account).expect("construct exact Credential Manager entry");
    match entry.get_password() {
        Ok(mut password) => {
            password.zeroize();
            true
        }
        Err(keyring::Error::NoEntry) => false,
        Err(error) => panic!("cannot safely inspect exact test credential: {error}"),
    }
}

#[test]
#[ignore = "writes uniquely scoped entries to the current user's Windows Credential Manager"]
fn native_credential_manager_provisions_rotates_and_reopens_workspace_keys() {
    assert_eq!(
        std::env::var("LITECOWORK_RUN_WINDOWS_CREDENTIAL_TEST").as_deref(),
        Ok("1"),
        "set the explicit opt-in variable to run this native Credential Manager test"
    );

    let scope = uuid_v4_scope();
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
            panic!("Credential Manager provisions the first key: {error}");
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
            panic!("Credential Manager persists the next key and active pointer: {error}");
        }
    };
    created.track_created(key_two_account);
    assert_eq!(second.version, 2);

    // Reconstruct the provider to exercise persisted Credential Manager state rather
    // than relying on an in-memory test double or provider-local cache.
    let reopened = OsWorkspaceBlobKeyProvider;
    let active = reopened
        .current_key(&scope, BlobPurpose::Resource)
        .expect("active key survives provider reconstruction");
    let retained = reopened
        .key_by_version(&scope, BlobPurpose::Resource, 1)
        .expect("prior key remains available after rotation");
    assert_eq!(active.version, 2);
    assert!(
        active.bytes.as_ref() == second.bytes.as_ref(),
        "active key changed across provider reconstruction"
    );
    assert!(
        retained.bytes.as_ref() == first.bytes.as_ref(),
        "retained key changed across rotation"
    );

    created
        .cleanup()
        .expect("test-created credentials were deleted by exact account name");
    drop(created);

    // The test's own exact entries are removed; verify that cleanup succeeded without
    // probing or touching any other Credential Manager record.
    for account in [
        &account_for(&scope, "key-1"),
        &account_for(&scope, "key-2"),
        &active_account,
    ] {
        assert_absent(account);
    }
}
