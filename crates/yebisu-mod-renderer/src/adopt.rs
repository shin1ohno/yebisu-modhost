//! One copy's receiving state: publication transfer, validation and atomic
//! adoption. Rows and assets (in their own modules) borrow the adopted copy.
use crate::{Error, Result, SharedScopes};
use std::collections::BTreeMap;
use yebisu_mod_wire as wire;

/// The separately negotiated decompressor for publication bytes. It must stop
/// producing output past `limit` bytes (use `wire::read_bounded`).
pub trait Inflate {
    fn inflate(&self, bytes: &[u8], limit: usize) -> wire::Result<Vec<u8>>;
}
/// Protocol 1 enables no compression format implicitly.
pub struct Uncompressed;
impl Inflate for Uncompressed {
    fn inflate(&self, bytes: &[u8], limit: usize) -> wire::Result<Vec<u8>> {
        wire::read_bounded(bytes, limit)
    }
}

pub(crate) struct Outstanding<T> {
    pub request: T,
    pub reserved: Vec<(Scope, wire::Resource)>,
}
/// Which ledger a reservation was taken from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Scope {
    List(wire::Counter, String),
    Copy,
    Host,
    App,
}

pub struct CopyReceiver {
    pub(crate) context: wire::ReceiveContext,
    pub(crate) limits: wire::NegotiatedLimits,
    pub(crate) tables: wire::ValidationTables,
    floors: wire::ReceiptFloors,
    generations: wire::GenerationTracker,
    tombstones: Vec<wire::EntityRelease>,
    pub(crate) adopted: Option<wire::Publish>,
    transfer: Option<wire::PublishAssembler>,
    pub(crate) copy_ledger: wire::ResourceLedger,
    pub(crate) list_ledgers: BTreeMap<(wire::Counter, String), wire::ResourceLedger>,
    pub(crate) rows: BTreeMap<String, Outstanding<wire::RowsRequest>>,
    pub(crate) assets: BTreeMap<String, Outstanding<wire::AssetRequest>>,
    pub(crate) asset_transfers: BTreeMap<String, wire::AssetAssembler>,
}

impl CopyReceiver {
    pub fn new(
        context: wire::ReceiveContext,
        limits: wire::NegotiatedLimits,
        tables: wire::ValidationTables,
    ) -> Result<Self> {
        Ok(Self {
            copy_ledger: wire::ResourceLedger::negotiated(&limits)?,
            context,
            limits,
            tables,
            floors: wire::ReceiptFloors::default(),
            generations: wire::GenerationTracker::default(),
            tombstones: Vec::new(),
            adopted: None,
            transfer: None,
            list_ledgers: BTreeMap::new(),
            rows: BTreeMap::new(),
            assets: BTreeMap::new(),
            asset_transfers: BTreeMap::new(),
        })
    }
    pub fn adopted(&self) -> Option<&wire::Publish> {
        self.adopted.as_ref()
    }
    pub fn generation_high_watermark(&self) -> wire::Counter {
        self.generations.high_watermark()
    }
    pub fn context(&self) -> &wire::ReceiveContext {
        &self.context
    }

