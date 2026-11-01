#!/usr/bin/env python3
"""Reproducible static V1 design-depth inventory and source-contract cross checks."""
from __future__ import annotations
import argparse, collections, hashlib, json, re
from pathlib import Path
import yaml

ROOT=Path(__file__).resolve().parents[1]
SKIP={".git","node_modules","target",".venv",".pytest_cache",".agent-worktrees","dist","build","coverage",".cache"}
OWNER={
"Shell":("ARCHITECTURE.md","docs/EXPERIENCE.md","docs/DESKTOP-SHELL-LLD.md","implementation/UI.md"),
"Agents":("docs/AGENT-FABRIC.md","docs/AGENT-CONTROL.md","docs/NATIVE-COMPOSER-LLD.md","implementation/epics/E03.md"),
"Native chat":("docs/EXPERIENCE.md","docs/AGENT-CONTROL.md","docs/NATIVE-COMPOSER-LLD.md","implementation/epics/E03.md"),
"Tasks":("docs/TASK-RUNTIME.md","docs/STATE-MACHINES.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E03.md"),
"Delegation":("docs/AGENT-FABRIC.md","docs/DELEGATION.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E05.md"),
"Trust/Effects":("docs/TRUST.md","docs/CAPABILITY-INVOCATIONS.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E04.md"),
"Environments":("docs/ENVIRONMENTS.md","docs/NETWORK-SECURITY.md","docs/CHROME-BROWSER-LLD.md","implementation/epics/E07.md"),
"Browser/Computer":("docs/ENVIRONMENTS.md","docs/TRUST.md","docs/CHROME-BROWSER-LLD.md","implementation/epics/E07.md"),
"Outputs":("docs/ARTIFACTS-EVIDENCE.md","docs/PRESENTATION-RUNTIME.md","docs/RENDERING-CONFORMANCE-LLD.md","implementation/epics/E08.md"),
"Workbench":("docs/EXPERIENCE.md","docs/PRESENTATION-RUNTIME.md","docs/DESKTOP-SHELL-LLD.md","implementation/epics/E08.md"),
"Coworkers":("docs/PRODUCT.md","docs/COWORKERS-TARGET.md","docs/DESKTOP-SHELL-LLD.md","implementation/epics/E10.md"),
"Integrations":("docs/CAPABILITY-FABRIC.md","docs/CONNECTORS-WORKFLOWS.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E04.md"),
"Memory/Knowledge":("docs/CONTEXT.md","docs/LEARNING.md","docs/DESKTOP-SHELL-LLD.md","implementation/epics/E08.md"),
"Automations":("docs/ROUTINES.md","docs/AUTOMATION.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E09.md"),
"Notifications":("docs/EXPERIENCE.md","docs/CHANNELS.md","docs/DESKTOP-SHELL-LLD.md","implementation/epics/E09.md"),
"Storage":("docs/DATA-MODEL.md","docs/STORAGE.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E01.md"),
"IPC/Runtime":("docs/RUNTIME-LIFECYCLE.md","docs/LOCAL-OPERATOR-IPC.md","docs/V1-FUNCTION-FILE-CATALOG.md","implementation/epics/E13.md"),
"Recovery":("docs/FAILURE-RECOVERY.md","docs/STATE-MACHINES.md","docs/V1-WORKFLOW-EVENT-LLD.md","implementation/epics/E04.md"),
"Settings":("docs/EXPERIENCE.md","docs/DESKTOP-SHELL-LLD.md","docs/V1-FUNCTION-FILE-CATALOG.md","implementation/UI.md"),
"Release":("implementation/SCOPE.md","implementation/RELEASE.md","docs/RUNTIME-LIFECYCLE.md","implementation/epics/E13.md")
}
def read(f):return (ROOT/f).read_text(encoding="utf-8-sig")
def norm(s):return re.sub(r"[^a-zA-Z0-9]+","",s).lower()
def canon(s):return re.sub(r"\{[^}]+\}","{}",s)

