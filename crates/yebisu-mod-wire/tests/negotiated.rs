//! Smaller negotiated A8 values are applied at each receiving and resource
//! scope: every case passes at the fixed ceiling and fails once the session
//! negotiated a lower value.
use yebisu_mod_wire::*;

fn id(n: u8) -> Id128 {
    Id128::new(format!("{n:032x}")).unwrap()
}
fn entity_ref(n: u64, kind: EntityKind) -> EntityRef {
    EntityRef {
        id: Counter(n),
        incarnation: id(1),
        kind,
    }
}
fn node(children: Vec<usize>, ops: Vec<Op>) -> Node {
    Node {
        component: Component::Div,
        text: None,
        entity: None,
        asset: None,
        text_format: None,
        child_view: None,
        list_id: None,
        children,
        ops,
    }
}
fn subscriptions() -> Subscriptions {
    Subscriptions {
        rev: Counter(1),
        events: vec![],
    }
}
/// A three-node chain (depth 3) with one callback, one coordinate argument,
/// a text node, an Input entity, an SVG asset and an empty measured list.
fn publish() -> Publish {
    let mut text = node(vec![], vec![]);
    text.component = Component::Text;
    text.text = Some("hello".into());
    let root = node(
        vec![1],
        vec![
            Op::Callback {
                name: "on_click".into(),
                id: Counter(1),
            },
            Op::ParamStyle {
                name: "w".into(),
                args: vec![Argument::Coordinate(500.0)],
            },
        ],
    );
    let mut svg = node(vec![2], vec![]);
    svg.component = Component::Svg;
    svg.asset = Some(AssetRef {
        id: "icon".into(),
        digest: bytes_digest(b"svg"),
    });
    let view = ViewSnapshot {
        view_ref: entity_ref(1, EntityKind::View),
        generation: Counter(1),
        root: 0,
        nodes: vec![root, svg, text],
        state_rev: Counter(2),
    };
    let view_entity = EntityCreate {
        r#ref: view.view_ref.clone(),
        rev: Counter(1),
        config: EntityConfig::View {},
        state: EntityState::View {
            parent: None,
            root: 0,
            generation: Counter(1),
            props_rev: Counter(1),
        },
        subscriptions: subscriptions(),
    };
    let input = EntityCreate {
        r#ref: entity_ref(2, EntityKind::Input),
        rev: Counter(1),
        config: EntityConfig::Input {
            placeholder: "name".into(),
            masked: false,
            loading: false,
            readonly: false,
            number: None,
        },
        state: EntityState::Input {
            text: "abcdef".into(),
            selection: Selection {
                start: 0,
                end: 0,
                direction: Direction::Forward,
            },
        },
        subscriptions: subscriptions(),
    };
    let publish = Publish {
        pub_rev: Counter(1),
        wm: OpWatermark {
            processed_op_seq: Counter(1),
            state_rev: Counter(2),
            settled_local_seq: Counter(1),
        },
        entities_full: vec![view_entity, input],
        views_full: vec![view],
        focus_state: vec![],
        overlays_full: vec![],
        list_manifests: vec![ListManifest {
            list_id: "list".into(),
            content_rev: Counter(1),
            kind: ListKind::Measured,
            item_count: 40,
            sizes_or_estimate: ListSizes::Estimate(20.0),
            handler_generation: Counter(1),
        }],
        asset_manifests: vec![svg_manifest()],
    };
    publish.validate_shape().unwrap();
    publish
}
fn svg_manifest() -> AssetManifest {
    AssetManifest {
        id: "icon".into(),
        digest: bytes_digest(b"svg"),
        kind: AssetKind::Svg,
        encoded_bytes: 2_000,
        decoded_bytes: 64 * 32 * 4,
        width: 64,
        height: 32,
    }
}
fn lowered(change: impl FnOnce(&mut NegotiatedLimits)) -> NegotiatedLimits {
    let mut limits = NegotiatedLimits::default();
    change(&mut limits);
    limits.validate().unwrap();
    limits
}
/// The publication passes at the ceiling and fails once `change` lowers a value.
fn publish_rejected_by(change: impl FnOnce(&mut NegotiatedLimits)) {
    let publish = publish();
    NegotiatedLimits::default().check_publish(&publish).unwrap();
    let err = lowered(change).check_publish(&publish).unwrap_err();
    assert!(matches!(err, Error::Limit(_)), "{err:?}");
}

