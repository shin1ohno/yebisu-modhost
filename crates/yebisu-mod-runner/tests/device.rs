//! Runner <-> renderer: the A12 device-permission hand-off over control
//! frames, with the device's expiry, cancellation and one-use checks.
mod common;
use common::*;
use yebisu_mod_renderer::{DeviceEffect, DevicePermissions};
use yebisu_mod_runner::{CopyLink, DeviceClient, DeviceRefusal, DeviceSend, encode_control};
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{ActionKind, Counter, DeviceOperation, DeviceResult};

const POLICY: Counter = Counter(5);

/// One connection: the device ledger, the runner client and copy "a".
struct Device {
    pair: Pair,
    copy: wire::CopyInfo,
    ledger: DevicePermissions,
    client: DeviceClient,
    link: CopyLink,
    tokens: u8,
}
impl Device {
    fn new() -> Self {
        Self::with(wire::NegotiatedLimits::default(), true)
    }
    fn with(limits: wire::NegotiatedLimits, remote_clipboard: bool) -> Self {
        let pair = connect(limits, wire::NegotiatedLimits::default());
        let copy = copy("a", 3);
        let link = pair.link(&copy);
        Self {
            pair,
            copy,
            ledger: DevicePermissions::new(POLICY, remote_clipboard),
            client: DeviceClient::new(),
            link,
            tokens: 100,
        }
    }
    fn context(&self) -> wire::ReceiveContext {
        self.pair.renderer_context(&self.copy)
    }
    /// The user taps at `now_ms`: the device issues permissions, sends the
    /// Action, and the runner makes it the current event.
    fn tap(&mut self, event: &str, ops: &[DeviceOperation], now_ms: u64) -> wire::Action {
        let context = self.context();
        let mut next = self.tokens;
        let permissions = self
            .ledger
            .issue(&context, ActionKind::Click, event, ops, now_ms, || {
                next += 1;
                id(next)
            })
            .unwrap();
        self.tokens = next;
        self.send_action(event, permissions, None)
    }
    fn send_action(
        &mut self,
        event: &str,
        permissions: Vec<wire::DevicePermission>,
        text: Option<String>,
    ) -> wire::Action {
        let mut action = wire::Action {
            op_seq: Counter(1),
            event_id: event.into(),
            after_local_seq: Counter(0),
            pub_rev: Counter(1),
            generation: Counter(1),
            lease_id: "lease".into(),
            target: 0,
            kind: ActionKind::Click,
            data: wire::ActionData {
                key: None,
                text,
                point: None,
                value: None,
                row_key: None,
                permissions,
            },
            digest: wire::bytes_digest(b""),
        };
        action.digest = wire::canonical_digest(&action, "digest").unwrap();
        let context = self.context();
        let envelope = wire::Envelope::new(
            context.session,
            context.scope,
            context.owner_fence,
            format!("action-{event}"),
            Counter(1),
            action,
        );
        self.ledger.sent(&envelope).unwrap();
        let frame = encode_control(&envelope, &wire::NegotiatedLimits::default()).unwrap();
        self.client.on_action(&self.link, &frame).unwrap()
    }
    fn sent(
        &mut self,
        op: DeviceOperation,
        value: &str,
    ) -> Box<wire::Envelope<wire::DeviceRequest>> {
        let retained = self.client.retain(op).unwrap();
        match self
            .client
            .request(Some(&mut self.link), retained, value)
            .unwrap()
        {
            DeviceSend::Sent { envelope, .. } => envelope,
            DeviceSend::Refused(r) => panic!("refused: {r:?}"),
        }
    }
    /// Deliver a request to the device at `now_ms` and its reply to the runner.
    fn deliver(
        &mut self,
        envelope: &wire::Envelope<wire::DeviceRequest>,
        now_ms: u64,
    ) -> (DeviceResult, Option<DeviceEffect>) {
        let received = to_renderer(envelope, &self.pair.limits);
        let context = self.context();
        let (reply, effect) = self.ledger.handle(&received, &context, now_ms);
        let reply = wire::Envelope::new(
            context.session,
            context.scope,
            context.owner_fence,
            "device-reply".into(),
            Counter(1),
            reply,
        );
        let frame = encode_control(&reply, &self.pair.limits).unwrap();
        if let Ok((_, result)) = self.client.receive(&self.link, &frame) {
            assert_eq!(result, reply.body.result);
        }
        (reply.body.result, effect)
    }
}
/// A second request spending the same permission, built without the runner.
fn respend(
    device: &mut Device,
    first: &wire::Envelope<wire::DeviceRequest>,
) -> Box<wire::Envelope<wire::DeviceRequest>> {
    let mut again = device.link.envelope(first.body.clone());
    again.body.request_id = "device-again".into();
    Box::new(again)
}

