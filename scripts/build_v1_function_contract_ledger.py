#!/usr/bin/env python3
"""Enumerate V1 function signatures with owning authority and minimum pre/post/error gates."""
import re
from pathlib import Path
R=Path(__file__).resolve().parents[1]
s=(R/"docs/V1-FUNCTION-FILE-CATALOG.md").read_text(encoding="utf-8-sig")
GROUP={
"F01":("React Shell/Window","UI-only or local preference, never Trust", "DESKTOP-SHELL-LLD, EXPERIENCE"),
"F02":("Agent Lifecycle/Binding, LiteSPM/Trust","qualified local probe or versioned Operator mutation","AGENT-CONTROL, CAPABILITY-FABRIC"),
"F03":("ConversationService + AgentTurnCoordinator","canonical Conversation turn/descriptor","API, STATE-MACHINES, NATIVE-COMPOSER-LLD"),
"F04":("PresentationService / ArtifactService / Workbench","authorized exact-version view or Artifact mutation","PRESENTATION-RUNTIME, ARTIFACTS-EVIDENCE"),
"F05":("BrowserSessionService/Environment/Trust","control-epoch, origin and OS-provider mediated effects","CHROME-BROWSER-LLD, ENVIRONMENTS"),
"F06":("TaskService/Attempt scheduler","durable Task/Step/lease/evidence","TASK-RUNTIME, DELEGATION, STATE-MACHINES"),
"F07":("CapabilityFabric/Trust/Gateway","approved scoped Invocation/Effect","CAPABILITY-INVOCATIONS, TRUST"),
"F08":("Coworker/Context/Memory owner","versioned and provenance-scoped model","COWORKERS-TARGET, LEARNING"),
"F09":("Routine/Automation/Trigger coordinator","immutable revision/Occurrence idempotency","ROUTINES, AUTOMATION"),
"F10":("RuntimeLifecycle/Storage/Trust","OS/daemon and versioned storage authority","RUNTIME-LIFECYCLE, STORAGE"),
"F11":("Usage/Telemetry/HealthObserver","authenticated observation and deduped inference leaf; billing confidence","OBSERVABILITY, V1-TELEMETRY-USAGE-RESILIENCE-LLD"),
"F12":("Context/Learning/PersonalContextProvider","scoped source policy/REVOCATION and native-agent tool autonomy","LEARNING, CONTEXT, V1-AGENT-MEMORY-FAST-PATH-LLD"),
"F13":("AgentLifecycle/AgentModuleRegistry","Runtime OS/Module identity and qualified native probes","AGENT-CONTROL, V1-AGENT-DISCOVERY-AND-CATALOG-LLD"),
"F14":("TerminalSessionService/EnvironmentProvider/Trust","authorized PTY session, Runtime lease and control epoch","ENVIRONMENTS, V1-INTEGRATED-TERMINAL-LLD"),
}
def classify(name,group):
 if group in {'F11','F12','F13','F14'}:
  mutation_prefixes=('telemetryReceive','telemetryReconcile','telemetryRecord','telemetryInvalidate','telemetryApply','memoryObserve','memoryInvalidate','memoryPropose','memoryRevoke','agentApprove','agentAdmit','terminalCreate','terminalWrite','terminalResize','terminalChange','terminalRestore','terminalRequest','terminalDetach')
  native_prefixes=('telemetryExport','memorySearch','memoryFetch','agentProbe','agentEnumerate','agentScan','agentRefresh','agentPreview','agentValidate','memoryResolve','telemetryCollect','terminalDetect','terminalValidate','terminalAttach','terminalRead','terminalObserve','terminalExport')
  if name.startswith(mutation_prefixes):
   return ('MUTATING/EXTERNAL-BOUNDARY', 'Authenticated owner, exact scope, relevant lease/version and stable idempotency for durable actions; no secret source material.', 'Committed owner receipt or safe observation, never optimistic fabricated success.', 'Conflicting epoch, restart, duplicate request, provider timeout-after-effect and permission revocation must be handled explicitly.')
  if name.startswith(native_prefixes):
   return ('QUALIFIED-READ/PROBE', 'Authorized exact Runtime/Workspace/AgentModule; bounded timeouts and redacted metadata.', 'Fresh result or explicit UNKNOWN/UNAVAILABLE with source and age.', 'Late probe, staleness, permission failures and provider crash never count as success.')
  if name.startswith(('telemetrySnapshot','telemetryCalculate','telemetryAggregate','telemetrySummarize','telemetryClassify','telemetryDerive','memoryCheck','memoryLookup','memoryBuild','agentValidate','agentDerive','agentResolve','agentRender','terminalSanitize')):
   return ('PURE/SCOPED-DECISION', 'Versioned source/provenance, bounded inputs and privacy scope; final Trust gate still required for any use.', 'Deterministic typed decision or UNKNOWN.', 'Unit/property test including stale source, overflow, cross-Workspace and failure states.')
  return ('REQUIRES-OWNER-CLASSIFICATION', 'Owner must explicitly classify action before implementation.', 'No implicit success or side effect.', 'Fail closed until pre/post/race/ownership contract is assigned.')
 if re.match(r"^(validate|calculate|derive|normalize|compile|sanitize|merge|reduce|filter|parse|choose|construct|build|check|hash|rank|scheduleWorkUnit|compare|plan)",name,re.I):
  return ("PURE/DECISION", "Inputs pinned, bounded and non-secret; no authority inferred from UI.", "Deterministic typed result or explicit unsupported/unknown.", "No external side effects; reject stale inputs or bounds; property/unit+negative fixture.")
 if re.match(r"^(list|load|fetch|query|inspect|read|probe|observe|discover|resolve|look|enumerate|search|subscribe|watch|get|scan)",name,re.I):
  return ("READ/OBSERVE", "Authenticated owner scope or local qualified Runtime; no mutation.", "Scoped redacted version/observation with time and provider freshness.", "Denied, stale/offline or unknown distinct from empty; no secret leakage; scope-switch race fixture.")
 if re.search(r"(browser|chrome|capture|computer|input|download|connect|invoke|install|auth|credential|effect|session|tool|send)",name,re.I):
  return ("NATIVE/EXTERNAL", "Authoritative actor, grant, Task/Turn, Runtime incarnation, effect/lease where needed; exact provider version.", "Admitted receipt and observed or reconciled outcome; uncertain != verified.", "Timeout-after-effect, revoke, epoch conflict, provider crash and idempotency replay fixture.")
 if re.match(r"^(create|commit|accept|save|update|enable|pause|archive|publish|delete|revoke|set|start|stop|complete|admit|cancel|resume|reserve|materialize|trigger|grant|release|append|mark|retry)",name,re.I):
  return ("MUTATE/CAS", "Authenticated scope, permission, stable Idempotency-Key, If-Match and validated payload.", "Atomic owner version/receipt/outbox and justified state transition.", "Stale version, duplicate key, crash between admission/effect, closure or cancellation race fixture.")
 return ("SCOPED-CONTROLLER", "Pinned scope and input type, target owner chosen explicitly.", "Typed result/projection, no implicit mutation.", "Unsupported or unknown explicit; evaluate whether call needs CAS/Effect before serving.")
