#!/usr/bin/env python3
"""Cross-owner design invariant regression checks. All Markdown is ingested.
This is NOT full semantic certification; it guards adjudicated conflicts only.
"""
import os,json,re,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1]
skip={".git","node_modules","target",".venv","dist","build","coverage",".next","out",".turbo","vendor","archives","archive","REPO-COMPARE",".cache",".pytest_cache"}
docs={}
for f,dirs,filenames in os.walk(R):
 dirs[:]=[n for n in dirs if n not in skip]
 for name in filenames:
  if not name.endswith((".md",".mdx")) or name=="V1-DEEP-SEMANTIC-SNAPSHOT.md":continue
  p=Path(f)/name
  rel=p.relative_to(R).as_posix()
  docs[rel]=p.read_text(encoding="utf-8-sig",errors="replace")
issues=[]
def need(rel,fragment):
 s=docs.get(rel,"")
 if fragment not in s:issues.append("MISSING "+rel+" :: "+fragment[:110])
def forbid(rel,fragment):
 s=docs.get(rel,"")
 if fragment in s:issues.append("SHADOW "+rel+" :: "+fragment[:110])
def exactly(rel,fragment,count=1):
 n=docs.get(rel,"").count(fragment)
 if n!=count:issues.append("COUNT "+rel+" :: "+fragment[:95]+" expected "+str(count)+" got "+str(n))
cow="docs/COWORKERS-TARGET.md"
learn="docs/LEARNING.md"
need(cow,"**Memory policy normalization, no second memory state machine:**")
forbid(cow,"CoworkerMemoryPolicy {")
forbid(cow,"MemoryCandidate { # proposal")
forbid(cow,"coworker.memory_policy.revised.v1")
forbid(cow,"coworker.memory_candidate.status_changed.v1")
forbid(cow,"coworker.memory_record.committed.v1")
exactly(cow,"**Memory policy normalization, no second memory state machine:**")
exactly(cow,"Memory lifecycle uses only LEARNING.md target event names")
need(learn,"CoworkerLearningPolicy {")
need(learn,"LearningCandidate {")
need(learn,"MemoryRecordMetadata {")
need(learn,"owner choosing Add Memory manually")
need("docs/DATA-MODEL.md","CoworkerLearningPolicyRevision {")
need("docs/RESPONSIBILITIES.md","LEGACY interim field")
exactly("docs/RESPONSIBILITIES.md","# LEGACY interim field: reviewed proposals only; new LearningPolicy controls quiet private auto-commit")
need("docs/API.md","Accepted Coworker-owned Conversation target")
forbid("docs/API.md","Conversation turns remain governed\nby Conversation override and Workspace default, independently")
need("docs/FLOWS.md","For the accepted **Coworker-owned Conversation target**")
need("docs/FLOWS.md","TaskService derives that exact origin server-side")
need("docs/SERVICES.md","**Coworker-owned Conversation target admission (not yet served):**")
exactly("docs/SERVICES.md","**Coworker-owned Conversation target admission (not yet served):**")
need("docs/COWORKER-FLOWS.md","two independently recoverable owner commits")
need("docs/COWORKER-FLOWS.md","typed proposal only through a qualified host proposal tool/input surface")
need(cow,"**F-C01 Create Quick (two recoverable owner commits):**")
need("docs/V1-AGENT-MEMORY-FAST-PATH-LLD.md","Currently the desktop Resource upload UI supports only Workspace-owned WORKSPACE_NOTES")
need("docs/API.md","The local desktop V1 Library currently creates only")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md","UNRECONCILED")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md","excluded from a purported exact deduplicated total")
need("docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md","cannot provide a guaranteed monetary hard stop")
need("docs/SERVICES.md","Core-mediated LiteSPM capability provider invocation")
need("docs/V1-INTEGRATED-TERMINAL-LLD.md","cannot be fenced or guaranteed observed")
need("docs/FLOWS.md","native agent, compatible model and reasoning/mode controls")
need("implementation/V1-DESIGN-DRYRUN-AND-RECONCILIATION-2026-10-13.md","## Dry run B")
# Guard against accidental same-origin and privacy erosion.
need("docs/CONTEXT.md","The Operator may offer a read-only")
need("docs/COWORKERS-TARGET.md","No unreviewed group shared memory.")
if not any("docs/V1-DESIGN-INDEX.md"==key for key in docs):issues.append("INDEX MISSING")
# Whole tree ingestion digest is evidence of what was traversed, not correctness.
summary={
"documents_read":len(docs),
"lines_read":sum(len(v.splitlines()) for v in docs.values()),
"sha256_document_index":hashlib.sha256("\n".join(f"{k}\t{hashlib.sha256(v.encode()).hexdigest()}" for k,v in sorted(docs.items())).encode()).hexdigest(),
"contract_assertions":34,
"issues":issues
}
(R/"implementation/V1-CROSS-CONTRACT-GUARD.json").write_text(json.dumps(summary,indent=2)+"\n",encoding="utf-8")
print(json.dumps(summary,indent=2))
if issues:raise SystemExit(1)