def main():
 ap=argparse.ArgumentParser()
 ap.add_argument("--write",action="store_true")
 ap.add_argument("--strict",action="store_true")
 arg=ap.parse_args()
 docs=[]
 import os
 for folder,dirnames,files in os.walk(ROOT):
  dirnames[:]=[name for name in dirnames if name not in SKIP]
  for name in files:
   p=Path(folder)/name
   if p.suffix.lower() in {".md",".mdx"} and p.name not in {"V1-EVERY-MD-AUDIT-2026-10-12.md","V1-DESIGN-DEPTH-AUDIT.md"}:
    docs.append(p)
 docs.sort()
 records=[]
 for p in docs:
  text=p.read_text(encoding="utf-8-sig")
  records.append((p.relative_to(ROOT).as_posix(),len(text.splitlines()),sum(bool(re.match(r"^#{1,6} ",x)) for x in text.splitlines()),hashlib.sha256(p.read_bytes()).hexdigest()[:12]))
 spec=yaml.safe_load(read("docs/schemas/operator-api.openapi.yaml"))
 routes={(method.upper(),canon("/v1"+path)):detail.get("operationId")
         for path,ops in spec["paths"].items()
         for method,detail in ops.items() if method in {"get","post","put","patch","delete"}}
 catalog=read("docs/V1-FUNCTION-FILE-CATALOG.md")
 functions=[];group=None
 for i,line in enumerate(catalog.splitlines(),1):
  h=re.match(r"^### (F\d{2})\s+.",line)
  if h:group=h[1]
  m=re.match(r"^([A-Za-z]\w*)\((.*?)\)\s*:\s*",line)
  if m and group:functions.append((group,m[1],m[2],i))
 settings=[]
 in_table=False
 for i,line in enumerate(read("docs/DESKTOP-SHELL-LLD.md").splitlines(),1):
  if line.startswith("## Full Settings controls"):in_table=True;continue
  if in_table and line.startswith("**Settings page controller"):break
  if not in_table:continue
  m=re.match(r"^\| ([^|]+) \| ([^|]+) \| ([^|]+) \|$",line)
  if not m or m[1].startswith("---") or m[1]=="Category":continue
  for name in m[2].split(","):settings.append((m[1].strip(),name.strip(),m[3].strip(),i))
 exact={norm(f[1]) for f in functions}
 ledger=read("docs/V1-SETTINGS-CONTRACT-LEDGER.md") if (ROOT/"docs/V1-SETTINGS-CONTRACT-LEDGER.md").is_file() else ""
 missing_settings=[s for s in settings if norm(s[1]) not in exact and not re.search(r"(?m)^\|\s*"+re.escape(s[1])+r"\s*\|",ledger)]
 machine=read("docs/STATE-MACHINES.md")
 sql=read("docs/schemas/sqlite-v1.sql")
 tables=set(re.findall(r"(?im)^CREATE TABLE(?: IF NOT EXISTS)?\s+([a-zA-Z_]\w*)",sql))
 paths=[]
 for i,line in enumerate(catalog.splitlines(),1):
  m=re.match(r"^\| (GET|POST|PUT|PATCH|DELETE) (/v1/[^\s|]+)",line)
  if m:
   method,path=m.groups()
   paths.append((i,method,path,routes.get((method,canon(path)))))
 failures=[]
 for area,owner in OWNER.items():
  for name in owner:
   if not (ROOT/name).is_file():failures.append(f"{area} missing {name}")
 if len(set(re.findall(r"(?m)^- UIV1-(\d{3})\b",read("implementation/V1-DESIGN-COVERAGE.md"))))!=158:failures.append("V1 acceptance cases not 158 unique")
 if "## ConversationTurn" not in machine:failures.append("ConversationTurn owner missing")
 dup=collections.defaultdict(list)
 for g,n,parameters,i in functions:dup[n].append((g,parameters,i))
 contradictions=[(n,items) for n,items in dup.items() if len({x[1] for x in items})>1]
 body=[
 "# V1 HLD–LLD–UI–API–Process Depth Audit","",
 "Generated by scripts/audit_v1_design_depth.py. Per-file ingestion is exhaustive for first-party Markdown; semantic witness checking is NOT a formal complete human sentence-level review.",
 "",
 f"- First-party Markdown source files ingested: {len(docs)}; lines: {sum(x[1] for x in records)}; headings: {sum(x[2] for x in records)}.",
 f"- HLD→domain authority→LLD→epic ownership rows: {len(OWNER)}.",
 f"- Named function signatures in V1 F01–F14 catalog: {len(functions)}.",
 f"- Settings operations: {len(settings)}, exact catalog/ledger matches: {len(settings)-len(missing_settings)}, unmapped: {len(missing_settings)}.",
 f"- Machine OpenAPI methods: {len(routes)}; candidate/existing function catalog route rows: {len(paths)}.",
 f"- Baseline SQLite tables: {len(tables)}.",
 f"- Functions with multiple incompatible signatures: {len(contradictions)}.",
 f"- Blocking structural problems: {len(failures)}.","",
 "## Authority ownership per subsystem","",
 "| Subsystem | HLD | Domain authority | Detailed LLD | Epic |",
 "|---|---|---|---|---|"]
 for area,owner in OWNER.items():body.append("| "+" | ".join([area,*owner])+" |")
 body+=["","## Settings controls with no exact callable contract mapping",""]
 body += [f"- {area} / {control} (shell LLD line {line}): {errors}" for area,control,errors,line in missing_settings] or ["None."]
 body+=["","## Function names with conflicting signatures",""]
 body += [f"- {n}: {items}" for n,items in contradictions] or ["None."]
 body+=["","## Proposed catalog routes vs actual OpenAPI","",
        "| Line | Method | Path | Served in schema | OperationId |","|---:|---|---|---|---|"]
 for i,m,path,op in paths:body.append(f"| {i} | {m} | {path} | {'YES' if op else 'NO; candidate'} | {op or '-'} |")
 body+=["","## Structural errors",""]
 body += ["- "+f for f in failures] or ["None."]
 body+=["","## All first-party Markdown ingested","",
        "| File | Lines | Headings | SHA-256 prefix |","|---|---:|---:|---|"]
 for rel,n,head,hash_prefix in records:body.append(f"| {rel} | {n} | {head} | {hash_prefix} |")
 body+=["","## What this does not establish","",
        "Named HLD/LLD links do not prove all behaviors are complete. A matching function name does not prove input/output/error semantics. No runtime tests, UI accessibility tests or Chrome/provider control execution were performed. Owner-by-owner adversarial review and executable CODE/SYSTEM/USER tests remain separate gates."]
 if arg.write:(ROOT/"implementation/V1-DESIGN-DEPTH-AUDIT.md").write_text("\n".join(body)+"\n",encoding="utf-8")
 print(f"DEPTH files={len(docs)} lines={sum(x[1] for x in records)} domains={len(OWNER)} functions={len(functions)} settings={len(settings)} unmapped={len(missing_settings)} dup_signature={len(contradictions)} routes={len(routes)} db_tables={len(tables)} blockers={len(failures)}")
 print("UNMAPPED SAMPLE:",[(a,b) for a,b,_,_ in missing_settings[:25]])
 print("DUPLICATES:",contradictions[:8])
 for f in failures:print("ERROR:",f)
 return int(bool(arg.strict and failures))
if __name__=="__main__":raise SystemExit(main())
