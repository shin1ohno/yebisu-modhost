use yebisu_mod_wire::*;
fn id(n: u8) -> Id128 {
    Id128::new(format!("{n:032x}")).unwrap()
}
fn digest() -> Digest {
    bytes_digest(b"test")
}
fn entity_ref(n: u64, kind: EntityKind) -> EntityRef {
    EntityRef {
        id: Counter(n),
        incarnation: id(1),
        kind,
    }
}
fn node() -> Node {
    Node {
        component: Component::Div,
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
fn wm() -> OpWatermark {
    OpWatermark {
        processed_op_seq: Counter(1),
        state_rev: Counter(2),
        settled_local_seq: Counter(1),
    }
}
fn view_entity(view: &ViewSnapshot, parent: Option<EntityRef>) -> EntityCreate {
    EntityCreate {
        r#ref: view.view_ref.clone(),
        rev: Counter(1),
        config: EntityConfig::View {},
        state: EntityState::View {
            parent,
            root: view.root,
            generation: view.generation,
            props_rev: Counter(1),
        },
        subscriptions: Subscriptions {
            rev: Counter(1),
            events: vec![],
        },
    }
}
fn validate_rows(
    reply: &RowsReply,
    request: &RowsRequest,
    manifest: &ListManifest,
    tables: &ValidationTables,
) -> Result<()> {
    let mut publication = publish();
    publication.list_manifests.push(manifest.clone());
    let context = RowValidation::new(&publication, tables)?;
    reply.validate_against(request, manifest, &context)
}
fn publish() -> Publish {
    let mut p = Publish {
        pub_rev: Counter(1),
        wm: wm(),
        entities_full: vec![],
        views_full: vec![ViewSnapshot {
            view_ref: entity_ref(1, EntityKind::View),
            generation: Counter(1),
            root: 0,
            nodes: vec![node()],
            state_rev: Counter(2),
        }],
        focus_state: vec![],
        overlays_full: vec![],
        list_manifests: vec![],
        asset_manifests: vec![],
    };
    p.entities_full.push(view_entity(&p.views_full[0], None));
    p
}
fn envelope<T: Payload>(body: T) -> Envelope<T> {
    Envelope::new(
        Session {
            host: "SHA256:verified-by-SSH".into(),
            boot: id(1),
            client: "ipad".into(),
            epoch: id(2),
        },
        Scope::Copy {
            id: "mod".into(),
            incarnation: id(3),
        },
        Counter(1),
        "message-1".into(),
        Counter(1),
        body,
    )
}
fn roundtrip<T: Payload + std::fmt::Debug + PartialEq>(body: T) {
    let value = envelope(body);
    let bytes = encode(&value).unwrap();
    assert_eq!(decode::<T>(&bytes).unwrap(), value);
}
fn edit() -> Edit {
    let mut value = Edit {
        op_seq: Counter(2),
        edit_id: "edit-1".into(),
        entity: entity_ref(2, EntityKind::Input),
        subscription_rev: Counter(1),
        base_rev: Counter(1),
        local_seq: Counter(2),
        origin: UserOrigin::User,
        text: "あ🦀".into(),
        selection: Selection {
            start: 3,
            end: 7,
            direction: Direction::Forward,
        },
        digest: digest(),
    };
    value.digest = canonical_digest(&value, "digest").unwrap();
    value
}
fn input() -> EntityCreate {
    EntityCreate {
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
            text: "あ".into(),
            selection: Selection {
                start: 3,
                end: 3,
                direction: Direction::Forward,
            },
        },
        subscriptions: Subscriptions {
            rev: Counter(1),
            events: vec![SubscriptionEvent::Change],
        },
    }
}
fn list() -> ListManifest {
    ListManifest {
        list_id: "list".into(),
        content_rev: Counter(1),
        kind: ListKind::Measured,
        item_count: 1,
        sizes_or_estimate: ListSizes::Estimate(20.0),
        handler_generation: Counter(1),
    }
}
fn rows() -> (RowsRequest, RowsReply) {
    let req = RowsRequest {
        request_id: "rows-1".into(),
        list_id: "list".into(),
        view_generation: Counter(1),
        content_rev: Counter(1),
        range: RowRange { start: 0, end: 1 },
        viewport_rev: Counter(1),
    };
    let mut row = Row {
        key: "row-0".into(),
        content_hash: digest(),
        root: 0,
        nodes: vec![node()],
    };
    row.content_hash = canonical_digest(&row, "content_hash").unwrap();
    let reply = RowsReply {
        request_id: req.request_id.clone(),
        list_id: req.list_id.clone(),
        view_generation: req.view_generation,
        content_rev: req.content_rev,
        range: req.range.clone(),
        viewport_rev: req.viewport_rev,
        rows: vec![row],
    };
    (req, reply)
}
#[test]
fn snapshot_and_messages_roundtrip() {
    let p = publish();
    p.validate(&ValidationTables::default()).unwrap();
    roundtrip(p);
    roundtrip(input());
    roundtrip(edit());
    roundtrip(EditAck {
        edit_id: "e".into(),
        op_seq: Counter(2),
        wm: wm(),
        applied_rev: Counter(2),
        result_rev: Counter(3),
        state: input().state,
        digest: digest(),
    });
    roundtrip(ActionAck {
        event_id: "a".into(),
        op_seq: Counter(3),
        wm: wm(),
        affected_entities: vec![],
        result: ActionResult::Ok,
        digest: digest(),
    });
    roundtrip(EditConflict {
        edit_id: "e".into(),
        op_seq: Counter(2),
        wm: wm(),
        entity: input().r#ref,
        current_rev: Counter(4),
        state: input().state,
        digest: digest(),
    });
    roundtrip(OpRejected {
        id: "a".into(),
        op_seq: Counter(2),
        wm: wm(),
        reason: RejectionReason::Cancelled,
        digest: digest(),
    });
    roundtrip(OpQuery {
        op_seq: Counter(2),
        id: "e".into(),
        digest: digest(),
        on_absent: OnAbsent::Cancel,
    });
    roundtrip(OpNotReceived {
        op_seq: Counter(2),
        id: "e".into(),
        wm: wm(),
        cancelled: true,
    });
    let (req, reply) = rows();
    validate_rows(&reply, &req, &list(), &ValidationTables::default()).unwrap();
    roundtrip(req);
    roundtrip(reply);
    roundtrip(StopAck {
        op_id: "s".into(),
        boot: id(1),
        copy: "mod".into(),
        incarnation: id(3),
        status: StopStatus::Incomplete,
        managed_tasks: 0,
        children: vec![StoppedChild {
            id: "child".into(),
            result: ChildResult::Unknown,
        }],
        escaped_or_remote_unknown: true,
        persisted: false,
    });
    roundtrip(DeviceRequest {
        request_id: "url".into(),
        permission: id(4),
        event_id: "tap".into(),
        operation: DeviceOperation::OpenUrl,
        policy_rev: Counter(1),
        value: "https://example.com/".into(),
    });
}
#[test]
fn counters_are_canonical_u64_strings() {
    assert_eq!(
        serde_json::to_string(&Counter(u64::MAX)).unwrap(),
        "\"18446744073709551615\""
    );
    for s in [
        "1",
        "-1",
        "\"-1\"",
        "\"01\"",
        "\"\"",
        "\"18446744073709551616\"",
        "\"+1\"",
        "\"١\"",
    ] {
        assert!(serde_json::from_str::<Counter>(s).is_err(), "{s}");
    }
}
#[test]
fn unknown_fields_and_duplicate_keys_are_rejected_at_all_depths() {
    let value = envelope(publish());
    let bytes = encode(&value).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["future"] = true.into();
    assert!(decode::<Publish>(&serde_json::to_vec(&json).unwrap()).is_err());
    json.as_object_mut().unwrap().remove("future");
    json["body"]["views_full"][0]["nodes"][0]["future"] = true.into();
    assert!(decode::<Publish>(&serde_json::to_vec(&json).unwrap()).is_err());
    for json in [
        r#"{"x":1,"x":2}"#,
        r#"{"outer":{"x":1,"x":2}}"#,
        r#"{"x":1,"\u0078":2}"#,
    ] {
        assert!(strict_json(json.as_bytes()).is_err());
    }
    assert!(strict_json(b"{} {}").is_err());
}
#[test]
fn unknown_components_ops_and_unsupported_profiles_fail_even_in_slots() {
    for component in [
        "Module",
        "Registered",
        "Editor",
        "DockArea",
        "DockContent",
        "FutureWidget",
    ] {
        let mut json = serde_json::to_value(envelope(publish())).unwrap();
        json["body"]["views_full"][0]["nodes"][0]["component"] = component.into();
        assert!(decode::<Publish>(&serde_json::to_vec(&json).unwrap()).is_err());
    }
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![Op::Method {
        name: "unknown".into(),
        args: vec![],
    }];
    assert_eq!(
        p.validate(&ValidationTables::default()),
        Err(Error::Unknown("op table entry"))
    );
    let mut json = serde_json::to_value(envelope(p)).unwrap();
    json["body"]["views_full"][0]["nodes"][0]["ops"][0]["op"] = "FutureOp".into();
    assert!(decode::<Publish>(&serde_json::to_vec(&json).unwrap()).is_err());
}
#[test]
fn negotiated_tables_check_style_index_and_name_and_all_op_names() {
    let mut tables = ValidationTables::default();
    tables.styles.insert(1, "flex".into());
    tables.param_styles.insert("width".into());
    tables.methods.insert("disabled".into());
    tables.callbacks.insert("click".into());
    tables.slots.insert("content".into());
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![Op::NullaryStyle {
        index: 1,
        name: "flex".into(),
    }];
    p.validate(&tables).unwrap();
    for op in [
        Op::NullaryStyle {
            index: 2,
            name: "flex".into(),
        },
        Op::NullaryStyle {
            index: 1,
            name: "wrong".into(),
        },
        Op::ParamStyle {
            name: "unknown".into(),
            args: vec![],
        },
        Op::Method {
            name: "unknown".into(),
            args: vec![],
        },
        Op::Callback {
            name: "unknown".into(),
            id: Counter(1),
        },
    ] {
        p.views_full[0].nodes[0].ops = vec![op];
        assert_eq!(p.validate(&tables), Err(Error::Unknown("op table entry")));
    }
}
#[test]
fn message_and_json_size_limits() {
    let mut e = envelope(Pause {
        op_id: "x".repeat(limits::CONTROL_BYTES),
    });
    assert_eq!(encode(&e), Err(Error::Limit("message bytes")));
    e.body.op_id = "ok".into();
    roundtrip(e.body);
    assert_eq!(
        decode::<Pause>(&vec![b' '; limits::CONTROL_BYTES + 1]),
        Err(Error::Limit("message bytes"))
    );
    let quoted = format!("\"{}\"", "a".repeat(limits::STRING_BYTES + 1));
    assert!(strict_json(quoted.as_bytes()).is_err());
    let nested = format!(
        "{}0{}",
        "[".repeat(limits::JSON_DEPTH + 1),
        "]".repeat(limits::JSON_DEPTH + 1)
    );
    assert!(strict_json(nested.as_bytes()).is_err());
    let okay = format!("{}0{}", "[".repeat(8), "]".repeat(8));
    strict_json(okay.as_bytes()).unwrap();
    assert_eq!(
        read_bounded(&b"12345"[..], 4),
        Err(Error::Limit("decompressed bytes"))
    );
    assert_eq!(read_bounded(&b"1234"[..], 4).unwrap(), b"1234");
}
#[test]
fn publish_encoded_limit_precedes_parse() {
    assert_eq!(
        decode::<Publish>(&vec![b' '; limits::PUBLISH_BYTES + 1]),
        Err(Error::Limit("message bytes"))
    );
}
#[test]
fn graph_node_depth_cycle_and_unreachable_limits() {
    let mut p = publish();
    p.views_full[0].nodes = vec![node(); limits::NODES + 1];
    assert_eq!(p.validate_shape(), Err(Error::Limit("nodes")));
    let mut p = publish();
    p.views_full[0].nodes = vec![node(); limits::DEPTH];
    for (i, node) in p.views_full[0]
        .nodes
        .iter_mut()
        .enumerate()
        .take(limits::DEPTH - 1)
    {
        node.children = vec![i + 1];
    }
    p.validate_shape().unwrap();
    p.views_full[0].nodes.push(node());
    p.views_full[0].nodes[limits::DEPTH - 1].children = vec![limits::DEPTH];
    assert_eq!(p.validate_shape(), Err(Error::Limit("depth")));
    let mut p = publish();
    p.views_full[0].nodes[0].children = vec![0];
    assert_eq!(p.validate_shape(), Err(Error::Invalid("graph cycle")));
    let mut p = publish();
    p.views_full[0].nodes.push(node());
    assert_eq!(p.validate_shape(), Err(Error::Invalid("unreachable node")));
    p.views_full[0].nodes[0].children = vec![usize::MAX];
    assert_eq!(p.validate_shape(), Err(Error::Invalid("node index")));
}
#[test]
fn repeated_edges_slots_and_state_styles_count_expansion() {
    let mut p = publish();
    p.views_full[0].nodes.push(node());
    p.views_full[0].nodes[0].children = vec![1; limits::VISITS - 1];
    p.validate_shape().unwrap();
    p.views_full[0].nodes[0].children.push(1);
    assert_eq!(p.validate_shape(), Err(Error::Limit("expanded visits")));
    for slot in [true, false] {
        let mut p = publish();
        p.views_full[0].nodes[0].ops = vec![if slot {
            Op::Slot {
                name: "content".into(),
                node: 0,
            }
        } else {
            Op::StateStyle {
                name: StateStyle::Hover,
                node: 0,
            }
        }];
        assert_eq!(p.validate_shape(), Err(Error::Invalid("graph cycle")));
    }
}
#[test]
fn weighted_expanded_ops_reject_small_dag_amplification() {
    let mut p = publish();
    p.views_full[0].nodes.push(node());
    p.views_full[0].nodes[0].children = vec![1; 1000];
    p.views_full[0].nodes[1].ops = vec![
        Op::NullaryStyle {
            index: 0,
            name: "flex".into()
        };
        160
    ];
    p.validate_shape().unwrap();
    p.views_full[0].nodes[1].ops.push(Op::NullaryStyle {
        index: 0,
        name: "flex".into(),
    });
    assert_eq!(p.validate_shape(), Err(Error::Limit("expanded ops")));
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![
        Op::NullaryStyle {
            index: 0,
            name: "".into()
        };
        limits::STORED_OPS + 1
    ];
    assert!(p.validate_shape().is_err());
}
#[test]
fn strings_have_individual_and_aggregate_limits() {
    let mut p = publish();
    p.views_full[0].nodes[0].text = Some("a".repeat(limits::STRING_BYTES + 1));
    assert_eq!(p.validate_shape(), Err(Error::Limit("string bytes")));
    let mut p = publish();
    p.views_full[0].nodes = vec![node(); 5];
    p.views_full[0].nodes[0].children = vec![1, 2, 3, 4];
    for n in &mut p.views_full[0].nodes {
        n.text = Some("a".repeat(limits::STRING_BYTES));
    }
    assert_eq!(p.validate_shape(), Err(Error::Limit("aggregate strings")));
}
#[test]
fn entity_state_limits_kinds_and_utf8_boundaries() {
    let mut e = input();
    e.validate().unwrap();
    e.state = EntityState::Input {
        text: "x".repeat(limits::ENTITY_STRING_BYTES + 1),
        selection: Selection {
            start: 0,
            end: 0,
            direction: Direction::Forward,
        },
    };
    assert_eq!(e.validate(), Err(Error::Limit("entity text")));
    let mut e = input();
    e.r#ref.kind = EntityKind::Otp;
    assert_eq!(e.validate(), Err(Error::Invalid("entity kind/revision")));
    let text = "a🦀あ";
    Selection::from_utf16(text, 1, 3, Direction::Forward).unwrap();
    assert!(Selection::from_utf16(text, 1, 2, Direction::Forward).is_err());
    assert!(
        Selection {
            start: 2,
            end: 3,
            direction: Direction::Forward
        }
        .validate(text)
        .is_err()
    );
    assert!(
        Selection {
            start: 0,
            end: 99,
            direction: Direction::Forward
        }
        .validate(text)
        .is_err()
    );
    let mut p = publish();
    p.entities_full = vec![input(); limits::ENTITIES + 1];
    assert_eq!(p.validate_shape(), Err(Error::Limit("entities")));
}
#[test]
fn entity_revision_and_create_only_fields() {
    let old = input();
    let mut update = EntityUpdate {
        r#ref: old.r#ref.clone(),
        base_rev: Counter(1),
        rev: Counter(2),
        origin: Origin::HostSet,
        config: old.config.clone(),
        state: old.state.clone(),
        subscriptions: old.subscriptions.clone(),
    };
    update.validate_against(&old).unwrap();
    update.rev = Counter(3);
    assert!(update.validate_against(&old).is_err());
    let mut e = input();
    e.r#ref.kind = EntityKind::Otp;
    e.config = EntityConfig::Otp {
        digits: 6,
        masked: true,
    };
    e.state = EntityState::Otp {
        text: "12".into(),
        selection: Selection {
            start: 2,
            end: 2,
            direction: Direction::Forward,
        },
    };
    e.validate().unwrap();
    update = EntityUpdate {
        r#ref: e.r#ref.clone(),
        base_rev: Counter(1),
        rev: Counter(2),
        origin: Origin::HostSet,
        config: EntityConfig::Otp {
            digits: 8,
            masked: true,
        },
        state: e.state.clone(),
        subscriptions: e.subscriptions.clone(),
    };
    assert_eq!(
        update.validate_against(&e),
        Err(Error::Invalid("create-only OTP digits"))
    );
    e.config = EntityConfig::Otp {
        digits: 0,
        masked: true,
    };
    assert_eq!(e.validate(), Err(Error::Limit("OTP digits")));
}
#[test]
fn numeric_limits_and_nonfinite_values() {
    assert!(
        Size {
            width: 0.0,
            height: 1.0
        }
        .validate()
        .is_err()
    );
    assert!(
        Size {
            width: f64::INFINITY,
            height: 1.0
        }
        .validate()
        .is_err()
    );
    assert!(
        Point {
            x: limits::COORDINATE_ABS,
            y: 0.0
        }
        .validate()
        .is_ok()
    );
    assert_eq!(
        Point {
            x: limits::COORDINATE_ABS + 1.0,
            y: 0.0
        }
        .validate(),
        Err(Error::Limit("coordinate"))
    );
    for range in [
        NumberRange {
            min: 1.0,
            max: 1.0,
            step: 1.0,
        },
        NumberRange {
            min: 0.0,
            max: 1.0,
            step: 0.0,
        },
        NumberRange {
            min: f64::NAN,
            max: 1.0,
            step: 1.0,
        },
    ] {
        assert!(range.validate().is_err());
    }
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![Op::Method {
        name: "x".into(),
        args: vec![Argument::Number(f64::NAN)],
    }];
    assert_eq!(p.validate_shape(), Err(Error::Invalid("nonfinite number")));
}
#[test]
fn stale_session_scope_fence_and_generation_rejected() {
    let e = envelope(publish());
    let context = ReceiveContext {
        session: e.session.clone(),
        scope: e.scope.clone(),
        owner_fence: e.owner_fence,
    };
    context.validate(&e).unwrap();
    let mut wrong = e.clone();
    wrong.session.boot = id(9);
    assert_eq!(
        context.validate(&wrong),
        Err(Error::Stale("session identity"))
    );
    wrong = e.clone();
    wrong.session.epoch = id(9);
    assert!(context.validate(&wrong).is_err());
    wrong = e.clone();
    wrong.session.host = "unverified".into();
    assert!(context.validate(&wrong).is_err());
    wrong = e.clone();
    wrong.scope = Scope::Copy {
        id: "mod".into(),
        incarnation: id(9),
    };
    assert_eq!(
        context.validate(&wrong),
        Err(Error::Stale("copy incarnation"))
    );
    wrong = e.clone();
    wrong.owner_fence = Counter(0);
    assert_eq!(context.validate(&wrong), Err(Error::Stale("owner fence")));
    let mut tracker = GenerationTracker::default();
    tracker.commit(&e.body).unwrap();
    assert_eq!(
        tracker.commit(&e.body),
        Err(Error::Stale("view generation reused"))
    );
    let mut next = e.body;
    next.views_full[0].generation = Counter(2);
    tracker.commit(&next).unwrap();
}
#[test]
fn row_identity_cardinality_keys_and_callbacks() {
    let (req, reply) = rows();
    let m = list();
    let tables = ValidationTables::default();
    validate_rows(&reply, &req, &m, &tables).unwrap();
    let mut bad = reply.clone();
    bad.viewport_rev = Counter(2);
    assert_eq!(
        validate_rows(&bad, &req, &m, &tables),
        Err(Error::Stale("rows request/revision"))
    );
    bad = reply.clone();
    bad.rows.clear();
    assert_eq!(
        validate_rows(&bad, &req, &m, &tables),
        Err(Error::Invalid("row cardinality"))
    );
    bad = reply.clone();
    bad.rows[0].nodes[0].ops.push(Op::ActionCallback {
        name: "click".into(),
        id: Counter(1),
    });
    assert_eq!(
        validate_rows(&bad, &req, &m, &tables),
        Err(Error::Invalid("row callback"))
    );
    bad = reply.clone();
    bad.rows[0].nodes[0].text = Some("changed".into());
    assert_eq!(
        validate_rows(&bad, &req, &m, &tables),
        Err(Error::DigestMismatch)
    );
    let mut req = req.clone();
    req.range.end = limits::ROWS + 1;
    assert_eq!(req.validate(), Err(Error::Limit("row count")));
    req.range.start = usize::MAX;
    assert_eq!(req.validate(), Err(Error::Invalid("row range")));
    let mut m = m;
    m.item_count = limits::LIST_ITEMS + 1;
    assert_eq!(m.validate(), Err(Error::Limit("list items")));
    m.item_count = 1;
    m.sizes_or_estimate = ListSizes::Estimate(0.0);
    assert_eq!(m.validate(), Err(Error::Invalid("nonpositive size")));
}
#[test]
fn url_scheme_and_credentials_checked_and_host_normalized() {
    assert_eq!(
        validate_url("https://EXAMPLE.com/path").unwrap(),
        "example.com"
    );
    for url in [
        "http://example.com",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/plain,x",
        "ssh://example.com",
        "https://user:pass@example.com",
        "/relative",
    ] {
        assert!(validate_url(url).is_err(), "{url}");
    }
}
#[test]
fn asset_limits_digest_and_request_binding() {
    let bytes = b"asset".to_vec();
    let mut m = AssetManifest {
        id: "a".into(),
        digest: bytes_digest(&bytes),
        kind: AssetKind::Png,
        encoded_bytes: bytes.len(),
        decoded_bytes: 4,
        width: 1,
        height: 1,
    };
    m.validate().unwrap();
    let req = AssetRequest {
        request_id: "req".into(),
        asset: AssetRef {
            id: "a".into(),
            digest: m.digest.clone(),
        },
    };
    let mut reply = AssetReply {
        request_id: "req".into(),
        asset: req.asset.clone(),
        bytes,
    };
    reply.validate_against(&req, &m).unwrap();
    reply.bytes[0] = 0;
    assert_eq!(reply.validate_against(&req, &m), Err(Error::DigestMismatch));
    m.encoded_bytes = limits::IMAGE_BYTES + 1;
    assert_eq!(m.validate(), Err(Error::Limit("encoded asset bytes")));
    m.kind = AssetKind::Svg;
    m.encoded_bytes = limits::SVG_BYTES + 1;
    assert_eq!(m.validate(), Err(Error::Limit("encoded asset bytes")));
    m.encoded_bytes = 1;
    m.decoded_bytes = limits::ASSET_DECODED_BYTES + 1;
    assert_eq!(m.validate(), Err(Error::Limit("decoded asset bytes")));
    m.width = limits::IMAGE_DIMENSION + 1;
    assert_eq!(m.validate(), Err(Error::Limit("asset width")));
    m.width = 4096;
    m.height = 4096;
    m.decoded_bytes = limits::ASSET_DECODED_BYTES;
    assert_eq!(
        m.validate(),
        Err(Error::Invalid("decoded asset undercount"))
    );
}
#[test]
fn frame_layout_length_kind_and_credit_are_bounded_before_allocation() {
    let f = Frame {
        channel: 0x01020304,
        kind: FrameKind::Data,
        payload: b"abc".to_vec(),
    };
    let bytes = f.encode().unwrap();
    assert_eq!(&bytes[..9], &[4, 3, 2, 1, 0, 3, 0, 0, 0]);
    assert_eq!(Frame::decode(&bytes).unwrap(), f);
    assert_eq!(Frame::read_from(&bytes[..]).unwrap(), f);
    let mut bad = bytes.clone();
    bad[4] = 3;
    assert_eq!(Frame::decode(&bad), Err(Error::Unknown("frame kind")));
    bad = bytes.clone();
    bad[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(Frame::decode(&bad), Err(Error::Limit("frame bytes")));
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        Frame::decode(&trailing),
        Err(Error::Invalid("frame length mismatch"))
    );
    assert!(Frame::decode(&bytes[..5]).is_err());
    assert!(Frame::read_from(&bytes[..10]).is_err());
    assert_eq!(
        FrameHeader {
            channel: 1,
            kind: FrameKind::Credit,
            len: 3
        }
        .encode(),
        Err(Error::Invalid("credit length"))
    );
    let mut c = Credit::default();
    c.consume(limits::CREDIT_BYTES).unwrap();
    assert_eq!(c.consume(1), Err(Error::Limit("credit exhausted")));
    c.replenish(1).unwrap();
    assert!(c.replenish(limits::CREDIT_BYTES).is_err());
}
#[test]
fn publish_assembly_checks_order_total_and_both_digests() {
    let bytes = b"publication";
    let begin = PublishBegin {
        total_bytes: bytes.len(),
        chunk_count: 2,
        digest: bytes_digest(bytes),
    };
    let mut a = PublishAssembler::new(begin.clone()).unwrap();
    assert_eq!(a.push(1, b"x"), Err(Error::Invalid("chunk order")));
    a.push(0, &bytes[..4]).unwrap();
    a.push(1, &bytes[4..]).unwrap();
    assert_eq!(
        a.finish(PublishCommit {
            digest: begin.digest.clone()
        })
        .unwrap(),
        bytes
    );
    let a = PublishAssembler::new(begin.clone()).unwrap();
    assert_eq!(
        a.finish(PublishCommit {
            digest: begin.digest.clone()
        }),
        Err(Error::Invalid("incomplete publish"))
    );
    let mut a = PublishAssembler::new(begin.clone()).unwrap();
    a.push(0, &bytes[..4]).unwrap();
    a.push(1, &bytes[4..]).unwrap();
    assert_eq!(
        a.finish(PublishCommit { digest: digest() }),
        Err(Error::DigestMismatch)
    );
    assert!(
        PublishAssembler::new(PublishBegin {
            total_bytes: limits::PUBLISH_BYTES + 1,
            ..begin
        })
        .is_err()
    );
}
#[test]
fn resource_ledgers_reject_every_scope_ceiling_and_keep_state_on_failure() {
    for r in [
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
    ] {
        let mut ledger = ResourceLedger::default();
        ledger.reserve(r, r.limit()).unwrap();
        assert_eq!(
            ledger.reserve(r, 1),
            Err(Error::Limit("resource budget")),
            "{r:?}"
        );
        assert_eq!(ledger.used(r), r.limit());
        ledger.release(r, r.limit()).unwrap();
        assert!(ledger.release(r, 1).is_err());
    }
}
#[test]
fn receipt_floor_is_monotone_and_checks_all_watermarks_and_entity_revisions() {
    let mut floors = ReceiptFloors::default();
    let mut p = publish();
    floors.check(&p, &[]).unwrap();
    floors.raise(&wm(), &[]);
    floors.raise(
        &OpWatermark {
            processed_op_seq: Counter(0),
            state_rev: Counter(0),
            settled_local_seq: Counter(0),
        },
        &[],
    );
    assert_eq!(floors.state, Counter(2));
    p.wm.state_rev = Counter(1);
    assert_eq!(floors.check(&p, &[]), Err(Error::Stale("BehindReceipt")));
    p.wm = wm();
    p.wm.processed_op_seq = Counter(0);
    assert!(floors.check(&p, &[]).is_err());
    p.wm = wm();
    p.wm.settled_local_seq = Counter(0);
    assert!(floors.check(&p, &[]).is_err());
    p.wm = wm();
    let e = input();
    floors.raise(
        &wm(),
        &[EntityResult {
            r#ref: e.r#ref.clone(),
            rev: Counter(2),
            state: e.state.clone(),
        }],
    );
    assert_eq!(
        floors.check(&p, &[]),
        Err(Error::Stale("entity receipt floor"))
    );
    let release = EntityRelease {
        r#ref: e.r#ref,
        base_rev: Counter(1),
        rev: Counter(2),
        reason: "released".into(),
    };
    floors.check(&p, &[release]).unwrap();
}
#[test]
fn hello_negotiates_only_downward_and_bootstrap_survives_unknown_protocol() {
    let mut limits = NegotiatedLimits {
        nodes: 100,
        ..Default::default()
    };
    let negotiated = limits.intersect(&NegotiatedLimits::default()).unwrap();
    assert_eq!(negotiated.nodes, 100);
    limits.nodes = limits::NODES + 1;
    assert_eq!(
        limits.validate(),
        Err(Error::Limit("negotiated limit increase"))
    );
    let json = format!(
        r#"{{"type":"HelloAck","body":{{"nonce":"{}","protocol":99,"serve":{{"version":"9.0","commit":"abc"}},"future":42}}}}"#,
        id(1).as_str()
    );
    let boot = decode_hello_ack_bootstrap(json.as_bytes()).unwrap();
    assert_eq!(boot.protocol, 99);
    assert_eq!(boot.serve.version, "9.0");
    assert!(serde_json::from_str::<HelloAck>(&json).is_err());
}
#[test]
fn edit_digest_prevents_same_id_with_changed_body() {
    let mut e = edit();
    e.validate().unwrap();
    e.text = "changed".into();
    e.selection = Selection {
        start: 0,
        end: 0,
        direction: Direction::Forward,
    };
    assert_eq!(e.validate(), Err(Error::DigestMismatch));
}

#[test]
fn receive_api_rejects_unknown_tables_and_generation_without_adopting_state() {
    let mut message = envelope(publish());
    let tables = ValidationTables::default();
    let context = ReceiveContext {
        session: message.session.clone(),
        scope: message.scope.clone(),
        owner_fence: message.owner_fence,
    };
    let floors = ReceiptFloors::default();
    let mut generations = GenerationTracker::default();
    let bytes = encode_publish(&message, &tables).unwrap();
    decode_publish(
        &bytes,
        &PublishValidation {
            tables: &tables,
            context: &context,
            floors: &floors,
            generations: &generations,
            tombstones: &[],
        },
    )
    .unwrap();
    generations.commit(&message.body).unwrap();
    assert!(
        decode_publish(
            &bytes,
            &PublishValidation {
                tables: &tables,
                context: &context,
                floors: &floors,
                generations: &generations,
                tombstones: &[]
            }
        )
        .is_err()
    );
    message.body.views_full[0].nodes[0].ops.push(Op::Method {
        name: "unknown".into(),
        args: vec![],
    });
    assert_eq!(
        encode_publish(&message, &tables),
        Err(Error::Unknown("op table entry"))
    );
}
#[test]
fn stored_operations_and_callback_count_have_independent_guards() {
    let mut n = node();
    n.ops = vec![
        Op::NullaryStyle {
            index: 0,
            name: String::new()
        };
        limits::STORED_OPS + 1
    ];
    let s = Snapshot {
        copy: "mod".into(),
        generation: Counter(1),
        root: 0,
        nodes: vec![n],
    };
    assert_eq!(
        s.validate(&ValidationTables::default()),
        Err(Error::Limit("stored ops"))
    );
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![
        Op::Callback {
            name: "x".into(),
            id: Counter(1)
        };
        limits::CALLBACKS + 1
    ];
    assert_eq!(p.validate_shape(), Err(Error::Limit("callbacks")));
}
#[test]
fn entity_aggregate_state_and_lists_have_independent_guards() {
    let mut p = publish();
    for i in 0..limits::ENTITIES - 1 {
        let mut e = input();
        e.r#ref.id = Counter(i as u64 + 2);
        e.state = EntityState::Input {
            text: "x".repeat(750),
            selection: Selection {
                start: 0,
                end: 0,
                direction: Direction::Forward,
            },
        };
        p.entities_full.push(e);
    }
    assert_eq!(
        p.validate_shape(),
        Err(Error::Limit("entity aggregate state"))
    );
    let mut p = publish();
    p.list_manifests = vec![list(); limits::LISTS + 1];
    assert_eq!(p.validate_shape(), Err(Error::Limit("lists")));
}
#[test]
fn duplicate_rows_and_old_content_generation_are_rejected() {
    let (mut req, mut reply) = rows();
    let mut m = list();
    m.item_count = 2;
    req.range.end = 2;
    reply.range.end = 2;
    reply.rows.push(reply.rows[0].clone());
    assert_eq!(
        validate_rows(&reply, &req, &m, &ValidationTables::default()),
        Err(Error::Invalid("duplicate row key"))
    );
    reply.content_rev = Counter(0);
    assert_eq!(
        validate_rows(&reply, &req, &m, &ValidationTables::default()),
        Err(Error::Stale("rows request/revision"))
    );
}
#[test]
fn row_byte_size_limit_and_asset_aggregate_pixel_budget() {
    let (req, mut reply) = rows();
    reply.rows[0].nodes[0].text = Some("x".repeat(limits::ROWS_BYTES));
    assert_eq!(
        validate_rows(&reply, &req, &list(), &ValidationTables::default()),
        Err(Error::Limit("rows bytes"))
    );
    let mut p = publish();
    let a = AssetManifest {
        id: "a".into(),
        digest: digest(),
        kind: AssetKind::Png,
        encoded_bytes: 1,
        decoded_bytes: 16_000_000,
        width: 2000,
        height: 2000,
    };
    for i in 0..4 {
        let mut a = a.clone();
        a.id = i.to_string();
        p.asset_manifests.push(a);
    }
    p.validate_shape().unwrap();
    let mut a = a;
    a.id = "extra".into();
    a.width = 1;
    a.height = 1;
    a.decoded_bytes = 4;
    p.asset_manifests.push(a);
    assert_eq!(
        p.validate_shape(),
        Err(Error::Limit("asset pixels aggregate"))
    );
}
#[test]
fn child_view_edges_participate_in_cycle_and_depth_checks() {
    let mut p = publish();
    let mut child = p.views_full[0].clone();
    child.view_ref.id = Counter(2);
    child.generation = Counter(2);
    p.entities_full
        .push(view_entity(&child, Some(p.views_full[0].view_ref.clone())));
    p.views_full[0].nodes[0].component = Component::ChildView;
    p.views_full[0].nodes[0].child_view = Some(Counter(2));
    p.views_full.push(child);
    p.validate_shape().unwrap();
    p.views_full[1].nodes[0].component = Component::ChildView;
    p.views_full[1].nodes[0].child_view = Some(Counter(1));
    assert_eq!(
        p.validate_shape(),
        Err(Error::Invalid("cyclic ChildView roots"))
    );
}
#[test]
fn entity_and_asset_references_cannot_cross_incarnations_or_manifests() {
    let mut p = publish();
    p.entities_full.push(input());
    p.views_full[0].nodes[0].component = Component::Input;
    p.views_full[0].nodes[0].entity = Some(input().r#ref);
    p.validate_shape().unwrap();
    p.views_full[0].nodes[0]
        .entity
        .as_mut()
        .unwrap()
        .incarnation = id(99);
    assert_eq!(p.validate_shape(), Err(Error::Invalid("entity reference")));
    let mut p = publish();
    p.views_full[0].nodes[0].asset = Some(AssetRef {
        id: "missing".into(),
        digest: digest(),
    });
    assert_eq!(p.validate_shape(), Err(Error::Invalid("asset reference")));
}
#[test]
fn strict_boolean_and_ids_preserve_background_ack_contract() {
    roundtrip(BgGrantAck::Bound(BgGrantBound {
        op_id: "grant-1".into(),
        grant_id: id(1),
        grant_rev: Counter(1),
        bound: True,
        expires_at: Counter(100),
    }));
    roundtrip(BgGrantAck::Unbound(BgGrantUnbound {
        op_id: "grant-1".into(),
        bound: False,
        reason: "write failed".into(),
    }));
    assert!(serde_json::from_str::<Id128>("\"123\"").is_err());
    assert!(serde_json::from_str::<Digest>("\"bad\"").is_err());
    assert!(serde_json::from_str::<True>("false").is_err());
    assert!(serde_json::from_str::<False>("true").is_err());
}

// Regression cases from the independent appendix-A review.
fn receive_publish(
    p: &Publish,
    floors: &ReceiptFloors,
    tombstones: &[EntityRelease],
    generations: &GenerationTracker,
    tables: &ValidationTables,
) -> Result<Envelope<Publish>> {
    let message = envelope(p.clone());
    let context = ReceiveContext {
        session: message.session.clone(),
        scope: message.scope.clone(),
        owner_fence: message.owner_fence,
    };
    decode_publish(
        &serde_json::to_vec(&message).unwrap(),
        &PublishValidation {
            tables,
            context: &context,
            floors,
            generations,
            tombstones,
        },
    )
}
fn hello_ack() -> HelloAck {
    HelloAck {
        nonce: id(1),
        protocol: PROTOCOL,
        serve: ServeVersion {
            version: "0.1.0".into(),
            commit: "abc".into(),
        },
        boot: id(2),
        epoch: id(3),
        limits: NegotiatedLimits::default(),
        copies: vec![],
    }
}
#[test]
fn bootstrap_reads_the_encoded_hello_ack_body_in_both_protocol_versions() {
    let message = envelope(hello_ack());
    let bytes = encode(&message).unwrap();
    assert_eq!(decode::<HelloAck>(&bytes).unwrap(), message);
    let boot = decode_hello_ack_bootstrap(&bytes).unwrap();
    assert_eq!(boot.nonce, message.body.nonce);
    assert_eq!(boot.serve, message.body.serve);
    let mut future = strict_json(&bytes).unwrap();
    future["v"] = 99.into();
    future["schema"] = "future".into();
    future["body"]["protocol"] = 99.into();
    future["body"]["future"] = true.into();
    // Incompatibility must not require interpreting the rest of the envelope/body.
    future["session"] = serde_json::Value::Null;
    future["body"]["limits"] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&future).unwrap();
    let boot = decode_hello_ack_bootstrap(&bytes).unwrap();
    assert_eq!(boot.protocol, 99);
    assert_eq!(boot.nonce, message.body.nonce);
    assert_eq!(boot.serve, message.body.serve);
    assert!(decode::<HelloAck>(&bytes).is_err());
    future["type"] = "Hello".into();
    assert!(decode_hello_ack_bootstrap(&serde_json::to_vec(&future).unwrap()).is_err());
}
#[test]
fn release_tombstone_cannot_cover_present_or_resurrected_entity() {
    let e = input();
    let floors = ReceiptFloors {
        entities: vec![(e.r#ref.clone(), Counter(2))],
        ..Default::default()
    };
    let release = EntityRelease {
        r#ref: e.r#ref.clone(),
        base_rev: Counter(2),
        rev: Counter(3),
        reason: "released".into(),
    };
    let mut p = publish();
    let tables = ValidationTables::default();
    let tracker = GenerationTracker::default();
    receive_publish(
        &p,
        &floors,
        std::slice::from_ref(&release),
        &tracker,
        &tables,
    )
    .unwrap();
    p.entities_full.push(e.clone());
    assert!(
        receive_publish(
            &p,
            &floors,
            std::slice::from_ref(&release),
            &tracker,
            &tables
        )
        .is_err()
    );
    p.entities_full.last_mut().unwrap().rev = Counter(4);
    assert!(
        receive_publish(
            &p,
            &floors,
            std::slice::from_ref(&release),
            &tracker,
            &tables
        )
        .is_err()
    );
    // A release is final even when no receipt introduced a floor for this entity.
    assert!(
        receive_publish(
            &p,
            &ReceiptFloors::default(),
            std::slice::from_ref(&release),
            &tracker,
            &tables
        )
        .is_err()
    );
    p.entities_full.last_mut().unwrap().r#ref.incarnation = id(99);
    receive_publish(&p, &floors, &[release], &tracker, &tables).unwrap();
    p.entities_full.last_mut().unwrap().r#ref = e.r#ref;
    p.entities_full.last_mut().unwrap().rev = Counter(1);
    assert!(receive_publish(&p, &floors, &[], &tracker, &tables).is_err());
}
#[test]
fn action_callbacks_require_their_own_negotiated_table() {
    let mut p = publish();
    p.views_full[0].nodes[0].ops = vec![Op::ActionCallback {
        name: "command".into(),
        id: Counter(1),
    }];
    let mut tables = ValidationTables::default();
    let receive = |tables: &ValidationTables| {
        receive_publish(
            &p,
            &ReceiptFloors::default(),
            &[],
            &GenerationTracker::default(),
            tables,
        )
    };
    assert_eq!(receive(&tables), Err(Error::Unknown("op table entry")));
    tables.callbacks.insert("command".into());
    assert_eq!(receive(&tables), Err(Error::Unknown("op table entry")));
    tables.actions.insert("command".into());
    receive(&tables).unwrap();
    tables.actions.clear();
    tables.actions.insert("other".into());
    assert_eq!(receive(&tables), Err(Error::Unknown("op table entry")));
}
#[test]
fn generations_are_copy_wide_unique_and_history_is_constant_space() {
    let mut tracker = GenerationTracker::default();
    let mut p = publish();
    tracker.commit(&p).unwrap();
    p.views_full[0].view_ref.id = Counter(999);
    assert_eq!(
        tracker.check(&p),
        Err(Error::Stale("view generation reused"))
    );
    for generation in 2..=4096 {
        p.views_full[0].generation = Counter(generation);
        p.views_full[0].view_ref.id = Counter(generation);
        tracker.commit(&p).unwrap();
    }
    assert_eq!(tracker.high_watermark(), Counter(4096));
    assert_eq!(
        std::mem::size_of_val(&tracker),
        std::mem::size_of::<Counter>()
    );
    let mut p = publish();
    let mut second = p.views_full[0].clone();
    second.view_ref.id = Counter(99);
    p.entities_full.push(view_entity(&second, None));
    p.views_full.push(second);
    assert_eq!(
        p.validate_shape(),
        Err(Error::Stale("view generation reused"))
    );
    assert_eq!(
        GenerationTracker::default().check(&p),
        Err(Error::Stale("view generation reused"))
    );
    p.views_full[1].generation = Counter(2);
    p.entities_full[1] = view_entity(&p.views_full[1], None);
    p.validate_shape().unwrap();
    // Vector order is not temporal order; each generation must exceed the old high.
    p.views_full.reverse();
    let mut fresh = GenerationTracker::default();
    fresh.commit(&p).unwrap();
    assert_eq!(fresh.high_watermark(), Counter(2));
}
fn rehash(reply: &mut RowsReply) {
    for row in &mut reply.rows {
        row.content_hash = canonical_digest(row, "content_hash").unwrap();
    }
}
#[test]
fn rows_resolve_component_entity_asset_list_and_op_references_in_current_copy() {
    let (request, mut reply) = rows();
    let mut p = publish();
    let tables = ValidationTables {
        methods: ["bind".into()].into(),
        ..Default::default()
    };
    p.list_manifests.push(list());
    p.entities_full.push(input());
    let asset = AssetManifest {
        id: "icon".into(),
        digest: digest(),
        kind: AssetKind::Png,
        encoded_bytes: 1,
        decoded_bytes: 4,
        width: 1,
        height: 1,
    };
    p.asset_manifests.push(asset.clone());
    let context = RowValidation::new(&p, &tables).unwrap();
    let m = &p.list_manifests[0];
    let check = |reply: &mut RowsReply| {
        rehash(reply);
        reply.validate_against(&request, m, &context)
    };
    check(&mut reply).unwrap();
    reply.rows[0].nodes[0].component = Component::Input;
    reply.rows[0].nodes[0].entity = Some(input().r#ref);
    check(&mut reply).unwrap();
    reply.rows[0].nodes[0].entity.as_mut().unwrap().id = Counter(999);
    assert_eq!(check(&mut reply), Err(Error::Invalid("entity reference")));
    reply.rows[0].nodes[0].entity = Some(input().r#ref);
    reply.rows[0].nodes[0].entity.as_mut().unwrap().incarnation = id(99);
    assert_eq!(check(&mut reply), Err(Error::Invalid("entity reference")));
    reply.rows[0].nodes[0].entity = Some(input().r#ref);
    reply.rows[0].nodes[0].component = Component::Calendar;
    assert_eq!(
        check(&mut reply),
        Err(Error::Invalid("component entity kind"))
    );
    reply.rows[0].nodes[0].entity = None;
    assert_eq!(
        check(&mut reply),
        Err(Error::Invalid("missing component entity"))
    );
    reply.rows[0].nodes[0].component = Component::Image;
    assert_eq!(check(&mut reply), Err(Error::Invalid("missing asset")));
    reply.rows[0].nodes[0].asset = Some(AssetRef {
        id: asset.id.clone(),
        digest: asset.digest.clone(),
    });
    check(&mut reply).unwrap();
    reply.rows[0].nodes[0].asset.as_mut().unwrap().digest = bytes_digest(b"other");
    assert_eq!(check(&mut reply), Err(Error::Invalid("asset reference")));
    reply.rows[0].nodes[0] = node();
    reply.rows[0].nodes[0].component = Component::VirtualList;
    assert_eq!(
        check(&mut reply),
        Err(Error::Invalid("missing list manifest"))
    );
    reply.rows[0].nodes[0].list_id = Some("list".into());
    check(&mut reply).unwrap();
    reply.rows[0].nodes[0].list_id = Some("other-copy-list".into());
    assert_eq!(check(&mut reply), Err(Error::Invalid("list reference")));
    reply.rows[0].nodes[0] = node();
    reply.rows[0].nodes[0].component = Component::TextView;
    assert_eq!(check(&mut reply), Err(Error::Invalid("TextView payload")));
    reply.rows[0].nodes[0] = node();
    reply.rows[0].nodes[0].ops = vec![Op::Method {
        name: "bind".into(),
        args: vec![Argument::Entity(input().r#ref)],
    }];
    check(&mut reply).unwrap();
    if let Op::Method { args, .. } = &mut reply.rows[0].nodes[0].ops[0] {
        args[0] = Argument::Entity(entity_ref(999, EntityKind::Input));
    }
    assert_eq!(
        check(&mut reply),
        Err(Error::Invalid("op entity reference"))
    );
    let mut other = p.clone();
    other.list_manifests[0].content_rev = Counter(2);
    let other_context = RowValidation::new(&other, &tables).unwrap();
    assert_eq!(
        reply.validate_against(&request, m, &other_context),
        Err(Error::Stale("rows publication context"))
    );
}
#[test]
fn retained_views_validate_unrendered_identity_parent_root_and_generation() {
    let mut p = publish();
    p.validate_shape().unwrap();
    // A retained view is checked even though no node refers to it.
    let mut orphan = view_entity(&p.views_full[0], None);
    orphan.r#ref.id = Counter(999);
    p.entities_full.push(orphan);
    assert_eq!(
        p.validate_shape(),
        Err(Error::Invalid("retained view reference"))
    );
    p.entities_full.pop();
    for root in [1, usize::MAX] {
        let mut bad = p.clone();
        if let EntityState::View { root: r, .. } = &mut bad.entities_full[0].state {
            *r = root;
        }
        assert_eq!(
            bad.validate_shape(),
            Err(Error::Invalid("retained view root/generation"))
        );
    }
    let mut bad = p.clone();
    if let EntityState::View { generation, .. } = &mut bad.entities_full[0].state {
        *generation = Counter(2);
    }
    assert_eq!(
        bad.validate_shape(),
        Err(Error::Invalid("retained view root/generation"))
    );
    for parent in [
        entity_ref(999, EntityKind::View),
        entity_ref(1, EntityKind::Input),
        EntityRef {
            incarnation: id(99),
            ..entity_ref(1, EntityKind::View)
        },
    ] {
        let mut bad = p.clone();
        if let EntityState::View { parent: value, .. } = &mut bad.entities_full[0].state {
            *value = Some(parent);
        }
        assert_eq!(
            bad.validate_shape(),
            Err(Error::Invalid("retained view parent"))
        );
    }
    let mut bad = p.clone();
    bad.entities_full[0].r#ref.incarnation = id(99);
    assert_eq!(
        bad.validate_shape(),
        Err(Error::Invalid("retained view reference"))
    );
    let mut child = p.views_full[0].clone();
    child.view_ref.id = Counter(3);
    child.generation = Counter(2);
    p.entities_full
        .push(view_entity(&child, Some(p.views_full[0].view_ref.clone())));
    p.views_full.push(child);
    assert_eq!(
        p.validate_shape(),
        Err(Error::Invalid("retained view parent edge"))
    );
    p.views_full[0].nodes[0].component = Component::ChildView;
    p.views_full[0].nodes[0].child_view = Some(Counter(3));
    p.validate_shape().unwrap();
}
fn calendar() -> EntityCreate {
    let mut e = input();
    e.r#ref.kind = EntityKind::Calendar;
    e.config = EntityConfig::Calendar { months: 1 };
    e.state = EntityState::Calendar {
        selected: None,
        year: 2024,
        month: 2,
        mode: CalendarMode::Day,
        today: Date {
            year: 2024,
            month: 2,
            day: 29,
        },
    };
    e
}
#[test]
fn date_picker_and_calendar_both_reference_calendar_entities() {
    let mut p = publish();
    p.entities_full.push(calendar());
    p.views_full[0].nodes[0].entity = Some(calendar().r#ref);
    for component in [Component::Calendar, Component::DatePicker] {
        p.views_full[0].nodes[0].component = component;
        receive_publish(
            &p,
            &ReceiptFloors::default(),
            &[],
            &GenerationTracker::default(),
            &ValidationTables::default(),
        )
        .unwrap();
    }
    p.entities_full[1] = input();
    p.entities_full[1].r#ref.kind = EntityKind::Focus;
    p.entities_full[1].config = EntityConfig::Focus {};
    p.entities_full[1].state = EntityState::Focus {
        id: "focus".into(),
        focused: false,
    };
    p.views_full[0].nodes[0].entity = Some(p.entities_full[1].r#ref.clone());
    assert_eq!(
        p.validate_shape(),
        Err(Error::Invalid("component entity kind"))
    );
}
#[test]
fn every_entity_state_string_has_the_entity_ceiling() {
    for which in 0..4 {
        let make = |len| {
            let text = "x".repeat(len);
            match which {
                0 => EntityState::Focus {
                    id: text,
                    focused: false,
                },
                1 => EntityState::VirtualScroll {
                    offset: Point { x: 0.0, y: 0.0 },
                    anchor_key: text,
                    anchor_index: 0,
                    content_rev: Counter(1),
                    request: None,
                },
                2 => EntityState::VirtualScroll {
                    offset: Point { x: 0.0, y: 0.0 },
                    anchor_key: "key".into(),
                    anchor_index: 0,
                    content_rev: Counter(1),
                    request: Some(ScrollRequest { id: text, index: 0 }),
                },
                _ => EntityState::Scroll {
                    offset: Point { x: 0.0, y: 0.0 },
                    viewport: Size {
                        width: 1.0,
                        height: 1.0,
                    },
                    content: Size {
                        width: 1.0,
                        height: 1.0,
                    },
                    request: Some(ScrollRequest { id: text, index: 0 }),
                },
            }
        };
        make(limits::ENTITY_STRING_BYTES).validate().unwrap();
        assert_eq!(
            make(limits::ENTITY_STRING_BYTES + 1).validate(),
            Err(Error::Limit("entity string"))
        );
    }
}
fn wire_rejects<T: Payload + std::fmt::Debug>(body: T) {
    let message = envelope(body);
    assert!(encode(&message).is_err(), "encode accepted {message:?}");
    let bytes = serde_json::to_vec(&message).unwrap();
    assert!(decode::<T>(&bytes).is_err(), "decode accepted {message:?}");
}
#[test]
fn receipts_and_updates_validate_state_kind_and_numbers_before_json() {
    let state = EntityState::Scroll {
        offset: Point { x: 0.0, y: 0.0 },
        viewport: Size {
            width: -1.0,
            height: 1.0,
        },
        content: Size {
            width: 1.0,
            height: 1.0,
        },
        request: None,
    };
    let result = EntityResult {
        r#ref: entity_ref(2, EntityKind::Scroll),
        rev: Counter(2),
        state,
    };
    wire_rejects(HostStateCommit {
        wm: wm(),
        affected_entities: vec![result.clone()],
    });
    wire_rejects(ActionAck {
        event_id: "event".into(),
        op_seq: Counter(2),
        wm: wm(),
        affected_entities: vec![result],
        result: ActionResult::Ok,
        digest: digest(),
    });
    let result = EntityResult {
        r#ref: entity_ref(2, EntityKind::Calendar),
        rev: Counter(2),
        state: input().state,
    };
    wire_rejects(HostStateCommit {
        wm: wm(),
        affected_entities: vec![result],
    });
    wire_rejects(EditConflict {
        edit_id: "edit".into(),
        op_seq: Counter(2),
        wm: wm(),
        entity: entity_ref(2, EntityKind::Calendar),
        current_rev: Counter(2),
        state: input().state,
        digest: digest(),
    });
    let invalid_input = EntityState::Input {
        text: "🦀".into(),
        selection: Selection {
            start: 1,
            end: 1,
            direction: Direction::Forward,
        },
    };
    wire_rejects(EditAck {
        edit_id: "edit".into(),
        op_seq: Counter(2),
        wm: wm(),
        applied_rev: Counter(2),
        result_rev: Counter(2),
        state: invalid_input,
        digest: digest(),
    });
    let update = EntityUpdate {
        r#ref: entity_ref(2, EntityKind::Slider),
        base_rev: Counter(1),
        rev: Counter(2),
        origin: Origin::HostSet,
        config: EntityConfig::Slider {
            min: 0.0,
            max: 1.0,
            step: 0.1,
            scale: 1.0,
            axis: Axis::Horizontal,
        },
        state: EntityState::Slider {
            value: 0.5,
            end: Some(f64::NAN),
        },
        subscriptions: input().subscriptions,
    };
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut bad = update.clone();
        bad.state = EntityState::Slider {
            value: 0.5,
            end: Some(number),
        };
        assert_eq!(
            encode(&envelope(bad)),
            Err(Error::Invalid("nonfinite number"))
        );
        let result = EntityResult {
            r#ref: update.r#ref.clone(),
            rev: Counter(2),
            state: EntityState::Slider {
                value: 0.5,
                end: Some(number),
            },
        };
        assert_eq!(
            encode(&envelope(HostStateCommit {
                wm: wm(),
                affected_entities: vec![result]
            })),
            Err(Error::Invalid("nonfinite number"))
        );
    }
    let mut good = update;
    good.state = EntityState::Slider {
        value: 0.5,
        end: Some(1.0),
    };
    roundtrip(good.clone());
    if let EntityConfig::Slider { min, .. } = &mut good.config {
        *min = 2.0;
    }
    wire_rejects(good);
}
