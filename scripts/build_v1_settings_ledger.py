#!/usr/bin/env python3
"""Explicit crosswalk for all Settings controls in Desktop Shell V1 LLD."""
from pathlib import Path
import re
ROOT=Path(__file__).resolve().parents[1]
M={
"Global":("Local UI/OS preference","Window","LOCAL_PREF","DESIGN-SYSTEM, MOTION"),
"Workspaces":("WorkspaceService","Workspace","OPERATOR_CAS","API, STATE-MACHINES"),
"Roots":("ResourceService","Workspace/Root","OPERATOR_CAS","WORLD-RESOURCES, TRUST"),
"Instructions":("WorkspaceInstructionRevision owner","Workspace","OPERATOR_CAS","DATA-MODEL, STORAGE"),
"Agents catalog":("AgentModuleRegistry","Runtime/AgentProfile","QUALIFIED_READ","AGENT-CONTROL"),
"Agents install":("AgentLifecycleService","Runtime/AgentModule","NATIVE_MUTATION","AGENT-CONTROL, TRUST"),
"Agents auth":("Native harness auth/OS SecretStore","Principal/AgentProfile","NATIVE_MUTATION","AGENT-CONTROL, SECURITY"),
"Agents defaults":("AgentBinding/Coworker/Workspace owner","Workspace/Binding","OPERATOR_CAS","AGENT-CONTROL"),
"Local models":("Qualified agent/Runtime offer","Runtime/Binding","QUALIFIED_READ","AGENT-CONTROL, ENVIRONMENTS"),
"Subagents":("DelegationProfileService","Workspace/ProfileRevision","OPERATOR_CAS","DELEGATION"),
"Connections":("LiteSPM + ConnectionService + Trust","Workspace/Connection","EXTERNAL_EFFECT","CONNECTORS-WORKFLOWS"),
"MCP":("LiteSPM/CapabilityFabric","Workspace/Server","EXTERNAL_EFFECT","CAPABILITY-FABRIC, PROTOCOLS"),
"Skills":("LiteSPM package owner","Workspace/SkillRevision","EXTERNAL_EFFECT","CAPABILITY-FABRIC"),
"Browser":("BrowserSessionService/Provider","Runtime/Workspace/Profile","NATIVE_MUTATION","CHROME-BROWSER-LLD"),
"Browser grants":("TrustService/BrowserLeaseArbiter","Workspace/Origin/Profile","AUTHORITY_CHANGE","TRUST, CHROME-BROWSER-LLD"),
"Computer":("EnvironmentProvider/TrustService","Runtime/Environment/Display","AUTHORITY_CHANGE","ENVIRONMENTS"),
"Coworker identity":("CoworkerService/TaskService","Workspace/Coworker","OPERATOR_CAS","COWORKERS-TARGET"),
"Memory":("Learning/Context authority","Workspace/Coworker/Context","OPERATOR_CAS","LEARNING, CONTEXT"),
"Knowledge":("ResourceIndex/Context authority","Workspace/Resource","OPERATOR_CAS","CONTEXT, WORLD-RESOURCES"),
"Goals":("GoalService","Workspace/Goal","OPERATOR_CAS","RESPONSIBILITIES"),
"Routines":("RoutineService","Workspace/Routine","OPERATOR_CAS","ROUTINES"),
"Automations":("AutomationService/TriggerCoordinator","Workspace/Automation/Occurrence","OPERATOR_CAS","AUTOMATION, STATE-MACHINES"),
"Notifications":("NotificationService/NeedsYou","Workspace/Principal","OPERATOR_CAS","EXPERIENCE, CHANNELS"),
"Runtime":("RuntimeLifecycleService","Local Runtime","NATIVE_MUTATION","RUNTIME-LIFECYCLE"),
"Security":("TrustService/OS SecretStore","Workspace/Principal/Grant","AUTHORITY_CHANGE","TRUST, SECURITY"),
"Backup":("StorageBackup/Restore service","Workspace/Backup","NATIVE_MUTATION","STORAGE"),
"Storage":("Storage/ArtifactRetention service","Runtime/Workspace","NATIVE_MUTATION","STORAGE, ARTIFACTS-EVIDENCE"),
"Usage & Monitoring":("UsageProjection/Telemetry/HealthObserver","Workspace/Task/Turn/AgentBinding","SCOPED_READ/OBSERVATION","OBSERVABILITY, V1-TELEMETRY-USAGE-RESILIENCE-LLD"),
"Terminal":("TerminalSessionService/TerminalProfileRegistry","Workspace/Runtime/Environment","NATIVE_MUTATION","V1-INTEGRATED-TERMINAL-LLD, ENVIRONMENTS"),
"About/Diagnostics":("Runtime diagnostics/Tauri bridge","Runtime/Window","QUALIFIED_READ","OBSERVABILITY"),
}
text=(ROOT/"docs/DESKTOP-SHELL-LLD.md").read_text(encoding="utf-8")
rows=[];started=False
for line in text.splitlines():
 if line.startswith("## Full Settings controls"):started=True;continue
 if started and line.startswith("**Settings page controller"):break
 if not started:continue
 m=re.match(r"^\| ([^|]+) \| ([^|]+) \| ([^|]+) \|$",line)
 if not m or m[1] in ("Category","---") or m[1].startswith("---"):continue
 category=m[1].strip()
 if category not in M:raise RuntimeError("Unowned category: "+category)
 for control in m[2].split(","):rows.append((category,control.strip(),m[3].strip()))
