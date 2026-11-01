#!/usr/bin/env python3
"""Read all first-party Markdown and extract reviewer queues; not semantic certification."""
import os,re,json,collections
from pathlib import Path
R=Path(__file__).resolve().parents[1]
SKIP={".git","node_modules","target",".venv","dist","build","coverage",".next","out",".turbo","vendor","REPO-COMPARE","archive","archives",".cache",".pytest_cache"}
docs=[]
for folder,dirs,names in os.walk(R):
 dirs[:]=[x for x in dirs if x not in SKIP]
 for name in names:
  p=Path(folder)/name
  if name.endswith((".md",".mdx")) and not name.startswith("V1-DEEP-SEMANTIC-SNAPSHOT"):
   docs.append(p)
docs.sort()
heads=[]; paras=[]; status=[]; owners=[]; docstats=[]; total=0
for p in docs:
 rel=p.relative_to(R).as_posix()
 lines=p.read_text(encoding="utf-8-sig",errors="replace").splitlines()
 total+=len(lines)
 fence=False;para=[]
 for n,line in enumerate(lines+[""],1):
  if line.strip().startswith((chr(96)*3,"~~~")):fence=not fence
  if para and (not line.strip() or n>len(lines)):
   raw=" ".join(para)
   norm=re.sub(r"\s+"," ",re.sub(r"\[([^]]+)\]\([^)]+\)",r"\1",raw)).lower()
   if len(norm)>165 and not raw.startswith("|"):paras.append((rel,n-len(para),norm,raw[:220]))
   para=[]
  if line.strip() and not fence:para.append(line.strip())
  if fence:continue
  if re.match(r"^#{1,6} +",line):heads.append((rel,n,line.strip()))
  if re.search(r"\b(?:TODO|TBD|FIXME|not implemented|unimplemented|candidate|proposal|not yet|future|unpromoted|target only)\b",line,re.I):status.append((rel,n,line.strip()[:250]))
  if re.search(r"\b(?:owns|owner|must not|MUST|only|never|authoritative|source.of.truth)\b",line,re.I) and len(line)>20:owners.append((rel,n,line.strip()[:310]))
 docstats.append((rel,len(lines)))
bypara=collections.defaultdict(list)
for rel,n,norm,raw in paras:bypara[norm].append((rel,n,raw))
exact=[v for v in bypara.values() if len(set(x[0] for x in v))>1]
exact.sort(key=lambda v:-len(v))
byhead=collections.defaultdict(list)
for rel,n,t in heads:byhead[re.sub(r"^#+\s*","",t).lower()].append((rel,n))
overlap=[(t,v) for t,v in byhead.items() if len(set(x[0] for x in v))>1]
overlap.sort(key=lambda x:-len(x[1]))
core=("ARCHITECTURE.md","AGENT-CONTROL.md","COWORKERS-TARGET.md","COWORKER-FLOWS.md","LEARNING.md","CONTEXT.md","TASK-RUNTIME.md","API.md","SERVICES.md","STORAGE.md","EVENTS.md","STATE-MACHINES.md","RESPONSIBILITIES.md","TRUST.md","OBSERVABILITY.md","ENVIRONMENTS.md","AUTOMATION.md","ROUTINES.md","V1-")
def relevant(x):
 rel=x[0]
 return rel=="ARCHITECTURE.md" or rel.startswith("docs/") and rel.split("/")[-1].startswith(core)
report=["# Deep Semantic Snapshot / Reviewer Evidence","",
"Line ingestion is mechanical; statuses are review leads and are NOT automatic semantic conflict proof.",
f"Files: {len(docs)}; lines: {total}; headings: {len(heads)}; authority statements: {len(owners)}; uncertainty: {len(status)}; long repeated paragraphs across files: {len(exact)}.",
"","## All first-party files"]
report += [f"- {a} ({b} lines)" for a,b in docstats]
report += ["","## Exact repeated long prose across different docs"]
for v in exact[:100]:
 report.append("- "+", ".join(f"{r}:{n}" for r,n,raw in v[:8])+" :: "+v[0][2][:160])
report += ["","## Same-named cross-document headings"]
for t,v in overlap[:120]:report.append("- "+t+": "+", ".join(f"{r}:{n}" for r,n in v[:10]))
report+=["","## Uncertainty/proposal statements in canonical owners"]
report += [f"- {r}:{n}: {line}" for r,n,line in status if relevant((r,n,line))]
report+=["","## Normative authority/invariant statements in canonical owners"]
report += [f"- {r}:{n}: {line}" for r,n,line in owners if relevant((r,n,line))]
Path("/tmp/litecowork-first-party-semantic-snapshot-2026-10-13.md").write_text("\n".join(report)+"\n",encoding="utf-8")
summary={"docs":len(docs),"lines":total,"headings":len(heads),"ownership_statements":len(owners),"candidate_statements":len(status),"exact_repeated_paragraphs":len(exact),"overlapping_headings":len(overlap)}
(R/"implementation/V1-DEEP-SEMANTIC-SUMMARY.json").write_text(json.dumps(summary,indent=2),encoding="utf-8")
print(json.dumps(summary,indent=2))
for v in exact[:12]:print("REPEAT",[(r,n) for r,n,raw in v[:5]])
