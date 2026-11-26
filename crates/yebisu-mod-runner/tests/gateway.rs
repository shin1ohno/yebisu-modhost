//! Runner <-> renderer: handshake and A13 gateway routing over control frames.
//! Each rejection must reach the runner call that is waiting for it.
mod common;
use common::*;
use std::collections::BTreeSet;
use yebisu_mod_renderer::{Gateway, GatewayEffect, GatewayPolicy};
use yebisu_mod_runner::{GatewayClient, GatewaySend, answer_hello};
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{Counter, GatewayFailure, GatewayOperation, GatewayResult, OverlayKind};

fn policy() -> GatewayPolicy {
    GatewayPolicy {
        rev: Counter(5),
        commands: BTreeSet::from(["pane.split".to_string()]),
        agent_order: false,
    }
}
fn run(command: &str) -> GatewayOperation {
    GatewayOperation::Run {
        command: command.into(),
        args: vec![],
    }
}

struct End {
    pair: Pair,
    gateway: Gateway,
}
impl End {
    fn new() -> Self {
        let pair = connect(
            wire::NegotiatedLimits::default(),
            wire::NegotiatedLimits::default(),
        );
        let mut gateway = Gateway::new(policy());
        gateway.set_operating(true);
        // Names from the device's approval records, not from HelloAck.
        gateway.register_copy(&copy("a", 3), "Git Graph".into(), "pro-dev".into());
        gateway.register_copy(&copy("b", 4), "Reviewer".into(), "pro-dev".into());
        Self { pair, gateway }
    }
    /// Runner request -> frames -> renderer gateway -> frames -> runner reply.
    fn call(
        &mut self,
        client: &mut GatewayClient,
        link: &mut yebisu_mod_runner::CopyLink,
        at: &wire::CopyInfo,
        operation: GatewayOperation,
        now_ms: u64,
    ) -> (GatewayResult, Option<GatewayEffect>) {
        let GatewaySend::Sent { envelope, .. } = client.request(Some(link), operation).unwrap()
        else {
            panic!("request settled locally")
        };
        let received = to_renderer(&envelope, &self.pair.limits);
        let context = self.pair.renderer_context(at);
        let (reply, effect) = self.gateway.handle(&received, &context, now_ms);
        let frame = reply_frame(&context, reply, &self.pair.limits);
        let (_, result) = client.receive(link, &frame).unwrap();
        (result, effect)
    }
}
fn failure(result: GatewayResult) -> GatewayFailure {
    match result {
        GatewayResult::Failed { reason } => reason,
        GatewayResult::Ok {} => panic!("accepted"),
    }
}

#[test]
fn the_runner_answers_hello_only_for_exact_tables_and_required_features() {
    let pair = connect(
        lowered(|l| l.rows = 50),
        lowered(|l| l.asset_transfers_host = 1),
    );
    assert_eq!(pair.limits.rows, 50);
    assert_eq!(pair.limits.asset_transfers_host, 1);
    let mut hello = wire::Hello {
        client: CLIENT.into(),
        nonce: id(9),
        protocol: wire::PROTOCOL,
        schema: wire::SCHEMA.into(),
        digests: digests(),
        features: vec!["gateway".into()],
        limits: wire::NegotiatedLimits::default(),
    };
    let identity = serve(wire::NegotiatedLimits::default());
    answer_hello(&hello, &identity).unwrap();
    hello.features.clear();
    assert!(answer_hello(&hello, &identity).is_err());
    hello.features = vec!["gateway".into()];
    hello.digests.command = wire::bytes_digest(b"other commands");
    assert!(answer_hello(&hello, &identity).is_err());
}

#[test]
fn an_allowed_command_runs_once_and_settles_the_waiting_call() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let (result, effect) = end.call(&mut client, &mut link, &a, run("pane.split"), 0);
    assert_eq!(result, GatewayResult::Ok {});
    assert_eq!(
        effect,
        Some(GatewayEffect::Run {
            command: "pane.split".into(),
            args: vec![]
        })
    );
    assert_eq!(client.pending(), 0);
}

