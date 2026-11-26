# Follow-up work

- [ ] Give the runner binary a live transport to a renderer.
  - Description: the protocol cores (`yebisu-mod-runner`, `yebisu-mod-renderer`)
    implement the handshake, adoption, rows/assets, gateway and device-permission
    rules and are tested against each other over in-memory control frames, but
    `yebisu-modhost` still writes offline captures and its `HostGateway` has no
    `CopyLink`. Carry channel-0 control frames and data chunks over the SSH
    channel, answer Hello with `answer_hello`, send publications as
    PublishBegin + raw chunks, route GatewayReply/DeviceReply to their clients,
    and pass received Actions to `DeviceClient`.
  - Reason: the offline prototype has no attach command or session transport
    (crates/yebisu-modhost/README.md "Scope and remaining contracts").
  - First step: add a stdio framed mode to `yebisu-modhost` that serves one copy
    to a `yebisu-mod-renderer` test client in a child process.
  - Close condition: an end-to-end test drives the binary over that transport
    from Hello to an accepted publication, a gateway rejection and a device
    permission expiry.
- [ ] Map herdr-app's host API arguments onto the wire gateway operations.
  - Description: herdr-app's `run(name, args)` takes a JSON object of named
    arguments while `GatewayOperation::Run.args` is a string list;
    `openOverlay(spec)` names a mod or layout and `notify(n)` a notification,
    while the wire carries an `Overlay`; both herdr-app calls return an overlay
    ID synchronously, while the gateway answers asynchronously
    (herdr-app src/mods.rs:1263-1346). `HostGateway` therefore maps only
    `setAgentOrder`, `closeOverlay` and `run` without arguments.
  - Reason: changing the wire shapes is a contract decision for the design
    owner, not an adapter detail.
  - First step: propose the `Run.args` and overlay-spec representations in a
    design note and get the owner's decision.
  - Close condition: every gateway call of the five bundled/top mods has a wire
    mapping with a receiving-side test.
- [ ] Route the shell's clipboard and open-URL calls through device permissions.
  - Description: gpui-shell's `__clipboard_write_text` and `__open_url` call
    GPUI directly (vendor/gpui-kit/crates/shell/src/engine/quickjs/host.rs:92-94);
    on the runner they must instead retain the current Action's permission via
    `DeviceClient` and send a `DeviceRequest` (open-URL values as
    `normalize_url` output).
  - Reason: the vendored shell has no hook for a host-side device gateway.
  - First step: add a host callback to the vendored shell for these two calls,
    gated by the existing capability check.
  - Close condition: a headless fixture's tap handler opens a URL through a
    retained permission and the renderer core accepts it once.
