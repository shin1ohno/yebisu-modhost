use crate::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub host: String,
    pub boot: Id128,
    pub client: String,
    pub epoch: Id128,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    Copy { id: String, incarnation: Id128 },
    Session,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Digests {
    pub style: Digest,
    pub component: Digest,
    pub op: Digest,
    pub entity: Digest,
    pub command: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegotiatedLimits {
    pub publish_bytes: usize,
    pub decompressed_bytes: usize,
    pub nodes: usize,
    pub depth: usize,
    pub visits: usize,
    pub stored_ops: usize,
    pub expanded_ops: usize,
    pub control_bytes: usize,
    pub rows: usize,
    pub rows_bytes: usize,
    pub image_bytes: usize,
    pub svg_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub client: String,
    pub nonce: Id128,
    pub protocol: u32,
    pub schema: String,
    pub digests: Digests,
    pub features: Vec<String>,
    pub limits: NegotiatedLimits,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServeVersion {
    pub version: String,
    pub commit: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyInfo {
    pub id: String,
    pub incarnation: Id128,
    pub r#mod: ModIdentity,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloAck {
    pub nonce: Id128,
    pub protocol: u32,
    pub serve: ServeVersion,
    pub boot: Id128,
    pub epoch: Id128,
    pub limits: NegotiatedLimits,
    pub copies: Vec<CopyInfo>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub op_seq: Counter,
    pub edit_id: String,
    pub entity: EntityRef,
    pub subscription_rev: Counter,
    pub base_rev: Counter,
    pub local_seq: Counter,
    pub origin: UserOrigin,
    pub text: String,
    pub selection: Selection,
    pub digest: Digest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserOrigin {
    User,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Click,
    Submit,
    Focus,
    Blur,
    Key,
    PointerMove,
    Scroll,
    SliderChange,
    SliderRelease,
    Change,
    Complete,
    Row,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub op_seq: Counter,
    pub event_id: String,
    pub after_local_seq: Counter,
    pub pub_rev: Counter,
    pub generation: Counter,
    pub lease_id: String,
    pub target: usize,
    pub kind: ActionKind,
    pub data: ActionData,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionData {
    pub key: Option<String>,
    pub text: Option<String>,
    pub point: Option<Point>,
    pub value: Option<f64>,
    pub row_key: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditAck {
    pub edit_id: String,
    pub op_seq: Counter,
    pub wm: OpWatermark,
    pub applied_rev: Counter,
    pub result_rev: Counter,
    pub state: EntityState,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionAck {
    pub event_id: String,
    pub op_seq: Counter,
    pub wm: OpWatermark,
    pub affected_entities: Vec<EntityResult>,
    pub result: ActionResult,
    pub digest: Digest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionResult {
    Ok,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditConflict {
    pub edit_id: String,
    pub op_seq: Counter,
    pub wm: OpWatermark,
    pub entity: EntityRef,
    pub current_rev: Counter,
    pub state: EntityState,
    pub digest: Digest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FailureReason {
    LeaseExpired,
    EntityReleased,
    SubscriptionChanged,
    PolicyDenied,
    NoExecAuthority,
    Unsupported,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RejectionReason {
    Gap,
    AfterLocalSeqMismatch,
    LocalSeqNotIncreasing,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpFailed {
    pub id: String,
    pub op_seq: Counter,
    pub wm: OpWatermark,
    pub reason: FailureReason,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpRejected {
    pub id: String,
    pub op_seq: Counter,
    pub wm: OpWatermark,
    pub reason: RejectionReason,
    pub digest: Digest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnAbsent {
    Cancel,
    Await,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpQuery {
    pub op_seq: Counter,
    pub id: String,
    pub digest: Digest,
    pub on_absent: OnAbsent,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpNotReceived {
    pub op_seq: Counter,
    pub id: String,
    pub wm: OpWatermark,
    pub cancelled: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayUnknown {
    pub wm: OpWatermark,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostStateCommit {
    pub wm: OpWatermark,
    pub affected_entities: Vec<EntityResult>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseRequest {
    pub request_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseAck {
    pub request_id: String,
    pub lease_id: String,
    pub duration_ms: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayAck {
    pub pub_rev: Counter,
    pub lease_id: String,
    pub replaced_pub: Option<Counter>,
    pub old_event_watermark: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeEdit {
    pub op_seq: Counter,
    pub edit_id: String,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeEdits {
    pub old_epoch: Id128,
    pub client: String,
    pub old_owner_fence: Counter,
    pub edits: Vec<ResumeEdit>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionItem {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateProjection {
    pub rev: Counter,
    pub spaces: Vec<ProjectionItem>,
    pub agents: Vec<ProjectionItem>,
    pub panes: Vec<ProjectionItem>,
    pub focused: Option<String>,
    pub layout: String,
    pub window: Size,
    pub theme: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlReason {
    Requested,
    LeaseExpired,
    BehindReceipt,
    ReplayUnknown,
    ResourceLimit,
    ProtocolError,
    UnsupportedProfile,
    PolicyChanged,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResyncRequired {
    pub reason: ControlReason,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detach {
    pub op_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pause {
    pub op_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resume {
    pub op_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unload {
    pub op_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PausedAck {
    pub op_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopStatus {
    Complete,
    Incomplete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildResult {
    Reaped,
    Unknown,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoppedChild {
    pub id: String,
    pub result: ChildResult,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopAck {
    pub op_id: String,
    pub boot: Id128,
    pub copy: String,
    pub incarnation: Id128,
    pub status: StopStatus,
    pub managed_tasks: usize,
    pub children: Vec<StoppedChild>,
    pub escaped_or_remote_unknown: bool,
    pub persisted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StopSource {
    Ipad,
    HostCli,
    ServiceStop,
    Update,
    Expiry,
    PrioritySwitch,
    ReconcileMismatch,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopRequested {
    pub op_id: String,
    pub grant_id: Option<Id128>,
    pub source: StopSource,
    pub boot: Id128,
    pub copy: String,
    pub incarnation: Id128,
    pub r#mod: ModIdentity,
    pub at: Counter,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModLocation {
    Yebisu,
    HerdrApp,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModIdentity {
    pub location: ModLocation,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantPolicy {
    pub rev: Counter,
    pub digest: Digest,
    pub full_authority: bool,
    pub on_targets: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantState {
    Active,
    Stopped,
    Expired,
    Superseded,
    Revoked,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrant {
    pub format: u32,
    pub grant_id: Id128,
    pub grant_rev: Counter,
    pub host_key: String,
    pub bundle_digest: Digest,
    pub policy: GrantPolicy,
    pub ttl_secs: Counter,
    pub last_valid_contact_at: Counter,
    pub expires_at: Counter,
    pub os_boot_id: String,
    pub cont_at_contact: Counter,
    pub cont_deadline: Counter,
    pub wall_high_water: Counter,
    pub client: String,
    pub state: GrantState,
    pub state_reason: String,
    pub updated_at: Counter,
    pub record_digest: Digest,
    pub r#mod: ModIdentity,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantIssue {
    pub op_id: String,
    pub copy: String,
    pub incarnation: Id128,
    pub bundle_digest: Digest,
    pub policy_digest: Digest,
    pub ttl_secs: Counter,
    pub r#mod: ModIdentity,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BgGrantAck {
    Bound(BgGrantBound),
    Unbound(BgGrantUnbound),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantBound {
    pub op_id: String,
    pub grant_id: Id128,
    pub grant_rev: Counter,
    pub bound: True,
    pub expires_at: Counter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantUnbound {
    pub op_id: String,
    pub bound: False,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantSummary {
    pub grant_id: Id128,
    pub grant_rev: Counter,
    pub bundle_digest: Digest,
    pub policy_digest: Digest,
    pub ttl_secs: Counter,
    pub expires_at: Counter,
    pub state: GrantState,
    pub state_reason: String,
    pub restored_boot: Option<Id128>,
    pub r#mod: ModIdentity,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantList {
    pub grants: Vec<BgGrantSummary>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgGrantVerdict {
    pub grant_id: Id128,
    pub grant_rev: Counter,
    pub r#match: bool,
}
/// Descriptive execution authority only. Runtime authorization belongs to the runner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ExecAuthority {
    Operating {
        conn: ConnectionRef,
    },
    Background {
        grant_id: Id128,
        grant_rev: Counter,
        deadline: Counter,
    },
    None,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reconcile {
    Pending,
    Matched { grant_id: Id128, grant_rev: Counter },
    Mismatched,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionRef {
    pub epoch: Id128,
    pub owner_fence: Counter,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceOperation {
    ClipboardWrite,
    OpenUrl,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRequest {
    pub request_id: String,
    pub permission: Id128,
    pub event_id: String,
    pub operation: DeviceOperation,
    pub policy_rev: Counter,
    pub value: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceReply {
    pub request_id: String,
    pub result: DeviceResult,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceResult {
    Ok,
    Denied,
    Expired,
    UnsupportedRemoteClipboard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MessageKind {
    Hello,
    HelloAck,
    EntityCreate,
    EntityUpdate,
    EntityRelease,
    FocusAck,
    Publish,
    PublishBegin,
    PublishChunk,
    PublishCommit,
    RowsRequest,
    RowsReply,
    AssetRequest,
    AssetReply,
    Edit,
    Action,
    EditAck,
    ActionAck,
    EditConflict,
    OpFailed,
    OpRejected,
    OpQuery,
    OpNotReceived,
    ReplayUnknown,
    HostStateCommit,
    LeaseRequest,
    LeaseAck,
    DisplayAck,
    ResumeEdits,
    StateProjection,
    ResyncRequired,
    Detach,
    Pause,
    Resume,
    Unload,
    PausedAck,
    StopAck,
    StopRequested,
    BgGrantIssue,
    BgGrantAck,
    BgGrantList,
    BgGrantVerdict,
    DeviceRequest,
    DeviceReply,
}
impl Payload for Hello {
    const KIND: MessageKind = MessageKind::Hello;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for HelloAck {
    const KIND: MessageKind = MessageKind::HelloAck;
}
impl Payload for EntityCreate {
    const KIND: MessageKind = MessageKind::EntityCreate;
    fn validate_wire(&self) -> Result<()> {
        if self.rev != Counter(1) {
            return Err(Error::Invalid("create revision"));
        }
        self.validate()
    }
}
impl Payload for EntityUpdate {
    const KIND: MessageKind = MessageKind::EntityUpdate;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for EntityRelease {
    const KIND: MessageKind = MessageKind::EntityRelease;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for FocusAck {
    const KIND: MessageKind = MessageKind::FocusAck;
}
impl Payload for Publish {
    const KIND: MessageKind = MessageKind::Publish;
    fn validate_wire(&self) -> Result<()> {
        self.validate_shape()
    }
}
impl Payload for PublishBegin {
    const KIND: MessageKind = MessageKind::PublishBegin;
}
impl Payload for PublishChunk {
    const KIND: MessageKind = MessageKind::PublishChunk;
}
impl Payload for PublishCommit {
    const KIND: MessageKind = MessageKind::PublishCommit;
}
impl Payload for RowsRequest {
    const KIND: MessageKind = MessageKind::RowsRequest;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for RowsReply {
    const KIND: MessageKind = MessageKind::RowsReply;
}
impl Payload for AssetRequest {
    const KIND: MessageKind = MessageKind::AssetRequest;
}
impl Payload for AssetReply {
    const KIND: MessageKind = MessageKind::AssetReply;
}
impl Payload for Edit {
    const KIND: MessageKind = MessageKind::Edit;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for Action {
    const KIND: MessageKind = MessageKind::Action;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for EditAck {
    const KIND: MessageKind = MessageKind::EditAck;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for ActionAck {
    const KIND: MessageKind = MessageKind::ActionAck;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for EditConflict {
    const KIND: MessageKind = MessageKind::EditConflict;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for OpFailed {
    const KIND: MessageKind = MessageKind::OpFailed;
}
impl Payload for OpRejected {
    const KIND: MessageKind = MessageKind::OpRejected;
}
impl Payload for OpQuery {
    const KIND: MessageKind = MessageKind::OpQuery;
}
impl Payload for OpNotReceived {
    const KIND: MessageKind = MessageKind::OpNotReceived;
}
impl Payload for ReplayUnknown {
    const KIND: MessageKind = MessageKind::ReplayUnknown;
}
impl Payload for HostStateCommit {
    const KIND: MessageKind = MessageKind::HostStateCommit;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for LeaseRequest {
    const KIND: MessageKind = MessageKind::LeaseRequest;
}
impl Payload for LeaseAck {
    const KIND: MessageKind = MessageKind::LeaseAck;
}
impl Payload for DisplayAck {
    const KIND: MessageKind = MessageKind::DisplayAck;
}
impl Payload for ResumeEdits {
    const KIND: MessageKind = MessageKind::ResumeEdits;
}
impl Payload for StateProjection {
    const KIND: MessageKind = MessageKind::StateProjection;
    fn validate_wire(&self) -> Result<()> {
        self.window.validate()
    }
}
impl Payload for ResyncRequired {
    const KIND: MessageKind = MessageKind::ResyncRequired;
}
impl Payload for Detach {
    const KIND: MessageKind = MessageKind::Detach;
}
impl Payload for Pause {
    const KIND: MessageKind = MessageKind::Pause;
}
impl Payload for Resume {
    const KIND: MessageKind = MessageKind::Resume;
}
impl Payload for Unload {
    const KIND: MessageKind = MessageKind::Unload;
}
impl Payload for PausedAck {
    const KIND: MessageKind = MessageKind::PausedAck;
}
impl Payload for StopAck {
    const KIND: MessageKind = MessageKind::StopAck;
}
impl Payload for StopRequested {
    const KIND: MessageKind = MessageKind::StopRequested;
}
impl Payload for BgGrantIssue {
    const KIND: MessageKind = MessageKind::BgGrantIssue;
}
impl Payload for BgGrantAck {
    const KIND: MessageKind = MessageKind::BgGrantAck;
}
impl Payload for BgGrantList {
    const KIND: MessageKind = MessageKind::BgGrantList;
}
impl Payload for BgGrantVerdict {
    const KIND: MessageKind = MessageKind::BgGrantVerdict;
}
impl Payload for DeviceRequest {
    const KIND: MessageKind = MessageKind::DeviceRequest;
    fn validate_wire(&self) -> Result<()> {
        self.validate()
    }
}
impl Payload for DeviceReply {
    const KIND: MessageKind = MessageKind::DeviceReply;
}