OVERRIDES={
 "selectWorkspace":"LOCAL_SELECTION_EPOCH",
 "selectFolder":"OS_PICKER_NO_PERSIST",
 "selectChromeChannel":"LOCAL_SELECTION_NO_CONNECTION",
 "openNativeConfig":"NATIVE_CONFIG_OPEN_WITH_CONSENT",
 "pairCompatibleAgent":"OPERATOR_CAS",
 "testNativeAuth":"QUALIFIED_PROVIDER_PROBE",
 "testServer":"QUALIFIED_PROCESS_OR_NETWORK_PROBE",
 "testSkill":"QUALIFIED_ISOLATED_EXECUTION",
 "verifyManifest":"SCOPED_READ/OBSERVATION",
 "runProviderDiagnostic":"QUALIFIED_PROVIDER_PROBE",
 "openRequest":"LOCAL_AUTHORIZED_NAVIGATION",
 "exportUsageReport":"REVIEWED_LOCAL_EXPORT",
 "filterUsageDateRange":"LOCAL_FILTER_NO_AUTHORITY",
 "filterUsageAgent":"LOCAL_FILTER_NO_AUTHORITY",
 "chooseDefaultShellProfile":"LOCAL_PROFILE_PREFERENCE",
 "validateCustomShellPath":"QUALIFIED_OS_PROBE",
 "clearTerminalViewport":"LOCAL_VIEWPORT_ONLY",
}
def is_read(name):
 return bool(re.match(r"^(view|inspect|list|search|preview|filter|show|discover|check|copy|openSupport|verify|compare)",name,re.I))
out=["# V1 Settings Per-Control Authority Ledger","",
"Status: proposed implementation design. Not evidence that a source handler or public API exists. This maps every Settings control specified by DESKTOP-SHELL-LLD to an authority and failure contract.",
"",
"## Universal before/after/failure contract","",
"- Precondition: selected Workspace/Runtime/owner version and capabilities are freshly checked. Principal identity comes only from the authenticated local IPC or HTTP transport, never a client-supplied principal ID.",
"- Mutation transport: stable Idempotency-Key across retries (domain RequestId), expected version via If-Match, Workspace scope via X-Workspace-ID. IPC request_id is distinct correlation, never idempotency authority.",
"- Postcondition: accepted committed owner version/receipt or verified native provider observation. UI feedback is based on acknowledged authority, not click intent; stale Workspace responses are dropped.",
"- Failure: permission denial, stale/version conflict, no capability, offline, canceled native login, partial external effect, ambiguous receipt, user rejection and safety drain each have distinguishable status and safe repair action. An ambiguous external side effect is reconciled before retry.",
"- Display-only layout preferences may persist locally but must never restore old grants or claim privileged effects. Do not expose active toggles for unserved operations.",
"",
"## Explicit control coverage","",
"| Control | Group | Authority | Selected scope | Class | Expected output and failure path | Source owners |",
"|---|---|---|---|---|---|---|"]
for cat,name,fail in rows:
 owner,scope,kind,refs=M[cat]
 if is_read(name):kind="SCOPED_READ/OBSERVATION"
 if name in OVERRIDES:kind=OVERRIDES[name]
 if kind.startswith("LOCAL_") or kind.startswith("OS_PICKER") or kind.startswith("LOCAL_AUTHORIZED") or kind=="REVIEWED_LOCAL_EXPORT":
  state="local selection/draft state only; no durable domain mutation"
 elif kind=="SCOPED_READ/OBSERVATION" or kind.startswith("QUALIFIED_"):
  state="scoped fresh, redacted observation; explicit unavailable/failed probe"
 else:
  state="durable receipt or reconciled provider observation; uncertain != success"
 out.append(f"| {name} | {cat} | {owner} | {scope} | {kind} | {state}; {fail} | {refs} |")
out+=["","## Cross-category race and handoff invariants","",
"- Installing an AgentModule is not authenticating it, creating an AgentBinding, making it ready, choosing its model or switching an active Attempt. Use separate owner revisions and native handshake proof.",
"- Install/activate/authenticate/grant/assign/invoke LiteSPM capabilities are distinct. A per-Coworker revoke must not globally revoke a shared account.",
"- Browser profile import (bookmarks/settings only) and existing Chrome consent/attach are different flows. Profile wipe and browser session detach require distinct confirmations.",
"- Take control and Return require control epoch compare-and-set and Task/Attempt/Origin reauthorization; old agent inputs never replay.",
"- Pause stops new admissions; running unknown external effects keep PENDING until reconciled. Never report PAUSED merely because a toggle changed.",
"- Schedule preview does not enable an Automation. A recurring trigger host must be durably ready before ENABLED; a Run now creates its own distinct idempotent occurrence.",
"- Restore, cleanup, revocation and credential changes show impact/dry run/version checks; interrupted results cannot automatically repeat uncertain destructive effects.",
"",
f"Ledger totals: {len(rows)} individually named controls, {len({x[0] for x in rows})} Settings groups. Existing canonical API, schemas, storage and service owners take precedence over these candidate handler names.",
]
dst=ROOT/"docs/V1-SETTINGS-CONTRACT-LEDGER.md"
dst.write_text("\n".join(out)+"\n",encoding="utf-8")
print("SETTINGS_LEDGER",len(rows),"GROUPS",len({x[0] for x in rows}))
