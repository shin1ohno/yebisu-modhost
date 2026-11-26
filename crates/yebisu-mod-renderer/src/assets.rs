//! Asset delivery: an `AssetReply` header names the data channel and the
//! bytes follow as raw chunks. Transfers are counted at copy, host and
//! application scope with the negotiated values.
use crate::adopt::{Outstanding, Scope};
use crate::{CopyReceiver, Error, Result};
use yebisu_mod_wire as wire;

/// Ledgers shared beyond one copy: `host` by every copy and connection of one
/// host, `app` by every host. The owner keeps them for the application's
/// lifetime and lends both to each call.
pub struct SharedScopes<'a> {
    pub host: &'a mut wire::ResourceLedger,
    pub app: &'a mut wire::ResourceLedger,
}

impl CopyReceiver {
    /// Register an asset request for an asset of the adopted copy.
    pub fn request_asset(
        &mut self,
        request: wire::AssetRequest,
        scopes: &mut SharedScopes<'_>,
    ) -> Result<()> {
        let adopted = self.adopted.as_ref().ok_or(Error::NotAdopted)?;
        let manifest = adopted
            .asset_manifests
            .iter()
            .find(|a| a.id == request.asset.id && a.digest == request.asset.digest)
            .ok_or(Error::NotInAdoptedCopy("asset"))?;
        self.limits.check_asset(manifest)?;
        if self.assets.contains_key(&request.request_id) {
            return Err(Error::DuplicateRequest(request.request_id));
        }
        let reserved = self.reserve(
            &[
                (Scope::Copy, wire::Resource::AssetTransfersCopy),
                (Scope::Host, wire::Resource::AssetTransfersHost),
                (Scope::App, wire::Resource::AssetTransfersApp),
            ],
            scopes,
        )?;
        self.assets.insert(
            request.request_id.clone(),
            Outstanding { request, reserved },
        );
        Ok(())
    }
    /// Accept the reply header before any chunk: envelope identity, then the
    /// outstanding request and manifest, then the negotiated size.
    pub fn begin_asset(&mut self, reply: &wire::Envelope<wire::AssetReply>) -> Result<()> {
        self.context.validate(reply)?;
        let id = &reply.body.request_id;
        let outstanding = self
            .assets
            .get(id)
            .ok_or_else(|| Error::UnknownRequest(id.clone()))?;
        if self.asset_transfers.contains_key(id) {
            return Err(Error::DuplicateRequest(id.clone()));
        }
        let manifest = self
            .adopted
            .as_ref()
            .ok_or(Error::NotAdopted)?
            .asset_manifests
            .iter()
            .find(|a| a.id == outstanding.request.asset.id)
            .ok_or(Error::NotInAdoptedCopy("asset"))?;
        self.limits.check_asset_reply(&reply.body, manifest)?;
        let assembler = wire::AssetAssembler::new(&reply.body, &outstanding.request, manifest)?;
        self.asset_transfers.insert(id.clone(), assembler);
        Ok(())
    }
    pub fn push_asset_frame(&mut self, request_id: &str, frame: &wire::Frame) -> Result<()> {
        let transfer = self
            .asset_transfers
            .get_mut(request_id)
            .ok_or(Error::NoTransfer)?;
        Ok(transfer.push_frame(frame)?)
    }
    /// Release the bytes once size and SHA-256 match the manifest, and return
    /// the transfer's reservations. A failed transfer also ends the request.
    pub fn finish_asset(
        &mut self,
        request_id: &str,
        scopes: &mut SharedScopes<'_>,
    ) -> Result<Vec<u8>> {
        let transfer = self
            .asset_transfers
            .remove(request_id)
            .ok_or(Error::NoTransfer)?;
        let result = transfer.finish();
        if let Some(o) = self.assets.remove(request_id) {
            self.release(&o.reserved, scopes)?;
        }
        Ok(result?)
    }
    pub fn outstanding_assets(&self) -> usize {
        self.assets.len()
    }
}
