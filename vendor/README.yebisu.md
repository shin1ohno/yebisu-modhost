# Provenance of the runtime copy

These directories are copied from https://github.com/herdrdev/herdr-app at
`6a0cbc041979b16756d3d0456c571afb6aff2e34` (read-only local reference).
Herdr's existing patches and notices are retained. The gpui-kit workspace
identifies its upstream base as longbridge/gpui-kit `1ccf011` and uses GPUI
`=0.3.8`. This is a copy for yebisu-modhost; no change is made to Herdr projects.

- gpui-kit, gpui-pre, gpui-pre-{linux,macos,windows,apple,wgpu}: Apache-2.0;
  each original LICENSE-APACHE remains in place. The embedded shell
  `rquickjs-compat` facade declares MIT in its original Cargo.toml; that
  declaration is retained.
- quickjs-jit-runtime, taffy, zed-xim, xim-ctext: MIT; their original LICENSE
  files and copyright notices remain in place. These are transitive source
  patches used by the reference app and are not relicensed.

## Yebisu delta

Only these copies change from the reference:

- shell/snapshot.rs: public immutable `root`, `arena`, `generation` accessors;
  reexports of existing node/op/value and style-table types/functions.
- shell/spec.rs: visibility of Component and two component payload types so
  the external adapter can match a frozen node. Mutable arena access is not
  added to RenderSnapshot; unsupported components are rejected by the adapter.
- shell/view.rs: read access to the retained input, its registered events,
  focus handle and native scroll state. No JS execution API is exposed.
- shell/entities.rs: retain the names of registered input events alongside
  their existing subscriptions; no event is emitted by export.
- base/input/base/state.rs: read the active selection in UTF-16 units.
- gpui-pre/window.rs: install a headless wake callback and drive one bounded
  draw/layout/arena-clear/callback cycle. The callback batch is taken before
  drawing; callbacks queued by the batch wait for the next cycle.
- gpui-pre/profiler/journal.rs: use AtomicUsize::try_update, the Rust 1.99 name
  for fetch_update, to keep the strict warning gate clean.
- quickjs-jit-runtime: retain upstream optional diagnostic code with two narrow
  dead-code annotations and an underscore parameter for test-only diagnostics.

The root Cargo.lock pins rsa `0.10.0-rc.18` and pkcs1 `0.8.0-rc.4` to the
reference lock. Fresh resolution to rc.19 / 0.8.0 caused six LLRT RSA generic
inference failures; no crypto implementation was modified to work around them.

`scripts/audit-vendor.py` in the runner crate compares every copied file with
an exact, clean reference checkout and emits the changed files and line counts.

`vendor/rustfmt.toml` disables formatting of copied sources so workspace fmt
does not create unrelated upstream formatting changes.
