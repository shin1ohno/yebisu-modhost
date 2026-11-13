#!/usr/bin/env python3
"""Compare the entire runtime copy to an exact local reference commit."""
import difflib
import json
from pathlib import Path
import subprocess
import sys

reference, target = map(Path, sys.argv[1:])
commit = subprocess.check_output(["git", "-C", str(reference), "rev-parse", "HEAD"], text=True).strip()
assert commit == "6a0cbc041979b16756d3d0456c571afb6aff2e34"
assert not subprocess.check_output(["git", "-C", str(reference), "status", "--porcelain", "--", "vendor"], text=True)
changes = []
compared = 0
for directory in sorted(p for p in target.iterdir() if p.is_dir()):
    source = reference / "vendor" / directory.name
    paths = set(p.relative_to(source) for p in source.rglob("*") if p.is_file()) | set(p.relative_to(directory) for p in directory.rglob("*") if p.is_file())
    for rel in sorted(paths):
        old, new = source / rel, directory / rel
        compared += 1
        if old.is_file() and new.is_file() and old.read_bytes() == new.read_bytes():
            continue
        before = old.read_text().splitlines() if old.exists() else []
        after = new.read_text().splitlines() if new.exists() else []
        delta = list(difflib.unified_diff(before, after, n=3))
        changes.append({"path": str(Path(directory.name) / rel), "added": sum(l.startswith("+") and not l.startswith("+++") for l in delta), "removed": sum(l.startswith("-") and not l.startswith("---") for l in delta), "diff": delta})
print(json.dumps({"source_commit": commit, "files_compared": compared, "changes": changes}, indent=2))
