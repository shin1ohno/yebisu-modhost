# yebisu-modhost

Host runner for yebisu option-E mods: runs herdr-app JS mods on the host and
sends render snapshots to the iPad.

The workspace contains:

- [`yebisu-mod-wire`](crates/yebisu-mod-wire): GPUI-independent snapshot, retained
  entity, input/event receipt, virtual-row, asset, control and framed-transport
  types with bounded decoding and validation.
- [`yebisu-mod-renderer`](crates/yebisu-mod-renderer/README.md): GPUI-independent
  receiving core of the renderer: handshake, atomic publication adoption,
  row/asset delivery under the negotiated limits at each resource scope, the
  command gateway and the device-permission ledger.
- [`yebisu-mod-runner`](crates/yebisu-mod-runner/README.md): GPUI-independent
  protocol core of the runner: the HelloAck answer, per-copy links, and the
  gateway and device-permission clients; its integration tests talk to `yebisu-mod-renderer`.
- [`yebisu-modhost`](crates/yebisu-modhost/README.md): Linux headless prototype
  that loads copied mod bundles, drives native frames on notifications, and
  writes wire snapshots with an independent receiving-side inspector.

See the [wire contract and API](crates/yebisu-mod-wire/README.md) for receiving
order, protocol representations, limits and the consumer's runtime obligations.

```sh
cargo fmt --check --all
cargo build --workspace
cargo test --workspace
RUSTFLAGS='-D warnings' cargo clippy --workspace --tests
```