#[test]
fn acknowledged_limits_cannot_raise_an_offered_value() {
    let offered = lowered(|l| l.nodes = 100);
    offered
        .check_acknowledged(&lowered(|l| l.nodes = 50))
        .unwrap();
    offered.check_acknowledged(&offered).unwrap();
    assert!(
        offered
            .check_acknowledged(&NegotiatedLimits::default())
            .is_err()
    );
    let coordinate = lowered(|l| l.coordinate_abs = 10.0);
    assert!(
        coordinate
            .check_acknowledged(&lowered(|l| l.coordinate_abs = 20.0))
            .is_err()
    );
}

#[test]
fn control_frames_and_messages_use_the_negotiated_size() {
    let limits = lowered(|l| l.control_bytes = 1_024);
    let control = FrameHeader {
        channel: 0,
        kind: FrameKind::Data,
        len: 2_000,
    };
    control.validate().unwrap();
    NegotiatedLimits::default().check_frame(&control).unwrap();
    assert!(limits.check_frame(&control).is_err());
    // Data chunks keep the fixed chunk size; no control limit applies to them.
    let data = FrameHeader {
        channel: 3,
        kind: FrameKind::Data,
        len: limits::CHUNK_BYTES as u32,
    };
    limits.check_frame(&data).unwrap();
    limits.check_control(1_024).unwrap();
    assert!(limits.check_control(1_025).is_err());
}

#[test]
fn publish_transfer_and_decompressed_bytes_use_negotiated_values() {
    let begin = PublishBegin {
        total_bytes: 100_000,
        chunk_count: 7,
        digest: bytes_digest(b"x"),
        channel: 1,
    };
    PublishAssembler::new(begin.clone()).unwrap();
    NegotiatedLimits::default()
        .check_publish_begin(&begin)
        .unwrap();
    assert!(
        lowered(|l| l.publish_bytes = 65_536)
            .check_publish_begin(&begin)
            .is_err()
    );
    let limits = lowered(|l| l.decompressed_bytes = 4_096);
    limits.check_decompressed(4_096).unwrap();
    assert!(limits.check_decompressed(4_097).is_err());
    publish_rejected_by(|l| l.decompressed_bytes = 64);
}

#[test]
fn publication_graph_costs_use_negotiated_values() {
    publish_rejected_by(|l| l.nodes = 2);
    publish_rejected_by(|l| l.depth = 2);
    publish_rejected_by(|l| l.visits = 2);
    publish_rejected_by(|l| l.expanded_ops = 1);
    publish_rejected_by(|l| l.stored_ops = 1);
    // Exactly at the measured values the publication is still accepted.
    lowered(|l| {
        l.nodes = 3;
        l.depth = 3;
        l.visits = 3;
        l.expanded_ops = 2;
        l.stored_ops = 2;
        l.callbacks = 1;
    })
    .check_publish(&publish())
    .unwrap();
}

#[test]
fn publication_callbacks_entities_and_strings_use_negotiated_values() {
    // The callback ceiling is 10,000 and cannot be lowered to zero, so add one.
    let mut publish = publish();
    publish.views_full[0].nodes[0].ops.push(Op::Callback {
        name: "on_hover".into(),
        id: Counter(2),
    });
    publish.validate_shape().unwrap();
    NegotiatedLimits::default().check_publish(&publish).unwrap();
    assert!(
        lowered(|l| l.callbacks = 1)
            .check_publish(&publish)
            .is_err()
    );
    publish_rejected_by(|l| l.entities = 1);
    publish_rejected_by(|l| l.entity_string_bytes = 5);
    publish_rejected_by(|l| l.entity_state_bytes = 100);
    publish_rejected_by(|l| l.string_bytes = 5);
    publish_rejected_by(|l| l.strings_bytes = 20);
    publish_rejected_by(|l| l.coordinate_abs = 100.0);
}

