//! Regression tests for the appendix A conformance audit of the merged crate
//! (SH1-217, audit items X1-X20). Each test fails without its fix.
use yebisu_mod_wire::*;

fn id(n: u8) -> Id128 {
    Id128::new(format!("{n:032x}")).unwrap()
}
fn session() -> Session {
    Session {
        host: "SHA256:verified-by-SSH".into(),
        boot: id(1),
        client: "ipad".into(),
        epoch: id(2),
    }
}
fn copy_scope() -> Scope {
    Scope::Copy {
        id: "mod".into(),
        incarnation: id(3),
    }
}
fn envelope<T: Payload>(body: T) -> Envelope<T> {
    Envelope::new(
        session(),
        copy_scope(),
        Counter(1),
        "message-1".into(),
        Counter(1),
        body,
    )
}
fn gateway_reply_json(scope: serde_json::Value, result: serde_json::Value) -> Vec<u8> {
    let failed = GatewayReply {
        request_id: "gateway".into(),
        result: GatewayResult::Failed {
            reason: GatewayFailure::Cancelled,
        },
    };
    let mut value = strict_json(&encode(&envelope(failed)).unwrap()).unwrap();
    value["scope"] = scope;
    value["body"]["result"] = result;
    serde_json::to_vec(&value).unwrap()
}

// X1: the envelope's session scope is an empty struct variant, so copy fields
// (or any other key) next to kind:"session" are rejected instead of ignored.
#[test]
fn x1_session_scope_rejects_unknown_fields() {
    let ok = serde_json::json!({"kind": "ok"});
    let plain = gateway_reply_json(serde_json::json!({"kind": "session"}), ok.clone());
    let message = decode::<GatewayReply>(&plain).unwrap();
    assert_eq!(
        serde_json::to_value(&message.scope).unwrap(),
        serde_json::json!({"kind": "session"})
    );
    for extra in [
        serde_json::json!({"kind": "session", "id": "c", "incarnation": id(3).as_str()}),
        serde_json::json!({"kind": "session", "extra": 1}),
    ] {
        let bytes = gateway_reply_json(extra.clone(), ok.clone());
        assert!(
            matches!(decode::<GatewayReply>(&bytes), Err(Error::Json(_))),
            "{extra}"
        );
    }
}

// X2: a gateway success carrying a failure reason is not silently a success.
#[test]
fn x2_gateway_ok_rejects_unknown_fields() {
    let session_scope = serde_json::json!({"kind": "session"});
    let ok = gateway_reply_json(session_scope.clone(), serde_json::json!({"kind": "ok"}));
    let message = decode::<GatewayReply>(&ok).unwrap();
    assert_eq!(
        serde_json::to_value(&message.body.result).unwrap(),
        serde_json::json!({"kind": "ok"})
    );
    let bytes = gateway_reply_json(
        session_scope,
        serde_json::json!({"kind": "ok", "reason": "cancelled"}),
    );
    assert!(matches!(
        decode::<GatewayReply>(&bytes),
        Err(Error::Json(_))
    ));
}

