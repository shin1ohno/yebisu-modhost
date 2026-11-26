//! Shared counters for resources whose ownership spans messages (A8).
use crate::*;
use std::collections::BTreeMap;

/// One counted resource. Scope-specific ledgers must be shared across all
/// copies/hosts at that scope, not per socket.
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
    /// The fixed A8 ceiling.
    pub fn limit(self) -> usize {
        self.negotiated(&NegotiatedLimits::default())
    }
    /// The value a session negotiated for this resource.
    pub fn negotiated(self, limits: &NegotiatedLimits) -> usize {
        match self {
            Self::HostCopies => limits.copies_host,
            Self::AppCopies => limits.copies_app,
            Self::CallbackGenerations => limits.callback_generations,
            Self::Callbacks => limits.callbacks,
            Self::Events => limits.event_queue,
            Self::ControlPending => limits.control_pending_bytes,
            Self::ControlPerSecond => limits.control_per_second,
            Self::Receipts => limits.receipts,
            Self::ReceiptBytes => limits.receipts_bytes,
            Self::RowRequestsList => limits.row_requests_list,
            Self::RowRequestsCopy => limits.row_requests_copy,
            Self::RowRequestsApp => limits.row_requests_app,
            Self::RowCacheBytes => limits.row_cache_bytes,
            Self::AssetCacheBytes => limits.asset_cache_bytes,
            Self::AssetTransfersCopy => limits.asset_transfers_copy,
            Self::AssetTransfersHost => limits.asset_transfers_host,
            Self::AssetTransfersApp => limits.asset_transfers_app,
            Self::AssetDecodedBytes => limits.asset_decoded_bytes,
            Self::AssetsDecodedBytes => limits.assets_decoded_bytes,
            Self::AssetsPixels => limits.assets_pixels,
            Self::AppBytes => limits.app_bytes,
            Self::JsHeapBytes => limits.js_heap_bytes,
            Self::WorkerRssBytes => limits.worker_rss_bytes,
            Self::WorkersRssBytes => limits.workers_rss_bytes,
            Self::ChildrenCopy => limits.children_copy,
            Self::ChildrenHost => limits.children_host,
            Self::BackgroundCopies => limits.background_copies,
        }
    }
}

/// Reserve before allocating. `default()` uses the A8 ceilings; a receiver that
/// negotiated smaller values builds the ledger with `negotiated`, so every
/// reservation at that scope is bounded by the session's values.
#[derive(Debug, Default)]
pub struct ResourceLedger {
    used: BTreeMap<Resource, usize>,
    limits: NegotiatedLimits,
}
impl ResourceLedger {
    pub fn negotiated(limits: &NegotiatedLimits) -> Result<Self> {
        limits.validate()?;
        Ok(Self {
            used: BTreeMap::new(),
            limits: limits.clone(),
        })
    }
    pub fn limit(&self, r: Resource) -> usize {
        r.negotiated(&self.limits)
    }
    pub fn used(&self, r: Resource) -> usize {
        self.used.get(&r).copied().unwrap_or(0)
    }
    pub fn reserve(&mut self, r: Resource, amount: usize) -> Result<()> {
        let next = self
            .used(r)
            .checked_add(amount)
            .ok_or(Error::Limit("resource overflow"))?;
        ceiling(next, self.limit(r), "resource budget")?;
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