#[test]
fn publication_lists_and_assets_use_negotiated_values() {
    publish_rejected_by(|l| l.list_items = 39);
    publish_rejected_by(|l| l.image_dimension = 63);
    publish_rejected_by(|l| l.svg_bytes = 1_999);
    publish_rejected_by(|l| l.asset_decoded_bytes = 8_000);
    publish_rejected_by(|l| l.assets_decoded_bytes = 8_000);
    publish_rejected_by(|l| l.assets_pixels = 2_000);
    let mut two = publish();
    two.list_manifests.push(ListManifest {
        list_id: "other".into(),
        ..two.list_manifests[0].clone()
    });
    two.validate_shape().unwrap();
    NegotiatedLimits::default().check_publish(&two).unwrap();
    assert!(lowered(|l| l.lists = 1).check_publish(&two).is_err());
}

fn rows(count: usize) -> (RowsRequest, RowsReply) {
    let request = RowsRequest {
        request_id: "rows".into(),
        list_id: "list".into(),
        view_generation: Counter(1),
        content_rev: Counter(1),
        range: RowRange {
            start: 0,
            end: count,
        },
        viewport_rev: Counter(1),
    };
    let rows = (0..count)
        .map(|i| {
            let mut text = node(vec![], vec![]);
            text.component = Component::Text;
            text.text = Some(format!("row {i}"));
            let mut row = Row {
                key: format!("row-{i}"),
                content_hash: bytes_digest(b""),
                root: 0,
                nodes: vec![node(vec![1], vec![]), text],
            };
            row.content_hash = canonical_digest(&row, "content_hash").unwrap();
            row
        })
        .collect();
    let reply = RowsReply {
        request_id: request.request_id.clone(),
        list_id: request.list_id.clone(),
        view_generation: request.view_generation,
        content_rev: request.content_rev,
        range: request.range.clone(),
        viewport_rev: request.viewport_rev,
        rows,
    };
    reply.validate_shape().unwrap();
    (request, reply)
}

#[test]
fn row_requests_and_replies_use_negotiated_values() {
    let (request, reply) = rows(3);
    let defaults = NegotiatedLimits::default();
    defaults.check_rows_request(&request).unwrap();
    defaults.check_rows_reply(&reply).unwrap();
    for change in [
        (|l: &mut NegotiatedLimits| l.rows = 2) as fn(&mut NegotiatedLimits),
        |l| l.list_items = 2,
    ] {
        let limits = lowered(change);
        assert!(limits.check_rows_request(&request).is_err());
        assert!(limits.check_rows_reply(&reply).is_err());
    }
    for change in [
        (|l: &mut NegotiatedLimits| l.rows_bytes = 200) as fn(&mut NegotiatedLimits),
        |l| l.nodes = 5,
        |l| l.depth = 1,
        |l| l.visits = 5,
        |l| l.string_bytes = 4,
        |l| l.strings_bytes = 10,
    ] {
        assert!(lowered(change).check_rows_reply(&reply).is_err());
    }
}

#[test]
fn asset_replies_use_negotiated_values() {
    let manifest = svg_manifest();
    let reply = AssetReply {
        request_id: "asset".into(),
        asset: AssetRef {
            id: manifest.id.clone(),
            digest: manifest.digest.clone(),
        },
        channel: 2,
        total_bytes: manifest.encoded_bytes,
        chunk_count: 1,
    };
    let request = AssetRequest {
        request_id: "asset".into(),
        asset: reply.asset.clone(),
    };
    reply.validate_against(&request, &manifest).unwrap();
    NegotiatedLimits::default()
        .check_asset_reply(&reply, &manifest)
        .unwrap();
    assert!(
        lowered(|l| l.svg_bytes = 1_000)
            .check_asset_reply(&reply, &manifest)
            .is_err()
    );
    let mut png = manifest.clone();
    png.kind = AssetKind::Png;
    assert!(
        lowered(|l| l.image_bytes = 1_000)
            .check_asset_reply(&reply, &png)
            .is_err()
    );
    // The SVG ceiling does not apply to a raster image, nor the image ceiling to SVG.
    lowered(|l| l.image_bytes = 1_000)
        .check_asset_reply(&reply, &manifest)
        .unwrap();
}

