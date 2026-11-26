//! Receiving checks against the values a session negotiated (A8).
//!
//! The wire validators enforce the fixed A8 ceilings. A peer may negotiate
//! smaller values, and the receiver must apply them before accepting work.
//! Each check here measures a message that already passed the wire validation
//! for its type and compares the measurement with the negotiated value. Call
//! the check that matches the receiving scope: frame header before reading the
//! payload, `PublishBegin` before reserving the transfer, decompressed bytes
//! while inflating, then the decoded body before adoption.
use crate::*;
use serde_json::Value;

/// Measured cost of a validated forest of nodes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Cost {
    nodes: usize,
    depth: usize,
    visits: usize,
    expanded_ops: usize,
    stored_ops: usize,
    callbacks: usize,
}

/// Longest single string and the sum of all strings in a JSON value.
fn strings(value: &Value) -> (usize, usize) {
    match value {
        Value::String(s) => (s.len(), s.len()),
        Value::Array(items) => items.iter().fold((0, 0), |(max, sum), v| {
            let (m, s) = strings(v);
            (max.max(m), sum.saturating_add(s))
        }),
        Value::Object(items) => items.values().fold((0, 0), |(max, sum), v| {
            let (m, s) = strings(v);
            (max.max(m), sum.saturating_add(s))
        }),
        _ => (0, 0),
    }
}

fn node_edges(node: &Node, offset: usize) -> Vec<usize> {
    let mut edges: Vec<usize> = node.children.iter().map(|c| c + offset).collect();
    for op in &node.ops {
        if let Op::Slot { node, .. } | Op::StateStyle { node, .. } = op {
            edges.push(node + offset);
        }
    }
    edges
}

/// Visits, weighted ops and depth with repeated edges counted, as A8 counts them.
fn forest_cost(nodes: &[&Node], edges: &[Vec<usize>], roots: &[usize]) -> Result<Cost> {
    fn visit(
        i: usize,
        nodes: &[&Node],
        edges: &[Vec<usize>],
        state: &mut [Option<(usize, usize, usize)>],
        open: &mut [bool],
    ) -> Result<(usize, usize, usize)> {
        if let Some(cost) = state.get(i).copied().flatten() {
            return Ok(cost);
        }
        let node = nodes.get(i).ok_or(Error::Invalid("node reference"))?;
        if open[i] {
            return Err(Error::Invalid("graph cycle"));
        }
        open[i] = true;
        let (mut visits, mut ops, mut height) = (1usize, node.ops.len(), 1usize);
        for &edge in &edges[i] {
            let (v, w, h) = visit(edge, nodes, edges, state, open)?;
            visits = visits.saturating_add(v);
            ops = ops.saturating_add(w);
            height = height.max(h.saturating_add(1));
        }
        open[i] = false;
        state[i] = Some((visits, ops, height));
        Ok((visits, ops, height))
    }
    let mut state = vec![None; nodes.len()];
    let mut open = vec![false; nodes.len()];
    let mut cost = Cost {
        nodes: nodes.len(),
        ..Cost::default()
    };
    for node in nodes {
        cost.stored_ops = cost.stored_ops.saturating_add(node.ops.len());
        cost.callbacks = cost.callbacks.saturating_add(
            node.ops
                .iter()
                .filter(|op| matches!(op, Op::Callback { .. } | Op::ActionCallback { .. }))
                .count(),
        );
    }
    for &root in roots {
        let (v, w, h) = visit(root, nodes, edges, &mut state, &mut open)?;
        cost.visits = cost.visits.saturating_add(v);
        cost.expanded_ops = cost.expanded_ops.saturating_add(w);
        cost.depth = cost.depth.max(h);
    }
    Ok(cost)
}

