# Desktop Presentation Runtime

This directory contains the first typed, read-only renderer layer for
`docs/PRESENTATION-RUNTIME.md`.

- `presentation-types.ts` validates bounded untrusted projection values before UI use.
- `PresentationRuntime.tsx` orders valid items by their stable source order and renders
  built-in safe fallbacks. React text rendering is used throughout; payloads are never
  interpreted as HTML or executable content.
- `presentation-stream.ts` is a pure reducer for ready/snapshot/upsert/resync and bounded
  transient-turn frames. It drops duplicate/out-of-order frames and discards partial text
  after an unreplayable sequence gap.
- `presentation-runtime.css` provides responsive, keyboard-visible, reduced-motion
  styles.

The component intentionally has no API subscription or domain mutation of its own. The
authenticated Operator projection/stream and source-owning routes must be connected by
the caller. Approval and UserRequest items point users to their owning route; this
renderer cannot resolve them. Artifact and Task callbacks are navigation only.

`TaskPresentationPanel` currently re-fetches the authenticated finite Task snapshot every
five seconds while its panel intersects the viewport and the desktop document is visible.
It stops polling when hidden or unmounted, serializes snapshot requests, and ignores
results after the selected Task changes. This is a snapshot refresh fallback, not a live
stream: it does not synthesize activity or progress, and source freshness remains whatever
the snapshot endpoint reports.

This is not yet a complete Presentation Runtime integration: authenticated stream
transport/projection wiring, real Artifact preview/editors, browser/terminal control leases,
MCP App sandboxing, and end-to-end rendering are separate implementation stories.
