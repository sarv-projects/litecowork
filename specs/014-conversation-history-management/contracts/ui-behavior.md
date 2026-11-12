# UI behavior - Sidebar search, row actions, header actions and delete dialog (Feature 014)

**Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification. Binds UI to owner contracts; not pixels.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md) left sidebar section.
**Wire:** [api-events.md](api-events.md).

The UI renders Conversation pages, search results and deletion status, and sends typed commands. It never decides fences or versions and never parses English error strings.

## 1. Screens and controls

| Surface | Control | Command | Today |
|---|---|---|---|
| Sidebar | Search field (label "Search conversations") | `search_conversations` | absent |
| Sidebar | Active / Archived filter | `list_conversations` with status | absent |
| Sidebar | Recent Chats rows | `list_conversations` | present in `App.tsx`; selected row shows literal "now" |
| Sidebar row | Row menu: Rename, Archive or Unarchive, Delete | `rename_conversation`, `archive_conversation`, `unarchive_conversation`, `get_conversation_deletion_impact`, `delete_conversation` | absent |
| Conversation header | Title | `conversationTitle(selected?.title)` | `Untitled conversation` when no title is committed |
| Conversation header | More options: Rename, Archive, Delete | same commands | explicitly disabled and labelled unavailable; no handler |
| Delete dialog | Impact summary, Delete, Cancel | `delete_conversation` | absent |
| Deleted view | "This conversation was deleted" | `get_conversation_deletion_status` | absent |

## 2. States

| Surface | States | Meaning |
|---|---|---|
| Search | `IDLE`, `LOADING`, `RESULTS`, `NO_RESULTS`, `INDEX_BUILDING`, `ERROR`, `OFFLINE`, `DENIED`, `UNAVAILABLE` | `NO_RESULTS` only when the index is READY; `UNAVAILABLE` when the route is not served |
| Row action | `ENABLED`, `DISABLED(reason)`, `PENDING` | Disabled while a turn is active, with "A response is in progress" |
| Rename | `VIEW`, `EDITING`, `SAVING`, `CONFLICT`, `INVALID` | Conflict refreshes the row and keeps the typed title |
| Delete | `IMPACT_LOADING`, `CONFIRMING`, `BLOCKED(blockers)`, `DELETING`, `DELETED`, `FAILED` | `DELETING` returns after 202; purge continues in the background |

## 3. Transitions

1. Type in search -> 250 ms debounce -> `LOADING` -> `RESULTS`/`NO_RESULTS`/`INDEX_BUILDING`; a newer query aborts the older request.
2. Escape in search clears the query and restores the Recent Chats list.
3. Rename -> inline field with current title, Enter saves, Escape cancels, focus returns to the row.
4. Archive -> row leaves Active list with a polite announcement; it appears under Archived; Unarchive reverses.
5. Delete -> impact loads -> dialog lists linked Tasks, Artifacts and memory sources that will be kept, says it deletes LiteCowork-owned local history only (native-harness transcripts and external-channel copies are not erased), and states deletion cannot be undone and older backups are not changed -> Delete -> row removed -> open view shows deleted state.
6. App restart -> lists reload; deleted rows absent; pending purges are invisible to the user beyond the deletion status route.

## 4. Rules

- No inert control: an unserved action is hidden or disabled with an "unavailable" accessible name.
- Titles render as plain text; "Untitled conversation" replaces missing titles; no fabricated title.
- Row time comes from persisted data, never the literal "now".
- The list is virtualized or count-capped.
- Keyboard: search field, filter, rows and row menus reachable by Tab and arrow keys; the dialog traps focus, Escape cancels, focus returns to the invoker; status via `role="status"`, failures via `role="alert"`.
- Search text and titles are never sent to logs or telemetry.

## 5. Cross-layer trace (completed by 014-T16)

| Operation | Control | Tauri command | Route | Authorization | Transaction | Event | Restart | Test |
|---|---|---|---|---|---|---|---|---|
| Search | Sidebar search field | `search_conversations` | `GET /v1/conversations/search` | owner + Workspace | read | none | stateless | to be added |
| Rename | Row menu / header | `rename_conversation` | `PATCH /v1/conversations/{conversation_id}` | owner, If-Match | tx-rename | `conversation.renamed.v1` | durable | to be added |
| Archive / Unarchive | Row menu / header | `archive_conversation` / `unarchive_conversation` | archive / unarchive | owner, If-Match | tx-archive | archived / unarchived | durable | to be added |
| Delete | Row menu / header, dialog | `delete_conversation` | `DELETE /v1/conversations/{conversation_id}` | owner, If-Match | tx-request-deletion, reconciler | deletion_requested, deleted | reconciler resumes | to be added |
