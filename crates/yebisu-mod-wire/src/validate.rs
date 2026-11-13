use crate::*;
use std::collections::{BTreeMap, BTreeSet};

fn ceiling(value: usize, max: usize, name: &'static str) -> Result<()> {
    if value > max {
        Err(Error::Limit(name))
    } else {
        Ok(())
    }
}
fn finite(value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::Invalid("nonfinite number"))
    }
}
fn positive(value: f64) -> Result<()> {
    finite(value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(Error::Invalid("nonpositive size"))
    }
}
fn coordinate(value: f64) -> Result<()> {
    finite(value)?;
    if value.abs() <= limits::COORDINATE_ABS {
        Ok(())
    } else {
        Err(Error::Limit("coordinate"))
    }
}
impl Point {
    pub fn validate(&self) -> Result<()> {
        coordinate(self.x)?;
        coordinate(self.y)
    }
}
impl Size {
    pub fn validate(&self) -> Result<()> {
        positive(self.width)?;
        positive(self.height)
    }
}
impl NumberRange {
    pub fn validate(&self) -> Result<()> {
        finite(self.min)?;
        finite(self.max)?;
        positive(self.step)?;
        if self.min >= self.max {
            return Err(Error::Invalid("number range"));
        }
        Ok(())
    }
}
impl Selection {
    pub fn validate(&self, text: &str) -> Result<()> {
        if self.start > self.end
            || !text.is_char_boundary(self.start)
            || !text.is_char_boundary(self.end)
        {
            return Err(Error::Invalid("UTF-8 selection"));
        }
        Ok(())
    }
    /// UTF-16 offsets are interpreted against the exact local event buffer.
    pub fn from_utf16(text: &str, start: usize, end: usize, direction: Direction) -> Result<Self> {
        let boundary = |offset| {
            let mut utf16 = 0;
            for (byte, ch) in text.char_indices() {
                if utf16 == offset {
                    return Ok(byte);
                }
                utf16 += ch.len_utf16();
            }
            if utf16 == offset {
                Ok(text.len())
            } else {
                Err(Error::Invalid("UTF-16 selection"))
            }
        };
        let selection = Self {
            start: boundary(start)?,
            end: boundary(end)?,
            direction,
        };
        selection.validate(text)?;
        Ok(selection)
    }
}
impl Date {
    pub fn validate(&self) -> Result<()> {
        let leap = self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0);
        let days = match self.month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            _ => 0,
        };
        if !(1..=9999).contains(&self.year) || self.day == 0 || self.day > days {
            return Err(Error::Invalid("date"));
        }
        Ok(())
    }
}
fn entity_strings<T: serde::Serialize>(value: &T) -> Result<()> {
    fn visit(value: &serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::String(s) => {
                ceiling(s.len(), limits::ENTITY_STRING_BYTES, "entity string")
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    visit(v)?;
                }
                Ok(())
            }
            serde_json::Value::Object(o) => {
                for v in o.values() {
                    visit(v)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    visit(&serde_json::to_value(value)?)
}
impl EntityState {
    pub fn kind(&self) -> EntityKind {
        match self {
            Self::Input { .. } => EntityKind::Input,
            Self::Textarea { .. } => EntityKind::Textarea,
            Self::Slider { .. } => EntityKind::Slider,
            Self::Otp { .. } => EntityKind::Otp,
            Self::Calendar { .. } => EntityKind::Calendar,
            Self::Focus { .. } => EntityKind::Focus,
            Self::Scroll { .. } => EntityKind::Scroll,
            Self::VirtualScroll { .. } => EntityKind::VirtualScroll,
            Self::View { .. } => EntityKind::View,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Input { text, selection }
            | Self::Textarea { text, selection }
            | Self::Otp { text, selection } => {
                ceiling(text.len(), limits::ENTITY_STRING_BYTES, "entity text")?;
                selection.validate(text)?;
            }
            Self::Slider { value, end } => {
                finite(*value)?;
                if let Some(end) = end {
                    finite(*end)?;
                    if end < value {
                        return Err(Error::Invalid("slider range"));
                    }
                }
            }
            Self::Calendar {
                selected,
                year,
                month,
                today,
                ..
            } => {
                today.validate()?;
                Date {
                    year: *year,
                    month: *month,
                    day: 1,
                }
                .validate()?;
                if let Some(d) = selected {
                    d.start.validate()?;
                    if let Some(end) = &d.end {
                        end.validate()?;
                        if (end.year, end.month, end.day)
                            < (d.start.year, d.start.month, d.start.day)
                        {
                            return Err(Error::Invalid("date range"));
                        }
                    }
                }
            }
            Self::Scroll {
                offset,
                viewport,
                content,
                ..
            } => {
                offset.validate()?;
                viewport.validate()?;
                content.validate()?;
            }
            Self::VirtualScroll {
                offset,
                anchor_index,
                ..
            } => {
                offset.validate()?;
                ceiling(*anchor_index, limits::LIST_ITEMS, "anchor index")?;
            }
            _ => {}
        }
        entity_strings(self)
    }
}
impl EntityConfig {
    pub fn kind(&self) -> EntityKind {
        match self {
            Self::Input { .. } => EntityKind::Input,
            Self::Textarea { .. } => EntityKind::Textarea,
            Self::Slider { .. } => EntityKind::Slider,
            Self::Otp { .. } => EntityKind::Otp,
            Self::Calendar { .. } => EntityKind::Calendar,
            Self::Focus { .. } => EntityKind::Focus,
            Self::Scroll { .. } => EntityKind::Scroll,
            Self::VirtualScroll { .. } => EntityKind::VirtualScroll,
            Self::View { .. } => EntityKind::View,
        }
    }
}
impl EntityCreate {
    /// In a full publication rev is current; standalone EntityCreate requires rev=1.
    pub fn validate(&self) -> Result<()> {
        if self.rev.0 == 0
            || self.subscriptions.rev.0 == 0
            || self.r#ref.kind != self.config.kind()
            || self.r#ref.kind != self.state.kind()
        {
            return Err(Error::Invalid("entity kind/revision"));
        }
        let mut events = BTreeSet::new();
        for event in &self.subscriptions.events {
            if !events.insert(*event) {
                return Err(Error::Invalid("duplicate subscription"));
            }
        }
        self.state.validate()?;
        match &self.config {
            EntityConfig::Input {
                placeholder,
                number,
                ..
            } => {
                ceiling(
                    placeholder.len(),
                    limits::ENTITY_STRING_BYTES,
                    "placeholder",
                )?;
                if let Some(r) = number {
                    r.validate()?;
                }
            }
            EntityConfig::Textarea {
                placeholder,
                rows,
                min_height,
                max_height,
                ..
            } => {
                ceiling(
                    placeholder.len(),
                    limits::ENTITY_STRING_BYTES,
                    "placeholder",
                )?;
                if *rows == 0 {
                    return Err(Error::Invalid("textarea rows"));
                }
                positive(*min_height)?;
                positive(*max_height)?;
                if min_height > max_height {
                    return Err(Error::Invalid("textarea height"));
                }
            }
            EntityConfig::Slider {
                min,
                max,
                step,
                scale,
                ..
            } => {
                NumberRange {
                    min: *min,
                    max: *max,
                    step: *step,
                }
                .validate()?;
                positive(*scale)?;
                if let EntityState::Slider { value, end } = &self.state
                    && (value < min || value > max || end.is_some_and(|v| v < *min || v > *max))
                {
                    return Err(Error::Invalid("slider value"));
                }
            }
            EntityConfig::Otp { digits, .. } => {
                if *digits == 0 || *digits > limits::OTP_DIGITS {
                    return Err(Error::Limit("OTP digits"));
                }
                if let EntityState::Otp { text, .. } = &self.state {
                    ceiling(text.chars().count(), *digits, "OTP text")?;
                }
            }
            EntityConfig::Calendar { months }
                if (*months == 0 || *months > limits::CALENDAR_MONTHS) =>
            {
                return Err(Error::Limit("calendar months"));
            }
            _ => {}
        }
        entity_strings(self)
    }
}
impl EntityUpdate {
    pub fn validate(&self) -> Result<()> {
        if self.base_rev.0 == 0 || self.base_rev.0.checked_add(1) != Some(self.rev.0) {
            return Err(Error::Stale("entity update revision"));
        }
        EntityCreate {
            r#ref: self.r#ref.clone(),
            rev: self.rev,
            config: self.config.clone(),
            state: self.state.clone(),
            subscriptions: self.subscriptions.clone(),
        }
        .validate()
    }

    pub fn validate_against(&self, old: &EntityCreate) -> Result<()> {
        self.validate()?;
        if self.r#ref != old.r#ref
            || self.base_rev != old.rev
            || self.base_rev.0.checked_add(1) != Some(self.rev.0)
        {
            return Err(Error::Stale("entity update"));
        }
        match (&old.config, &self.config) {
            (EntityConfig::Otp { digits: a, .. }, EntityConfig::Otp { digits: b, .. })
                if a != b =>
            {
                return Err(Error::Invalid("create-only OTP digits"));
            }
            _ => {}
        }
        EntityCreate {
            r#ref: self.r#ref.clone(),
            rev: self.rev,
            config: self.config.clone(),
            state: self.state.clone(),
            subscriptions: self.subscriptions.clone(),
        }
        .validate()
    }
}
impl EntityRelease {
    pub fn validate(&self) -> Result<()> {
        if self.base_rev.0 == 0 || self.base_rev.0.checked_add(1) != Some(self.rev.0) {
            return Err(Error::Stale("entity release revision"));
        }
        entity_strings(self)
    }

    pub fn validate_against(&self, old: &EntityCreate) -> Result<()> {
        self.validate()?;
        if self.r#ref != old.r#ref
            || self.base_rev != old.rev
            || self.base_rev.0.checked_add(1) != Some(self.rev.0)
        {
            return Err(Error::Stale("entity release"));
        }
        Ok(())
    }
}

impl EntityResult {
    pub fn validate(&self) -> Result<()> {
        if self.r#ref.kind != self.state.kind() || self.rev.0 == 0 {
            return Err(Error::Invalid("entity result kind/revision"));
        }
        self.state.validate()
    }
}
fn validate_results(results: &[EntityResult]) -> Result<()> {
    ceiling(results.len(), limits::ENTITIES, "entities")?;
    let mut ids = BTreeSet::new();
    for result in results {
        result.validate()?;
        if !ids.insert(result.r#ref.id) {
            return Err(Error::Invalid("duplicate entity result"));
        }
    }
    Ok(())
}
impl HostStateCommit {
    pub fn validate(&self) -> Result<()> {
        validate_results(&self.affected_entities)
    }
}
impl ActionAck {
    pub fn validate(&self) -> Result<()> {
        validate_results(&self.affected_entities)
    }
}
impl EditAck {
    pub fn validate(&self) -> Result<()> {
        if self.applied_rev.0 == 0 || self.result_rev < self.applied_rev {
            return Err(Error::Invalid("edit receipt revision"));
        }
        if !matches!(
            self.state.kind(),
            EntityKind::Input | EntityKind::Textarea | EntityKind::Otp
        ) {
            return Err(Error::Invalid("edit receipt kind"));
        }
        self.state.validate()
    }
}
impl EditConflict {
    pub fn validate(&self) -> Result<()> {
        if self.entity.kind != self.state.kind()
            || !matches!(
                self.entity.kind,
                EntityKind::Input | EntityKind::Textarea | EntityKind::Otp
            )
            || self.current_rev.0 == 0
        {
            return Err(Error::Invalid("edit conflict kind/revision"));
        }
        self.state.validate()
    }
}
/// Reflection tables from the exact negotiated gpui-kit version. Empty by
/// default: unregistered style, method, slot and callback names are rejected.
#[derive(Debug, Clone, Default)]
pub struct ValidationTables {
    pub styles: BTreeMap<u16, String>,
    pub param_styles: BTreeSet<String>,
    pub methods: BTreeSet<String>,
    pub callbacks: BTreeSet<String>,
    pub actions: BTreeSet<String>,
    pub slots: BTreeSet<String>,
}
impl ValidationTables {
    fn validate_op(&self, op: &Op) -> Result<()> {
        let known = match op {
            Op::NullaryStyle { index, name } => self.styles.get(index) == Some(name),
            Op::ParamStyle { name, .. } => self.param_styles.contains(name),
            Op::Method { name, .. } => self.methods.contains(name),
            Op::Callback { name, .. } => self.callbacks.contains(name),
            Op::Slot { name, .. } => self.slots.contains(name),
            Op::ActionCallback { name, .. } => self.actions.contains(name),
            Op::StateStyle { .. } => true,
        };
        if known {
            Ok(())
        } else {
            Err(Error::Unknown("op table entry"))
        }
    }
}
fn arguments(op: &Op) -> &[Argument] {
    match op {
        Op::ParamStyle { args, .. } | Op::Method { args, .. } => args,
        _ => &[],
    }
}
fn string_bytes<T: serde::Serialize>(value: &T) -> Result<usize> {
    fn count(v: &serde_json::Value) -> Result<usize> {
        match v {
            serde_json::Value::String(s) => {
                ceiling(s.len(), limits::STRING_BYTES, "string bytes")?;
                Ok(s.len())
            }
            serde_json::Value::Array(a) => a
                .iter()
                .try_fold(0usize, |n, v| Ok(n.saturating_add(count(v)?))),
            serde_json::Value::Object(o) => o
                .values()
                .try_fold(0usize, |n, v| Ok(n.saturating_add(count(v)?))),
            _ => Ok(0),
        }
    }
    count(&serde_json::to_value(value)?)
}

struct Graph<'a> {
    nodes: Vec<&'a Node>,
    edges: Vec<Vec<usize>>,
    roots: Vec<usize>,
}
impl Graph<'_> {
    fn validate(&self, allow_callbacks: bool) -> Result<()> {
        ceiling(self.nodes.len(), limits::NODES, "nodes")?;
        let mut stored = 0usize;
        let mut callbacks = 0usize;
        for node in &self.nodes {
            stored = stored.saturating_add(node.ops.len());
            for op in &node.ops {
                if matches!(op, Op::Callback { .. } | Op::ActionCallback { .. }) {
                    callbacks += 1;
                    if !allow_callbacks {
                        return Err(Error::Invalid("row callback"));
                    }
                }
                for arg in arguments(op) {
                    match arg {
                        Argument::Number(v) => finite(*v)?,
                        Argument::Coordinate(v) => coordinate(*v)?,
                        Argument::Dimension(v) => positive(*v)?,
                        _ => {}
                    }
                }
            }
        }
        ceiling(stored, limits::STORED_OPS, "stored ops")?;
        ceiling(callbacks, limits::CALLBACKS, "callbacks")?;
        let mut colors = vec![0; self.nodes.len()];
        let mut costs = vec![(0usize, 0usize, 0usize); self.nodes.len()];
        fn visit(
            g: &Graph<'_>,
            i: usize,
            depth: usize,
            colors: &mut [u8],
            costs: &mut [(usize, usize, usize)],
        ) -> Result<(usize, usize, usize)> {
            ceiling(depth, limits::DEPTH, "depth")?;
            if i >= g.nodes.len() {
                return Err(Error::Invalid("node reference"));
            }
            if colors[i] == 1 {
                return Err(Error::Invalid("graph cycle"));
            }
            if colors[i] == 2 {
                return Ok(costs[i]);
            }
            colors[i] = 1;
            let mut v = 1usize;
            let mut w = g.nodes[i].ops.len();
            let mut height = 1;
            for &edge in &g.edges[i] {
                let (cv, cw, ch) = visit(g, edge, depth + 1, colors, costs)?;
                v = v.saturating_add(cv).min(limits::VISITS + 1);
                w = w.saturating_add(cw).min(limits::EXPANDED_OPS + 1);
                height = height.max(ch + 1);
            }
            ceiling(v, limits::VISITS, "expanded visits")?;
            ceiling(w, limits::EXPANDED_OPS, "expanded ops")?;
            ceiling(height, limits::DEPTH, "depth")?;
            colors[i] = 2;
            costs[i] = (v, w, height);
            Ok(costs[i])
        }
        let (mut visits, mut ops) = (0usize, 0usize);
        for &root in &self.roots {
            let (v, w, _) = visit(self, root, 1, &mut colors, &mut costs)?;
            visits = visits.saturating_add(v);
            ops = ops.saturating_add(w);
        }
        ceiling(visits, limits::VISITS, "expanded visits")?;
        ceiling(ops, limits::EXPANDED_OPS, "expanded ops")?;
        if colors.contains(&0) {
            return Err(Error::Invalid("unreachable node"));
        }
        Ok(())
    }
}
fn local_edges(node: &Node, offset: usize, count: usize) -> Result<Vec<usize>> {
    let mut edges = node.children.clone();
    for op in &node.ops {
        if let Op::Slot { node, .. } | Op::StateStyle { node, .. } = op {
            edges.push(*node);
        }
    }
    for edge in &mut edges {
        if *edge >= count {
            return Err(Error::Invalid("node index"));
        }
        *edge += offset;
    }
    Ok(edges)
}
impl Snapshot {
    pub fn validate(&self, tables: &ValidationTables) -> Result<()> {
        validate_single_tree(&self.nodes, self.root, tables, true)
    }
}
fn validate_single_tree_shape(nodes: &[Node], root: usize, callbacks: bool) -> Result<()> {
    let edges = nodes
        .iter()
        .map(|n| local_edges(n, 0, nodes.len()))
        .collect::<Result<Vec<_>>>()?;
    if nodes.iter().any(|n| n.child_view.is_some()) {
        return Err(Error::Invalid("unresolved child view"));
    }
    Graph {
        nodes: nodes.iter().collect(),
        edges,
        roots: vec![root],
    }
    .validate(callbacks)?;
    ceiling(
        string_bytes(&nodes)?,
        limits::STRINGS_BYTES,
        "aggregate strings",
    )?;
    Ok(())
}
fn validate_single_tree(
    nodes: &[Node],
    root: usize,
    tables: &ValidationTables,
    callbacks: bool,
) -> Result<()> {
    validate_single_tree_shape(nodes, root, callbacks)?;
    for n in nodes {
        for op in &n.ops {
            tables.validate_op(op)?;
        }
    }
    Ok(())
}
fn validate_node_payload(
    node: &Node,
    entities: &BTreeMap<Counter, &EntityCreate>,
    assets: &BTreeMap<&String, &AssetManifest>,
    lists: &BTreeMap<&String, &ListManifest>,
) -> Result<()> {
    if let Some(e) = &node.entity {
        if entities.get(&e.id).is_none_or(|v| v.r#ref != *e) {
            return Err(Error::Invalid("entity reference"));
        }
        let expected = match node.component {
            Component::Input | Component::NumberInput => Some(EntityKind::Input),
            Component::Textarea => Some(EntityKind::Textarea),
            Component::OtpInput => Some(EntityKind::Otp),
            Component::Slider
            | Component::SliderTrack
            | Component::SliderIndicator
            | Component::SliderThumb => Some(EntityKind::Slider),
            Component::Calendar | Component::DatePicker => Some(EntityKind::Calendar),
            Component::ScrollArea => Some(EntityKind::Scroll),
            _ => None,
        };
        if expected.is_some_and(|k| k != e.kind) {
            return Err(Error::Invalid("component entity kind"));
        }
    } else if matches!(
        node.component,
        Component::Input
            | Component::Textarea
            | Component::NumberInput
            | Component::OtpInput
            | Component::Slider
            | Component::SliderTrack
            | Component::SliderIndicator
            | Component::SliderThumb
            | Component::Calendar
            | Component::DatePicker
            | Component::ScrollArea
    ) {
        return Err(Error::Invalid("missing component entity"));
    }
    for arg in node.ops.iter().flat_map(arguments) {
        if let Argument::Entity(e) = arg
            && entities.get(&e.id).is_none_or(|v| v.r#ref != *e)
        {
            return Err(Error::Invalid("op entity reference"));
        }
    }
    if let Some(asset) = &node.asset
        && assets
            .get(&asset.id)
            .is_none_or(|a| a.digest != asset.digest)
    {
        return Err(Error::Invalid("asset reference"));
    }
    if matches!(
        node.component,
        Component::Image | Component::Svg | Component::AvatarImage
    ) && node.asset.is_none()
    {
        return Err(Error::Invalid("missing asset"));
    }
    if let Some(id) = &node.list_id
        && !lists.contains_key(id)
    {
        return Err(Error::Invalid("list reference"));
    }
    if matches!(node.component, Component::VirtualList | Component::List) && node.list_id.is_none()
    {
        return Err(Error::Invalid("missing list manifest"));
    }
    if node.component == Component::TextView && (node.text.is_none() || node.text_format.is_none())
    {
        return Err(Error::Invalid("TextView payload"));
    }
    Ok(())
}

impl Publish {
    pub fn validate_shape(&self) -> Result<()> {
        ceiling(
            serde_json::to_vec(self)?.len(),
            limits::PUBLISH_DECOMPRESSED_BYTES,
            "publish decompressed bytes",
        )?;
        ceiling(self.entities_full.len(), limits::ENTITIES, "entities")?;
        ceiling(self.list_manifests.len(), limits::LISTS, "lists")?;
        ceiling(
            string_bytes(self)?,
            limits::STRINGS_BYTES,
            "aggregate strings",
        )?;
        let mut entities = BTreeMap::new();
        let mut entity_bytes = 0usize;
        for e in &self.entities_full {
            e.validate()?;
            if entities.insert(e.r#ref.id, e).is_some() {
                return Err(Error::Invalid("duplicate entity id"));
            }
            entity_bytes = entity_bytes.saturating_add(serde_json::to_vec(e)?.len());
        }
        ceiling(
            entity_bytes,
            limits::ENTITY_STATE_BYTES,
            "entity aggregate state",
        )?;
        let mut views = BTreeMap::new();
        let mut offsets = Vec::new();
        let mut count = 0usize;
        for v in &self.views_full {
            if v.state_rev != self.wm.state_rev
                || v.view_ref.kind != EntityKind::View
                || v.generation.0 == 0
            {
                return Err(Error::Invalid("view revision/kind"));
            }
            if v.root >= v.nodes.len()
                || views
                    .insert(v.view_ref.id, (count + v.root, v.view_ref.clone()))
                    .is_some()
            {
                return Err(Error::Invalid("view root/identity"));
            }
            offsets.push(count);
            count = count.saturating_add(v.nodes.len());
        }
        ceiling(count, limits::NODES, "nodes")?;
        GenerationTracker::default().check(self)?;
        for e in &self.entities_full {
            if let EntityState::View {
                parent,
                root,
                generation,
                ..
            } = &e.state
            {
                let view = self
                    .views_full
                    .iter()
                    .find(|v| v.view_ref == e.r#ref)
                    .ok_or(Error::Invalid("retained view reference"))?;
                if *root != view.root || *generation != view.generation {
                    return Err(Error::Invalid("retained view root/generation"));
                }
                if let Some(parent) = parent
                    && (parent.kind != EntityKind::View
                        || entities.get(&parent.id).is_none_or(|e| e.r#ref != *parent)
                        || self.views_full.iter().all(|v| v.view_ref != *parent))
                {
                    return Err(Error::Invalid("retained view parent"));
                }
            }
        }
        for v in &self.views_full {
            if entities
                .get(&v.view_ref.id)
                .is_none_or(|e| e.r#ref != v.view_ref)
            {
                return Err(Error::Invalid("view entity reference"));
            }
        }
        let mut graph = Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            roots: Vec::new(),
        };
        let mut child_roots = BTreeSet::new();
        let mut lists = BTreeMap::new();
        for m in &self.list_manifests {
            m.validate()?;
            if lists.insert(&m.list_id, m).is_some() {
                return Err(Error::Invalid("duplicate list"));
            }
        }
        let mut assets = BTreeMap::new();
        let (mut decoded, mut pixels) = (0usize, 0usize);
        for a in &self.asset_manifests {
            a.validate()?;
            if assets.insert(&a.id, a).is_some() {
                return Err(Error::Invalid("duplicate asset"));
            }
            decoded = decoded.saturating_add(a.decoded_bytes);
            pixels = pixels.saturating_add(a.width.saturating_mul(a.height));
        }
        ceiling(
            decoded,
            limits::ASSETS_DECODED_BYTES,
            "decoded asset aggregate",
        )?;
        ceiling(pixels, limits::ASSETS_PIXELS, "asset pixels aggregate")?;
        for (v, &offset) in self.views_full.iter().zip(&offsets) {
            for node in &v.nodes {
                let mut edges = local_edges(node, offset, v.nodes.len())?;
                if (node.component == Component::ChildView) != node.child_view.is_some() {
                    return Err(Error::Invalid("ChildView payload"));
                }
                if let Some(id) = node.child_view {
                    let (root, _) = views
                        .get(&id)
                        .ok_or(Error::Invalid("ChildView reference"))?;
                    edges.push(*root);
                    child_roots.insert(*root);
                }
                validate_node_payload(node, &entities, &assets, &lists)?;
                graph.nodes.push(node);
                graph.edges.push(edges);
            }
        }
        for (v, offset) in self.views_full.iter().zip(offsets) {
            if !child_roots.contains(&(offset + v.root)) {
                graph.roots.push(offset + v.root);
            }
        }
        // A forest with no roots is cyclic. Check it explicitly rather than accepting an empty sum.
        if graph.roots.is_empty() && !graph.nodes.is_empty() {
            return Err(Error::Invalid("cyclic ChildView roots"));
        }
        graph.validate(true)?;
        for e in &self.entities_full {
            if let EntityState::View { parent, .. } = &e.state {
                let actual: BTreeSet<_> = self
                    .views_full
                    .iter()
                    .filter(|v| v.nodes.iter().any(|n| n.child_view == Some(e.r#ref.id)))
                    .map(|v| v.view_ref.id)
                    .collect();
                let expected: BTreeSet<_> = parent.iter().map(|r| r.id).collect();
                if actual != expected {
                    return Err(Error::Invalid("retained view parent edge"));
                }
            }
        }
        for overlay in &self.overlays_full {
            overlay.validate()?;
        }
        for focus in &self.focus_state {
            if focus.entity.kind != EntityKind::Focus
                || entities
                    .get(&focus.entity.id)
                    .is_none_or(|e| e.r#ref != focus.entity)
            {
                return Err(Error::Invalid("focus reference"));
            }
        }
        Ok(())
    }
    pub fn validate(&self, tables: &ValidationTables) -> Result<()> {
        self.validate_shape()?;
        for v in &self.views_full {
            for n in &v.nodes {
                for op in &n.ops {
                    tables.validate_op(op)?;
                }
            }
        }
        Ok(())
    }
}
impl ListManifest {
    pub fn validate(&self) -> Result<()> {
        ceiling(self.item_count, limits::LIST_ITEMS, "list items")?;
        match (&self.kind, &self.sizes_or_estimate) {
            (ListKind::Virtual, ListSizes::Sizes(s)) if s.len() == self.item_count => {
                for v in s {
                    positive(*v)?;
                }
            }
            (ListKind::Uniform | ListKind::Measured, ListSizes::Estimate(v)) => positive(*v)?,
            _ => return Err(Error::Invalid("list sizes/kind")),
        }
        Ok(())
    }
}
impl RowsRequest {
    pub fn validate(&self) -> Result<()> {
        if self.range.start > self.range.end || self.range.end > limits::LIST_ITEMS {
            return Err(Error::Invalid("row range"));
        }
        ceiling(self.range.end - self.range.start, limits::ROWS, "row count")
    }
}
/// Borrowed immutable state from the currently adopted copy. Construct after
/// ReceiveContext/floor checks and atomic adoption; never from another copy.
pub struct RowValidation<'a> {
    publish: &'a Publish,
    tables: &'a ValidationTables,
    entities: BTreeMap<Counter, &'a EntityCreate>,
    assets: BTreeMap<&'a String, &'a AssetManifest>,
    lists: BTreeMap<&'a String, &'a ListManifest>,
}
impl<'a> RowValidation<'a> {
    pub fn new(publish: &'a Publish, tables: &'a ValidationTables) -> Result<Self> {
        publish.validate(tables)?;
        Ok(Self {
            publish,
            tables,
            entities: publish
                .entities_full
                .iter()
                .map(|e| (e.r#ref.id, e))
                .collect(),
            assets: publish.asset_manifests.iter().map(|a| (&a.id, a)).collect(),
            lists: publish
                .list_manifests
                .iter()
                .map(|m| (&m.list_id, m))
                .collect(),
        })
    }
}
impl RowsReply {
    /// Context-free checks also run in ordinary encode/decode, before floats can
    /// be converted to JSON null. References and freshness still need context.
    pub fn validate_shape(&self) -> Result<()> {
        let count = self
            .range
            .end
            .checked_sub(self.range.start)
            .ok_or(Error::Invalid("row range"))?;
        if self.range.end > limits::LIST_ITEMS {
            return Err(Error::Invalid("row range"));
        }
        ceiling(count, limits::ROWS, "row count")?;
        if self.rows.len() != count {
            return Err(Error::Invalid("row cardinality"));
        }
        ceiling(
            serde_json::to_vec(self)?.len(),
            limits::ROWS_BYTES,
            "rows bytes",
        )?;
        let mut keys = BTreeSet::new();
        let (mut nodes, mut ops) = (0usize, 0usize);
        for row in &self.rows {
            if !keys.insert(&row.key) {
                return Err(Error::Invalid("duplicate row key"));
            }
            validate_single_tree_shape(&row.nodes, row.root, false)?;
            if canonical_digest(row, "content_hash")? != row.content_hash {
                return Err(Error::DigestMismatch);
            }
            nodes = nodes.saturating_add(row.nodes.len());
            ops = ops.saturating_add(row.nodes.iter().map(|n| n.ops.len()).sum::<usize>());
        }
        ceiling(nodes, limits::NODES, "row aggregate nodes")?;
        ceiling(ops, limits::STORED_OPS, "row aggregate ops")?;
        Ok(())
    }

    pub fn validate_against(
        &self,
        request: &RowsRequest,
        manifest: &ListManifest,
        context: &RowValidation<'_>,
    ) -> Result<()> {
        if context.lists.get(&manifest.list_id).copied() != Some(manifest)
            || request.view_generation != manifest.handler_generation
            || !context
                .publish
                .views_full
                .iter()
                .any(|v| v.generation == request.view_generation)
        {
            return Err(Error::Stale("rows publication context"));
        }
        request.validate()?;
        manifest.validate()?;
        if self.request_id != request.request_id
            || self.list_id != request.list_id
            || self.view_generation != request.view_generation
            || self.content_rev != request.content_rev
            || self.range != request.range
            || self.viewport_rev != request.viewport_rev
            || self.list_id != manifest.list_id
            || self.content_rev != manifest.content_rev
            || self.range.end > manifest.item_count
        {
            return Err(Error::Stale("rows request/revision"));
        }
        self.validate_shape()?;
        for row in &self.rows {
            for node in &row.nodes {
                validate_node_payload(node, &context.entities, &context.assets, &context.lists)?;
                for op in &node.ops {
                    context.tables.validate_op(op)?;
                }
            }
        }
        Ok(())
    }
}
impl AssetManifest {
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::Invalid("asset dimensions"));
        }
        ceiling(self.width, limits::IMAGE_DIMENSION, "asset width")?;
        ceiling(self.height, limits::IMAGE_DIMENSION, "asset height")?;
        let max = if self.kind == AssetKind::Svg {
            limits::SVG_BYTES
        } else {
            limits::IMAGE_BYTES
        };
        ceiling(self.encoded_bytes, max, "encoded asset bytes")?;
        ceiling(
            self.decoded_bytes,
            limits::ASSET_DECODED_BYTES,
            "decoded asset bytes",
        )?;
        let rgba = self
            .width
            .checked_mul(self.height)
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::Limit("asset dimensions overflow"))?;
        if self.decoded_bytes < rgba {
            return Err(Error::Invalid("decoded asset undercount"));
        }
        Ok(())
    }
}
impl AssetReply {
    pub fn validate_against(&self, request: &AssetRequest, manifest: &AssetManifest) -> Result<()> {
        manifest.validate()?;
        if self.request_id != request.request_id
            || self.asset != request.asset
            || self.asset.id != manifest.id
            || self.asset.digest != manifest.digest
        {
            return Err(Error::Stale("asset request"));
        }
        if self.bytes.len() != manifest.encoded_bytes
            || bytes_digest(&self.bytes) != manifest.digest
        {
            return Err(Error::DigestMismatch);
        }
        Ok(())
    }
}
/// URL confirmation uses this normalized host; no URL is fetched here.
pub fn validate_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value).map_err(|_| Error::Invalid("URL"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(Error::Invalid("URL scheme/credentials"));
    }
    url.host_str()
        .map(str::to_owned)
        .filter(|h| !h.is_empty())
        .ok_or(Error::Invalid("URL host"))
}
impl DeviceRequest {
    pub fn validate(&self) -> Result<()> {
        match self.operation {
            DeviceOperation::OpenUrl => {
                validate_url(&self.value)?;
            }
            DeviceOperation::ClipboardWrite => {
                ceiling(
                    self.value.len(),
                    limits::ENTITY_STRING_BYTES,
                    "clipboard bytes",
                )?;
            }
        }
        Ok(())
    }
}
/// The host fingerprint comes from the SSH verifier, not from HelloAck.
#[derive(Debug, Clone)]
pub struct ReceiveContext {
    pub session: Session,
    pub scope: Scope,
    pub owner_fence: Counter,
}
impl ReceiveContext {
    pub fn validate<T: Payload>(&self, message: &Envelope<T>) -> Result<()> {
        message.validate()?;
        if message.session != self.session {
            return Err(Error::Stale("session identity"));
        }
        if message.scope != self.scope {
            return Err(Error::Stale("copy incarnation"));
        }
        if message.owner_fence != self.owner_fence {
            return Err(Error::Stale("owner fence"));
        }
        Ok(())
    }
}
/// Commit this only after atomic display adoption. Reconnecting resets transport
/// sequence numbers, but callers must keep these generation floors for the copy.
#[derive(Debug, Default, Clone)]
pub struct GenerationTracker {
    high: Counter,
}
impl GenerationTracker {
    pub fn high_watermark(&self) -> Counter {
        self.high
    }
    pub fn check(&self, publish: &Publish) -> Result<()> {
        ceiling(publish.views_full.len(), limits::NODES, "views")?;
        let mut seen = BTreeSet::new();
        for v in &publish.views_full {
            if v.generation <= self.high || !seen.insert(v.generation) {
                return Err(Error::Stale("view generation reused"));
            }
        }
        Ok(())
    }
    pub fn commit(&mut self, publish: &Publish) -> Result<()> {
        self.check(publish)?;
        self.high = publish
            .views_full
            .iter()
            .map(|v| v.generation)
            .max()
            .unwrap_or(self.high);
        Ok(())
    }
}
#[derive(Debug, Clone, Default)]
pub struct ReceiptFloors {
    pub displayed_pub: Counter,
    pub state: Counter,
    pub processed_op: Counter,
    pub settled_edit: Counter,
    pub entities: Vec<(EntityRef, Counter)>,
}
impl ReceiptFloors {
    pub fn raise(&mut self, wm: &OpWatermark, entities: &[EntityResult]) {
        self.state = self.state.max(wm.state_rev);
        self.processed_op = self.processed_op.max(wm.processed_op_seq);
        self.settled_edit = self.settled_edit.max(wm.settled_local_seq);
        for e in entities {
            match self.entities.iter_mut().find(|(r, _)| *r == e.r#ref) {
                Some((_, rev)) => *rev = (*rev).max(e.rev),
                None => self.entities.push((e.r#ref.clone(), e.rev)),
            }
        }
    }
    /// Missing entities need explicit release tombstones supplied by the caller's
    /// validated entity ledger; absence alone never proves a release.
    pub fn check(&self, publish: &Publish, tombstones: &[EntityRelease]) -> Result<()> {
        if publish.pub_rev <= self.displayed_pub
            || publish.wm.state_rev < self.state
            || publish.wm.processed_op_seq < self.processed_op
            || publish.wm.settled_local_seq < self.settled_edit
        {
            return Err(Error::Stale("BehindReceipt"));
        }
        // A release is final for this exact incarnation, even without a receipt floor.
        for e in &publish.entities_full {
            if tombstones.iter().any(|release| release.r#ref == e.r#ref) {
                return Err(Error::Stale("released entity reappeared"));
            }
        }
        for (r, rev) in &self.entities {
            let covered = match publish.entities_full.iter().find(|e| e.r#ref == *r) {
                Some(e) => e.rev >= *rev,
                None => tombstones.iter().any(|e| e.r#ref == *r && e.rev >= *rev),
            };
            if !covered {
                return Err(Error::Stale("entity receipt floor"));
            }
        }
        Ok(())
    }
}
impl Hello {
    pub fn validate(&self) -> Result<()> {
        if self.protocol != PROTOCOL || self.schema != SCHEMA {
            return Err(Error::Unknown("hello protocol/schema"));
        }
        self.limits.validate()
    }
    pub fn validate_compatibility(&self, expected: &Digests, required: &[String]) -> Result<()> {
        self.validate()?;
        if self.digests != *expected {
            return Err(Error::DigestMismatch);
        }
        if required.iter().any(|f| !self.features.contains(f)) {
            return Err(Error::Unknown("required feature"));
        }
        Ok(())
    }
}
impl HelloAck {
    pub fn validate_nonce(&self, nonce: &Id128) -> Result<()> {
        if &self.nonce != nonce {
            return Err(Error::Stale("hello nonce"));
        }
        if self.protocol != PROTOCOL {
            return Err(Error::Unknown("hello protocol"));
        }
        self.limits.validate()
    }
}
impl Edit {
    pub fn validate(&self) -> Result<()> {
        ceiling(self.text.len(), limits::ENTITY_STRING_BYTES, "edit text")?;
        if !matches!(
            self.entity.kind,
            EntityKind::Input | EntityKind::Textarea | EntityKind::Otp
        ) {
            return Err(Error::Invalid("edit entity kind"));
        }
        self.selection.validate(&self.text)?;
        if canonical_digest(self, "digest")? != self.digest {
            return Err(Error::DigestMismatch);
        }
        Ok(())
    }
}
impl Action {
    pub fn validate(&self) -> Result<()> {
        if let Some(p) = &self.data.point {
            p.validate()?;
        }
        if let Some(n) = self.data.value {
            finite(n)?;
        }
        if !self.data.permissions.is_empty()
            && !matches!(self.kind, ActionKind::Click | ActionKind::Key)
        {
            return Err(Error::Invalid("permission requires individual tap/key"));
        }
        let mut permissions = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for permission in &self.data.permissions {
            if permission.policy_rev.0 == 0
                || !permissions.insert(permission.permission.clone())
                || !operations.insert(permission.operation)
            {
                return Err(Error::Invalid("duplicate/invalid device permission"));
            }
        }
        if canonical_digest(self, "digest")? != self.digest {
            return Err(Error::DigestMismatch);
        }
        Ok(())
    }
}
/// Shared counters for resources whose ownership spans messages. Scope-specific
/// ledgers must be shared across all copies/hosts at that scope, not per socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resource {
    HostCopies,
    AppCopies,
    CallbackGenerations,
    Callbacks,
    Events,
    ControlPending,
    ControlPerSecond,
    Receipts,
    ReceiptBytes,
    RowRequestsList,
    RowRequestsCopy,
    RowRequestsApp,
    RowCacheBytes,
    AssetCacheBytes,
    AssetTransfersCopy,
    AssetTransfersHost,
    AssetTransfersApp,
    AssetDecodedBytes,
    AssetsDecodedBytes,
    AssetsPixels,
    AppBytes,
    JsHeapBytes,
    WorkerRssBytes,
    WorkersRssBytes,
    ChildrenCopy,
    ChildrenHost,
    BackgroundCopies,
}
impl Resource {
    pub fn limit(self) -> usize {
        match self {
            Self::HostCopies => limits::COPIES_HOST,
            Self::AppCopies => limits::COPIES_APP,
            Self::CallbackGenerations => limits::CALLBACK_GENERATIONS,
            Self::Callbacks => limits::CALLBACKS,
            Self::Events => limits::EVENT_QUEUE,
            Self::ControlPending => limits::CONTROL_PENDING_BYTES,
            Self::ControlPerSecond => limits::CONTROL_PER_SECOND,
            Self::Receipts => limits::RECEIPTS,
            Self::ReceiptBytes => limits::RECEIPTS_BYTES,
            Self::RowRequestsList => limits::ROW_REQUESTS_LIST,
            Self::RowRequestsCopy => limits::ROW_REQUESTS_COPY,
            Self::RowRequestsApp => limits::ROW_REQUESTS_APP,
            Self::RowCacheBytes => limits::ROW_CACHE_BYTES,
            Self::AssetCacheBytes => limits::ASSET_CACHE_BYTES,
            Self::AssetTransfersCopy => limits::ASSET_TRANSFERS_COPY,
            Self::AssetTransfersHost => limits::ASSET_TRANSFERS_HOST,
            Self::AssetTransfersApp => limits::ASSET_TRANSFERS_APP,
            Self::AssetDecodedBytes => limits::ASSET_DECODED_BYTES,
            Self::AssetsDecodedBytes => limits::ASSETS_DECODED_BYTES,
            Self::AssetsPixels => limits::ASSETS_PIXELS,
            Self::AppBytes => limits::APP_BYTES,
            Self::JsHeapBytes => limits::JS_HEAP_BYTES,
            Self::WorkerRssBytes => limits::WORKER_RSS_BYTES,
            Self::WorkersRssBytes => limits::WORKERS_RSS_BYTES,
            Self::ChildrenCopy => limits::CHILDREN_COPY,
            Self::ChildrenHost => limits::CHILDREN_HOST,
            Self::BackgroundCopies => limits::BACKGROUND_COPIES,
        }
    }
}
#[derive(Debug, Default)]
pub struct ResourceLedger {
    used: BTreeMap<Resource, usize>,
}
impl ResourceLedger {
    pub fn used(&self, r: Resource) -> usize {
        self.used.get(&r).copied().unwrap_or(0)
    }
    pub fn reserve(&mut self, r: Resource, amount: usize) -> Result<()> {
        let next = self
            .used(r)
            .checked_add(amount)
            .ok_or(Error::Limit("resource overflow"))?;
        ceiling(next, r.limit(), "resource budget")?;
        self.used.insert(r, next);
        Ok(())
    }
    pub fn release(&mut self, r: Resource, amount: usize) -> Result<()> {
        let next = self
            .used(r)
            .checked_sub(amount)
            .ok_or(Error::Invalid("resource release underflow"))?;
        self.used.insert(r, next);
        Ok(())
    }
}

impl Overlay {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.copy.is_empty() {
            return Err(Error::Invalid("overlay identity"));
        }
        match (self.kind, &self.toast) {
            (OverlayKind::Overlay, None) => Ok(()),
            (OverlayKind::Toast, Some(toast))
                if !toast.message.is_empty()
                    && !toast.mod_name.is_empty()
                    && !toast.host_name.is_empty() =>
            {
                Ok(())
            }
            _ => Err(Error::Invalid("overlay kind/content")),
        }
    }
}
impl GatewayTarget {
    /// Compare with an externally authenticated target, never a self-reported identity.
    pub fn validate_against(&self, expected: &ReceiveContext) -> Result<()> {
        match &expected.scope {
            Scope::Copy { id, incarnation }
                if self.boot == expected.session.boot
                    && self.copy == *id
                    && self.incarnation == *incarnation =>
            {
                Ok(())
            }
            _ => Err(Error::Stale("gateway target")),
        }
    }
}
impl GatewayRequest {
    pub fn validate(&self) -> Result<()> {
        if self.request_id.is_empty() || self.target.copy.is_empty() || self.policy_rev.0 == 0 {
            return Err(Error::Invalid("gateway identity/policy"));
        }
        match &self.operation {
            GatewayOperation::Run { command, .. } if command.is_empty() => {
                return Err(Error::Invalid("gateway command"));
            }
            GatewayOperation::OpenOverlay { overlay } => overlay.validate()?,
            GatewayOperation::Notify { overlay } => {
                overlay.validate()?;
                if overlay.kind != OverlayKind::Toast {
                    return Err(Error::Invalid("notify requires toast"));
                }
            }
            GatewayOperation::CloseOverlay { overlay_id } if overlay_id.is_empty() => {
                return Err(Error::Invalid("gateway overlay"));
            }
            GatewayOperation::SetAgentOrder { agents } => {
                let mut seen = BTreeSet::new();
                if agents
                    .iter()
                    .any(|agent| agent.is_empty() || !seen.insert(agent))
                {
                    return Err(Error::Invalid("gateway agent order"));
                }
            }
            _ => {}
        }
        ceiling(
            string_bytes(self)?,
            limits::STRINGS_BYTES,
            "aggregate strings",
        )?;
        Ok(())
    }
}
impl DeviceRequest {
    /// Check the explicit hand-off only. The device MUST also check its own
    /// one-use ledger, current epoch/owner/copy, policy, expiry and cancellation.
    pub fn validate_permission(&self, action: &Action) -> Result<()> {
        self.validate()?;
        action.validate()?;
        if self.event_id != action.event_id
            || !action.data.permissions.iter().any(|p| {
                p.permission == self.permission
                    && p.operation == self.operation
                    && p.policy_rev == self.policy_rev
            })
        {
            return Err(Error::Invalid("device permission hand-off"));
        }
        Ok(())
    }
}
