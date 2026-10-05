#!/usr/bin/env python3
"""Check that the planned backlog covers the current contracts and preserves its links."""
from __future__ import annotations
import csv, json, re, subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
PLAN = ROOT / 'implementation'
def fail(message: str) -> None:
    print('ERROR:', message, file=sys.stderr)
    raise SystemExit(1)

data = json.loads((PLAN/'backlog.json').read_text())
stories = data.get('stories', [])
by_id = {s.get('id'): s for s in stories}
if len(by_id) != len(stories) or not stories:
    fail('backlog story IDs must be unique and nonempty')
for s in stories:
    for field in ('title','implementation','acceptance','negative_cases','demo'):
        if not s.get(field): fail(f"{s.get('id')}: missing {field}")
    if s.get('status') != 'PLANNED': fail(f"{s['id']}: initial backlog status must be PLANNED")
    if not {'CODE','SYSTEM','USER'} <= {x.rsplit('-',1)[-1] for x in s.get('tests',[])}:
        fail(f"{s['id']}: requires CODE, SYSTEM and USER test IDs")
    for dep in s.get('depends_on',[]):
        if dep not in by_id: fail(f"{s['id']}: unknown dependency {dep}")
    for source in s.get('sources',[]):
        if source not in (PLAN/'SOURCES.md').read_text(): fail(f"{s['id']}: unknown research source {source}")
    for authority in s.get('authorities',[]):
        if not (ROOT/authority).is_file(): fail(f"{s['id']}: missing owning contract {authority}")
# Cycle check
visiting=set();visited=set()
def visit(node):
    if node in visiting: fail(f'cyclic story dependency at {node}')
    if node in visited:return
    visiting.add(node)
    for dep in by_id[node].get('depends_on',[]):visit(dep)
    visiting.remove(node);visited.add(node)
for node in by_id:visit(node)
with (PLAN/'coverage.csv').open(newline='') as f: coverage=list(csv.DictReader(f))
if not coverage: fail('coverage.csv must have contract rows')
covered={(r['kind'],r['contract_or_scenario']) for r in coverage}
for r in coverage:
    if r['primary_story'] not in by_id: fail(f"coverage points to unknown story {r['primary_story']}")
# Current numbered scenarios must all have a coverage row.
for path,pattern,kind in [(ROOT/'docs/FLOWS.md',r'^## (F\d+) — (.+)$','FLOW'),(ROOT/'docs/BENCHMARKS.md',r'^### (B\d+) (.+)$','BENCHMARK')]:
    txt=path.read_text()
    for ident,title in re.findall(pattern,txt,re.M):
        if (kind,ident+' '+title) not in covered: fail(f'{kind} {ident} missing coverage row')
# Every contract document named in current contract map and all machine operation/DDL inventories.
mapfile=json.loads((PLAN/'machine-inventory.json').read_text())
# Compare the recorded contract inventory to today's sources so new operations/events/DDL
# cannot silently disappear from the backlog map. Architecture CI validates their schemas.
import yaml
cur_event=json.loads((ROOT/'docs/schemas/domain-event.schema.json').read_text())
cur_delegation=json.loads((ROOT/'docs/schemas/delegation.schema.json').read_text())
cur_api=yaml.safe_load((ROOT/'docs/schemas/operator-api.openapi.yaml').read_text())
cur_sql=(ROOT/'docs/schemas/sqlite-v1.sql').read_text()
cur_ops=[]
for path,items in cur_api.get('paths',{}).items():
    for method,spec in items.items():
        if method.lower() in ('get','post','put','patch','delete','options','head') and isinstance(spec,dict):
            cur_ops.append({'method':method.upper(),'path':path,'operationId':spec.get('operationId')})