#[test]
fn without_an_operating_connection_requests_fail_explicitly() {
    let mut client = GatewayClient::new(Counter(5));
    assert_eq!(
        client.request(None, run("pane.split")).unwrap(),
        GatewaySend::Failed(GatewayFailure::NoOperatingConnection)
    );
    assert_eq!(client.pending(), 0);
    // Connected, but the device is not the operating one (background work).
    let mut end = End::new();
    end.gateway.set_operating(false);
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let (result, effect) = end.call(&mut client, &mut link, &a, run("pane.split"), 0);
    assert_eq!(failure(result), GatewayFailure::NoOperatingConnection);
    assert_eq!(effect, None);
}

#[test]
fn stale_targets_and_policy_are_rejected() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut client = GatewayClient::new(Counter(5));
    // The runner still speaks for incarnation 7 of copy "a".
    let old = copy("a", 7);
    let mut old_link = yebisu_mod_runner::CopyLink::new(
        wire::ReceiveContext {
            scope: wire::Scope::Copy {
                id: "a".into(),
                incarnation: old.incarnation.clone(),
            },
            ..end.pair.renderer_context(&a)
        },
        end.pair.limits.clone(),
    )
    .unwrap();
    let GatewaySend::Sent { envelope, .. } = client
        .request(Some(&mut old_link), run("pane.split"))
        .unwrap()
    else {
        panic!("not sent")
    };
    let (reply, effect) = end
        .gateway
        .handle(&envelope, &end.pair.renderer_context(&a), 0);
    assert_eq!(failure(reply.result), GatewayFailure::StaleTarget);
    assert_eq!(effect, None);
    // A current envelope whose body names another boot.
    let mut link = end.pair.link(&a);
    let GatewaySend::Sent { mut envelope, .. } =
        client.request(Some(&mut link), run("pane.split")).unwrap()
    else {
        panic!("not sent")
    };
    envelope.body.target.boot = id(8);
    let (reply, _) = end
        .gateway
        .handle(&envelope, &end.pair.renderer_context(&a), 0);
    assert_eq!(failure(reply.result), GatewayFailure::StaleTarget);
    // A correct body in an envelope from an older owner fence.
    let GatewaySend::Sent { mut envelope, .. } =
        client.request(Some(&mut link), run("pane.split")).unwrap()
    else {
        panic!("not sent")
    };
    envelope.owner_fence = Counter(0);
    let (reply, _) = end
        .gateway
        .handle(&envelope, &end.pair.renderer_context(&a), 0);
    assert_eq!(failure(reply.result), GatewayFailure::StaleTarget);
    // Policy: stale revision, command outside the allowlist, agent order.
    client.set_policy_rev(Counter(4));
    let (result, _) = end.call(&mut client, &mut link, &a, run("pane.split"), 0);
    assert_eq!(failure(result), GatewayFailure::PolicyDenied);
    client.set_policy_rev(Counter(5));
    let (result, _) = end.call(&mut client, &mut link, &a, run("app.quit"), 0);
    assert_eq!(failure(result), GatewayFailure::PolicyDenied);
    let order = GatewayOperation::SetAgentOrder {
        agents: vec!["x".into()],
    };
    let (result, _) = end.call(&mut client, &mut link, &a, order, 0);
    assert_eq!(failure(result), GatewayFailure::PolicyDenied);
}

