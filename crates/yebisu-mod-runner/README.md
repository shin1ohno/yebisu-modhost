# yebisu-mod-runner

GPUI-independent protocol core of the option-E runner. The `yebisu-modhost`
binary drives it from the mod's host API; this crate does not run JavaScript.

- `answer_hello` checks the renderer's protocol, schema, exact reflection
  table digests and required features, and acknowledges the smaller of both
  sides' limits.
- `CopyLink` is one copy's control link: it stamps outgoing envelopes with
  the session, copy scope and owner fence and authenticates incoming ones
  against the same identity. `encode_control` / `decode_control` carry one
  control message per channel-0 DATA frame and refuse anything over the
  negotiated control size before decoding.
- `GatewayClient` turns `run`, `set_agent_order`, `open_overlay`,
  `close_overlay` and `notify` into `GatewayRequest`s with an explicit target
  and Policy revision. With no operating connection a request fails at once
  with `no_operating_connection`; a request the wire would reject is
  `invalid_request` and one over the control size is `limit_exceeded`, both
  without being sent. A reply settles only the pending request it names, and
  only when it arrives in the scope that request was sent in; a disconnect
  fails every pending request with `cancelled`.

`tests/` joins a runner and a `yebisu-mod-renderer` over in-memory control
frames, from Hello/HelloAck to every gateway outcome.
