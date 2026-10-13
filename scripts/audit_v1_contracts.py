#!/usr/bin/env python3
"""Read-only V1 design/API/backlog and supplemental acceptance consistency checker."""
from __future__ import annotations
import argparse
import collections
import json
import re
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[1]
def get(name):
    return (ROOT/name).read_text(encoding="utf-8-sig")

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-report", action="store_true")
    parser.add_argument("--strict", action="store_true")
    args=parser.parse_args()
    errors=[]
    notes=[]
    spec=yaml.safe_load(get("docs/schemas/operator-api.openapi.yaml"))
    def canonical_path(path):
        return re.sub(r"\{[^}]+\}", "{}", path)
    routes={(method.upper(),canonical_path("/v1"+path)):data.get("operationId","")
            for path,methods in spec["paths"].items()
            for method,data in methods.items()
            if method in {"get","post","put","patch","delete"}}
    catalog=get("docs/V1-FUNCTION-FILE-CATALOG.md")
    entries=[]
    for line_number,line in enumerate(catalog.splitlines(),1):
        match=re.match(r"^\| (GET|POST|PUT|PATCH|DELETE) (/v1/[^\s|]+)",line)
        if not match: continue
        method,path=match.groups()
        exists=(method,canonical_path(path)) in routes
        identified=("existing" in line.lower() or "canonical" in line.lower())
        entries.append((method,path,exists,line_number))
        if exists and not identified:
            errors.append(f"Catalog line {line_number}: existing route lacks explicit existing/canonical label: {method} {path}")
        if not exists and identified:
            errors.append(f"Catalog line {line_number}: claims existing but not in machine OpenAPI: {method} {path}")
    canonical=[
        ("GET","/v1/agent-profiles/{agentProfileId}/control-descriptor"),
        ("POST","/v1/conversations/{id}/turns"),
        ("POST","/v1/conversations/{id}/turns/{turn_id}/retry"),
        ("POST","/v1/conversations/{id}/turns/{turn_id}/cancel"),
        ("GET","/v1/conversations/{id}/presentation"),
        ("GET","/v1/rich-presentations/{presentation_id}")]
    for operation in canonical:
        if (operation[0],canonical_path(operation[1])) not in routes:
            errors.append(f"Expected existing canonical route missing: {operation}")
    cover=get("implementation/V1-DESIGN-COVERAGE.md")
    counts=collections.Counter(int(x) for x in re.findall(r"(?m)^- UIV1-(\d{3})\b",cover))
    missing=[i for i in range(1,159) if not counts[i]]
    doubles=[(k,v) for k,v in sorted(counts.items()) if v!=1]
    if missing: errors.append("Missing individual supplementary test definitions: "+str(missing))
    if doubles: errors.append("Duplicate supplementary test definitions: "+str(doubles))
    if not re.search(r"(?m)^\|\s*U01\s*\|",get("implementation/TESTING.md")):
        errors.append("Existing U01 workflow case missing from implementation/TESTING.md")
    machine=get("docs/STATE-MACHINES.md")
    workflow=get("docs/V1-WORKFLOW-EVENT-LLD.md")
    if "## ConversationTurn" not in machine or "OPEN -> RUNNING" not in machine:
        errors.append("Canonical ConversationTurn machine unexpectedly changed")
    for old in ["Conversation: CREATED -> OPEN -> TURN_RUNNING","Automation: DRAFT -> REVIEWED",
                "Task: READY -> RUNNING/PLANNING"]:
        if old in workflow: errors.append("Obsolete pseudo-state remains: "+old)
    backlog=json.loads(get("implementation/backlog.json"))
    stories={story["id"] for story in backlog["stories"]}
    source_names=["docs/V1-DESIGN-INDEX.md","docs/NATIVE-COMPOSER-LLD.md",
                  "docs/CHROME-BROWSER-LLD.md","docs/DESKTOP-SHELL-LLD.md",
                  "docs/RENDERING-CONFORMANCE-LLD.md","docs/V1-WORKFLOW-EVENT-LLD.md",
                  "docs/V1-FUNCTION-FILE-CATALOG.md","implementation/V1-DESIGN-COVERAGE.md"]
    for name in source_names:
        for id in sorted(set(re.findall(r"\bE\d{2}-S\d{2}\b",get(name)))):
            if id not in stories: errors.append(f"Nonexistent story {id} referenced by {name}")
    conv=ROOT/"apps/litecowork-ui/src/conversations/ConversationsPage.tsx"
    if conv.exists():
        text=conv.read_text(encoding="utf-8")
        if "Sending messages is not available yet" in text:
            notes.append("ConversationsPage explicitly states native message submission is unavailable.")
        if "readOnly" in text and "disabled" in text:
            notes.append("ConversationsPage contains readOnly and disabled controls; verify exact JSX and real send execution before release claims.")
    else:
        notes.append("ConversationsPage not found at previously observed location; source may have moved.")
    notes.extend([f"OpenAPI machine routes: {len(routes)}",
                  f"Supplemental UIV1 explicit acceptance definitions: {len(counts)}",
                  f"Actual backlog stories: {len(stories)}"])
    lines=["# V1 Contracts vs Actual OpenAPI and Backlog", "",
           "Generated by scripts/audit_v1_contracts.py. It is a deterministic structural consistency audit, not a full semantic or runtime qualification.", "",
           f"Blocking structural findings: {len(errors)}", "",
           "## Findings", ""]
    lines.extend("- "+e for e in errors)
    if not errors: lines.append("No problems detected by these checks.")
    lines.extend(["","## Source observations",""])
    lines.extend("- "+n for n in notes)
    lines.extend(["","## Function catalog routes","",
                  "| Method | Route | Actual OpenAPI? | OperationId |",
                  "|---|---|---|---|"])
    for method,path,exists,line_number in entries:
        lines.append(f"| {method} | {path} | {'YES' if exists else 'NO: proposed'} | {routes.get((method,canonical_path(path)),'-')} |")
    lines.extend(["","## Boundaries","",
                  "Parser verification cannot prove Chrome/WebView equivalence, real harness parity, user acceptance, OS security gates, or all prose semantics. Run real provider/OS tests and owner qualification before a release claim."])
    if args.write_report:
        (ROOT/"implementation/V1-CONTRACT-CHECK.md").write_text("\n".join(lines)+"\n",encoding="utf-8")
    print(f"V1_CONTRACT_AUDIT errors={len(errors)} op_routes={len(routes)} ui_cases={len(counts)} stories={len(stories)}")
    for item in errors[:20]:print("ERROR",item)
    for item in notes:print("NOTE",item)
    return 1 if errors and args.strict else 0

if __name__=="__main__":
    raise SystemExit(main())
