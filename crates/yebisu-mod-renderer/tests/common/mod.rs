//! Shared fixtures: one session, copies "a"/"b", and a publication with a
//! List (VirtualScroll entity, uniform manifest) and an SVG asset.
#![allow(dead_code)]
use yebisu_mod_renderer::*;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{Counter, EntityKind, EntityRef, Id128};

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
    let d = wire::bytes_digest(b"table");
    wire::Digests {
        style: d.clone(),
        component: d.clone(),
        op: d.clone(),
        entity: d.clone(),
        command: d,
    }
}
pub fn ack(nonce: Id128, limits: wire::NegotiatedLimits) -> wire::HelloAck {
    wire::HelloAck {
        nonce,
        protocol: wire::PROTOCOL,
        serve: wire::ServeVersion {
            version: "0.1.0".into(),
            commit: "test".into(),
        },
        boot: id(1),
        epoch: id(2),
        limits,
        copies: vec![copy("a", 3), copy("b", 4)],
    }
}
pub fn negotiated(limits: wire::NegotiatedLimits) -> Negotiated {
    let handshake = ClientHandshake::new(
        CLIENT.into(),
        id(9),
        digests(),
        vec![],
        wire::NegotiatedLimits::default(),
    )
    .unwrap();
    handshake.accept(&ack(id(9), limits)).unwrap()
}
pub fn context(name: &str, n: u8) -> wire::ReceiveContext {
    negotiated(wire::NegotiatedLimits::default())
        .context(HOST.into(), CLIENT.into(), &copy(name, n), Counter(1))
        .unwrap()
}
pub fn receiver(name: &str, n: u8, limits: wire::NegotiatedLimits) -> CopyReceiver {
    CopyReceiver::new(context(name, n), limits, wire::ValidationTables::default()).unwrap()
}
pub fn envelope<T: wire::Payload>(
    context: &wire::ReceiveContext,
    seq: u64,
    body: T,
) -> wire::Envelope<T> {
    wire::Envelope::new(
        context.session.clone(),
        context.scope.clone(),
        context.owner_fence,
        format!("message-{seq}"),
        Counter(seq),
        body,
    )
}
pub fn lowered(change: impl FnOnce(&mut wire::NegotiatedLimits)) -> wire::NegotiatedLimits {
    let mut limits = wire::NegotiatedLimits::default();
    change(&mut limits);
    limits.validate().unwrap();
    limits
}
pub struct Ledgers {
    pub host: wire::ResourceLedger,
    pub app: wire::ResourceLedger,
}
impl Ledgers {
    pub fn new(limits: &wire::NegotiatedLimits) -> Self {
        Self {
            host: wire::ResourceLedger::negotiated(limits).unwrap(),
            app: wire::ResourceLedger::negotiated(limits).unwrap(),
        }
    }
    pub fn scopes(&mut self) -> SharedScopes<'_> {
        SharedScopes {
            host: &mut self.host,
            app: &mut self.app,
        }
    }
}

