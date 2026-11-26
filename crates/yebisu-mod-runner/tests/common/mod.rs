//! A runner and a renderer joined by in-memory control frames.
#![allow(dead_code)]
use yebisu_mod_renderer as renderer;
use yebisu_mod_runner as runner;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{Counter, Id128};

pub fn id(n: u8) -> Id128 {
    Id128::new(format!("{n:032x}")).unwrap()
}
pub const HOST: &str = "SHA256:verified-by-ssh";
pub const CLIENT: &str = "ipad";

pub fn copy(name: &str, n: u8) -> wire::CopyInfo {
    wire::CopyInfo {
        id: name.into(),
        incarnation: id(n),
        r#mod: wire::ModIdentity {
            location: wire::ModLocation::HerdrApp,
            name: name.into(),
        },
    }
}
pub fn digests() -> wire::Digests {
    let d = wire::bytes_digest(b"tables");
    wire::Digests {
        style: d.clone(),
        component: d.clone(),
        op: d.clone(),
        entity: d.clone(),
        command: d,
    }
}
pub fn serve(limits: wire::NegotiatedLimits) -> runner::ServeIdentity {
    runner::ServeIdentity {
        serve: wire::ServeVersion {
            version: "0.1.0".into(),
            commit: "test".into(),
        },
        boot: id(1),
        epoch: id(2),
        digests: digests(),
        required: vec!["gateway".into()],
        limits,
        copies: vec![copy("a", 3), copy("b", 4)],
    }
}
pub fn lowered(change: impl FnOnce(&mut wire::NegotiatedLimits)) -> wire::NegotiatedLimits {
    let mut limits = wire::NegotiatedLimits::default();
    change(&mut limits);
    limits.validate().unwrap();
    limits
}
/// The renderer's view of the session-scoped handshake transport.
pub fn session_context() -> wire::ReceiveContext {
    wire::ReceiveContext {
        session: wire::Session {
            host: HOST.into(),
            boot: id(1),
            client: CLIENT.into(),
            epoch: id(2),
        },
        scope: wire::Scope::Session {},
        owner_fence: Counter(1),
    }
}
pub fn session_envelope<T: wire::Payload>(body: T) -> wire::Envelope<T> {
    let c = session_context();
    wire::Envelope::new(
        c.session,
        c.scope,
        c.owner_fence,
        "hs".into(),
        Counter(1),
        body,
    )
}

/// Both ends of one connection after a real Hello/HelloAck over frames.
pub struct Pair {
    pub negotiated: renderer::Negotiated,
    pub limits: wire::NegotiatedLimits,
}
pub fn connect(
    renderer_limits: wire::NegotiatedLimits,
    runner_limits: wire::NegotiatedLimits,
) -> Pair {
    let client = renderer::ClientHandshake::new(
        CLIENT.into(),
        id(9),
        digests(),
        vec!["gateway".into()],
        renderer_limits.clone(),
    )
    .unwrap();
    let hello = runner::encode_control(&session_envelope(client.hello().clone()), &renderer_limits)
        .unwrap();
    let hello: wire::Envelope<wire::Hello> =
        runner::decode_control(&hello, &runner_limits).unwrap();
    let ack = runner::answer_hello(&hello.body, &serve(runner_limits.clone())).unwrap();
    let limits = ack.limits.clone();
    let ack = runner::encode_control(&session_envelope(ack), &limits).unwrap();
    let ack: wire::Envelope<wire::HelloAck> = wire::decode(&ack.payload).unwrap();
    let negotiated = client.accept(&ack.body).unwrap();
    Pair {
        limits: negotiated.limits.clone(),
        negotiated,
    }
}
impl Pair {
    /// The renderer's receiving context for a copy (host from the SSH verifier).
    pub fn renderer_context(&self, copy: &wire::CopyInfo) -> wire::ReceiveContext {
        self.negotiated
            .context(HOST.into(), CLIENT.into(), copy, Counter(1))
            .unwrap()
    }
    /// The runner's link for the same copy.
    pub fn link(&self, copy: &wire::CopyInfo) -> runner::CopyLink {
        runner::CopyLink::new(self.renderer_context(copy), self.limits.clone()).unwrap()
    }
}

/// Carry an envelope over a control frame to the renderer side.
pub fn to_renderer<T: wire::Payload>(
    envelope: &wire::Envelope<T>,
    limits: &wire::NegotiatedLimits,
) -> wire::Envelope<T> {
    let frame = runner::encode_control(envelope, limits).unwrap();
    limits.check_control(frame.payload.len()).unwrap();
    wire::decode(&frame.payload).unwrap()
}
/// The renderer answers in the request's own scope; return the frame.
pub fn reply_frame(
    context: &wire::ReceiveContext,
    reply: wire::GatewayReply,
    limits: &wire::NegotiatedLimits,
) -> wire::Frame {
    let envelope = wire::Envelope::new(
        context.session.clone(),
        context.scope.clone(),
        context.owner_fence,
        format!("renderer-{}", reply.request_id),
        Counter(1),
        reply,
    );
    runner::encode_control(&envelope, limits).unwrap()
}
pub fn overlay(kind: wire::OverlayKind, id: &str, copy: &wire::CopyInfo) -> wire::Overlay {
    wire::Overlay {
        kind,
        toast: (kind == wire::OverlayKind::Toast).then(|| wire::ToastContent {
            message: "done".into(),
            mod_name: copy.id.clone(),
            host_name: "host".into(),
        }),
        id: id.into(),
        parent_pub: Counter(1),
        copy: copy.id.clone(),
        incarnation: copy.incarnation.clone(),
        pub_rev: Counter(1),
    }
}
