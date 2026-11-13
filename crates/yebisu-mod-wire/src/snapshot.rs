use crate::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Input,
    Textarea,
    Slider,
    Otp,
    Calendar,
    Focus,
    Scroll,
    VirtualScroll,
    View,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    User,
    HostSet,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Forward,
    Backward,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarMode {
    Day,
    Month,
    Year,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextFormat {
    Html,
    Markdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListKind {
    Virtual,
    Uniform,
    Measured,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionEvent {
    Change,
    Submit,
    Focus,
    Blur,
    Release,
    Complete,
    Scroll,
    RequestAck,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub id: Counter,
    pub incarnation: Id128,
    pub kind: EntityKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
    pub direction: Direction,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollRequest {
    pub id: String,
    pub index: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateSelection {
    pub start: Date,
    pub end: Option<Date>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscriptions {
    pub rev: Counter,
    pub events: Vec<SubscriptionEvent>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EntityConfig {
    Input {
        placeholder: String,
        masked: bool,
        loading: bool,
        readonly: bool,
        number: Option<NumberRange>,
    },
    Textarea {
        placeholder: String,
        masked: bool,
        loading: bool,
        readonly: bool,
        rows: usize,
        min_height: f64,
        max_height: f64,
        wrap: bool,
    },
    Slider {
        min: f64,
        max: f64,
        step: f64,
        scale: f64,
        axis: Axis,
    },
    Otp {
        digits: usize,
        masked: bool,
    },
    Calendar {
        months: usize,
    },
    Focus {},
    Scroll {},
    VirtualScroll {},
    View {},
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EntityState {
    Input {
        text: String,
        selection: Selection,
    },
    Textarea {
        text: String,
        selection: Selection,
    },
    Slider {
        value: f64,
        end: Option<f64>,
    },
    Otp {
        text: String,
        selection: Selection,
    },
    Calendar {
        selected: Option<DateSelection>,
        year: i32,
        month: u8,
        mode: CalendarMode,
        today: Date,
    },
    Focus {
        id: String,
        focused: bool,
    },
    Scroll {
        offset: Point,
        viewport: Size,
        content: Size,
        request: Option<ScrollRequest>,
    },
    VirtualScroll {
        offset: Point,
        anchor_key: String,
        anchor_index: usize,
        content_rev: Counter,
        request: Option<ScrollRequest>,
    },
    View {
        parent: Option<Counter>,
        root: usize,
        generation: Counter,
        props_rev: Counter,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityCreate {
    pub r#ref: EntityRef,
    pub rev: Counter,
    pub config: EntityConfig,
    pub state: EntityState,
    pub subscriptions: Subscriptions,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityUpdate {
    pub r#ref: EntityRef,
    pub base_rev: Counter,
    pub rev: Counter,
    pub origin: Origin,
    pub config: EntityConfig,
    pub state: EntityState,
    pub subscriptions: Subscriptions,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRelease {
    pub r#ref: EntityRef,
    pub base_rev: Counter,
    pub rev: Counter,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityResult {
    pub r#ref: EntityRef,
    pub rev: Counter,
    pub state: EntityState,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FocusAck {
    pub entity: EntityRef,
    pub request_id: String,
    pub focused: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Component {
    Div,
    HFlex,
    VFlex,
    Text,
    TextView,
    ChildView,
    Button,
    Link,
    Checkbox,
    Switch,
    Scrollbar,
    Input,
    Textarea,
    NumberInput,
    OtpInput,
    Svg,
    Accordion,
    AccordionItem,
    AccordionHeader,
    AccordionPanel,
    AccordionTrigger,
    Pagination,
    Avatar,
    AvatarImage,
    AvatarFallback,
    Image,
    Path,
    Tabs,
    Tab,
    Progress,
    ProgressTrack,
    ProgressIndicator,
    FpsMonitor,
    Slider,
    SliderTrack,
    SliderIndicator,
    SliderThumb,
    Radio,
    Toggle,
    RadioGroup,
    ToggleGroup,
    Table,
    TableHeader,
    TableBody,
    TableRow,
    TableHead,
    TableCell,
    TableCaption,
    Resizable,
    ResizablePanel,
    Collapsible,
    Popover,
    HoverCard,
    Popup,
    Select,
    Combobox,
    DatePicker,
    Calendar,
    ScrollArea,
    VirtualList,
    List,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    pub id: String,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub component: Component,
    pub text: Option<String>,
    pub entity: Option<EntityRef>,
    pub asset: Option<AssetRef>,
    pub text_format: Option<TextFormat>,
    pub child_view: Option<Counter>,
    pub list_id: Option<String>,
    pub children: Vec<usize>,
    pub ops: Vec<Op>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Op {
    NullaryStyle { index: u16, name: String },
    ParamStyle { name: String, args: Vec<Argument> },
    Method { name: String, args: Vec<Argument> },
    Callback { name: String, id: Counter },
    ActionCallback { name: String, id: Counter },
    StateStyle { name: StateStyle, node: usize },
    Slot { name: String, node: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateStyle {
    Hover,
    Active,
    Focus,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum Argument {
    Number(f64),
    Text(String),
    Bool(bool),
    Entity(EntityRef),
    Coordinate(f64),
    Dimension(f64),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSnapshot {
    pub view_ref: EntityRef,
    pub generation: Counter,
    pub root: usize,
    pub nodes: Vec<Node>,
    pub state_rev: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub copy: String,
    pub generation: Counter,
    pub root: usize,
    pub nodes: Vec<Node>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpWatermark {
    pub processed_op_seq: Counter,
    pub state_rev: Counter,
    pub settled_local_seq: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    pub id: String,
    pub parent_pub: Counter,
    pub copy: String,
    pub incarnation: Id128,
    pub pub_rev: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    pub pub_rev: Counter,
    pub wm: OpWatermark,
    pub entities_full: Vec<EntityCreate>,
    pub views_full: Vec<ViewSnapshot>,
    pub focus_state: Vec<FocusAck>,
    pub overlays_full: Vec<Overlay>,
    pub list_manifests: Vec<ListManifest>,
    pub asset_manifests: Vec<AssetManifest>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishBegin {
    pub total_bytes: usize,
    pub chunk_count: usize,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishChunk {
    pub index: usize,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishCommit {
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListManifest {
    pub list_id: String,
    pub content_rev: Counter,
    pub kind: ListKind,
    pub item_count: usize,
    pub sizes_or_estimate: ListSizes,
    pub handler_generation: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum ListSizes {
    Sizes(Vec<f64>),
    Estimate(f64),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowRange {
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowsRequest {
    pub request_id: String,
    pub list_id: String,
    pub view_generation: Counter,
    pub content_rev: Counter,
    pub range: RowRange,
    pub viewport_rev: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub key: String,
    pub content_hash: Digest,
    pub root: usize,
    pub nodes: Vec<Node>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowsReply {
    pub request_id: String,
    pub list_id: String,
    pub view_generation: Counter,
    pub content_rev: Counter,
    pub range: RowRange,
    pub viewport_rev: Counter,
    pub rows: Vec<Row>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Png,
    Jpeg,
    WebP,
    Gif,
    Svg,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetManifest {
    pub id: String,
    pub digest: Digest,
    pub kind: AssetKind,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub width: usize,
    pub height: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRequest {
    pub request_id: String,
    pub asset: AssetRef,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReply {
    pub request_id: String,
    pub asset: AssetRef,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetCacheKey {
    pub boot: Id128,
    pub bundle_digest: Digest,
    pub policy_rev: Counter,
    pub asset_digest: Digest,
}