#[test]
fn edits_and_actions_received_by_the_runner_use_negotiated_values() {
    let mut edit = Edit {
        op_seq: Counter(1),
        edit_id: "edit".into(),
        entity: entity_ref(2, EntityKind::Input),
        subscription_rev: Counter(1),
        base_rev: Counter(1),
        local_seq: Counter(1),
        origin: UserOrigin::User,
        text: "abcdef".into(),
        selection: Selection {
            start: 0,
            end: 0,
            direction: Direction::Forward,
        },
        digest: bytes_digest(b""),
    };
    edit.digest = canonical_digest(&edit, "digest").unwrap();
    edit.validate().unwrap();
    NegotiatedLimits::default().check_edit(&edit).unwrap();
    assert!(
        lowered(|l| l.entity_string_bytes = 5)
            .check_edit(&edit)
            .is_err()
    );
    let mut action = Action {
        op_seq: Counter(2),
        event_id: "event".into(),
        after_local_seq: Counter(1),
        pub_rev: Counter(1),
        generation: Counter(1),
        lease_id: "lease".into(),
        target: 0,
        kind: ActionKind::Click,
        data: ActionData {
            key: None,
            text: Some("typed".into()),
            point: Some(Point { x: 300.0, y: 1.0 }),
            value: None,
            row_key: None,
            permissions: vec![],
        },
        digest: bytes_digest(b""),
    };
    action.digest = canonical_digest(&action, "digest").unwrap();
    action.validate().unwrap();
    NegotiatedLimits::default().check_action(&action).unwrap();
    assert!(
        lowered(|l| l.coordinate_abs = 100.0)
            .check_action(&action)
            .is_err()
    );
    assert!(
        lowered(|l| l.string_bytes = 4)
            .check_action(&action)
            .is_err()
    );
}

const RESOURCES: [Resource; 27] = [
    Resource::HostCopies,
    Resource::AppCopies,
    Resource::CallbackGenerations,
    Resource::Callbacks,
    Resource::Events,
    Resource::ControlPending,
    Resource::ControlPerSecond,
    Resource::Receipts,
    Resource::ReceiptBytes,
    Resource::RowRequestsList,
    Resource::RowRequestsCopy,
    Resource::RowRequestsApp,
    Resource::RowCacheBytes,
    Resource::AssetCacheBytes,
    Resource::AssetTransfersCopy,
    Resource::AssetTransfersHost,
    Resource::AssetTransfersApp,
    Resource::AssetDecodedBytes,
    Resource::AssetsDecodedBytes,
    Resource::AssetsPixels,
    Resource::AppBytes,
    Resource::JsHeapBytes,
    Resource::WorkerRssBytes,
    Resource::WorkersRssBytes,
    Resource::ChildrenCopy,
    Resource::ChildrenHost,
    Resource::BackgroundCopies,
];

#[test]
fn every_resource_ledger_uses_its_negotiated_value() {
    let ones = {
        let mut limits = NegotiatedLimits::default();
        for r in RESOURCES {
            // Lower exactly the field behind this resource.
            let mut probe = NegotiatedLimits::default();
            set_resource(&mut probe, r, 1);
            assert_eq!(r.negotiated(&probe), 1, "{r:?}");
            set_resource(&mut limits, r, 1);
        }
        limits
    };
    for r in RESOURCES {
        let mut ceiling = ResourceLedger::default();
        ceiling.reserve(r, 2).unwrap();
        let mut ledger = ResourceLedger::negotiated(&ones).unwrap();
        assert_eq!(ledger.limit(r), 1);
        ledger.reserve(r, 1).unwrap();
        assert!(ledger.reserve(r, 1).is_err(), "{r:?}");
        // A failed reservation leaves the count unchanged.
        assert_eq!(ledger.used(r), 1);
        ledger.release(r, 1).unwrap();
        ledger.reserve(r, 1).unwrap();
    }
    let zero = NegotiatedLimits {
        row_requests_list: 0,
        ..NegotiatedLimits::default()
    };
    assert!(ResourceLedger::negotiated(&zero).is_err());
}

