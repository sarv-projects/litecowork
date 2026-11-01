# Research - Golden Connection and One Capability Write

**Feature:** 006 | **Checked:** 2026-11-01

## Sources

Authorities: docs/CONNECTORS-WORKFLOWS.md, docs/CAPABILITY-FABRIC.md, docs/NETWORK-SECURITY.md, docs/LITESPM-LITECOWORK-INTEROP-LLD.md. Reject embedding third-party automation runtimes. Checked 2026-11-01.

## Findings

- The owner documents already decide the trust boundaries this slice must keep.
- Hosted competitors often collapse connection, permission and execution into one toggle. LiteCowork forbids that collapse.
- Reference desktop repositories are architecture comparisons, not code to vendor. Pins and paths: [REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md), [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md).

## Alternatives

- Wait for full multi-provider parity before any slice. Rejected: the release contract asks for one qualified flow per area.
- Generate a complete schema inside this package. Rejected: machine contracts live in `docs/schemas/` and the owner doc.

## Limitations

- Rival web pages were not re-fetched on 2026-11-01 for this package; use [COMPETITIVE-RESEARCH.md](../../docs/COMPETITIVE-RESEARCH.md) and refresh URLs before lock.
- No runtime was executed.

## Unresolved questions

See SPEC section 5 blockers. Owners are named on the package README.