pub fn node(component: wire::Component) -> wire::Node {
    wire::Node {
        component,
        text: None,
        entity: None,
        asset: None,
        text_format: None,
        child_view: None,
        list_id: None,
        children: vec![],
        ops: vec![],
    }
}
fn subscriptions() -> wire::Subscriptions {
    wire::Subscriptions {
        rev: Counter(1),
        events: vec![],
    }
}
pub fn scroll_ref() -> EntityRef {
    EntityRef {
        id: Counter(2),
        incarnation: id(3),
        kind: EntityKind::VirtualScroll,
    }
}
pub const SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'/>";
pub fn svg_manifest() -> wire::AssetManifest {
    wire::AssetManifest {
        id: "icon".into(),
        digest: wire::bytes_digest(SVG),
        kind: wire::AssetKind::Svg,
        encoded_bytes: SVG.len(),
        decoded_bytes: 8 * 8 * 4,
        width: 8,
        height: 8,
    }
}
/// Root Div with a List child (and its VirtualScroll entity) and an SVG child,
/// in view `generation`; `lists` names the List nodes and manifests.
pub fn publish(pub_rev: u64, generation: u64, lists: &[&str]) -> wire::Publish {
    let mut nodes = vec![node(wire::Component::Div)];
    let mut svg = node(wire::Component::Svg);
    svg.asset = Some(wire::AssetRef {
        id: "icon".into(),
        digest: wire::bytes_digest(SVG),
    });
    nodes.push(svg);
    let mut manifests = vec![];
    for list in lists {
        let mut n = node(wire::Component::List);
        n.entity = Some(scroll_ref());
        n.list_id = Some((*list).into());
        nodes.push(n);
        manifests.push(wire::ListManifest {
            list_id: (*list).into(),
            content_rev: Counter(1),
            kind: wire::ListKind::Uniform,
            item_count: 50,
            sizes_or_estimate: wire::ListSizes::Estimate(20.0),
            handler_generation: Counter(generation),
        });
    }
    nodes[0].children = (1..nodes.len()).collect();
    let view_ref = EntityRef {
        id: Counter(1),
        incarnation: id(3),
        kind: EntityKind::View,
    };
    let entities = vec![
        wire::EntityCreate {
            r#ref: view_ref.clone(),
            rev: Counter(1),
            config: wire::EntityConfig::View {},
            state: wire::EntityState::View {
                parent: None,
                root: 0,
                generation: Counter(generation),
                props_rev: Counter(1),
            },
            subscriptions: subscriptions(),
        },
        wire::EntityCreate {
            r#ref: scroll_ref(),
            rev: Counter(1),
            config: wire::EntityConfig::VirtualScroll {},
            state: wire::EntityState::VirtualScroll {
                offset: wire::Point { x: 0.0, y: 0.0 },
                anchor_key: String::new(),
                anchor_index: 0,
                content_rev: Counter(1),
                request: None,
            },
            subscriptions: subscriptions(),
        },
    ];
    let publish = wire::Publish {
        pub_rev: Counter(pub_rev),
        wm: wire::OpWatermark {
            processed_op_seq: Counter(0),
            state_rev: Counter(pub_rev),
            settled_local_seq: Counter(0),
        },
        entities_full: entities,
        views_full: vec![wire::ViewSnapshot {
            view_ref,
            generation: Counter(generation),
            root: 0,
            nodes,
            state_rev: Counter(pub_rev),
        }],
        focus_state: vec![],
        overlays_full: vec![],
        list_manifests: manifests,
        asset_manifests: vec![svg_manifest()],
    };
    publish.validate_shape().unwrap();
    publish
}
pub fn publish_bytes(context: &wire::ReceiveContext, body: wire::Publish) -> Vec<u8> {
    let seq = body.pub_rev.0;
    wire::encode_publish(
        &envelope(context, seq, body),
        &wire::ValidationTables::default(),
    )
    .unwrap()
}
pub fn rows_request(id: &str, list: &str, generation: u64, count: usize) -> wire::RowsRequest {
    wire::RowsRequest {
        request_id: id.into(),
        list_id: list.into(),
        view_generation: Counter(generation),
        content_rev: Counter(1),
        range: wire::RowRange {
            start: 0,
            end: count,
        },
        viewport_rev: Counter(1),
    }
}
/// A reply whose rows are Text nodes; `entity` adds an Input node bound to it.
pub fn rows_reply(request: &wire::RowsRequest, entity: Option<EntityRef>) -> wire::RowsReply {
    let rows = (request.range.start..request.range.end)
        .map(|i| {
            let mut text = node(wire::Component::Text);
            text.text = Some(format!("row {i}"));
            if let Some(entity) = &entity {
                text.component = wire::Component::Input;
                text.text = None;
                text.entity = Some(entity.clone());
            }
            let mut row = wire::Row {
                key: format!("row-{i}"),
                content_hash: wire::bytes_digest(b""),
                root: 0,
                nodes: vec![text],
            };
            row.content_hash = wire::canonical_digest(&row, "content_hash").unwrap();
            row
        })
        .collect();
    wire::RowsReply {
        request_id: request.request_id.clone(),
        list_id: request.list_id.clone(),
        view_generation: request.view_generation,
        content_rev: request.content_rev,
        range: request.range.clone(),
        viewport_rev: request.viewport_rev,
        rows,
    }
}
pub fn asset_request(id: &str) -> wire::AssetRequest {
    wire::AssetRequest {
        request_id: id.into(),
        asset: wire::AssetRef {
            id: "icon".into(),
            digest: wire::bytes_digest(SVG),
        },
    }
}
