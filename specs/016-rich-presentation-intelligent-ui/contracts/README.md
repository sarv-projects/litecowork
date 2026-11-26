# Rich Presentation and Intelligent UI Contracts

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation.

These documents bind Feature 016 to the global owners without duplicating them.

- [API and event contract](api-events.md)
- [Errors and recovery contract](errors-recovery.md)
- [UI behavior: rich answers, chart, calculator, media and interactive preview](ui-behavior.md) (behavior, not pixels)
- Canonical owners: `docs/RICH-RESPONSE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/RENDERING-CONFORMANCE-LLD.md`, `docs/schemas/rich-presentation.schema.json`.

Names marked "accepted target" are PROPOSED in this design and become canonical only when TASKS T02 promotes them.

Revision 0.4 records the user's direction to keep Saved Views in V1 and amend Feature 007 B-21. The detailed non-executable Artifact contract is in Feature 007 `contracts/saved-view.md`; cross-owner review and OpenAPI promotion remain pending. B-40 remains unchanged for the component: calculator input is transient and the component cannot submit or persist it. A separate explicit Library save may preserve validated numeric values. Device-local Remember/Forget remains separate under T22. Under A-84, a host-owned Ask a follow-up action may prepare an ordinary composer draft; input values are omitted unless the user selects them, reviews the exact readable text, and presses the existing Send control. A-85 keeps model-visible MCP proposal input unavailable until its schema and host sequence/replay mapping are resolved. No standalone SavedView store or permanent Saved Apps UI exists.
