//! Reflection-table digests for the Hello/HelloAck exact-match check.
use crate::*;
use serde_json::json;

impl ValidationTables {
    /// The canonical digests both peers compare in the handshake. The style,
    /// op and command digests cover this side's tables; the component and
    /// entity sets are closed by schema `e1`, so their digests bind the schema.
    pub fn digests(&self) -> Result<Digests> {
        let styles: Vec<_> = self.styles.iter().collect();
        Ok(Digests {
            style: canonical_digest(
                &json!({"styles": styles, "param_styles": self.param_styles}),
                "",
            )?,
            component: canonical_digest(&json!({"schema": SCHEMA, "table": "component"}), "")?,
            op: canonical_digest(
                &json!({"methods": self.methods, "callbacks": self.callbacks, "slots": self.slots}),
                "",
            )?,
            entity: canonical_digest(&json!({"schema": SCHEMA, "table": "entity"}), "")?,
            command: canonical_digest(&json!({"actions": self.actions}), "")?,
        })
    }
}
