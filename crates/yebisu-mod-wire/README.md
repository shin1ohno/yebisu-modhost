# yebisu-mod-wire

GPUI-independent, serde-based wire contract for the option-E host runner and
yebisu renderer. This crate neither executes JavaScript nor grants permissions.

The contract follows option-E v5, appendix A (2026-11-14). The constants in
`limits` are proposed ceilings, not measured performance claims.

## API and receiving order

1. Use `Hello`, `Digests`, `NegotiatedLimits` and `HelloAck` to negotiate protocol
   `1`, schema `e1`, exact reflection-table digests and required features. Use
   `decode_hello_ack_bootstrap` only to display the peer's version when incompatible.
2. Use `FrameHeader::decode` before reserving memory. The nine-byte header is
   `[channel u32 LE][kind u8][len u32 LE]`. DATA=0, CREDIT=1, CLOSE=2. Channel zero
   carries control messages (64 KiB); other channels carry fragments (16 KiB).
   This is a separate protocol; terminal RESIZE/FOCUS/FACTS kinds are rejected.
3. `encode`/`decode::<T>` handle a typed `Envelope<T>`, validate its `type`, protocol,
   schema, JSON grammar, counters, duplicate keys and message limits. Body-specific
   shape checks run for publications, edits, actions, entity creation/updates,
   receipt entity states, row shapes, state projections and URL requests before
   serialization. These functions do not establish freshness, authority or table
   identity.
4. **Use `encode_publish` / `decode_publish` for snapshots.** The latter additionally
   checks the externally supplied `ReceiveContext`, `ValidationTables`,
   `ReceiptFloors`, and `GenerationTracker`. Tables default to empty (deny unknown).
   Keep the scalar, copy-wide generation high watermark until the copy incarnation
   changes, including across epoch changes. Generations are unique across views. Commit the tracker and displayed publication only after atomic
   adoption. Receipt floors advance from terminal receipts, never from OpRejected.
5. Use `EntityUpdate::validate_against`, `EntityRelease::validate_against`,
   `RowsReply::validate_against` and `AssetReply::validate_against` with the actual
   prior entity, outstanding request, or manifest. For rows, first construct
   `RowValidation::new(&adopted_publish, &tables)` and pass it as the third argument.
   Its immutable borrow resolves row entities, assets, lists and op arguments
   against the currently adopted copy. List identity within that copy is
   `(view_generation, list_id)`: `handler_generation` must match the view
   containing the List/VirtualList node. The same ID can appear in different
   view generations, but duplicate manifests for the same pair are rejected.
   Rows require the matching list node in the requested view; an unused manifest
   alone does not authorize delivery. Rows cannot contain List/VirtualList nodes
   or any list reference: a list drawn inside a row is not owned by an adopted
   view node, so no RowsRequest for it could ever be valid (A7).
   Authenticate the reply envelope with
   `ReceiveContext` before checking the outstanding request. No request identity is inferred
   from an ID alone. A missing entity never substitutes for a release tombstone;
   the receiver supplies its validated release ledger to `ReceiptFloors::check`.
   A tombstone can cover only a missing entity. An exact released incarnation
   cannot reappear at any revision.
6. Reserve shared resources with `ResourceLedger` **before** allocation. Keep one
   ledger at each applicable list/copy/host/application scope, shared across all
   connections. `NegotiatedLimits::intersect` carries every A8 ceiling (including strings, entities, copies, decoded assets,
   application-wide budgets, timeouts and the coordinate cap) and only lowers
   them; zero, non-finite and over-ceiling values are rejected. The transport
   and receiver must apply any smaller negotiated values before accepting work.

Graph validation covers ordinary children, slots, state styles and ChildViews
in one graph. It rejects cycles, unreachable nodes and bad indexes, and counts
each repeated edge with saturating visit and weighted operation costs. RowsReply
sums expanded visits and operations across every row root, even when each row
individually fits. This is performed before materialization. Row trees cannot
register callbacks or contain ChildViews (including a ChildView with no child
payload) or nested lists. Row content hashes include the root, nodes and stable key.

## Concrete protocol-1 representations

The design leaves these representations to E-wire:

- All u64 counters are canonical decimal JSON strings (`Counter`); indexes,
  lengths and dimensions are JSON integers. IDs/nonces are 32 lowercase hex
  digits (`Id128`); SHA-256 digests are 64 lowercase hex digits (`Digest`).