    /// Check one encoded control message before it is decoded.
    pub fn check_control(&self, bytes: &[u8]) -> Result<()> {
        Ok(self.limits.check_control(bytes.len())?)
    }
    /// Start a publication transfer. A new header replaces (drops) any
    /// unfinished one; nothing partial is ever adopted.
    pub fn begin_publish(&mut self, begin: &wire::Envelope<wire::PublishBegin>) -> Result<()> {
        self.transfer = None;
        self.context.validate(begin)?;
        self.limits.check_publish_begin(&begin.body)?;
        self.transfer = Some(wire::PublishAssembler::new(begin.body.clone())?);
        Ok(())
    }
    pub fn push_publish_frame(&mut self, frame: &wire::Frame) -> Result<()> {
        let transfer = self.transfer.as_mut().ok_or(Error::NoTransfer)?;
        if let Err(e) = transfer.push_frame(frame) {
            self.transfer = None;
            return Err(e.into());
        }
        Ok(())
    }
    /// Finish the transfer, inflate within the negotiated bound and adopt.
    pub fn commit_publish(
        &mut self,
        commit: &wire::Envelope<wire::PublishCommit>,
        inflate: &dyn Inflate,
        scopes: &mut SharedScopes<'_>,
    ) -> Result<&wire::Publish> {
        let transfer = self.transfer.take().ok_or(Error::NoTransfer)?;
        self.context.validate(commit)?;
        let bytes = transfer.finish(commit.body.clone())?;
        let json = inflate.inflate(&bytes, self.limits.decompressed_bytes)?;
        self.limits.check_decompressed(json.len())?;
        self.adopt_bytes(&json, scopes)
    }
    /// Validate a complete publication envelope and adopt it atomically: the
    /// generation tracker, floors and adopted copy change only on success.
    pub fn adopt_bytes(
        &mut self,
        json: &[u8],
        scopes: &mut SharedScopes<'_>,
    ) -> Result<&wire::Publish> {
        self.limits.check_decompressed(json.len())?;
        let message = wire::decode_publish(
            json,
            &wire::PublishValidation {
                tables: &self.tables,
                context: &self.context,
                floors: &self.floors,
                generations: &self.generations,
                tombstones: &self.tombstones,
            },
        )?;
        self.limits.check_publish(&message.body)?;
        let mut generations = self.generations.clone();
        generations.commit(&message.body)?;
        self.generations = generations;
        self.floors.displayed_pub = message.body.pub_rev;
        self.adopted = Some(message.body);
        self.drop_stale_requests(scopes)?;
        self.adopted.as_ref().ok_or(Error::NotAdopted)
    }
    /// Raise floors from a terminal receipt (never from OpRejected).
    pub fn record_receipt(&mut self, wm: &wire::OpWatermark, entities: &[wire::EntityResult]) {
        self.floors.raise(wm, entities);
    }
    /// Record a validated release so the entity can never reappear.
    pub fn record_release(&mut self, release: wire::EntityRelease) -> Result<()> {
        let adopted = self.adopted.as_ref().ok_or(Error::NotAdopted)?;
        let entity = adopted
            .entities_full
            .iter()
            .find(|e| e.r#ref == release.r#ref)
            .ok_or(Error::NotInAdoptedCopy("entity"))?;
        release.validate_against(entity)?;
        self.tombstones.push(release);
        Ok(())
    }

    /// A new adoption ends every request the old copy owned that the new one
    /// cannot answer, and returns its reservations.
    fn drop_stale_requests(&mut self, scopes: &mut SharedScopes<'_>) -> Result<()> {
        let Some(adopted) = &self.adopted else {
            return Ok(());
        };
        let stale_rows: Vec<String> = self
            .rows
            .iter()
            .filter(|(_, o)| !has_list(adopted, &o.request))
            .map(|(id, _)| id.clone())
            .collect();
        let stale_assets: Vec<String> = self
            .assets
            .iter()
            .filter(|(_, o)| {
                !adopted
                    .asset_manifests
                    .iter()
                    .any(|a| a.id == o.request.asset.id && a.digest == o.request.asset.digest)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale_rows {
            if let Some(o) = self.rows.remove(&id) {
                self.release(&o.reserved, scopes)?;
            }
        }
        for id in stale_assets {
            self.asset_transfers.remove(&id);
            if let Some(o) = self.assets.remove(&id) {
                self.release(&o.reserved, scopes)?;
            }
        }
        Ok(())
    }

    /// Take one unit at every scope in order; on any failure return the units
    /// already taken, so a refused request leaves every ledger unchanged.
    pub(crate) fn reserve(
        &mut self,
        wanted: &[(Scope, wire::Resource)],
        scopes: &mut SharedScopes<'_>,
    ) -> Result<Vec<(Scope, wire::Resource)>> {
        let mut taken = Vec::new();
        for (scope, resource) in wanted {
            let result = match scope {
                Scope::Copy => self.copy_ledger.reserve(*resource, 1),
                Scope::Host => scopes.host.reserve(*resource, 1),
                Scope::App => scopes.app.reserve(*resource, 1),
                Scope::List(generation, list) => {
                    let ledger = match self.list_ledgers.entry((*generation, list.clone())) {
                        std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                        std::collections::btree_map::Entry::Vacant(e) => {
                            e.insert(wire::ResourceLedger::negotiated(&self.limits)?)
                        }
                    };
                    ledger.reserve(*resource, 1)
                }
            };
            if let Err(e) = result {
                self.release(&taken, scopes)?;
                return Err(e.into());
            }
            taken.push((scope.clone(), *resource));
        }
        Ok(taken)
    }
    pub(crate) fn release(
        &mut self,
        reserved: &[(Scope, wire::Resource)],
        scopes: &mut SharedScopes<'_>,
    ) -> Result<()> {
        for (scope, resource) in reserved {
            match scope {
                Scope::Copy => self.copy_ledger.release(*resource, 1)?,
                Scope::Host => scopes.host.release(*resource, 1)?,
                Scope::App => scopes.app.release(*resource, 1)?,
                Scope::List(generation, list) => {
                    if let Some(ledger) = self.list_ledgers.get_mut(&(*generation, list.clone())) {
                        ledger.release(*resource, 1)?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn has_list(publish: &wire::Publish, request: &wire::RowsRequest) -> bool {
    publish
        .list_manifests
        .iter()
        .any(|m| m.handler_generation == request.view_generation && m.list_id == request.list_id)
        && publish.views_full.iter().any(|v| {
            v.generation == request.view_generation
                && v.nodes.iter().any(|n| {
                    matches!(
                        n.component,
                        wire::Component::List | wire::Component::VirtualList
                    ) && n.list_id.as_ref() == Some(&request.list_id)
                })
        })
}
