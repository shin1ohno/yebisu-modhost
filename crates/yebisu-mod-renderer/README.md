# yebisu-mod-renderer

GPUI-independent receiving core of the option-E renderer. The iPad renderer
(E-1, in yebisu) draws what this crate accepts; this crate never executes mod
code and never draws. It applies the receiving rules that the
[wire README](../yebisu-mod-wire/README.md) assigns to the renderer.

- `ClientHandshake` builds `Hello` and accepts `HelloAck` only with the same
  nonce and protocol and with no limit above the offer. `Negotiated::context`
  builds a `ReceiveContext` only for a copy that HelloAck announced; the host
  is the SSH-verified fingerprint, never a value the runner reports.
- `CopyReceiver` holds one copy's receiving state. A publication arrives as a
  `PublishBegin` header, raw data chunks and a `PublishCommit`; the header is
  checked against the negotiated transfer size before the assembler reserves
  it, inflation stops at the negotiated decompressed size, and the decoded
  publication passes `decode_publish` and `NegotiatedLimits::check_publish`
  before it is adopted. The generation tracker and receipt floors change only
  on adoption.
- Row and asset requests must name a list node or asset of the adopted copy.
  Each is counted with ledgers built from the negotiated values: rows at list,
  copy and application scope, asset transfers at copy, host and application
  scope (`SharedScopes` lends the host and application ledgers, which the
  owner shares across copies, connections and hosts). A refused request leaves
  every ledger unchanged. Replies are authenticated with the copy's
  `ReceiveContext` before the outstanding request is looked up; rows are then
  validated through `RowValidation` built from the adopted copy, and asset
  bytes are released only when size and SHA-256 match the manifest. Adopting
  a copy that no longer has the list or asset ends those requests and returns
  their reservations.

- `Gateway` answers every `GatewayRequest` with one `GatewayReply` and yields
  a `GatewayEffect` only for an accepted request. In order: an envelope from
  another session, copy incarnation or owner fence, or a body target that
  does not match the authenticated copy, is `stale_target`; a paused or
  unloaded copy is `cancelled`; a connection that is not the operating one
  (including background execution) is `no_operating_connection`; a stale
  Policy revision, a `run` command outside the allowlist or a disallowed
  `set_agent_order` is `policy_denied`. Overlays and toasts belong to the
  copy incarnation whose envelope opened them; naming another copy, reusing an
  open ID or closing another copy's overlay is `not_owned`. Toasts are limited
  to one per second (`rate_limited`) and three visible (`limit_exceeded`).
  `open_overlay` carries only an ordinary overlay, so a toast always passes
  these limits. A toast shows the mod and host names the device registered
  for that copy incarnation (`register_copy`) from its own records (the
  approved mod record, the SSH-verified host), never the payload's or
  HelloAck's.
  Authentication runs before the replay cache: only an authenticated request
  is remembered, keyed by the authenticated session, copy scope, owner fence
  and ID, so a resent request gets the remembered reply and does not run
  again, a forged one can neither read nor fill the cache, and a stopped
  copy's remembered replies are dropped.

Protocol 1 enables no compression format implicitly; `Uncompressed` is the
only `Inflate` provided here.