entries=[];group=None
for line,txt in enumerate(s.splitlines(),1):
 m=re.match(r"^### (F\d{2})\s+.",txt)
 if m:group=m[1]
 n=re.match(r"^([a-zA-Z]\w*)\((.*?)\)\s*:\s*(.+)$",txt)
 if n and group:
  name,params,ret=n.groups()
  if group not in GROUP:raise ValueError("No group owner "+group)
  entries.append((group,name,params,ret,line))
if not entries:raise RuntimeError("No function names extracted")
out=["# V1 Function-by-Function Authority and Failure Ledger","",
"Status: NORMATIVE TARGET. Generated from all named function signatures in V1-FUNCTION-FILE-CATALOG. Classification expresses the minimum required owner-side contract, not proof that each function/handler has been implemented or individually proven. Where source-specific domain contract is stricter, it overrides these inferred minimum gates.",
"",
"## Interpretation and required implementation proof","",
"- A pure function cannot mutate the Operator, provider, settings, state machine or filesystem. A read must be owner-scoped and cannot mint grants. All side-effecting commands must use actual authenticated identity, stable idempotency key, expected-version check, transactional receipt/outbox and reconciliation.",
"- A user-visible function marked READ, PURE or SCOPED-CONTROLLER still needs an explicit negative test: no permission, stale selection epoch, cancel, malformed input and limited/unknown provider metadata.",
"- Session/provider/browser functions additionally fence Runtime incarnation, version, lease epoch, source origin, and Effect identity when required. Any function that hides a write behind a read-classified name must be reclassified before implementation.",
"- Function source signatures are not guaranteed to be final API methods. The canonical existing service name, payload DTO, event enum and SQL table remain owned by API/SCHEMAS/SERVICES/STORAGE; never invent routes solely from these function names.",
"",
"## Enumerated functions: inputs, outputs, preconditions, postconditions, failure and concurrency",
"",
"| Function | Catalog source | Owning service / authority | Input fields (proposed) | Output | Effect class | Preconditions | Postcondition | Failure & race qualification |",
"|---|---|---|---|---|---|---|---|---|"]
for group,name,params,ret,line in entries:
 owner,scope,refs=GROUP[group]
 kind,pre,post,neg=classify(name,group)
 pre=scope+"; "+pre
 out.append(f"| {name} | {group}, line {line} | {owner} | {params.replace('|','/')} | {ret.replace('|','/')} | {kind} | {pre} | {post} | {neg} |")
out+=["","## Do not infer a pass from generated rows","",
f"{len(entries)} functions enumerated. Every row still needs a human reviewer to confirm whether its classification fits the actual upstream side effects, explicit backend owner, event name and acceptance fixture. An unimplemented target function never becomes a working UI control because it appears here."]
(R/"docs/V1-FUNCTION-CONTRACT-LEDGER.md").write_text("\n".join(out)+"\n",encoding="utf-8")
print("FUNCTION_LEDGER",len(entries),"GROUPS",len(set(x[0] for x in entries)))