- `Node` has a closed `Component` tag, optional label/document text, entity,
  asset, document format, ChildView/list references, children and typed operations.
  Component-specific behavior and style arguments use `Op::Method` / `ParamStyle`.
  Module, Registered, Editor, DockArea, DockContent and inline-token inputs have
  no accepted wire variant. Native object references and executable code never
  cross this boundary.
- Retained View entities and every ViewSnapshot must agree on the full entity
  reference, root and generation. `EntityState::View.parent` is an `EntityRef`,
  and must match the incoming ChildView edge, including kind and incarnation.
  This also applies to retained views with no node entity binding.
- `EntityConfig` / `EntityState` are closed tagged unions. OTP supports 1–32
  Unicode scalar values; Calendar supports 1–12 displayed months, years 1–9999
  and valid Gregorian dates. These ranges belong in the consumer's compatibility
  record, as required by A2. OTP length is immutable on update.
- JSON nesting has a separate ceiling of 64; view depth remains 256 because
  arena edges are indexes, not recursive JSON objects.
- A canonical digest uses compact UTF-8 JSON with object keys sorted
  lexicographically and the indicated top-level digest field omitted. Strings
  retain their original Unicode normalization and line endings. `Edit` and
  `Action` validate their body digest before encoding/after decoding.
- `PublishAssembler` reserves one whole publication, requires numbered,
  nonempty bounded fragments and matching Begin/Commit/content digests, then
  returns only complete bytes. Its owner drops it on replacement, expiry or
  Pause. No compression format is enabled implicitly; `read_bounded` is the
  output ceiling for a separately negotiated decompressor.
- `AssetReply` is a logical, assembled response; raw image bytes should travel
  over credited data fragments, not as a single channel-zero control message.
  Verify the image header and decoded size before decoding the content. An
  asset manifest does not prove that its bytes form a safe image or SVG.

## Runtime boundaries

The runner/renderer owns per-copy FIFO execution, op deduplication and cancellation
ledgers, subscriptions and callback ownership, displayed leases, replay age,
IME composition buffering, atomic entity/view adoption, overlay parent lifetime,
resource reservation and release, CPU/time budgets, image decoding, SVG/document
sanitization, and current Policy checks on admission and execution. The types
`ExecAuthority`, `Reconcile`, `BgGrant`, and device requests are data contracts;
they are not an authorization engine or a persistent grant store. URL validation
allows HTTPS only and returns a normalized host for the separate confirmation UI.

F1 from the final design review belongs to the setup update/recovery workflow;
this crate includes update as a `StopSource` and carries the exact op ID and
`persisted` flag. F2 belongs to the runner's clock adapter; this crate preserves
OS boot ID and continuous-clock fields without asserting any LXC clock identity.
F3 is the design QA count; six IME scenarios must be retained by the integration
QA owner. None of these findings is presented here as a measured runtime fix.


## Device and command gateways (A12/A13)

The device attaches typed `DevicePermission` values to `ActionData.permissions`
on individual Click/Key events. Each opaque token binds one operation and Policy
revision; the Action supplies event_id, and its envelope supplies epoch, copy
incarnation and owner fence. An async continuation explicitly retains that
permission and sends its token, event and Policy revision in `DeviceRequest`.
`validate_permission` checks this hand-off, **not** authorization. The device
still owns the one-second monotonic-clock ledger, one-use consumption, duplicate
results and all invalidation/current-connection checks in A12.

`GatewayRequest` carries a request ID, Policy revision, explicit
`GatewayTarget { boot, copy, incarnation }`, and a tagged operation:
`run`, `set_agent_order`, `open_overlay`, `close_overlay`, or `notify`.
`GatewayTarget::validate_against` compares an externally authenticated target.
The receiving device checks its command allowlist, current target and Policy;
overlay ownership is inherited from the request envelope and checked against
the device's overlay registry. It returns `GatewayReply` with `ok` or
`failed { reason }`; absence of an operating connection is the explicit
`no_operating_connection` failure, including background execution.

`Overlay.kind` is `overlay` or `toast`. Toasts require
`ToastContent { message, mod_name, host_name }`; ordinary overlays have
`toast: null`. Notify accepts only a toast. The renderer enforces one toast
per second, at most three simultaneous toasts, parent/child publication
readiness and placement below yebisu confirmation sheets. These are runtime
obligations, not guarantees provided by serialization.
