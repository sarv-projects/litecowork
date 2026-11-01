# Desktop Presentation Runtime

This directory contains the first typed, read-only renderer layer for
`docs/PRESENTATION-RUNTIME.md`.

- `presentation-types.ts` validates bounded untrusted projection values before UI use.
  `parsePresentationItem` stays strict for IPC/stream boundaries; the renderer uses
  `projectPresentationEntries`, which turns a value it cannot validate into a bounded
  placeholder instead of dropping it.
- `PresentationRuntime.tsx` orders projected items by their stable source order and renders
  built-in safe fallbacks. Unsupported versions are labelled and malformed items state that
  they could not be displayed safely; only revalidated source identities are shown beside
  them. React text rendering is used throughout; payloads are never interpreted as HTML or
  executable content.
- `presentation-stream.ts` is a pure reducer for ready/snapshot/upsert/resync and bounded
  transient-turn frames. It drops duplicate/out-of-order frames and discards partial text
  after an unreplayable sequence gap.
- `task-presentation-items.ts` partitions an untrusted Task presentation snapshot into the
  outcome row, activity, saved outputs, other saved items, rejected items, and an omitted
  count. A saved item that fails validation becomes a visible placeholder rather than
  disappearing from the Task surface.
- `presentation-runtime.css` provides responsive, keyboard-visible, reduced-motion
  styles.

The component intentionally has no API subscription or domain mutation of its own. The
authenticated Operator projection/stream and source-owning routes must be connected by
the caller. Approval and UserRequest items point users to their owning route; this
renderer cannot resolve them. Artifact and Task callbacks are navigation only.

`TaskPresentationPanel` consumes the authenticated local Task snapshot stream and replaces
its projection only after a complete, closed snapshot passes Workspace/Task/session and
revision checks. It resumes from the last accepted opaque cursor and reconnects with bounded
backoff. The authenticated finite snapshot read still runs every five seconds while the
panel intersects the viewport and the desktop document is visible; it acts as a fallback
and never overwrites a newer stream snapshot. Progress is fetched separately and is not
claimed to arrive on the Task stream. Snapshot freshness remains whatever the source
projection reports.

This is not yet a complete Presentation Runtime integration: Runtime-inventory and
transient turn/rich-draft streams, real Artifact preview/editors, browser/terminal control
leases, MCP App sandboxing, and packaged end-to-end rendering remain separate work.
