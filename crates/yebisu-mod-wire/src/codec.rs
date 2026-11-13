use crate::*;
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub v: u32,
    pub schema: String,
    pub session: Session,
    pub scope: Scope,
    pub owner_fence: Counter,
    pub msg_id: String,
    pub seq: Counter,
    #[serde(rename = "type")]
    pub message_type: MessageKind,
    pub body: T,
}
impl<T: Payload> Envelope<T> {
    pub fn new(
        session: Session,
        scope: Scope,
        owner_fence: Counter,
        msg_id: String,
        seq: Counter,
        body: T,
    ) -> Self {
        Self {
            v: PROTOCOL,
            schema: SCHEMA.into(),
            session,
            scope,
            owner_fence,
            msg_id,
            seq,
            message_type: T::KIND,
            body,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.v != PROTOCOL || self.schema != SCHEMA {
            return Err(Error::Unknown("protocol/schema"));
        }
        if self.message_type != T::KIND {
            return Err(Error::Invalid("body/type mismatch"));
        }
        if self.seq.0 == 0
            || self.msg_id.is_empty()
            || self.session.host.is_empty()
            || self.session.client.is_empty()
        {
            return Err(Error::Invalid("envelope identity"));
        }
        self.body.validate_wire()
    }
}
fn message_limit(kind: MessageKind) -> usize {
    match kind {
        MessageKind::Publish => limits::PUBLISH_BYTES,
        MessageKind::RowsReply => limits::ROWS_BYTES,
        MessageKind::AssetReply => limits::IMAGE_BYTES + limits::CONTROL_BYTES,
        _ => limits::CONTROL_BYTES,
    }
}
pub fn encode<T: Payload>(message: &Envelope<T>) -> Result<Vec<u8>> {
    message.validate()?;
    let bytes = serde_json::to_vec(message)?;
    if bytes.len() > message_limit(T::KIND) {
        return Err(Error::Limit("message bytes"));
    }
    // Check that serialization did not silently turn a non-finite float into null.
    let _: Envelope<T> = decode(&bytes)?;
    Ok(bytes)
}
pub fn decode<T: Payload>(bytes: &[u8]) -> Result<Envelope<T>> {
    if bytes.len() > message_limit(T::KIND) {
        return Err(Error::Limit("message bytes"));
    }
    let value = strict_json(bytes)?;
    let message: Envelope<T> = serde_json::from_value(value)?;
    message.validate()?;
    Ok(message)
}

/// Parse bounded JSON while retaining duplicate-key detection at every depth.
pub fn strict_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > limits::IMAGE_BYTES + limits::CONTROL_BYTES {
        return Err(Error::Limit("JSON bytes"));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = JsonSeed(0).deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}
struct JsonSeed(usize);
impl<'de> DeserializeSeed<'de> for JsonSeed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<Value, D::Error> {
        if self.0 > limits::JSON_DEPTH {
            return Err(de::Error::custom("JSON nesting limit"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for JsonSeed {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        if v.len() > limits::STRING_BYTES {
            return Err(E::custom("string limit"));
        }
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        self.visit_str(&v)
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(v) = a.next_element_seed(JsonSeed(self.0 + 1))? {
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        let mut keys = BTreeSet::new();
        while let Some(k) = a.next_key::<String>()? {
            if k.len() > limits::STRING_BYTES || !keys.insert(k.clone()) {
                return Err(de::Error::custom("duplicate/oversized key"));
            }
            let v = a.next_value_seed(JsonSeed(self.0 + 1))?;
            values.insert(k, v);
        }
        Ok(Value::Object(values))
    }
}

/// Stable bootstrap fields may be read even when the rest of HelloAck is incompatible.
#[derive(Debug, Clone, PartialEq)]
pub struct HelloAckBootstrap {
    pub nonce: Id128,
    pub protocol: u32,
    pub serve: ServeVersion,
}
pub fn decode_hello_ack_bootstrap(bytes: &[u8]) -> Result<HelloAckBootstrap> {
    if bytes.len() > limits::CONTROL_BYTES {
        return Err(Error::Limit("HelloAck"));
    }
    let value = strict_json(bytes)?;
    let field = |name: &str| {
        value
            .get(name)
            .cloned()
            .ok_or(Error::Invalid("HelloAck bootstrap"))
    };
    Ok(HelloAckBootstrap {
        nonce: serde_json::from_value(field("nonce")?)?,
        protocol: serde_json::from_value(field("protocol")?)?,
        serve: serde_json::from_value(field("serve")?)?,
    })
}

/// SHA-256 of sorted-key, compact UTF-8 JSON, with the selected top-level field omitted.
/// Strings and bytes are not Unicode-normalized; counters remain decimal strings.
pub fn canonical_digest<T: Serialize>(value: &T, omit: &str) -> Result<Digest> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(value)?;
    let mut value = strict_json(&bytes)?;
    let map = value
        .as_object_mut()
        .ok_or(Error::Invalid("digest object"))?;
    map.remove(omit);
    let bytes = serde_json::to_vec(&value)?;
    Digest::new(format!("{:x}", Sha256::digest(bytes)))
}
pub fn bytes_digest(bytes: &[u8]) -> Digest {
    use sha2::{Digest as _, Sha256};
    Digest::new(format!("{:x}", Sha256::digest(bytes))).expect("SHA-256 is 64 lowercase hex digits")
}

/// Validate a complete publication against both schema and negotiated tables,
/// receiving identity, receipt floors and the copy's generation history.
pub struct PublishValidation<'a> {
    pub tables: &'a ValidationTables,
    pub context: &'a ReceiveContext,
    pub floors: &'a ReceiptFloors,
    pub generations: &'a GenerationTracker,
    pub tombstones: &'a [EntityRelease],
}
pub fn decode_publish(
    bytes: &[u8],
    validation: &PublishValidation<'_>,
) -> Result<Envelope<Publish>> {
    let message: Envelope<Publish> = decode(bytes)?;
    validation.context.validate(&message)?;
    message.body.validate(validation.tables)?;
    validation
        .floors
        .check(&message.body, validation.tombstones)?;
    validation.generations.check(&message.body)?;
    Ok(message)
}
pub fn encode_publish(message: &Envelope<Publish>, tables: &ValidationTables) -> Result<Vec<u8>> {
    message.body.validate(tables)?;
    encode(message)
}
