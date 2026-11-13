#!/usr/bin/env python3
"""Copy pinned reference mods to a NEW isolated home; never run installers."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

SOURCES = [
    ("herdr-app", "6a0cbc041979b16756d3d0456c571afb6aff2e34", ["workspace-status", "terminals-bar", "tabs", "workspace-chrome", "files", "changes", "pane"], "defaults/mods"),
    ("herdr-git-graph", "0da886243b2e27a6068d6adb7c5c26f349cdc04e", ["git-graph"], "mods"),
    ("herdr-reviewer", "01e6d108a8fcc6aeaa07df5c068af59bdd642117", ["reviewer"], "mods"),
]

def main():
    if len(sys.argv) != 5:
        sys.exit("usage: prepare-fixtures.py <herdr-app> <herdr-git-graph> <herdr-reviewer> <new-home>")
    output = Path(sys.argv[4]).resolve()
    if output.exists():
        sys.exit("output must not exist")
    records = []
    for raw, (name, expected, mods, prefix) in zip(sys.argv[1:4], SOURCES):
        source = Path(raw).resolve()
        actual = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
        if actual != expected:
            sys.exit(f"{name}: expected {expected}, got {actual}")
        # A clean reference is essential: a commit id alone does not identify modified files.
        dirty = subprocess.check_output(["git", "-C", str(source), "status", "--porcelain", "--", prefix], text=True)
        if dirty:
            sys.exit(f"{name}: reference mods are modified")
        for mod in mods:
            src = source / prefix / mod
            files = sorted(p for p in src.rglob("*") if p.is_file())
            if any(p.is_symlink() for p in src.rglob("*")):
                sys.exit(f"symlinks are not accepted: {src}")
            shutil.copytree(src, output / "mods" / mod)
            manifest = {str(p.relative_to(src)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
            records.append({"mod": mod, "source": name, "commit": actual, "files": manifest,
                            "bundle_sha256": hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()})
        for license_file in source.glob("*LICENSE*"):
            shutil.copyfile(license_file, output / f"{name}-{license_file.name}")
        notice = source / "THIRD-PARTY-NOTICES"
        if notice.exists():
            shutil.copyfile(notice, output / f"{name}-THIRD-PARTY-NOTICES")
    (output / "sources.json").write_text(json.dumps(records, indent=2) + "\n")

if __name__ == "__main__":
    main()
