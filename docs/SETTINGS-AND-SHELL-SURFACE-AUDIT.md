# Settings and Shell Surface Audit

**Status:** normative crosswalk — docs coverage for Settings UI and main shell  
**Updated:** 2026-11-01  
**Purpose:** verify LiteCowork docs specify **every Settings control** and **every primary shell surface** (tabs, buttons, routes) with behavior—not pixels—and wire them to authority.

## 1. Settings — control inventory

| Source | Coverage |
|---|---|
| [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md) § Full Settings controls | **30 groups**, operation names (e.g. `createWorkspace`, `installAgent`) |
| [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md) | **187 named controls** → authority, scope class, failure path, owner docs |
| [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) | Shell/global actions outside the 187 (titlebar, left rail partial) |
| [UI-CORE-TYPED-CONTRACTS.md](UI-CORE-TYPED-CONTRACTS.md) | Transport: OpenAPI + IPC; no string errors |

**Verdict:** Settings **functions/features/buttons are documented** at the contract level. Following the deep inspection of `repo_take/zcode`, the settings UI was promoted from a bare stub into a full 9-category desktop settings workspace (`apps/litecowork-ui/src/settings/SettingsView.tsx` and `settings-view.css`), directly mapped to the 187 ledger controls. Controls with unserved daemon routes visibly disclose their truthful status, while active workspace, instruction revisions, runtime lifecycle, and agent harness controls are fully functional.

### Settings implementation structure
1. **General & Desktop:** Theme (light/dark/system), density, text scaling, reduced motion, global keyboard shortcuts.
2. **Workspaces & Roots:** Active workspace inspector, create workspace form, replication policy, immutable instructions editor with byte limit, archive lifecycle guard.
3. **Agent Harnesses & Models:** Integrated `AgentCatalogSettings` with live socket probes, default lead assignment, and binding controls.
4. **Browser Use & Web Tools:** Built-in browser control toggle, mandatory TLS certificate verification status, and separate cache vs. full cookie/data cleanup actions.
5. **Terminal & Shell:** Shell engine selection (pwsh, cmd, bash), workspace root directory resolution, Windows Job Object isolation status, buffer limit.
6. **Permissions & Security:** Two-stage consent policy (always ask vs. project commands), filesystem write guards, Windows Credential Manager integration.
7. **Local Runtime & Diagnostics:** Daemon state, Runtime & Incarnation IDs, startup policy, blocker list with resolution hints, start/refresh actions.
8. **Skills & MCP (LiteSPM):** Package manager service bridge status, progressive capability discovery, human-in-the-loop review guards.
9. **Coworker Defaults & Memory:** Default coworker persona assignment, workspace-isolated memory retention.

### Gaps / research notes

| Note | Detail |
|---|---|
| **RV-SET-01** | Some ledger handler names are *candidate* service names; canonical API operationIds must be promoted in API.md/OpenAPI before SERVED — track in [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md). |
| **RV-SET-02** | Historical UI may expose controls before routes mount; [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md) forbids clickable sham controls. |
| **RV-SET-03** | Subagents/delegation rows depend on [DELEGATION.md](DELEGATION.md) and native child-model support per harness — qualify per agent route. |

---

## 2. Main shell — surfaces (non-Settings)

From [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md) and [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md):

| Surface | Documented actions | UI ledger / notes |
|---|---|---|
| Titlebar | Workspace switch, search/palette, Quick Entry, Notifications, Runtime health | UIV1-001–005, 140–158 |
| Left rail | New Session, Recent, Coworker roster, Work, Needs You, Automations, Library, Discover, Settings | UIV1-011–119 |
| Main chat | Header, stream, composer, agent picker, attachments, Send/Stop | UIV1-016–057 |
| Right Workbench | Start, Browser, Computer, Terminal, Artifacts, MCP App, Interactive Artifact, Inspector | UIV1-058–090 |
| Overlays | Palette, modals, toasts | UIV1-004–009 |

**Historical gap list** (still authoritative for *target vs historical snapshot*): see V1-UI-SCREEN-CONTRACT **Implementation status snapshot** — Send 503, target-only routes, missing stream frames, etc. These are **not doc missing**; they are **implementation frozen** until recovery + SDD 001.

---

## 3. Discover vs Settings vs Agents

| Area | Owner docs | UI contract |
|---|---|---|
| **Discover** (sidebar) | CAPABILITY-FABRIC, LITESPM-*, CONNECTORS | Screen contract Discover row; partial historical served read |
| **Agents** (settings/context) | AGENT-CONTROL, AGENT-FABRIC | Settings groups + composer agent picker |
| **Connections** (settings group) | CONNECTORS-WORKFLOWS, Trust | Ledger Connections rows |

No duplicate authority: Discover is **first-party navigation**; Settings Connections is **account/package mutation** with Trust.

---

## 4. End-to-end doc cross-check (Settings + shell)

| Check | Result |
|---|---|
| 187 ledger rows ↔ DESKTOP-SHELL-LLD settings table | **Aligned** (same control names) |
| Ledger ↔ COVERAGE-MATRIX concerns | **Linked** via owner docs column |
| Every Settings mutation has typed error posture | **Yes** (universal + per-row failure) |
| UI behavior spec per feature | **001 only**; Settings-heavy **SDD 011** planned |
| USER test for settings honesty | **UC-SET-04** in [COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md) |

---

## 5. Related indexes

- [FEATURE-AND-UI-CONTRACT-COVERAGE.md](FEATURE-AND-UI-CONTRACT-COVERAGE.md)  
- [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md)  
- [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md) BT-11.2
