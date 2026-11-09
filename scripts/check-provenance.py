#!/usr/bin/env python3
"""Check copied file coverage and immutable source revisions."""
import csv
import re
import subprocess
from pathlib import Path
rows = list(csv.DictReader(Path("PROVENANCE.tsv").open(), delimiter="\t"))
seen = set()
for row in rows:
    assert row["kind"] in {"copy", "model"}, row
    assert re.fullmatch(r"[a-f0-9]{40}", row["source_commit"]), row
    assert row["source_repository"].startswith("https://github.com/"), row
    assert Path(row["destination"]).is_file(), row
    assert row["destination"] not in seen, row
    seen.add(row["destination"])
tracked = subprocess.check_output(["git", "ls-files"], text=True).splitlines()
for path in tracked:
    if path.startswith(("assets/fonts/", "crates/yebisu/src/upstream/")):
        assert path in seen, f"unregistered copy: {path}"
print(f"Provenance: {len(rows)} immutable source rows checked")
