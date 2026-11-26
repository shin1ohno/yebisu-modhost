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
    /// The runner's reflection-table digests, checked in the handshake.
    digests: wire::Digests,
}
use yebisu_mod_renderer as renderer;
use yebisu_mod_runner as runner;
use yebisu_mod_wire as wire;

/// The data channel of the offline capture's one transfer (never 0, the control channel).
const CAPTURE_CHANNEL: u32 = 1;

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
        digests: tables().digests()?,
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
    // The same shape as a live transfer: a PublishBegin header naming the data
    // channel, then numbered `[index u32 LE][bytes]` chunks on that channel.
    let chunks: Vec<_> = compressed.chunks(wire::DATA_CHUNK_BYTES).collect();
    let begin = wire::PublishBegin {
        total_bytes: compressed.len(),
        chunk_count: chunks.len(),
        digest: wire::bytes_digest(&compressed),
        channel: CAPTURE_CHANNEL,
    };
    // Reject what a receiver would reject before writing anything it reads.
    wire::PublishAssembler::new(begin.clone()).context("publish header")?;
    std::fs::write(
        prefix.with_extension("begin.json"),
        serde_json::to_vec(&begin)?,
    )?;
    let mut frames = Vec::new();
    for (index, bytes) in chunks.into_iter().enumerate() {
        frames.extend(
            wire::DataChunk {
                index: u32::try_from(index)?,
                bytes: bytes.to_vec(),
            }
            .to_frame(CAPTURE_CHANNEL)?
            .encode()?,
        );
    }
    // The frames file is the last one written for a publication; renaming it
    // into place means a receiver that sees it also sees complete bytes and
    // complete sidecars.
    let partial = prefix.with_extension("frames.partial");
    std::fs::write(&partial, frames)?;
    std::fs::rename(&partial, prefix.with_extension("frames"))?;
    Ok(())
}

pub fn inspect(path: &Path) -> Result<Value> {
    let begin: wire::PublishBegin =
        serde_json::from_slice(&std::fs::read(path.with_extension("begin.json"))?)
            .context("publish header")?;
    anyhow::ensure!(
        begin.channel == CAPTURE_CHANNEL,
        "unexpected capture channel"
    );
    // The assembler checks the total against the Publish ceiling before any frame.
    let mut assembler = wire::PublishAssembler::new(begin.clone()).context("publish header")?;
    let bytes = std::fs::read(path)?;
    let frame_bytes = wire::FRAME_HEADER_BYTES + wire::DATA_CHUNK_HEADER_BYTES;
    anyhow::ensure!(
        bytes.len() <= begin.total_bytes + begin.chunk_count * frame_bytes,
        "capture file too large"
    );
    let mut reader = Cursor::new(bytes);
    while reader.position() < (reader.get_ref().len() as u64) {
        let frame = wire::Frame::read_from(&mut reader)?;
        assembler.push_frame(&frame).context("publish chunk")?;
    }
    // Count, total and SHA-256 must all match before any byte is used.
    let compressed = assembler
        .finish(wire::PublishCommit {
            digest: begin.digest,
        })
        .context("publish assembly")?;
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
    let (context, limits) = handshake(&identity)?;
    // The renderer core adopts the publication under the negotiated limits.
    let mut receiver = renderer::CopyReceiver::new(context, limits.clone(), tables())?;
    let mut host = wire::ResourceLedger::negotiated(&limits)?;
    let mut app = wire::ResourceLedger::negotiated(&limits)?;
    let body = receiver
        .adopt_bytes(
            &json_bytes,
            &mut renderer::SharedScopes {
                host: &mut host,
                app: &mut app,
            },
        )?
        .clone();
    let received = Received {
        scope: receiver.context().scope.clone(),
        body,
    };
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
        json!({"file":path,"scope":received.scope,"generation":received.body.pub_rev,"nodes":received.body.views_full.iter().map(|v|v.nodes.len()).sum::<usize>(),"texts":received.body.views_full.iter().flat_map(|v|v.nodes.iter().filter_map(|n|n.text.as_deref())).collect::<Vec<_>>(),"entities":received.body.entities_full.len(),"retained_entities":received.body.entities_full,"state_rev":received.body.wm.state_rev,"list_manifests":received.body.list_manifests,"list_scrolls":received.body.views_full.iter().flat_map(|v|v.nodes.iter().filter(|n|n.list_id.is_some()).map(|n|json!({"list_id":n.list_id,"entity":n.entity}))).collect::<Vec<_>>(),"assets":received.body.asset_manifests.len(),"compressed_bytes":compressed.len(),"json_bytes":json_bytes.len()}),
    )
}

struct Received {
    scope: wire::Scope,
    body: wire::Publish,
}

/// Hello from this inspector's own tables, answered by the runner identity
/// the capture recorded: the reflection tables must match exactly, and the
/// acknowledged limits become the receiving limits.
fn handshake(identity: &Identity) -> Result<(wire::ReceiveContext, wire::NegotiatedLimits)> {
    let wire::Scope::Copy { id, incarnation } = &identity.scope else {
        anyhow::bail!("capture scope is not a copy");
    };
    let copy = wire::CopyInfo {
        id: id.clone(),
        incarnation: incarnation.clone(),
        r#mod: wire::ModIdentity {
            location: wire::ModLocation::HerdrApp,
            name: id.clone(),
        },
    };
    let client = renderer::ClientHandshake::new(
        identity.session.client.clone(),
        identity.session.epoch.clone(),
        tables().digests()?,
        vec![],
        wire::NegotiatedLimits::default(),
    )?;
    let ack = runner::answer_hello(
        client.hello(),
        &runner::ServeIdentity {
            serve: wire::ServeVersion {
                version: env!("CARGO_PKG_VERSION").into(),
                commit: "offline".into(),
            },
            boot: identity.session.boot.clone(),
            epoch: identity.session.epoch.clone(),
            digests: identity.digests.clone(),
            required: vec![],
            limits: wire::NegotiatedLimits::default(),
            copies: vec![copy.clone()],
        },
    )?;
    let negotiated = client.accept(&ack)?;
    let context = negotiated.context(
        identity.session.host.clone(),
        identity.session.client.clone(),
        &copy,
        identity.owner_fence,
    )?;
    Ok((context, negotiated.limits))
}