#[test]
fn overlays_belong_to_the_copy_incarnation_that_opened_them() {
    let mut end = End::new();
    let (a, b) = (copy("a", 3), copy("b", 4));
    let (mut link_a, mut link_b) = (end.pair.link(&a), end.pair.link(&b));
    let mut client_a = GatewayClient::new(Counter(5));
    let mut client_b = GatewayClient::new(Counter(5));
    let open = GatewayOperation::OpenOverlay {
        overlay: overlay(OverlayKind::Overlay, "dialog", &a),
    };
    let (result, _) = end.call(&mut client_a, &mut link_a, &a, open.clone(), 0);
    assert_eq!(result, GatewayResult::Ok {});
    // Copy b cannot take over, close, or open an overlay in a's name.
    let (result, _) = end.call(&mut client_b, &mut link_b, &b, open, 0);
    assert_eq!(failure(result), GatewayFailure::NotOwned);
    let close = GatewayOperation::CloseOverlay {
        overlay_id: "dialog".into(),
    };
    let (result, _) = end.call(&mut client_b, &mut link_b, &b, close.clone(), 0);
    assert_eq!(failure(result), GatewayFailure::NotOwned);
    // Nor reuse the ID of a's open overlay for one of its own.
    let takeover = GatewayOperation::OpenOverlay {
        overlay: overlay(OverlayKind::Overlay, "dialog", &b),
    };
    let (result, _) = end.call(&mut client_b, &mut link_b, &b, takeover, 0);
    assert_eq!(failure(result), GatewayFailure::NotOwned);
    let foreign = GatewayOperation::OpenOverlay {
        overlay: overlay(OverlayKind::Overlay, "other", &a),
    };
    let (result, _) = end.call(&mut client_b, &mut link_b, &b, foreign, 0);
    assert_eq!(failure(result), GatewayFailure::NotOwned);
    let (result, effect) = end.call(&mut client_a, &mut link_a, &a, close, 0);
    assert_eq!(result, GatewayResult::Ok {});
    assert!(matches!(effect, Some(GatewayEffect::CloseOverlay { .. })));
    assert_eq!(end.gateway.open_overlays(), 0);
}

#[test]
fn toasts_are_limited_to_one_per_second_and_three_visible() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let notify = |id: &str| GatewayOperation::Notify {
        overlay: overlay(OverlayKind::Toast, id, &a),
    };
    let (result, _) = end.call(&mut client, &mut link, &a, notify("t1"), 1_000);
    assert_eq!(result, GatewayResult::Ok {});
    let (result, _) = end.call(&mut client, &mut link, &a, notify("t2"), 1_999);
    assert_eq!(failure(result), GatewayFailure::RateLimited);
    for (id, at) in [("t2", 2_000), ("t3", 3_000)] {
        let (result, _) = end.call(&mut client, &mut link, &a, notify(id), at);
        assert_eq!(result, GatewayResult::Ok {});
    }
    let (result, _) = end.call(&mut client, &mut link, &a, notify("t4"), 4_000);
    assert_eq!(failure(result), GatewayFailure::LimitExceeded);
    end.gateway.dismiss_toast("t1");
    let (result, _) = end.call(&mut client, &mut link, &a, notify("t4"), 5_000);
    assert_eq!(result, GatewayResult::Ok {});
    // Notify accepts only a toast; the runner refuses to send anything else.
    let plain = GatewayOperation::Notify {
        overlay: overlay(OverlayKind::Overlay, "t5", &a),
    };
    assert_eq!(
        client.request(Some(&mut link), plain).unwrap(),
        GatewaySend::Failed(GatewayFailure::InvalidRequest)
    );
}

#[test]
fn a_stopped_copy_is_cancelled_and_its_overlays_close() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let open = GatewayOperation::OpenOverlay {
        overlay: overlay(OverlayKind::Overlay, "dialog", &a),
    };
    end.call(&mut client, &mut link, &a, open, 0);
    assert_eq!(end.gateway.stop_copy("a", &a.incarnation), vec!["dialog"]);
    let (result, effect) = end.call(&mut client, &mut link, &a, run("pane.split"), 0);
    assert_eq!(failure(result), GatewayFailure::Cancelled);
    assert_eq!(effect, None);
}

#[test]
fn a_resent_request_gets_the_same_reply_and_does_not_run_again() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let GatewaySend::Sent { envelope, .. } =
        client.request(Some(&mut link), run("pane.split")).unwrap()
    else {
        panic!("not sent")
    };
    let context = end.pair.renderer_context(&a);
    let (first, effect) = end.gateway.handle(&envelope, &context, 0);
    assert!(effect.is_some());
    let (second, effect) = end.gateway.handle(&envelope, &context, 0);
    assert_eq!(first, second);
    assert_eq!(effect, None);
}

