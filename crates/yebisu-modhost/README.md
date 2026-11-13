# Linux headless runner prototype (SH1-162)

This crate loads unmodified Herdr JS mod bundles into gpui-shell 0.7.1 on
GPUI 0.3.8's native Linux headless platform. No X11/Wayland display or GPU
render target is required. Herdr, herdr-core and herdr-app are never modified,
launched, signalled, or contacted.

The prototype takes an **isolated, copied bundle home** and writes snapshots
to a separate output directory. It has no daemon, service installation,
approval store, SSH transport, attach command, or interactive client. Installation
and updates remain the owner's responsibility; the accepted production plan is
`cargo install` through the setup cookbook, not GitHub Release binaries.

## Inputs and execution

`scripts/prepare-fixtures.py` accepts local clean clones of herdr-app,
rabeeh-ta/herdr-git-graph and vytasgavelis/herdr-reviewer at the commits pinned
in that script. It creates a new scratch home and records each copied file's
SHA-256 plus the source commit. It does not execute the projects' installers.
The Git Graph and reviewer entries were checked in https://herdr.dev/gallery/?type=mods.

```sh
python3 crates/yebisu-modhost/scripts/prepare-fixtures.py \
  /path/to/herdr-app /path/to/herdr-git-graph /path/to/herdr-reviewer /scratch/pinned-home
cargo build -p yebisu-modhost
./target/debug/yebisu-modhost /scratch/pinned-home /scratch/capture 10 \
  workspace-status,terminals-bar,tabs,git-graph,reviewer
python3 crates/yebisu-modhost/scripts/measure.py \
  ./target/debug/yebisu-modhost /scratch/pinned-home /scratch/measurements
```

The Herdr adapter supplies one explicitly synthetic space and tab (no live sessions,
focused repository or agents), a fixed palette, and ephemeral shared state.
`exec`, `hc`, filesystem operations, client commands/overlays/toasts return
explicit errors; `nextCommand` waits without polling. These captures demonstrate
VM initialization, layout and snapshot export, not git operations, reviews sent
to agents, or compatibility with every screen of these mods.

Measured Linux results and their workload limits are in [MEASUREMENTS.md](MEASUREMENTS.md).

## Frame ownership

A capacity-one wake channel coalesces GPUI invalidations and next-frame demand.
The runner waits for a wake, limits each copy to one cycle every 34 ms, runs
native draw/layout, clears its element arena, and executes the callbacks which
were pending before the cycle. Newly queued callbacks run on a later cycle.
When no dirty view/callback remains it waits for the next wake. Native timers
and asynchronous completions run on GPUI's executor and invalidate their own
views. After native layout the capture checks both the shell generation and
all previously published retained entity state, config, selection and subscriptions.
An Input-only host update produces a new publication/state revision without
requiring its parent to rebuild. Unchanged wakeups do not publish. The single
root view receives a fresh copy-wide generation on each publication; this offline
prototype does not retain callback leases for interactive dispatch.

The copied Input selection getter returns UTF-16 positions. The adapter converts
those positions against the same captured text to wire UTF-8 byte offsets,
rejecting invalid boundaries. Its wire action table is empty, matching the offline
host's empty command catalog; named ActionCallback operations are rejected until
an interactive adapter supplies a negotiated action catalog. The new handshake,
row validation, permission hand-off, gateway and overlay/toast paths are not used
by this offline writer and remain production work in TODO.md.

The native headless integration test exercises a real JS timer, `cx.notify`,
draw and the wire-decoded text from initial state 0 through state 3. It also
checks a layout callback and rejects a valid frame under a mismatched receiving
session. Native Input fixtures also verify Japanese/emoji and mixed-accent cursor
positions and Input-only value/config/selection timer updates received without
a parent rebuild.
A missing mod must fail before starting the application.

Each publication writes `.frames` (bounded wire frames carrying zstd-compressed
wire JSON), `.identity.json` (offline expected receiving context), `.publish.json`
and `.tree` diagnostics, plus SVG assets by digest. Verify a capture with
`yebisu-modhost inspect /scratch/capture/reviewer-0001.frames`; this independently
reassembles, decompresses and validates the envelope and asset bytes. The local
identity sidecar is not authenticated and is not an SSH session or client lease.
The measurement script runs this receiving check on every emitted publication.

## Scope and remaining contracts

This is a Linux prototype, not the full G1 gate or a deployable resident service.
The adapter currently rejects unsupported components and nonempty virtual-list
row captures instead of inventing their state. Output is offline and cannot be
used as an interactive lease: callback retention/dispatch, input editing,
row/asset delivery, client-side publication, authority, pause/unload and
background grants are not implemented.

Design review F1: a future update recovery which finds a new disk binary must
skip **build/rename only**, then request shutdown and correlate this operation's
StopRequested/StopAck before restart. This prototype has no updater or shutdown
IPC, and does not claim that gate passed.

Design review F2: `/proc/sys/kernel/random/boot_id` on pro-dev is a separate
`none[/.lxc-boot-id]` tmpfs mount; the observed time-namespace offsets are zero.
Neither fact proves that the ID and CLOCK_BOOTTIME share a lifetime across CT,
parent-host or namespace restarts. No background-grant restoration is implemented.
A future implementation must validate those cases or stop safely until time
can be verified; it must never assume the pair comes from the parent host.

Review F3: the production G4 matrix has six sequences. No G4 check is claimed here.
