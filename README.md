# LiteCowork

LiteCowork is a durable execution workspace around replaceable external agents. It
keeps user intent, Task state, artifacts, effects, evidence, and execution ownership
durable while allowing agents, capabilities, Runtimes, and Environments to change.

[`ARCHITECTURE.md`](ARCHITECTURE.md) is the current HLD and ownership authority. The
complete architecture document map is in [`docs/COVERAGE-MATRIX.md`](docs/COVERAGE-MATRIX.md);
the product/UX contract is in [`docs/PRODUCT.md`](docs/PRODUCT.md) and
[`docs/EXPERIENCE.md`](docs/EXPERIENCE.md). Decision rationale lives in
[`docs/adr/`](docs/adr/).

## Current phase

LiteCowork is in architecture and contract definition. The predecessor implementation
is abandoned and is not a source of current requirements. Historical code and documents
belong in local, git-ignored archive folders; the new implementation starts from the
contracts in this repository.

## Product shape

One Conversation supports both ordinary questions and durable work. A durable outcome
is represented by a Task that survives agent-session, process, device, and Environment
failure. External agents own reasoning; LiteCowork coordinates bounded Attempts,
permissions, artifacts, Effects, evidence, Runtime ownership, and verification.

LitePSM is the selected external package/capability ecosystem. Its configured service
base URL is recorded in `docs/CAPABILITY-FABRIC.md`; its API and package contract are
intentionally deferred to the LitePSM authority.
