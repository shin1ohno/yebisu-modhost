# Follow-up work

- [ ] Adopt the revised wire contract in the runner and renderer.
  - Description: migrate retained View parent references, create a matching View
    entity for every snapshot, allocate copy-wide generations, register action
    names, supply RowValidation from the adopted copy, and construct all new
    handshake, ActionData and Overlay fields. Implement GatewayRequest/Reply
    routing and explicit device-permission hand-off with the receiver's policy,
    target, ownership, expiry and one-use checks.
  - Reason: PR #2 is a separate stacked runner change; this wire correction owns
    only feat/mod-wire and must not modify that branch or claim integration QA.
  - First step: update the runner branch to include feat/mod-wire, run
    cargo check --workspace --all-targets, then migrate call sites using the
    wire README's receiving order and gateway representations.
  - Close condition: runner/renderer compile with this contract, apply smaller
    negotiated limits at each receiving/resource scope, and their integration
    tests demonstrate gateway rejection and device permission expiry,
    cancellation and one-use consumption. Remove this entry in that commit.
