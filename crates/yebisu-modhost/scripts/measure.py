#!/usr/bin/env python3
"""Measure a real offline five-mod capture; retain proc samples and exit status."""
import json
from datetime import datetime, timezone
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

MODS = "workspace-status,terminals-bar,tabs,git-graph,reviewer"

def proc(pid):
    text = Path(f"/proc/{pid}/stat").read_text()
    fields = text[text.rindex(")") + 2:].split()
    rss = 0
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            rss = int(line.split()[1]) * 1024
    children = set()
    for task in Path(f"/proc/{pid}/task").iterdir():
        try:
            children.update((task / "children").read_text().split())
        except FileNotFoundError:
            pass
    return {"pid": pid, "ticks": int(fields[11]) + int(fields[12]), "rss_bytes": rss,
            "children": [proc(int(child)) for child in children]}

def main():
    if len(sys.argv) != 4:
        sys.exit("usage: measure.py <binary> <pinned-home> <new-output>")
    binary, home, output = map(lambda v: Path(v).resolve(), sys.argv[1:])
    output.mkdir(exist_ok=False)
    env = os.environ.copy()
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)
    raw = []
    started_utc = datetime.now(timezone.utc).isoformat()
    with (output / "stderr.log").open("w") as err, (output / "stdout.log").open("w") as out:
        child = subprocess.Popen([str(binary), str(home), str(output), "70", MODS], env=env, stdout=out, stderr=err)
        started = time.monotonic()
        try:
            while child.poll() is None:
                sample = proc(child.pid)
                sample["elapsed_seconds"] = time.monotonic() - started
                raw.append(sample)
                time.sleep(1)
        finally:
            if child.poll() is None:
                child.terminate()
            code = child.wait()
    (output / "proc-samples.json").write_text(json.dumps(raw, indent=2) + "\n")
    idle = [s for s in raw if 5 <= s["elapsed_seconds"] <= 65]
    if code or len(idle) < 50:
        sys.exit(f"capture failed: code={code}, idle samples={len(idle)}; see stderr.log")
    missing = [mod for mod in MODS.split(",") if not list(output.glob(f"{mod}-*.publish.json"))]
    if missing:
        sys.exit(f"no received snapshot for: {missing}")
    received = [json.loads(subprocess.check_output([str(binary), "inspect", str(p)], text=True))
                for p in sorted(output.glob("*.frames"))]
    (output / "received.json").write_text(json.dumps(received, indent=2) + "\n")
    sizes = sorted(p.stat().st_size for p in output.glob("*.zst"))
    if not sizes:
        sys.exit("no compressed wire snapshots received")
    dt = idle[-1]["elapsed_seconds"] - idle[0]["elapsed_seconds"]
    cpu = 100 * (idle[-1]["ticks"] - idle[0]["ticks"]) / os.sysconf("SC_CLK_TCK") / dt
    result = {
        "classification": "measured prototype; offline synthetic space/tab without repository; not full G1 qualification",
        "started_utc": started_utc,
        "process_model": "one runner process includes shared VM and five native windows; no worker subprocess implementation",
        "build_profile": "dev, debuginfo=0",
        "os": platform.platform(), "mods": MODS.split(","), "binary_sha256": __import__("hashlib").sha256(binary.read_bytes()).hexdigest(),
        "window_pixels": [800, 600], "windows": 5, "cpu_definition": "one logical CPU = 100%; process utime+stime / monotonic wall interval",
        "measurement_window_seconds": [idle[0]["elapsed_seconds"], idle[-1]["elapsed_seconds"]],
        "idle_cpu_percent": cpu, "idle_rss_peak_bytes": max(s["rss_bytes"] for s in idle),
        "idle_rss_last_bytes": idle[-1]["rss_bytes"], "sample_count": len(idle),
        "children_observed": [s for s in raw if s["children"]],
        "snapshot_samples": len(sizes), "snapshot_compressed_p95_bytes": sizes[math.ceil(.95 * len(sizes)) - 1],
        "snapshot_compressed_max_bytes": max(sizes), "compression": "zstd level 3, complete wire envelope",
        "thresholds": {"rss_bytes":150_000_000,"cpu_percent_exclusive":1,"snapshot_p95_bytes":65536},
        "exit_code":code,
    }
    (output / "measurements.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))

if __name__ == "__main__":
    main()
