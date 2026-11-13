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
/// One assembly per copy. The caller drops this at the five-second deadline,
/// Pause, reconnect, or replacement. This type never exposes partial bytes.
#[derive(Debug)]
pub struct PublishAssembler {
    begin: PublishBegin,
    next: usize,
    bytes: Vec<u8>,
}
impl PublishAssembler {
    pub fn new(begin: PublishBegin) -> Result<Self> {
        if begin.total_bytes == 0 || begin.total_bytes > limits::PUBLISH_BYTES {
            return Err(Error::Limit("publish assembly bytes"));
        }
        if begin.chunk_count == 0 || begin.chunk_count > begin.total_bytes {
            return Err(Error::Invalid("publish chunk count"));
        }
        Ok(Self {
            bytes: Vec::with_capacity(begin.total_bytes),
            begin,
            next: 0,
        })
    }
    pub fn push(&mut self, index: usize, bytes: &[u8]) -> Result<()> {
        if index != self.next || self.next >= self.begin.chunk_count {
            return Err(Error::Invalid("chunk order"));
        }
        if bytes.is_empty() || bytes.len() > limits::CHUNK_BYTES {
            return Err(Error::Limit("chunk bytes"));
        }
        if self.bytes.len().saturating_add(bytes.len()) > self.begin.total_bytes {
            return Err(Error::Limit("assembly overflow"));
        }
        self.bytes.extend_from_slice(bytes);
        self.next += 1;
        Ok(())
    }
    pub fn finish(self, commit: PublishCommit) -> Result<Vec<u8>> {
        if self.next != self.begin.chunk_count || self.bytes.len() != self.begin.total_bytes {
            return Err(Error::Invalid("incomplete publish"));
        }
        if commit.digest != self.begin.digest || bytes_digest(&self.bytes) != commit.digest {
            return Err(Error::DigestMismatch);
        }
        Ok(self.bytes)
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
