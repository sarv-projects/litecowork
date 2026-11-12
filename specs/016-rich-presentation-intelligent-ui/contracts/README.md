# Rich Presentation and Intelligent UI Contracts

**Status:** IN_REVIEW - authored for owner review; does not authorize implementation.

These documents bind Feature 016 to the global owners without duplicating them.

- [API and event contract](api-events.md)
- [Errors and recovery contract](errors-recovery.md)
- [UI behavior: rich answers, chart, calculator, media and interactive preview](ui-behavior.md) (behavior, not pixels)
- Canonical owners: `docs/RICH-RESPONSE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/RENDERING-CONFORMANCE-LLD.md`, `docs/schemas/rich-presentation.schema.json`.

Names marked "accepted target" are PROPOSED in this design and become canonical only when TASKS T02 promotes them.

Revision 0.2 adds [pipeline LLD](../../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) targets for qualified native proposal ingress, shared Canvas controller, explicit local-state consent and new-turn conversational modification. API/error/UI additions are review proposals, not served contract claims. Existing canonical schemas remain sole machine authorities; T20/T22 promote changes before implementation. B-40's no-persistence/no-agent-sharing baseline is unchanged.
