# Follow-up work

- [ ] Adopt the revised wire contract in the runner and renderer.
  - Description: migrate retained View parent references, create a matching View
    entity for every snapshot, allocate copy-wide generations, register action
    names, supply RowValidation from the adopted copy, and construct all new
    handshake, ActionData and Overlay fields. When the runner gains List or
    asset delivery, give List nodes a VirtualScroll entity and send asset bytes
    as raw data-channel chunks named by AssetReply.channel (VirtualList and the
    offline Publish frames already do), and put normalize_url output in
    DeviceRequest. Implement GatewayRequest/Reply
    routing and explicit device-permission hand-off with the receiver's policy,
    target, ownership, expiry and one-use checks.
  - Reason: the runner is an offline prototype; it emits no live control
    messages, List nodes, asset replies or device requests yet.
  - First step: run cargo check --workspace --all-targets, then migrate call
    sites using the wire README's receiving order and gateway representations.
  - Close condition: runner/renderer compile with this contract, apply smaller
    negotiated limits at each receiving/resource scope, and their integration
    tests demonstrate gateway rejection and device permission expiry,
    cancellation and one-use consumption. Remove this entry in that commit.