#[test]
fn a_retained_permission_opens_the_normalized_url_once() {
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::OpenUrl], 1_000);
    let request = d.sent(DeviceOperation::OpenUrl, "HTTPS://Example.COM/a b");
    d.client.end_action();
    assert_eq!(request.body.value, "https://example.com/a%20b");
    let (result, effect) = d.deliver(&request, 1_200);
    assert_eq!(result, DeviceResult::Ok);
    assert_eq!(
        effect,
        Some(DeviceEffect::OpenUrl {
            url: "https://example.com/a%20b".into(),
            host: "example.com".into()
        })
    );
    assert_eq!(d.client.pending(), 0);
    // One use: the same permission in a new request is denied.
    let again = respend(&mut d, &request);
    let (result, effect) = d.deliver(&again, 1_300);
    assert_eq!(result, DeviceResult::Denied);
    assert_eq!(effect, None);
}

#[test]
fn a_permission_expires_one_second_after_the_event() {
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::OpenUrl], 1_000);
    let on_time = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    d.tap("e2", &[DeviceOperation::OpenUrl], 1_000);
    let late = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    assert_eq!(d.deliver(&on_time, 2_000).0, DeviceResult::Ok);
    assert_eq!(d.deliver(&late, 2_001).0, DeviceResult::Expired);
    // Expired is final: the permission cannot be spent after a clock read error.
    let again = respend(&mut d, &late);
    assert_ne!(d.deliver(&again, 1_500).0, DeviceResult::Ok);
}

#[test]
fn pause_disconnect_and_policy_changes_cancel_unspent_permissions() {
    let mut d = Device::new();
    let ops = [DeviceOperation::OpenUrl];
    d.tap("pause", &ops, 0);
    let paused = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    d.ledger.cancel_copy(&d.context().scope);
    assert_eq!(d.deliver(&paused, 10).0, DeviceResult::Denied);

    d.tap("epoch", &ops, 0);
    let dropped = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    d.ledger.cancel_connection(&d.context().session);
    assert_eq!(d.deliver(&dropped, 10).0, DeviceResult::Denied);

    let mut d = Device::new();
    d.tap("policy", &ops, 0);
    let stale = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    d.ledger.set_policy(Counter(6));
    assert_eq!(d.deliver(&stale, 10).0, DeviceResult::Denied);
}

#[test]
fn the_hand_off_must_match_the_event_operation_and_sent_action() {
    let mut d = Device::new();
    d.tap(
        "e1",
        &[DeviceOperation::OpenUrl, DeviceOperation::ClipboardWrite],
        0,
    );
    let request = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    // Another event's ID.
    let mut other_event = respend(&mut d, &request);
    other_event.body.event_id = "e2".into();
    assert_eq!(d.deliver(&other_event, 10).0, DeviceResult::Denied);
    // The clipboard permission used to open a URL.
    let clip = d.client.retain(DeviceOperation::ClipboardWrite).unwrap();
    let DeviceSend::Sent { envelope, .. } =
        d.client.request(Some(&mut d.link), clip, "text").unwrap()
    else {
        panic!("not sent")
    };
    let mut swapped = envelope.clone();
    swapped.body.operation = DeviceOperation::OpenUrl;
    swapped.body.value = "https://example.com/".into();
    assert_eq!(d.deliver(&swapped, 10).0, DeviceResult::Denied);
    // A permission issued but never sent in the Action.
    let context = d.context();
    let mut issued = d
        .ledger
        .issue(
            &context,
            ActionKind::Click,
            "e3",
            &[DeviceOperation::OpenUrl],
            0,
            || id(200),
        )
        .unwrap();
    d.send_action("e3", vec![], None);
    let mut unsent = respend(&mut d, &request);
    unsent.body.permission = issued.remove(0).permission;
    unsent.body.event_id = "e3".into();
    assert_eq!(d.deliver(&unsent, 10).0, DeviceResult::Denied);
    // The untouched original still works: refusals spent nothing.
    assert_eq!(d.deliver(&request, 20).0, DeviceResult::Ok);
}