fn publish_cost(publish: &Publish) -> Result<Cost> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut view_roots = Vec::new();
    let mut offset = 0usize;
    for view in &publish.views_full {
        view_roots.push((view.view_ref.id, offset + view.root));
        offset += view.nodes.len();
    }
    let mut child_roots = Vec::new();
    let mut offset = 0usize;
    for view in &publish.views_full {
        for node in &view.nodes {
            let mut e = node_edges(node, offset);
            if let Some(child) = node.child_view {
                let root = view_roots
                    .iter()
                    .find(|(id, _)| *id == child)
                    .map(|(_, root)| *root)
                    .ok_or(Error::Invalid("ChildView reference"))?;
                e.push(root);
                child_roots.push(root);
            }
            nodes.push(node);
            edges.push(e);
        }
        offset += view.nodes.len();
    }
    let roots: Vec<usize> = view_roots
        .iter()
        .map(|(_, root)| *root)
        .filter(|root| !child_roots.contains(root))
        .collect();
    forest_cost(&nodes, &edges, &roots)
}

impl NegotiatedLimits {
    fn check_cost(&self, cost: Cost) -> Result<()> {
        ceiling(cost.nodes, self.nodes, "negotiated nodes")?;
        ceiling(cost.depth, self.depth, "negotiated depth")?;
        ceiling(cost.visits, self.visits, "negotiated visits")?;
        ceiling(
            cost.expanded_ops,
            self.expanded_ops,
            "negotiated expanded ops",
        )?;
        ceiling(cost.stored_ops, self.stored_ops, "negotiated stored ops")?;
        ceiling(cost.callbacks, self.callbacks, "negotiated callbacks")
    }
    fn check_strings<T: serde::Serialize>(&self, value: &T) -> Result<()> {
        let (max, sum) = strings(&serde_json::to_value(value)?);
        ceiling(max, self.string_bytes, "negotiated string bytes")?;
        ceiling(sum, self.strings_bytes, "negotiated aggregate strings")
    }
    fn check_coordinate(&self, value: f64) -> Result<()> {
        if value.abs() > self.coordinate_abs {
            return Err(Error::Limit("negotiated coordinate"));
        }
        Ok(())
    }
    fn check_ops(&self, nodes: &[Node]) -> Result<()> {
        for op in nodes.iter().flat_map(|n| &n.ops) {
            if let Op::ParamStyle { args, .. } | Op::Method { args, .. } = op {
                for arg in args {
                    if let Argument::Coordinate(v) = arg {
                        self.check_coordinate(*v)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The acknowledged limits must not raise any value this side offered.
    pub fn check_acknowledged(&self, acknowledged: &Self) -> Result<()> {
        if self.intersect(acknowledged)? != *acknowledged {
            return Err(Error::Limit("acknowledged limit increase"));
        }
        Ok(())
    }
    /// A frame header, before its payload is read. Data chunks keep the fixed
    /// chunk size; only the control channel has a negotiated size.
    pub fn check_frame(&self, header: &FrameHeader) -> Result<()> {
        header.validate()?;
        if header.channel == 0 {
            ceiling(
                header.len as usize,
                self.control_bytes,
                "negotiated control bytes",
            )?;
        }
        Ok(())
    }
    /// One encoded control message (a JSON envelope on channel 0).
    pub fn check_control(&self, bytes: usize) -> Result<()> {
        ceiling(bytes, self.control_bytes, "negotiated control bytes")
    }
    /// A publication transfer header, before the transfer is reserved.
    pub fn check_publish_begin(&self, begin: &PublishBegin) -> Result<()> {
        ceiling(
            begin.total_bytes,
            self.publish_bytes,
            "negotiated publish bytes",
        )
    }
    /// Bytes produced so far by the separately negotiated decompressor.
    pub fn check_decompressed(&self, bytes: usize) -> Result<()> {
        ceiling(
            bytes,
            self.decompressed_bytes,
            "negotiated decompressed bytes",
        )
    }
    /// One retained entity (create or update state).
    pub fn check_entity(&self, entity: &EntityCreate) -> Result<()> {
        let (max, _) = strings(&serde_json::to_value(&entity.state)?);
        let (config, _) = strings(&serde_json::to_value(&entity.config)?);
        ceiling(
            max.max(config),
            self.entity_string_bytes,
            "negotiated entity string",
        )?;
        if let EntityState::Scroll { offset, .. } | EntityState::VirtualScroll { offset, .. } =
            &entity.state
        {
            self.check_coordinate(offset.x)?;
            self.check_coordinate(offset.y)?;
        }
        Ok(())
    }
    /// A whole publication body after `Publish::validate_shape` passed.
    pub fn check_publish(&self, publish: &Publish) -> Result<()> {
        self.check_decompressed(serde_json::to_vec(publish)?.len())?;
        ceiling(
            publish.entities_full.len(),
            self.entities,
            "negotiated entities",
        )?;
        let mut state_bytes = 0usize;
        for entity in &publish.entities_full {
            self.check_entity(entity)?;
            state_bytes = state_bytes.saturating_add(serde_json::to_vec(entity)?.len());
        }
        ceiling(
            state_bytes,
            self.entity_state_bytes,
            "negotiated entity state",
        )?;
        ceiling(publish.list_manifests.len(), self.lists, "negotiated lists")?;
        for list in &publish.list_manifests {
            ceiling(list.item_count, self.list_items, "negotiated list items")?;
        }
        let (mut decoded, mut pixels) = (0usize, 0usize);
        for asset in &publish.asset_manifests {
            self.check_asset(asset)?;
            decoded = decoded.saturating_add(asset.decoded_bytes);
            pixels = pixels.saturating_add(asset.width.saturating_mul(asset.height));
        }
        ceiling(
            decoded,
            self.assets_decoded_bytes,
            "negotiated decoded assets",
        )?;
        ceiling(pixels, self.assets_pixels, "negotiated asset pixels")?;
        self.check_strings(publish)?;
        for view in &publish.views_full {
            self.check_ops(&view.nodes)?;
        }
        self.check_cost(publish_cost(publish)?)
    }
    /// A row request, before it is sent or reserved.
    pub fn check_rows_request(&self, request: &RowsRequest) -> Result<()> {
        ceiling(request.range.end, self.list_items, "negotiated list items")?;
        ceiling(
            request.range.end.saturating_sub(request.range.start),
            self.rows,
            "negotiated rows",
        )
    }
    /// A row reply after `RowsReply::validate_shape` passed.
    pub fn check_rows_reply(&self, reply: &RowsReply) -> Result<()> {
        ceiling(reply.rows.len(), self.rows, "negotiated rows")?;
        ceiling(reply.range.end, self.list_items, "negotiated list items")?;
        ceiling(
            serde_json::to_vec(reply)?.len(),
            self.rows_bytes,
            "negotiated rows bytes",
        )?;
        self.check_strings(reply)?;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut roots = Vec::new();
        for row in &reply.rows {
            let offset = nodes.len();
            roots.push(offset + row.root);
            for node in &row.nodes {
                nodes.push(node);
                edges.push(node_edges(node, offset));
            }
            self.check_ops(&row.nodes)?;
        }
        self.check_cost(forest_cost(&nodes, &edges, &roots)?)
    }
    /// One asset manifest, and with it the reply that delivers its bytes.
    pub fn check_asset(&self, asset: &AssetManifest) -> Result<()> {
        ceiling(asset.width, self.image_dimension, "negotiated asset width")?;
        ceiling(
            asset.height,
            self.image_dimension,
            "negotiated asset height",
        )?;
        let encoded = if asset.kind == AssetKind::Svg {
            self.svg_bytes
        } else {
            self.image_bytes
        };
        ceiling(asset.encoded_bytes, encoded, "negotiated encoded asset")?;
        ceiling(
            asset.decoded_bytes,
            self.asset_decoded_bytes,
            "negotiated decoded asset",
        )
    }
    /// An asset reply header, before its chunks are reserved.
    pub fn check_asset_reply(&self, reply: &AssetReply, manifest: &AssetManifest) -> Result<()> {
        self.check_asset(manifest)?;
        let encoded = if manifest.kind == AssetKind::Svg {
            self.svg_bytes
        } else {
            self.image_bytes
        };
        ceiling(reply.total_bytes, encoded, "negotiated encoded asset")
    }
    /// An Edit received by the runner.
    pub fn check_edit(&self, edit: &Edit) -> Result<()> {
        ceiling(
            edit.text.len(),
            self.entity_string_bytes,
            "negotiated entity string",
        )
    }
    /// An Action received by the runner.
    pub fn check_action(&self, action: &Action) -> Result<()> {
        if let Some(point) = &action.data.point {
            self.check_coordinate(point.x)?;
            self.check_coordinate(point.y)?;
        }
        self.check_strings(&action.data)
    }
}
