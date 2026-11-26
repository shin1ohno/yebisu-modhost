//! Row delivery (A7) against the adopted copy, counted at list, copy and
//! application scope with the negotiated values.
use crate::adopt::{Outstanding, Scope, has_list};
use crate::{CopyReceiver, Error, Result, SharedScopes};
use yebisu_mod_wire as wire;

impl CopyReceiver {
    /// Register a row request this renderer is about to send. It must name a
    /// list node of the adopted copy, and every scope must have room.
    pub fn request_rows(
        &mut self,
        request: wire::RowsRequest,
        scopes: &mut SharedScopes<'_>,
    ) -> Result<()> {
        request.validate()?;
        self.limits.check_rows_request(&request)?;
        let adopted = self.adopted.as_ref().ok_or(Error::NotAdopted)?;
        if !has_list(adopted, &request) {
            return Err(Error::NotInAdoptedCopy("list"));
        }
        if self.rows.contains_key(&request.request_id) {
            return Err(Error::DuplicateRequest(request.request_id));
        }
        let list = Scope::List(request.view_generation, request.list_id.clone());
        let reserved = self.reserve(
            &[
                (list, wire::Resource::RowRequestsList),
                (Scope::Copy, wire::Resource::RowRequestsCopy),
                (Scope::App, wire::Resource::RowRequestsApp),
            ],
            scopes,
        )?;
        self.rows.insert(
            request.request_id.clone(),
            Outstanding { request, reserved },
        );
        Ok(())
    }
    /// Accept a row reply: authenticate the envelope, then match the
    /// outstanding request, then validate against the adopted copy through
    /// `RowValidation`, then apply the negotiated row limits. The request is
    /// settled (and its reservations returned) only when the reply is accepted.
    pub fn receive_rows(
        &mut self,
        reply: &wire::Envelope<wire::RowsReply>,
        scopes: &mut SharedScopes<'_>,
    ) -> Result<wire::RowsReply> {
        self.context.validate(reply)?;
        let id = &reply.body.request_id;
        let outstanding = self
            .rows
            .get(id)
            .ok_or_else(|| Error::UnknownRequest(id.clone()))?;
        let adopted = self.adopted.as_ref().ok_or(Error::NotAdopted)?;
        let manifest = adopted
            .list_manifests
            .iter()
            .find(|m| {
                m.handler_generation == outstanding.request.view_generation
                    && m.list_id == outstanding.request.list_id
            })
            .ok_or(Error::NotInAdoptedCopy("list"))?;
        let context = wire::RowValidation::new(adopted, &self.tables)?;
        reply
            .body
            .validate_against(&outstanding.request, manifest, &context)?;
        self.limits.check_rows_reply(&reply.body)?;
        if let Some(settled) = self.rows.remove(id) {
            self.release(&settled.reserved, scopes)?;
        }
        Ok(reply.body.clone())
    }
    /// Give up on an outstanding row request (timeout or viewport change).
    pub fn cancel_rows(&mut self, request_id: &str, scopes: &mut SharedScopes<'_>) -> Result<()> {
        let o = self
            .rows
            .remove(request_id)
            .ok_or_else(|| Error::UnknownRequest(request_id.into()))?;
        self.release(&o.reserved, scopes)
    }
    pub fn outstanding_rows(&self) -> usize {
        self.rows.len()
    }
}
