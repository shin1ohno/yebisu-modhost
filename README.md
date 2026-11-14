# yebisu-modhost

Host runner for yebisu option-E mods: runs herdr-app JS mods on the host and
sends render snapshots to the iPad.

The workspace contains:

- [`yebisu-mod-wire`](crates/yebisu-mod-wire): GPUI-independent snapshot, retained
  entity, input/event receipt, virtual-row, asset, control and framed-transport
  types with bounded decoding and validation.
- `yebisu-modhost`: a placeholder for the separately developed host runner.

See the [wire contract and API](crates/yebisu-mod-wire/README.md) for receiving
order, protocol representations, limits and the consumer's runtime obligations.

```sh
cargo fmt --check --all
cargo build --workspace
cargo test --workspace
RUSTFLAGS='-D warnings' cargo clippy --workspace --tests
```
