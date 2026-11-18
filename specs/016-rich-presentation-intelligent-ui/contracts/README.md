# Rich Presentation and Intelligent UI Contracts

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation.

These documents bind Feature 016 to the global owners without duplicating them.

- [API and event contract](api-events.md)
- [Errors and recovery contract](errors-recovery.md)
- [UI behavior: rich answers, chart, calculator, media and interactive preview](ui-behavior.md) (behavior, not pixels)
- Canonical owners: `docs/RICH-RESPONSE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/RENDERING-CONFORMANCE-LLD.md`, `docs/schemas/rich-presentation.schema.json`.

Names marked "accepted target" are PROPOSED in this design and become canonical only when TASKS T02 promotes them.

Revision 0.4 records the owner's direction to keep Saved Views in V1 and amend Feature 007 B-21. The detailed non-executable Artifact contract is in Feature 007 `contracts/saved-view.md`; its cross-owner review and OpenAPI promotion remain pending. B-40 remains unchanged: Saved Views preserve declared defaults, not session-modified calculator values. Remember/Forget state and explicit input sharing remain separate proposals under B-46/T22/T23. A-85 keeps model-visible MCP proposal input unavailable until its schema and host sequence/replay mapping are resolved. No standalone SavedView store or permanent Saved Apps UI exists.