#[test]
fn permissions_exist_only_for_taps_and_keys_and_only_while_retained() {
    let mut d = Device::new();
    let context = d.context();
    assert!(
        d.ledger
            .issue(
                &context,
                ActionKind::Scroll,
                "s",
                &[DeviceOperation::OpenUrl],
                0,
                || id(1)
            )
            .is_none()
    );
    d.tap("e1", &[DeviceOperation::OpenUrl], 0);
    // Not retained during the callback: gone after it returns.
    d.client.end_action();
    assert!(d.client.retain(DeviceOperation::OpenUrl).is_none());
    d.tap("e2", &[DeviceOperation::OpenUrl], 0);
    assert!(d.client.retain(DeviceOperation::OpenUrl).is_some());
    assert!(d.client.retain(DeviceOperation::OpenUrl).is_none());
}

#[test]
fn urls_are_https_and_canonical_on_both_sides() {
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::OpenUrl], 0);
    let retained = d.client.retain(DeviceOperation::OpenUrl).unwrap();
    assert_eq!(
        d.client
            .request(Some(&mut d.link), retained, "http://example.com/")
            .unwrap(),
        DeviceSend::Refused(DeviceRefusal::InvalidUrl)
    );
    d.tap("e2", &[DeviceOperation::OpenUrl], 0);
    let request = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    let mut raw = respend(&mut d, &request);
    raw.body.request_id = "raw".into();
    raw.body.value = "https://EXAMPLE.com".into();
    // A runner that skipped normalize_url cannot get the device to open it.
    let context = d.context();
    let (reply, effect) = d.ledger.handle(&raw, &context, 0);
    assert_eq!(reply.result, DeviceResult::Denied);
    assert_eq!(effect, None);
}

#[test]
fn resent_requests_are_answered_once_and_forgeries_are_not_remembered() {
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::OpenUrl], 0);
    let request = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    let context = d.context();
    let mut forged = request.clone();
    forged.owner_fence = Counter(0);
    let (reply, _) = d.ledger.handle(&forged, &context, 0);
    assert_eq!(reply.result, DeviceResult::Denied);
    let (first, effect) = d.ledger.handle(&request, &context, 10);
    assert_eq!(first.result, DeviceResult::Ok);
    assert!(effect.is_some());
    let (second, effect) = d.ledger.handle(&request, &context, 20);
    assert_eq!(first, second);
    assert_eq!(effect, None);
    let (reply, _) = d.ledger.handle(&forged, &context, 30);
    assert_eq!(reply.result, DeviceResult::Denied);
}

#[test]
fn clipboard_writes_need_a_device_that_allows_them() {
    let mut d = Device::with(wire::NegotiatedLimits::default(), false);
    d.tap("e1", &[DeviceOperation::ClipboardWrite], 0);
    let request = d.sent(DeviceOperation::ClipboardWrite, "copied");
    assert_eq!(
        d.deliver(&request, 10),
        (DeviceResult::UnsupportedRemoteClipboard, None)
    );
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::ClipboardWrite], 0);
    let request = d.sent(DeviceOperation::ClipboardWrite, "copied");
    assert_eq!(
        d.deliver(&request, 10),
        (
            DeviceResult::Ok,
            Some(DeviceEffect::ClipboardWrite {
                text: "copied".into()
            })
        )
    );
}