#[test]
fn the_runner_routes_each_reply_to_exactly_one_waiting_call() {
    let end = End::new();
    let (a, b) = (copy("a", 3), copy("b", 4));
    let (mut link, other) = (end.pair.link(&a), end.pair.link(&b));
    let mut client = GatewayClient::new(Counter(5));
    let GatewaySend::Sent { request_id, .. } =
        client.request(Some(&mut link), run("pane.split")).unwrap()
    else {
        panic!("not sent")
    };
    let ok = wire::GatewayReply {
        request_id: request_id.clone(),
        result: GatewayResult::Ok {},
    };
    let context = end.pair.renderer_context(&a);
    // A reply in another copy's scope is not this link's.
    let foreign = reply_frame(&end.pair.renderer_context(&b), ok.clone(), &end.pair.limits);
    assert!(client.receive(&link, &foreign).is_err());
    assert!(client.receive(&other, &foreign).is_err());
    // The right scope and ID from an older owner fence is not authentic.
    let stale_fence = wire::ReceiveContext {
        owner_fence: Counter(0),
        ..context.clone()
    };
    let stale = reply_frame(&stale_fence, ok.clone(), &end.pair.limits);
    assert!(client.receive(&link, &stale).is_err());
    let unknown = wire::GatewayReply {
        request_id: "gateway-99".into(),
        ..ok.clone()
    };
    assert!(
        client
            .receive(&link, &reply_frame(&context, unknown, &end.pair.limits))
            .is_err()
    );
    let frame = reply_frame(&context, ok, &end.pair.limits);
    assert_eq!(
        client.receive(&link, &frame).unwrap(),
        (request_id, GatewayResult::Ok {})
    );
    assert!(client.receive(&link, &frame).is_err());
    // A disconnect cancels whatever is still waiting.
    client.request(Some(&mut link), run("pane.split")).unwrap();
    let cancelled = client.disconnect();
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0].1, GatewayFailure::Cancelled);
    assert_eq!(client.pending(), 0);
}

#[test]
fn gateway_messages_use_the_negotiated_control_size() {
    let pair = connect(
        lowered(|l| l.control_bytes = 8_192),
        wire::NegotiatedLimits::default(),
    );
    let a = copy("a", 3);
    let mut link = pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let big = GatewayOperation::Run {
        command: "pane.split".into(),
        args: vec!["x".repeat(10_000)],
    };
    assert_eq!(
        client.request(Some(&mut link), big).unwrap(),
        GatewaySend::Failed(GatewayFailure::LimitExceeded)
    );
    assert_eq!(client.pending(), 0);
    // An oversized frame from the renderer is refused before decoding.
    // A well-formed reply over the negotiated size (under the 64 KiB ceiling)
    // is refused for its size, before decoding.
    let long = wire::GatewayReply {
        request_id: "r".repeat(9_000),
        result: GatewayResult::Ok {},
    };
    let frame = reply_frame(
        &pair.renderer_context(&a),
        long,
        &wire::NegotiatedLimits::default(),
    );
    assert!(matches!(
        client.receive(&link, &frame),
        Err(yebisu_mod_runner::Error::Wire(wire::Error::Limit(_)))
    ));
}

#[test]
fn equal_request_ids_from_different_copies_are_different_requests() {
    let mut end = End::new();
    let (a, b) = (copy("a", 3), copy("b", 4));
    let (mut link_a, mut link_b) = (end.pair.link(&a), end.pair.link(&b));
    // Each copy numbers its own requests, so both send "gateway-1".
    let mut client_a = GatewayClient::new(Counter(5));
    let mut client_b = GatewayClient::new(Counter(5));
    let (result, _) = end.call(&mut client_a, &mut link_a, &a, run("pane.split"), 0);
    assert_eq!(result, GatewayResult::Ok {});
    let (result, effect) = end.call(&mut client_b, &mut link_b, &b, run("app.quit"), 0);
    assert_eq!(failure(result), GatewayFailure::PolicyDenied);
    assert_eq!(effect, None);
}

