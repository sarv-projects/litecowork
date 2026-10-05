# LiteCowork

> **In Progress**

LiteCowork is a durable execution workspace around replaceable external agents. It
keeps user intent, Task state, artifacts, effects, evidence, and execution ownership
durable while allowing agents, capabilities, Runtimes, and Environments to change.

[`ARCHITECTURE.md`](ARCHITECTURE.md) is the current HLD and ownership authority. The
complete architecture document map is in [`docs/COVERAGE-MATRIX.md`](docs/COVERAGE-MATRIX.md);
the product/UX contract is in [`docs/PRODUCT.md`](docs/PRODUCT.md) and
[`docs/EXPERIENCE.md`](docs/EXPERIENCE.md). Decision rationale lives in
[`docs/adr/`](docs/adr/).

## Product shape

One Conversation supports both ordinary questions and durable work. A durable outcome
is represented by a Task that survives agent-session, process, device, and Environment
failure. External agents own reasoning; LiteCowork coordinates bounded Attempts,
permissions, artifacts, Effects, evidence, Runtime ownership, and verification.

LitePSM is the selected external package/capability ecosystem. Its configured service
base URL is recorded in `docs/CAPABILITY-FABRIC.md`; its API and package contract are
intentionally deferred to the LitePSM authority.

## Runtime and reusable work

The desktop Operator and headless `litecoworkd` have independent lifetimes. Background
coordination, authorized resource observation and schedules run in the daemon; agents,
capability providers and Environments are activated only when work requires them.
See [Runtime lifecycle](docs/RUNTIME-LIFECYCLE.md).

A [Routine](docs/ROUTINES.md) defines reusable work; an
[Automation](docs/AUTOMATION.md) defines when it runs; a Task records one execution.
Trigger placement is separate from execution placement, so cloud schedules can wait
explicitly for local resources.