cur_events=(ROOT/'docs/EVENTS.md').read_text()
reg=re.search(r'Minimum v1 registry:\s*```\s*(.*?)```',cur_events,re.S)
cur_types=re.findall(r'^[a-z][a-z0-9_.-]+\.v\d+$',reg.group(1),re.M) if reg else []
cur_objects={
 'operator_operations':sorted(cur_ops,key=lambda x:(x['path'],x['method'])),
 'event_schema_definitions':sorted(cur_event.get('$defs',{})),
 'delegation_schema_definitions':sorted(cur_delegation.get('$defs',{})),
 'event_type_registry':sorted(cur_types),
 'sqlite_tables':sorted(re.findall(r'CREATE TABLE(?: IF NOT EXISTS)?\s+["`]?([\w]+)',cur_sql,re.I)),
 'sqlite_triggers':sorted(re.findall(r'CREATE TRIGGER(?: IF NOT EXISTS)?\s+["`]?([\w]+)',cur_sql,re.I)),
 'sqlite_indexes':sorted(re.findall(r'CREATE (?:UNIQUE )?INDEX(?: IF NOT EXISTS)?\s+["`]?([\w]+)',cur_sql,re.I)),
}
for key,current in cur_objects.items():
    if current != sorted(mapfile.get(key,[]),key=lambda x:(x.get('path',''),x.get('method','')) if isinstance(x,dict) else x):
        fail('machine-inventory.json is stale for '+key+'; regenerate inventory and coverage rows')
for name in mapfile.get('operator_operations',[]):
    key=name['method']+' '+name['path']
    if ('API_OPERATION',key) not in covered: fail('API operation missing: '+key)
for name in mapfile.get('event_type_registry',[]):
    if ('EVENT_TYPE',name) not in covered: fail('registered event type missing: '+name)
for key,kind,file in [('event_schema_definitions','EVENT_DEF','domain-event.schema.json'),('delegation_schema_definitions','SCHEMA_DEF','delegation.schema.json')]:
    for name in mapfile.get(key,[]):
        if (kind,file+'#'+name) not in covered: fail('schema definition missing: '+name)
for plural,kind in [('sqlite_tables','SQL_TABLE'),('sqlite_triggers','SQL_TRIGGER'),('sqlite_indexes','SQL_INDEX')]:
    for name in mapfile.get(plural,[]):
        if (kind,name) not in covered: fail(f'{kind} missing: '+name)
# Local Markdown links in implementation tree.
for md in PLAN.rglob('*.md'):
    text=md.read_text()
    if len(re.findall(r'^```',text,re.M))%2: fail(f'{md.relative_to(ROOT)}: unclosed code fence')
    for raw in re.findall(r'\[[^\]]*\]\(([^)]+)\)',text):
        target=raw.split()[0].strip('<>')
        if target.startswith(('https://','http://','mailto:','#')):continue
        target=target.split('#',1)[0]
        if target and not (md.parent/target).resolve().exists():fail(f'{md.relative_to(ROOT)}: broken link {raw}')
# Audit inventory digests should identify tracked baseline source files.
with (PLAN/'audit-inventory.csv').open(newline='') as f: inv=list(csv.DictReader(f))
if not inv: fail('audit inventory empty')
for required in ('ARCHITECTURE.md','AGENTS.md','docs/IMPLEMENTATION.md','docs/TESTING.md','docs/FLOWS.md','docs/BENCHMARKS.md','docs/schemas/operator-api.openapi.yaml','docs/schemas/domain-event.schema.json','docs/schemas/sqlite-v1.sql','scripts/validate_architecture.py'):
    if not any(r['path']==required for r in inv): fail('audit inventory missing '+required)
for r in inv:
    if len(r['baseline_sha256']) != 64 or int(r['baseline_lines']) < 1:fail('invalid audit inventory row: '+r['path'])
# V1 stage ordering is explicit and no gate is falsely accepted.
scope=(PLAN/'SCOPE.md').read_text()
for term in ('Desktop alpha','Cloud follows','Remote Runtime','user alone','provider','owner'):
    if term.lower() not in scope.lower(): fail('scope omits '+term)
coverage_check = subprocess.run(
    [sys.executable, str(ROOT/'scripts/validate_implementation_coverage.py')],
    cwd=ROOT,
    check=False,
)
if coverage_check.returncode:
    fail('architecture coverage validation failed')
print(f"Implementation plan validation passed: {len(stories)} stories, {len(coverage)} coverage rows, all current numbered flows/benchmarks and machine-contract inventory linked.")