#[test]
fn unauthenticated_requests_can_neither_read_nor_fill_the_replay_cache() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let GatewaySend::Sent { envelope, .. } =
        client.request(Some(&mut link), run("pane.split")).unwrap()
    else {
        panic!("not sent")
    };
    let context = end.pair.renderer_context(&a);
    // A forged copy of the request from an older owner fence arrives first.
    let mut forged = envelope.clone();
    forged.owner_fence = Counter(0);
    let (reply, _) = end.gateway.handle(&forged, &context, 0);
    assert_eq!(failure(reply.result), GatewayFailure::StaleTarget);
    // The authentic request still runs.
    let (reply, effect) = end.gateway.handle(&envelope, &context, 0);
    assert_eq!(reply.result, GatewayResult::Ok {});
    assert!(effect.is_some());
    // And the forged one does not receive the remembered Ok.
    let (reply, effect) = end.gateway.handle(&forged, &context, 0);
    assert_eq!(failure(reply.result), GatewayFailure::StaleTarget);
    assert_eq!(effect, None);
    // After a stop, a resent request is decided again, not replayed.
    end.gateway.stop_copy("a", &a.incarnation);
    let (reply, effect) = end.gateway.handle(&envelope, &context, 0);
    assert_eq!(failure(reply.result), GatewayFailure::Cancelled);
    assert_eq!(effect, None);
}

#[test]
fn open_overlay_cannot_show_a_toast_past_the_toast_limits() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let toast = GatewayOperation::OpenOverlay {
        overlay: overlay(OverlayKind::Toast, "t", &a),
    };
    // The runner refuses to send it...
    assert_eq!(
        client.request(Some(&mut link), toast.clone()).unwrap(),
        GatewaySend::Failed(GatewayFailure::InvalidRequest)
    );
    // ...and the renderer refuses it from a runner that does not.
    let request = wire::GatewayRequest {
        request_id: "forged".into(),
        target: link.target().unwrap(),
        policy_rev: Counter(5),
        operation: toast,
    };
    let envelope = link.envelope(request);
    let (reply, effect) = end
        .gateway
        .handle(&envelope, &end.pair.renderer_context(&a), 0);
    assert_eq!(failure(reply.result), GatewayFailure::InvalidRequest);
    assert_eq!(effect, None);
}

#[test]
fn a_toast_shows_the_device_names_for_its_copy_not_the_payload() {
    let mut end = End::new();
    let a = copy("a", 3);
    let mut link = end.pair.link(&a);
    let mut client = GatewayClient::new(Counter(5));
    let mut spoof = overlay(OverlayKind::Toast, "t", &a);
    spoof.toast = Some(wire::ToastContent {
        message: "Session expired, sign in".into(),
        mod_name: "System".into(),
        host_name: "Bank".into(),
    });
    let notify = GatewayOperation::Notify { overlay: spoof };
    let (result, effect) = end.call(&mut client, &mut link, &a, notify, 0);
    assert_eq!(result, GatewayResult::Ok {});
    let Some(GatewayEffect::ShowToast(shown)) = effect else {
        panic!("no toast")
    };
    let names = shown.toast.unwrap();
    assert_eq!(names.message, "Session expired, sign in");
    // HelloAck named this copy's mod "a"; the device's record says otherwise.
    assert_eq!(names.mod_name, "Git Graph");
    assert_eq!(names.host_name, "pro-dev");
    // A copy the device has no names for cannot show a toast.
    let mut unnamed = Gateway::new(policy());
    unnamed.set_operating(true);
    let GatewaySend::Sent { envelope, .. } = client
        .request(
            Some(&mut link),
            GatewayOperation::Notify {
                overlay: overlay(OverlayKind::Toast, "t2", &a),
            },
        )
        .unwrap()
    else {
        panic!("not sent")
    };
    let (reply, _) = unnamed.handle(&envelope, &end.pair.renderer_context(&a), 0);
    assert_eq!(failure(reply.result), GatewayFailure::InvalidRequest);
}
