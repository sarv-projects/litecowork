#!/usr/bin/env python3
"""Add absent first-party architecture Markdown to audit-inventory.csv without rewriting historical baseline rows."""
from pathlib import Path
import csv,hashlib,re
R=Path(__file__).resolve().parents[1]
file=R/"implementation/audit-inventory.csv"
with file.open(newline="",encoding="utf-8-sig") as f:
 reader=csv.DictReader(f)
 fields=list(reader.fieldnames or [])
 rows=list(reader)
if not fields or "path" not in fields:raise RuntimeError("Unexpected historical inventory format")
existing={r["path"] for r in rows}
added=[]
for doc in sorted((R/"docs").glob("*.md")):
 rel=doc.relative_to(R).as_posix()
 if rel in existing:continue
 data=doc.read_bytes();txt=data.decode("utf-8-sig")
 heads=[re.sub(r"^#{1,6}\s*","",line).strip() for line in txt.splitlines() if re.match(r"^#{1,6} ",line)]
 entry={"path":rel,"baseline_lines":str(len(txt.splitlines())),
        "baseline_sha256":hashlib.sha256(data).hexdigest(),
        "audit_method":"new current-tree design source; full bytes/lines read, owner and contract mapping must be reviewed",
        "headings_or_sections":"; ".join(heads[:22])}
 rows.append(entry);existing.add(rel);added.append(rel)
if added:
 with file.open("w",encoding="utf-8",newline="") as f:
  writer=csv.DictWriter(f,fieldnames=fields)
  writer.writeheader()
  writer.writerows(sorted(rows,key=lambda r:r["path"]))
print("AUDIT_INVENTORY_NEW",len(added),"TOTAL",len(rows))
for x in added:print("+",x)
