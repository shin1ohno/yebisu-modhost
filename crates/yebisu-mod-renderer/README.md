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

Protocol 1 enables no compression format implicitly; `Uncompressed` is the
only `Inflate` provided here.
