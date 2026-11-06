"""Compatibility entrypoint for the current active-corpus inventory builder.

The historical implementation/audit-inventory.csv format is retired. Run the
single canonical builder instead of mutating an old inventory in place.
"""
from __future__ import annotations
import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/build_active_corpus_inventory.py"

if __name__ == "__main__":
    namespace = runpy.run_path(str(SCRIPT))
    records = namespace["collect"]()
    namespace["emit"](records)