#[test]
fn the_runner_applies_negotiated_limits_to_actions_and_routes_replies_once() {
    let mut d = Device::with(lowered(|l| l.string_bytes = 1_000), true);
    let long = "x".repeat(1_001);
    let mut action = wire::Action {
        op_seq: Counter(1),
        event_id: "long".into(),
        after_local_seq: Counter(0),
        pub_rev: Counter(1),
        generation: Counter(1),
        lease_id: "lease".into(),
        target: 0,
        kind: ActionKind::Click,
        data: wire::ActionData {
            key: None,
            text: Some(long),
            point: None,
            value: None,
            row_key: None,
            permissions: vec![],
        },
        digest: wire::bytes_digest(b""),
    };
    action.digest = wire::canonical_digest(&action, "digest").unwrap();
    let context = d.context();
    let envelope = wire::Envelope::new(
        context.session.clone(),
        context.scope.clone(),
        context.owner_fence,
        "action".into(),
        Counter(1),
        action,
    );
    let frame = encode_control(&envelope, &wire::NegotiatedLimits::default()).unwrap();
    assert!(d.client.on_action(&d.link, &frame).is_err());

    d.tap("e1", &[DeviceOperation::OpenUrl], 0);
    let request = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    let ok = wire::DeviceReply {
        request_id: request.body.request_id.clone(),
        result: DeviceResult::Ok,
    };
    let b = copy("b", 4);
    let other = d.pair.renderer_context(&b);
    let foreign = wire::Envelope::new(
        other.session,
        other.scope,
        other.owner_fence,
        "r".into(),
        Counter(1),
        ok.clone(),
    );
    let foreign = encode_control(&foreign, &d.pair.limits).unwrap();
    assert!(d.client.receive(&d.pair.link(&b), &foreign).is_err());
    let own = wire::Envelope::new(
        context.session,
        context.scope,
        context.owner_fence,
        "r".into(),
        Counter(1),
        ok,
    );
    let own = encode_control(&own, &d.pair.limits).unwrap();
    d.client.receive(&d.link, &own).unwrap();
    assert!(d.client.receive(&d.link, &own).is_err());
    // A permission from copy a cannot be spent on copy b's link.
    d.tap("e2", &[DeviceOperation::OpenUrl], 0);
    let retained = d.client.retain(DeviceOperation::OpenUrl).unwrap();
    let mut link_b = d.pair.link(&b);
    assert_eq!(
        d.client
            .request(Some(&mut link_b), retained, "https://example.com/")
            .unwrap(),
        DeviceSend::Refused(DeviceRefusal::OtherCopy)
    );
    let retained = {
        d.tap("e3", &[DeviceOperation::OpenUrl], 0);
        d.client.retain(DeviceOperation::OpenUrl).unwrap()
    };
    assert_eq!(
        d.client
            .request(None, retained, "https://example.com/")
            .unwrap(),
        DeviceSend::Refused(DeviceRefusal::NoOperatingConnection)
    );
}

#[test]
fn a_permission_is_bound_to_the_connection_and_owner_it_was_issued_to() {
    let mut d = Device::new();
    d.tap("e1", &[DeviceOperation::OpenUrl], 0);
    let request = d.sent(DeviceOperation::OpenUrl, "https://example.com/");
    // Ownership moved to a new owner fence before the request arrived.
    let new_owner = wire::ReceiveContext {
        owner_fence: Counter(2),
        ..d.context()
    };
    let mut moved = request.clone();
    moved.owner_fence = Counter(2);
    let (reply, effect) = d.ledger.handle(&moved, &new_owner, 10);
    assert_eq!(reply.result, DeviceResult::Denied);
    assert_eq!(effect, None);
}

#[test]
fn issue_refuses_repeated_operations_and_reused_tokens() {
    let mut d = Device::new();
    let context = d.context();
    let twice = [DeviceOperation::OpenUrl, DeviceOperation::OpenUrl];
    let mut n = 0;
    assert!(
        d.ledger
            .issue(&context, ActionKind::Key, "k", &twice, 0, || {
                n += 1;
                id(n)
            })
            .is_none()
    );
    let one = [DeviceOperation::OpenUrl];
    assert!(
        d.ledger
            .issue(&context, ActionKind::Key, "k1", &one, 0, || id(50))
            .is_some()
    );
    // A token the ledger already holds is never issued again.
    assert!(
        d.ledger
            .issue(&context, ActionKind::Key, "k2", &one, 0, || id(50))
            .is_none()
    );
}