#[test]
fn list_copy_host_and_application_scopes_each_apply_their_own_value() {
    // One list asks for rows while its copy, host and application ledgers are
    // shared with others. Each scope stops at its own negotiated value.
    let limits = lowered(|l| {
        l.row_requests_list = 1;
        l.row_requests_copy = 2;
        l.asset_transfers_copy = 1;
        l.asset_transfers_host = 2;
        l.asset_transfers_app = 3;
    });
    let mut list_a = ResourceLedger::negotiated(&limits).unwrap();
    let mut list_b = ResourceLedger::negotiated(&limits).unwrap();
    let mut list_c = ResourceLedger::negotiated(&limits).unwrap();
    let mut copy = ResourceLedger::negotiated(&limits).unwrap();
    list_a.reserve(Resource::RowRequestsList, 1).unwrap();
    assert!(list_a.reserve(Resource::RowRequestsList, 1).is_err());
    copy.reserve(Resource::RowRequestsCopy, 1).unwrap();
    list_b.reserve(Resource::RowRequestsList, 1).unwrap();
    copy.reserve(Resource::RowRequestsCopy, 1).unwrap();
    list_c.reserve(Resource::RowRequestsList, 1).unwrap();
    assert!(copy.reserve(Resource::RowRequestsCopy, 1).is_err());

    let mut copy_a = ResourceLedger::negotiated(&limits).unwrap();
    let mut copy_b = ResourceLedger::negotiated(&limits).unwrap();
    let mut host = ResourceLedger::negotiated(&limits).unwrap();
    let mut app = ResourceLedger::negotiated(&limits).unwrap();
    copy_a.reserve(Resource::AssetTransfersCopy, 1).unwrap();
    assert!(copy_a.reserve(Resource::AssetTransfersCopy, 1).is_err());
    host.reserve(Resource::AssetTransfersHost, 1).unwrap();
    copy_b.reserve(Resource::AssetTransfersCopy, 1).unwrap();
    host.reserve(Resource::AssetTransfersHost, 1).unwrap();
    app.reserve(Resource::AssetTransfersApp, 2).unwrap();
    assert!(host.reserve(Resource::AssetTransfersHost, 1).is_err());
    // A second host shares only the application scope.
    app.reserve(Resource::AssetTransfersApp, 1).unwrap();
    assert!(app.reserve(Resource::AssetTransfersApp, 1).is_err());
}

fn set_resource(l: &mut NegotiatedLimits, r: Resource, v: usize) {
    let field = match r {
        Resource::HostCopies => &mut l.copies_host,
        Resource::AppCopies => &mut l.copies_app,
        Resource::CallbackGenerations => &mut l.callback_generations,
        Resource::Callbacks => &mut l.callbacks,
        Resource::Events => &mut l.event_queue,
        Resource::ControlPending => &mut l.control_pending_bytes,
        Resource::ControlPerSecond => &mut l.control_per_second,
        Resource::Receipts => &mut l.receipts,
        Resource::ReceiptBytes => &mut l.receipts_bytes,
        Resource::RowRequestsList => &mut l.row_requests_list,
        Resource::RowRequestsCopy => &mut l.row_requests_copy,
        Resource::RowRequestsApp => &mut l.row_requests_app,
        Resource::RowCacheBytes => &mut l.row_cache_bytes,
        Resource::AssetCacheBytes => &mut l.asset_cache_bytes,
        Resource::AssetTransfersCopy => &mut l.asset_transfers_copy,
        Resource::AssetTransfersHost => &mut l.asset_transfers_host,
        Resource::AssetTransfersApp => &mut l.asset_transfers_app,
        Resource::AssetDecodedBytes => &mut l.asset_decoded_bytes,
        Resource::AssetsDecodedBytes => &mut l.assets_decoded_bytes,
        Resource::AssetsPixels => &mut l.assets_pixels,
        Resource::AppBytes => &mut l.app_bytes,
        Resource::JsHeapBytes => &mut l.js_heap_bytes,
        Resource::WorkerRssBytes => &mut l.worker_rss_bytes,
        Resource::WorkersRssBytes => &mut l.workers_rss_bytes,
        Resource::ChildrenCopy => &mut l.children_copy,
        Resource::ChildrenHost => &mut l.children_host,
        Resource::BackgroundCopies => &mut l.background_copies,
    };
    *field = v;
}
