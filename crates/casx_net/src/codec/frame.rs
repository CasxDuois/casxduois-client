// crates/casx_net/src/codec/frame.rs
// Minecraft packet frame: [Length: VarInt] + [Packet ID: VarInt] + [Payload]

use super::varint::{read_varint_from_buf, varint_len, write_varint_to_buf};
use bytes::{Bytes, BytesMut};
use casx_core::ProtocolError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub id: i32,
    pub payload: Bytes,
}

impl RawPacket {
    pub fn new(id: i32, payload: Bytes) -> Self {
        Self { id, payload }
    }

    /// Decodes a packet frame from BytesMut buffer
    pub fn decode(src: &mut BytesMut) -> Result<Option<Self>, ProtocolError> {
        let mut temp = src.clone();
        if let Some(length) = read_varint_from_buf(&mut temp)? {
            let length_len = varint_len(length);
            let total_len = length_len + length as usize;

            if src.len() < total_len {
                return Ok(None); // Incomplete packet bytes
            }

            // Sufficient bytes available, consume from src
            let _ = read_varint_from_buf(src)?;
            let mut packet_data = src.split_to(length as usize);
            if let Some(packet_id) = read_varint_from_buf(&mut packet_data)? {
                let payload = packet_data.freeze();
                Ok(Some(Self {
                    id: packet_id,
                    payload,
                }))
            } else {
                Err(ProtocolError::UnexpectedEof)
            }
        } else {
            Ok(None)
        }
    }

    /// Encodes packet into BytesMut for TCP socket transmission
    pub fn encode(&self, dst: &mut BytesMut) {
        let id_len = varint_len(self.id);
        let total_length = id_len + self.payload.len();
        write_varint_to_buf(total_length as i32, dst);
        write_varint_to_buf(self.id, dst);
        dst.extend_from_slice(&self.payload);
    }
}
