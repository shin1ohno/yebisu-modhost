use crate::*;
use std::io::{Read, Write};
pub const FRAME_HEADER_BYTES: usize = 9;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    Data = 0,
    Credit = 1,
    Close = 2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub channel: u32,
    pub kind: FrameKind,
    pub len: u32,
}
impl FrameHeader {
    pub fn decode(bytes: &[u8; FRAME_HEADER_BYTES]) -> Result<Self> {
        let header = Self {
            channel: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            kind: match bytes[4] {
                0 => FrameKind::Data,
                1 => FrameKind::Credit,
                2 => FrameKind::Close,
                _ => return Err(Error::Unknown("frame kind")),
            },
            len: u32::from_le_bytes(bytes[5..9].try_into().unwrap()),
        };
        header.validate()?;
        Ok(header)
    }
    pub fn validate(&self) -> Result<()> {
        let max = if self.channel == 0 {
            limits::CONTROL_BYTES
        } else {
            limits::CHUNK_BYTES
        };
        if self.len as usize > max {
            return Err(Error::Limit("frame bytes"));
        }
        if self.kind == FrameKind::Credit && self.len != 4 {
            return Err(Error::Invalid("credit length"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<[u8; FRAME_HEADER_BYTES]> {
        self.validate()?;
        let mut out = [0; FRAME_HEADER_BYTES];
        out[0..4].copy_from_slice(&self.channel.to_le_bytes());
        out[4] = self.kind as u8;
        out[5..9].copy_from_slice(&self.len.to_le_bytes());
        Ok(out)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel: u32,
    pub kind: FrameKind,
    pub payload: Vec<u8>,
}
impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let len =
            u32::try_from(self.payload.len()).map_err(|_| Error::Limit("frame length overflow"))?;
        let mut out = FrameHeader {
            channel: self.channel,
            kind: self.kind,
            len,
        }
        .encode()?
        .to_vec();
        out.extend_from_slice(&self.payload);
        Ok(out)
    }
    /// Exact one-frame decoding; truncation and trailing bytes are errors.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let head = bytes
            .get(..FRAME_HEADER_BYTES)
            .ok_or(Error::Invalid("short frame"))?;
        let header = FrameHeader::decode(head.try_into().unwrap())?;
        if bytes.len() != FRAME_HEADER_BYTES + header.len as usize {
            return Err(Error::Invalid("frame length mismatch"));
        }
        Ok(Self {
            channel: header.channel,
            kind: header.kind,
            payload: bytes[FRAME_HEADER_BYTES..].to_vec(),
        })
    }
    /// Header is validated before any payload allocation. Credit is owned by the caller.
    pub fn read_from(mut reader: impl Read) -> Result<Self> {
        let mut bytes = [0; FRAME_HEADER_BYTES];
        reader
            .read_exact(&mut bytes)
            .map_err(|e| Error::Io(e.to_string()))?;
        let header = FrameHeader::decode(&bytes)?;
        let mut payload = vec![0; header.len as usize];
        reader
            .read_exact(&mut payload)
            .map_err(|e| Error::Io(e.to_string()))?;
        Ok(Self {
            channel: header.channel,
            kind: header.kind,
            payload,
        })
    }
    pub fn write_to(&self, mut writer: impl Write) -> Result<()> {
        writer
            .write_all(&self.encode()?)
            .map_err(|e| Error::Io(e.to_string()))
    }
}
#[derive(Debug, Clone)]
pub struct Credit {
    available: usize,
}
impl Default for Credit {
    fn default() -> Self {
        Self {
            available: limits::CREDIT_BYTES,
        }
    }
}
impl Credit {
    pub fn available(&self) -> usize {
        self.available
    }
    pub fn consume(&mut self, bytes: usize) -> Result<()> {
        self.available = self
            .available
            .checked_sub(bytes)
            .ok_or(Error::Limit("credit exhausted"))?;
        Ok(())
    }
    pub fn replenish(&mut self, bytes: usize) -> Result<()> {
        let next = self
            .available
            .checked_add(bytes)
            .ok_or(Error::Limit("credit overflow"))?;
        if next > limits::CREDIT_BYTES {
            return Err(Error::Limit("credit exceeds reservation"));
        }
        self.available = next;
        Ok(())
    }
}
/// `[index u32 LE]` prefix of every data chunk frame payload.
pub const DATA_CHUNK_HEADER_BYTES: usize = 4;
/// Largest chunk body: the chunk and its index fit one data-channel frame.
pub const DATA_CHUNK_BYTES: usize = limits::CHUNK_BYTES - DATA_CHUNK_HEADER_BYTES;
impl DataChunk {
    /// One DATA frame on `channel` (never 0, which carries JSON control messages).
    pub fn to_frame(&self, channel: u32) -> Result<Frame> {
        data_channel(channel)?;
        chunk_body(&self.bytes)?;
        let mut payload = Vec::with_capacity(DATA_CHUNK_HEADER_BYTES + self.bytes.len());
        payload.extend_from_slice(&self.index.to_le_bytes());
        payload.extend_from_slice(&self.bytes);
        Ok(Frame {
            channel,
            kind: FrameKind::Data,
            payload,
        })
    }
    pub fn from_frame(frame: &Frame) -> Result<Self> {
        data_channel(frame.channel)?;
        if frame.kind != FrameKind::Data {
            return Err(Error::Invalid("chunk frame kind"));
        }
        let (index, bytes) = frame
            .payload
            .split_at_checked(DATA_CHUNK_HEADER_BYTES)
            .ok_or(Error::Invalid("short chunk frame"))?;
        chunk_body(bytes)?;
        Ok(Self {
            index: u32::from_le_bytes(index.try_into().unwrap()),
            bytes: bytes.to_vec(),
        })
    }
}
fn data_channel(channel: u32) -> Result<()> {
    if channel == 0 {
        return Err(Error::Invalid("data chunk on control channel"));
    }
    Ok(())
}
fn chunk_body(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > DATA_CHUNK_BYTES {
        return Err(Error::Limit("chunk bytes"));
    }
    Ok(())
}
/// Reassembles numbered raw chunks into one reserved buffer and releases the bytes
/// only when the count, the total and the SHA-256 all match.
#[derive(Debug)]
struct ChunkAssembler {
    channel: u32,
    total_bytes: usize,
    chunk_count: usize,
    digest: Digest,
    next: usize,
    bytes: Vec<u8>,
}
impl ChunkAssembler {
    fn new(
        channel: u32,
        total_bytes: usize,
        chunk_count: usize,
        digest: Digest,
        ceiling: usize,
        what: &'static str,
    ) -> Result<Self> {
        data_channel(channel)?;
        if total_bytes == 0 || total_bytes > ceiling {
            return Err(Error::Limit(what));
        }
        if chunk_count == 0
            || chunk_count > total_bytes
            || chunk_count < total_bytes.div_ceil(DATA_CHUNK_BYTES)
            || chunk_count > u32::MAX as usize
        {
            return Err(Error::Invalid("chunk count"));
        }
        Ok(Self {
            channel,
            total_bytes,
            chunk_count,
            digest,
            next: 0,
            bytes: Vec::with_capacity(total_bytes),
        })
    }
    fn push(&mut self, index: usize, bytes: &[u8]) -> Result<()> {
        if index != self.next || self.next >= self.chunk_count {
            return Err(Error::Invalid("chunk order"));
        }
        chunk_body(bytes)?;
        if self.bytes.len().saturating_add(bytes.len()) > self.total_bytes {
            return Err(Error::Limit("assembly overflow"));
        }
        self.bytes.extend_from_slice(bytes);
        self.next += 1;
        Ok(())
    }
    fn push_frame(&mut self, frame: &Frame) -> Result<()> {
        if frame.channel != self.channel {
            return Err(Error::Invalid("chunk channel"));
        }
        let chunk = DataChunk::from_frame(frame)?;
        self.push(chunk.index as usize, &chunk.bytes)
    }
    fn finish(self, what: &'static str) -> Result<Vec<u8>> {
        if self.next != self.chunk_count || self.bytes.len() != self.total_bytes {
            return Err(Error::Invalid(what));
        }
        if bytes_digest(&self.bytes) != self.digest {
            return Err(Error::DigestMismatch);
        }
        Ok(self.bytes)
    }
}
/// One assembly per copy. The caller drops this at the five-second deadline,
/// Pause, reconnect, or replacement. This type never exposes partial bytes.
#[derive(Debug)]
pub struct PublishAssembler {
    begin: PublishBegin,
    inner: ChunkAssembler,
}
impl PublishAssembler {
    pub fn new(begin: PublishBegin) -> Result<Self> {
        let inner = ChunkAssembler::new(
            begin.channel,
            begin.total_bytes,
            begin.chunk_count,
            begin.digest.clone(),
            limits::PUBLISH_BYTES,
            "publish assembly bytes",
        )
        .map_err(|e| match e {
            Error::Invalid("chunk count") => Error::Invalid("publish chunk count"),
            e => e,
        })?;
        Ok(Self { begin, inner })
    }
    pub fn push(&mut self, index: usize, bytes: &[u8]) -> Result<()> {
        self.inner.push(index, bytes)
    }
    /// Accept one DATA frame from the channel named by `PublishBegin.channel`.
    pub fn push_frame(&mut self, frame: &Frame) -> Result<()> {
        self.inner.push_frame(frame)
    }
    pub fn finish(self, commit: PublishCommit) -> Result<Vec<u8>> {
        if self.inner.next != self.inner.chunk_count
            || self.inner.bytes.len() != self.inner.total_bytes
        {
            return Err(Error::Invalid("incomplete publish"));
        }
        if commit.digest != self.begin.digest {
            return Err(Error::DigestMismatch);
        }
        self.inner.finish("incomplete publish")
    }
}
/// Reassembles an `AssetReply` body from its data channel. The reply header is
/// checked against the request and manifest first; the bytes are released only
/// when they match the manifest's size and digest.
#[derive(Debug)]
pub struct AssetAssembler {
    inner: ChunkAssembler,
}
impl AssetAssembler {
    pub fn new(
        reply: &AssetReply,
        request: &AssetRequest,
        manifest: &AssetManifest,
    ) -> Result<Self> {
        reply.validate_against(request, manifest)?;
        let ceiling = if manifest.kind == AssetKind::Svg {
            limits::SVG_BYTES
        } else {
            limits::IMAGE_BYTES
        };
        Ok(Self {
            inner: ChunkAssembler::new(
                reply.channel,
                reply.total_bytes,
                reply.chunk_count,
                manifest.digest.clone(),
                ceiling,
                "encoded asset bytes",
            )?,
        })
    }
    pub fn push(&mut self, index: usize, bytes: &[u8]) -> Result<()> {
        self.inner.push(index, bytes)
    }
    pub fn push_frame(&mut self, frame: &Frame) -> Result<()> {
        self.inner.push_frame(frame)
    }
    pub fn finish(self) -> Result<Vec<u8>> {
        self.inner.finish("incomplete asset")
    }
}
/// Apply to a decompression reader *before* parsing. Reads at most limit+1 bytes.
/// No compression format is enabled implicitly by protocol 1.
pub fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    if limit > limits::PUBLISH_DECOMPRESSED_BYTES {
        return Err(Error::Limit("decompression ceiling"));
    }
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Io(e.to_string()))?;
    if bytes.len() > limit {
        return Err(Error::Limit("decompressed bytes"));
    }
    Ok(bytes)
}
