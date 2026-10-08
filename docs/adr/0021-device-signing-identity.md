# ADR-0021: Ed25519 Device Signing Identity

- **Status:** Accepted for vNext local Runtime identity
- **Date:** 2026-10-08

## Context

Runtime Mesh requires a stable device identity and authenticated Runtime registration.
The canonical `DeviceIdentity` shape previously named a public key but left the
signature algorithm and key encoding unspecified. Local startup now persists the
installation-scoped Runtime and each lock-holder incarnation, so the device identity
must be real, stable, and kept separate from Workspace blob encryption keys.

## Decision

- Use Ed25519 for device signing keys.
- Encode public keys as `ed25519:` followed by lowercase hex for the raw 32-byte key.
- Derive `DeviceId` as `ed25519-sha256:` followed by lowercase hex SHA-256 of those raw
  public-key bytes.
- Start at key version 1. Key rotation remains unsupported until an authenticated
  rotation/recovery protocol is specified; missing or mismatched private material fails
  closed rather than silently creating a replacement identity.
- Store the private seed only in a dedicated OS credential-store entry scoped to the
  local data-directory identity. It is not stored with Workspace blob keys, SQLite,
  bootstrap JSON, logs, events, replication, or backups.
- Treat the credential-store secret as OS-protected but potentially exportable software
  key material. This decision does not claim a hardware-backed or non-exportable key.
- Persist only the public `DeviceIdentity` in the local Runtime descriptor. Local
  persistence does not authenticate Mesh pairing, publish presence, or enroll a
  Workspace.

## Consequences

The implementation uses the platform keyring adapter and an Ed25519 implementation. A
credential-store failure prevents Runtime registration and Operator readiness. Moving a
data directory to a different local identity scope requires an explicit identity-recovery
flow; this implementation does not copy private keys into files to make moves work.
Platform-specific credential-store behavior and protected-at-rest assumptions require
separate qualification; the keyring abstraction alone does not prove hardware protection.
The current implementation enables `ed25519-dalek`'s `zeroize` Cargo feature. Its
`SigningKey` drop implementation zeroizes its private scalar under that feature; the
decoded seed is independently held in `zeroize::Zeroizing` and the key object is scoped
only to public-key derivation. Removing or changing that feature requires requalification
of secret-memory handling.

## Alternatives considered

- **Leave the signature algorithm unspecified:** rejected because peers cannot validate
  a signature without an algorithm and encoding contract.
- **Reuse Workspace blob keys:** rejected because encryption and device authentication
  have different scope, rotation, and exposure requirements.
- **Store the private key in SQLite or `runtime-state.json`:** rejected because those
  files are ordinary durable state and may be backed up or inspected independently of
  the OS credential store.