// X3 (nit): descriptive authority/reconcile records also reject unknown fields.
#[test]
fn x3_descriptive_unit_states_reject_unknown_fields() {
    for (good, bad) in [
        (r#"{"kind":"None"}"#, r#"{"kind":"None","extra":1}"#),
        (r#"{"state":"pending"}"#, r#"{"state":"pending","extra":1}"#),
        (
            r#"{"state":"mismatched"}"#,
            r#"{"state":"mismatched","extra":1}"#,
        ),
    ] {
        if good.contains("kind") {
            assert!(serde_json::from_str::<ExecAuthority>(good).is_ok());
            assert!(serde_json::from_str::<ExecAuthority>(bad).is_err());
        } else {
            assert!(serde_json::from_str::<Reconcile>(good).is_ok());
            assert!(serde_json::from_str::<Reconcile>(bad).is_err());
        }
    }
}

fn split_frames(channel: u32, bytes: &[u8]) -> Vec<Frame> {
    bytes
        .chunks(DATA_CHUNK_BYTES)
        .enumerate()
        .map(|(i, part)| {
            DataChunk {
                index: i as u32,
                bytes: part.to_vec(),
            }
            .to_frame(channel)
            .unwrap()
        })
        .collect()
}

// X4: Publish chunks are raw data-channel frames, so a full 16 KiB fragment of
// any content fits, and a 1 MiB publication of 0xFF assembles.
#[test]
fn x4_publish_chunks_are_raw_frames_that_fit_any_content() {
    let worst = vec![0xFF; DATA_CHUNK_BYTES];
    let frame = DataChunk {
        index: 7,
        bytes: worst.clone(),
    }
    .to_frame(1)
    .unwrap();
    let wire = frame.encode().unwrap();
    assert_eq!(wire.len(), FRAME_HEADER_BYTES + limits::CHUNK_BYTES);
    let back = DataChunk::from_frame(&Frame::decode(&wire).unwrap()).unwrap();
    assert_eq!((back.index, back.bytes), (7, worst));

    let publication = vec![0xFF; limits::PUBLISH_BYTES];
    let frames = split_frames(3, &publication);
    let begin = PublishBegin {
        total_bytes: publication.len(),
        chunk_count: frames.len(),
        digest: bytes_digest(&publication),
        channel: 3,
    };
    // The Begin/Commit control messages stay far below the channel-0 ceiling.
    assert!(encode(&envelope(begin.clone())).unwrap().len() < 1024);
    let mut a = PublishAssembler::new(begin.clone()).unwrap();
    for f in &frames {
        a.push_frame(f).unwrap();
    }
    let commit = PublishCommit {
        digest: begin.digest.clone(),
    };
    assert_eq!(a.finish(commit).unwrap(), publication);
}

#[test]
fn x4_data_chunks_reject_control_channel_wrong_channel_and_bad_counts() {
    let chunk = DataChunk {
        index: 0,
        bytes: vec![1],
    };
    assert_eq!(
        chunk.to_frame(0),
        Err(Error::Invalid("data chunk on control channel"))
    );
    let control = Frame {
        channel: 0,
        kind: FrameKind::Data,
        payload: vec![0, 0, 0, 0, 1],
    };
    assert!(DataChunk::from_frame(&control).is_err());
    let empty = Frame {
        channel: 1,
        kind: FrameKind::Data,
        payload: vec![0, 0, 0, 0],
    };
    assert_eq!(
        DataChunk::from_frame(&empty),
        Err(Error::Limit("chunk bytes"))
    );
    let close = Frame {
        channel: 1,
        kind: FrameKind::Close,
        payload: vec![0, 0, 0, 0, 1],
    };
    assert!(DataChunk::from_frame(&close).is_err());
    assert_eq!(
        DataChunk {
            index: 0,
            bytes: vec![0; DATA_CHUNK_BYTES + 1],
        }
        .to_frame(1),
        Err(Error::Limit("chunk bytes"))
    );

    let begin = PublishBegin {
        total_bytes: 1,
        chunk_count: 1,
        digest: bytes_digest(&[1]),
        channel: 2,
    };
    let mut a = PublishAssembler::new(begin.clone()).unwrap();
    assert_eq!(
        a.push_frame(&chunk.to_frame(5).unwrap()),
        Err(Error::Invalid("chunk channel"))
    );
    assert!(
        PublishAssembler::new(PublishBegin {
            channel: 0,
            ..begin.clone()
        })
        .is_err()
    );
    // Fewer chunks than DATA_CHUNK_BYTES allows could never complete.
    assert_eq!(
        PublishAssembler::new(PublishBegin {
            total_bytes: DATA_CHUNK_BYTES + 1,
            chunk_count: 1,
            ..begin
        })
        .err(),
        Some(Error::Invalid("publish chunk count"))
    );
}

fn png_manifest(bytes: &[u8]) -> AssetManifest {
    AssetManifest {
        id: "a".into(),
        digest: bytes_digest(bytes),
        kind: AssetKind::Png,
        encoded_bytes: bytes.len(),
        decoded_bytes: 4 * 1024 * 1024,
        width: 1024,
        height: 1024,
    }
}

// X5: AssetReply is a control-sized header; a 4 MiB encoded image of the worst
// content arrives as raw chunks and is released only when the digest matches.
#[test]
fn x5_four_mib_asset_arrives_over_data_chunks() {
    let image = vec![0xFF; limits::IMAGE_BYTES];
    let manifest = png_manifest(&image);
    manifest.validate().unwrap();
    let request = AssetRequest {
        request_id: "req".into(),
        asset: AssetRef {
            id: "a".into(),
            digest: manifest.digest.clone(),
        },
    };
    let frames = split_frames(4, &image);
    let reply = AssetReply {
        request_id: "req".into(),
        asset: request.asset.clone(),
        channel: 4,
        total_bytes: image.len(),
        chunk_count: frames.len(),
    };
    let header = encode(&envelope(reply.clone())).unwrap();
    assert!(header.len() < 1024);
    assert_eq!(decode::<AssetReply>(&header).unwrap().body, reply);
    let mut a = AssetAssembler::new(&reply, &request, &manifest).unwrap();
    for f in &frames {
        a.push_frame(f).unwrap();
    }
    assert_eq!(a.finish().unwrap(), image);

    let mut a = AssetAssembler::new(&reply, &request, &manifest).unwrap();
    for f in &frames[..frames.len() - 1] {
        a.push_frame(f).unwrap();
    }
    assert_eq!(a.finish(), Err(Error::Invalid("incomplete asset")));

    let mut short = reply.clone();
    short.total_bytes -= 1;
    assert_eq!(
        AssetAssembler::new(&short, &request, &manifest).err(),
        Some(Error::Invalid("asset reply bytes"))
    );
    let mut control = reply.clone();
    control.channel = 0;
    assert!(AssetAssembler::new(&control, &request, &manifest).is_err());
}

fn view_publish(component: Component, entity: Option<EntityRef>, kind: ListKind) -> Publish {
    let view_ref = EntityRef {
        id: Counter(1),
        incarnation: id(1),
        kind: EntityKind::View,
    };
    let mut p = Publish {
        pub_rev: Counter(1),
        wm: OpWatermark {
            processed_op_seq: Counter(1),
            state_rev: Counter(2),
            settled_local_seq: Counter(1),
        },
        entities_full: vec![],
        views_full: vec![ViewSnapshot {
            view_ref: view_ref.clone(),
            generation: Counter(1),
            root: 0,
            nodes: vec![Node {
                component,
                text: None,
                entity,
                asset: None,
                text_format: None,
                child_view: None,
                list_id: Some("list".into()),
                children: vec![],
                ops: vec![],
            }],
            state_rev: Counter(2),
        }],
        focus_state: vec![],
        overlays_full: vec![],
        list_manifests: vec![ListManifest {
            list_id: "list".into(),
            content_rev: Counter(1),
            kind,
            item_count: 2,
            sizes_or_estimate: match kind {
                ListKind::Virtual => ListSizes::Sizes(vec![20.0, 24.0]),
                _ => ListSizes::Estimate(20.0),
            },
            handler_generation: Counter(1),
        }],
        asset_manifests: vec![],
    };
    let subscriptions = Subscriptions {
        rev: Counter(1),
        events: vec![],
    };
    p.entities_full.push(EntityCreate {
        r#ref: view_ref,
        rev: Counter(1),
        config: EntityConfig::View {},
        state: EntityState::View {
            parent: None,
            root: 0,
            generation: Counter(1),
            props_rev: Counter(1),
        },
        subscriptions: subscriptions.clone(),
    });
    p.entities_full.push(EntityCreate {
        r#ref: scroll_ref(EntityKind::VirtualScroll),
        rev: Counter(1),
        config: EntityConfig::VirtualScroll {},
        state: EntityState::VirtualScroll {
            offset: Point { x: 0.0, y: 0.0 },
            anchor_key: "row-0".into(),
            anchor_index: 0,
            content_rev: Counter(1),
            request: None,
        },
        subscriptions: subscriptions.clone(),
    });
    p.entities_full.push(EntityCreate {
        r#ref: scroll_ref(EntityKind::Scroll),
        rev: Counter(1),
        config: EntityConfig::Scroll {},
        state: EntityState::Scroll {
            offset: Point { x: 0.0, y: 0.0 },
            viewport: Size {
                width: 10.0,
                height: 10.0,
            },
            content: Size {
                width: 10.0,
                height: 10.0,
            },
            request: None,
        },
        subscriptions,
    });
    p
}
fn scroll_ref(kind: EntityKind) -> EntityRef {
    EntityRef {
        id: Counter(if kind == EntityKind::VirtualScroll {
            2
        } else {
            3
        }),
        incarnation: id(1),
        kind,
    }
}

// X6: VirtualList/List report their position through a VirtualScroll entity.
#[test]
fn x6_lists_require_a_virtual_scroll_entity() {
    let tables = ValidationTables::default();
    for (component, kind) in [
        (Component::VirtualList, ListKind::Virtual),
        (Component::List, ListKind::Measured),
    ] {
        let good = view_publish(component, Some(scroll_ref(EntityKind::VirtualScroll)), kind);
        good.validate(&tables).unwrap();
        assert_eq!(
            view_publish(component, None, kind).validate(&tables),
            Err(Error::Invalid("missing component entity"))
        );
        assert_eq!(
            view_publish(component, Some(scroll_ref(EntityKind::Scroll)), kind).validate(&tables),
            Err(Error::Invalid("component entity kind"))
        );
    }
}

// X7: VirtualList needs the per-row sizes of a Virtual manifest; List needs a
// Uniform or Measured estimate.
#[test]
fn x7_list_component_matches_manifest_kind() {
    let tables = ValidationTables::default();
    let entity = || Some(scroll_ref(EntityKind::VirtualScroll));
    for (component, kind, ok) in [
        (Component::VirtualList, ListKind::Virtual, true),
        (Component::VirtualList, ListKind::Uniform, false),
        (Component::VirtualList, ListKind::Measured, false),
        (Component::List, ListKind::Uniform, true),
        (Component::List, ListKind::Measured, true),
        (Component::List, ListKind::Virtual, false),
    ] {
        let result = view_publish(component, entity(), kind).validate(&tables);
        if ok {
            assert_eq!(result, Ok(()), "{component:?} {kind:?}");
        } else {
            assert_eq!(
                result,
                Err(Error::Invalid("list component kind")),
                "{component:?} {kind:?}"
            );
        }
    }
}

fn grant() -> BgGrant {
    let mut g = BgGrant {
        format: GrantFormat,
        grant_id: id(7),
        grant_rev: Counter(1),
        host_key: "SHA256:host".into(),
        bundle_digest: bytes_digest(b"bundle"),
        policy: GrantPolicy {
            rev: Counter(1),
            digest: bytes_digest(b"policy"),
            full_authority: false,
            on_targets: vec![],
        },
        ttl_secs: Counter(86_400),
        last_valid_contact_at: Counter(1_700_000_000),
        expires_at: Counter(1_700_086_400),
        os_boot_id: "boot".into(),
        cont_at_contact: Counter(1),
        cont_deadline: Counter(86_400_000_000_001),
        wall_high_water: Counter(1_700_000_000),
        client: "ipad".into(),
        state: GrantState::Active,
        state_reason: String::new(),
        updated_at: Counter(1_700_000_000),
        record_digest: bytes_digest(b""),
        r#mod: ModIdentity {
            location: ModLocation::Yebisu,
            name: "clock".into(),
        },
    };
    g.seal().unwrap();
    g
}

// X8: a grant record whose format is not 1 is unreadable (quarantined).
#[test]
fn x8_grant_format_is_fixed_to_one() {
    let g = grant();
    let mut value = serde_json::to_value(&g).unwrap();
    assert_eq!(value["format"], 1);
    assert_eq!(
        BgGrant::decode(&serde_json::to_vec(&value).unwrap()).unwrap(),
        g
    );
    for format in [0, 2, 7] {
        value["format"] = format.into();
        assert!(serde_json::from_value::<BgGrant>(value.clone()).is_err());
        assert!(matches!(
            BgGrant::decode(&serde_json::to_vec(&value).unwrap()),
            Err(Error::Json(_))
        ));
    }
}

// X9 (nit): record_digest is computed and checked by the crate.
#[test]
fn x9_grant_record_digest_detects_a_changed_field() {
    let g = grant();
    g.validate().unwrap();
    let mut changed = g.clone();
    changed.ttl_secs = Counter(172_800);
    assert_eq!(changed.validate(), Err(Error::DigestMismatch));
    assert_eq!(
        BgGrant::decode(&serde_json::to_vec(&changed).unwrap()),
        Err(Error::DigestMismatch)
    );
    changed.seal().unwrap();
    changed.validate().unwrap();
}

// X10: receivers accept only the canonical URL string, so the confirmed host is
// the host of exactly the string that is opened.
#[test]
fn x10_device_request_url_must_be_canonical() {
    for raw in [
        "https:example.com",
        "https:\\\\example.com",
        " https://example.com/",
        "https://exa\nmple.com/",
        "https://EXAMPLE.com/",
        "https://example.com",
        "https://example.com:443/",
        "https://example.com/a/../b",
    ] {
        assert_eq!(
            validate_url(raw),
            Err(Error::Invalid("URL not canonical")),
            "{raw:?}"
        );
        let canonical = normalize_url(raw).unwrap();
        assert_eq!(validate_url(&canonical).unwrap(), "example.com", "{raw:?}");
    }
    assert_eq!(
        normalize_url("https:example.com").unwrap(),
        "https://example.com/"
    );
    let request = |value: &str| DeviceRequest {
        request_id: "r".into(),
        permission: id(5),
        event_id: "e".into(),
        operation: DeviceOperation::OpenUrl,
        policy_rev: Counter(1),
        value: value.into(),
    };
    assert!(encode(&envelope(request("https://example.com/"))).is_ok());
    assert!(encode(&envelope(request("https:example.com"))).is_err());
    assert!(normalize_url("http://example.com/").is_err());
}

// X14-X19 (nit): runtime values from A13/A14 live beside the A8 ceilings.
#[test]
fn x14_x19_runtime_values_match_the_design() {
    assert_eq!(limits::TOASTS_PER_SECOND, 1);
    assert_eq!(limits::TOASTS_VISIBLE, 3);
    assert_eq!(limits::CONTACT_WRITE_MS, 60_000);
    assert_eq!(limits::WALL_HIGH_WATER_MS, 60_000);
    assert_eq!(limits::CLOCK_SYNC_RECHECK_MS, 60_000);
    assert_eq!(limits::WALL_ROLLBACK_SECS, 300);
}

// X20 (nit): each stops.jsonl line carries a record_digest the crate verifies.
#[test]
fn x20_stop_log_lines_carry_a_verified_record_digest() {
    let requested = StopRequested {
        op_id: "stop-1".into(),
        grant_id: Some(id(7)),
        source: StopSource::Ipad,
        boot: id(1),
        copy: "mod".into(),
        incarnation: id(3),
        r#mod: ModIdentity {
            location: ModLocation::Yebisu,
            name: "clock".into(),
        },
        at: Counter(1),
    };
    let line = StopLogLine::new(StopRecord::StopRequested(requested)).unwrap();
    let bytes = serde_json::to_vec(&line).unwrap();
    assert!(!bytes.contains(&b'\n'));
    assert_eq!(StopLogLine::decode(&bytes).unwrap(), line);
    let mut value = serde_json::to_value(&line).unwrap();
    value["record"]["value"]["at"] = "2".into();
    assert_eq!(
        StopLogLine::decode(&serde_json::to_vec(&value).unwrap()),
        Err(Error::DigestMismatch)
    );
    let mut extra = serde_json::to_value(&line).unwrap();
    extra["extra"] = 1.into();
    assert!(matches!(
        StopLogLine::decode(&serde_json::to_vec(&extra).unwrap()),
        Err(Error::Json(_))
    ));
}
