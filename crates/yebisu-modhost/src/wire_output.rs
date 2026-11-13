//! Offline wire-file writer and independent receiving-side inspection.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::Path,
};

#[derive(Serialize, Deserialize)]
struct Identity {
    session: wire::Session,
    scope: wire::Scope,
    owner_fence: wire::Counter,
}
use yebisu_mod_wire as wire;

pub fn tables() -> wire::ValidationTables {
    // The wire table is pinned to the copied shell, not inferred from a received frame.
    wire::ValidationTables {
        styles: gpui_shell::snapshot::nullary_styles()
            .into_iter()
            .map(|(name, index)| (index, name.into()))
            .collect(),
        param_styles: gpui_shell::snapshot::param_styles()
            .map(str::to_owned)
            .collect(),
        methods: [
            "id",
            "track_focus",
            "content_focus_handle",
            "track_scroll",
            "overflow_y_scrollbar",
            "overflow_x_scrollbar",
            "tooltip",
            "font_family",
            "key_context",
            "tab_index",
            "tab_stop",
            "role",
            "axis",
            "viewport_from_layout",
            "with_sizing_behavior",
            "with_item_to_measure_index",
            "truncate",
            "drag_window",
            "disabled",
            "text_align",
            "scroll_to_item",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        callbacks: [
            "on_click",
            "on_size_change",
            "on_mouse_down_left",
            "on_mouse_down_middle",
            "on_mouse_down_right",
            "on_mouse_up_left",
            "on_mouse_up_middle",
            "on_mouse_up_right",
            "on_key_down",
            "on_key_up",
            "on_scroll_wheel",
            "on_mouse_move",
            "on_hover",
            "on_mouse_down_out",
            "on_change",
            "on_item_click",
            "on_item_secondary_click",
            "on_drop",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        // Offline host::commands returns an empty catalog; no actions are negotiated.
        actions: Default::default(),
        slots: ["content", "trigger"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    }
}

pub fn write(
    output: &Path,
    name: &str,
    revision: u64,
    boot: &str,
    incarnation: &str,
    value: &Value,
) -> Result<()> {
    let publish: wire::Publish =
        serde_json::from_value(value.clone()).context("shell to wire Publish")?;
    publish
        .validate(&tables())
        .context("validate shell publication")?;
    let envelope = wire::Envelope::new(
        wire::Session {
            host: "offline-prototype-no-ssh-identity".into(),
            boot: wire::Id128::new(boot)?,
            client: "offline-capture".into(),
            epoch: wire::Id128::new(boot)?,
        },
        wire::Scope::Copy {
            id: name.into(),
            incarnation: wire::Id128::new(incarnation)?,
        },
        wire::Counter(1),
        format!("{name}:{revision}"),
        wire::Counter(revision),
        publish,
    );
    let bytes = wire::encode_publish(&envelope, &tables())?;
    let compressed = zstd::stream::encode_all(bytes.as_slice(), 3)?;
    let prefix = output.join(format!("{name}-{revision:04}"));
    let identity = Identity {
        session: envelope.session.clone(),
        scope: envelope.scope.clone(),
        owner_fence: envelope.owner_fence,
    };
    std::fs::write(
        prefix.with_extension("identity.json"),
        serde_json::to_vec(&identity)?,
    )?;
    std::fs::write(
        prefix.with_extension("publish.json"),
        serde_json::to_vec_pretty(&envelope.body)?,
    )?;
    std::fs::write(prefix.with_extension("zst"), &compressed)?;
    let mut frames = Vec::new();
    for chunk in compressed.chunks(wire::limits::CHUNK_BYTES) {
        frames.extend(
            wire::Frame {
                channel: 1,
                kind: wire::FrameKind::Data,
                payload: chunk.to_vec(),
            }
            .encode()?,
        );
    }
    std::fs::write(prefix.with_extension("frames"), frames)?;
    Ok(())
}

pub fn inspect(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path)?;
    anyhow::ensure!(
        bytes.len() <= wire::limits::PUBLISH_BYTES + 4096,
        "capture file too large"
    );
    let mut reader = Cursor::new(bytes);
    let mut compressed = Vec::new();
    while reader.position() < (reader.get_ref().len() as u64) {
        let frame = wire::Frame::read_from(&mut reader)?;
        anyhow::ensure!(
            frame.channel == 1 && frame.kind == wire::FrameKind::Data,
            "invalid offline capture frame"
        );
        compressed.extend(frame.payload);
        anyhow::ensure!(
            compressed.len() <= wire::limits::PUBLISH_BYTES,
            "compressed publication too large"
        );
    }
    let mut json_bytes = Vec::new();
    zstd::stream::read::Decoder::new(compressed.as_slice())?
        .take(wire::limits::PUBLISH_DECOMPRESSED_BYTES as u64 + 1)
        .read_to_end(&mut json_bytes)?;
    anyhow::ensure!(
        json_bytes.len() <= wire::limits::PUBLISH_DECOMPRESSED_BYTES,
        "decompressed publication too large"
    );
    let identity: Identity =
        serde_json::from_slice(&std::fs::read(path.with_extension("identity.json"))?)?;
    let context = wire::ReceiveContext {
        session: identity.session,
        scope: identity.scope,
        owner_fence: identity.owner_fence,
    };
    let received = wire::decode_publish(
        &json_bytes,
        &wire::PublishValidation {
            tables: &tables(),
            context: &context,
            floors: &wire::ReceiptFloors::default(),
            generations: &wire::GenerationTracker::default(),
            tombstones: &[],
        },
    )?;
    for asset in &received.body.asset_manifests {
        let path = path
            .parent()
            .context("capture parent")?
            .join(format!("asset-{}.svg", asset.digest.as_str()));
        let bytes = std::fs::read(path)?;
        anyhow::ensure!(
            wire::bytes_digest(&bytes) == asset.digest && bytes.len() == asset.encoded_bytes,
            "asset integrity mismatch"
        );
    }
    Ok(
        json!({"file":path,"scope":received.scope,"generation":received.body.pub_rev,"nodes":received.body.views_full.iter().map(|v|v.nodes.len()).sum::<usize>(),"texts":received.body.views_full.iter().flat_map(|v|v.nodes.iter().filter_map(|n|n.text.as_deref())).collect::<Vec<_>>(),"entities":received.body.entities_full.len(),"retained_entities":received.body.entities_full,"state_rev":received.body.wm.state_rev,"assets":received.body.asset_manifests.len(),"compressed_bytes":compressed.len(),"json_bytes":json_bytes.len()}),
    )
}
