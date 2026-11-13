# Measured Linux prototype (2026-11-14 JST)

This is the SH1-162 headless prototype measurement, **not full G1 qualification**.
The actual inputs are one synthetic space/tab without a repository, agents or
external commands. Git Graph and reviewer display their `No folder` screens.
The three bundled mods render workspace chrome. All five unmodified JS bundles
loaded into one shared VM and five native 800 × 600 headless windows on pro-dev.
The source pins are in `scripts/prepare-fixtures.py`.

| Observed metric | Measured value | Design comparison |
| --- | ---: | ---: |
| Runner idle RSS peak (includes VM and five windows) | 99,524,608 bytes (99.52 MB) | ≤ 150 MB |
| Runner idle CPU (one logical CPU = 100%) | 0.0169214% | < 1% |
| Compressed snapshot p95 | 1,959 bytes | ≤ 65,536 bytes |
| Worker processes | none implemented | no separate worker measurement |
| Descendant processes observed | 0 in all process samples | no child workload |

Measurement start: 2026-11-13T17:43:00Z. Linux 7.0.14-19-pve x86_64,
Rust 1.99.1, dev build with debuginfo=0 (not release). The idle window is
5.009–64.106 seconds after launch: 60 `/proc` samples, 59.097 seconds between
first/last CPU counters. CPU uses process utime+stime divided by CLK_TCK and
monotonic elapsed time. Child detection reads every task's children file.

The size sample is **five initial publications, one per mod**, not an interactive
workload distribution. p95 is nearest rank (ceil(0.95 × 5), the maximum here).
Sizes include the complete JSON wire envelope compressed at zstd level 3;
frame headers and separate SVG asset files are excluded. All five `.frames`
files were independently reassembled, decompressed and decoded with
`yebisu-mod-wire`; SVG bytes matched the manifest digests and lengths.

Raw metrics, process samples, receiver summaries and the measured binary SHA-256
are in `measurements/`. Reproduce with the preparation and measurement commands
in README.md; write each run to a new output directory.

The native timer/layout integration test provides separate evidence that notify
advances frames and the receiver observes updates. This idle run does not test
populated git data, row delivery, input interaction, authenticated transport,
16 copies, backpressure, pause/resume, grants, updater, restart or 24-hour service
behavior. Those remain in TODO.md. Desktop portal D-Bus warnings are cosmetic
for this capture: each mod's received publication and clean process exit were
observed despite them; no host service was changed.
