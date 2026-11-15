// crates/casx_net/src/codec/compression.rs
// Giai nen va nen Zlib theo nguong threshold cua goi Set Compression (0x03)

use bytes::BytesMut;
use casx_core::ProtocolError;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};
use super::varint::{read_varint_from_buf, write_varint_to_buf, varint_len};
use super::frame::RawPacket;

#[derive(Debug, Clone)]
pub struct PacketCompressor {
    threshold: i32,
}

impl PacketCompressor {
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }

    pub fn threshold(&self) -> i32 {
        self.threshold
    }

    /// Giai ma khung goi tin da duoc nen Zlib
    pub fn decompress_packet(&self, compressed_frame: &mut BytesMut) -> Result<Option<RawPacket>, ProtocolError> {
        let mut temp = compressed_frame.clone();
        if let Some(packet_len) = read_varint_from_buf(&mut temp)? {
            let packet_len_len = varint_len(packet_len);
            let total_len = packet_len_len + packet_len as usize;

            if compressed_frame.len() < total_len {
                return Ok(None);
            }

            let _ = read_varint_from_buf(compressed_frame)?;
            let mut body = compressed_frame.split_to(packet_len as usize);

            let data_length = read_varint_from_buf(&mut body)?
                .ok_or(ProtocolError::UnexpectedEof)?;

            if data_length == 0 {
                // Khong bi nen (payload < threshold)
                if let Some(packet_id) = read_varint_from_buf(&mut body)? {
                    Ok(Some(RawPacket::new(packet_id, body.freeze())))
                } else {
                    Err(ProtocolError::UnexpectedEof)
                }
            } else {
                // Bi nen bang Zlib
                let mut decoder = ZlibDecoder::new(&body[..]);
                let mut decompressed = Vec::with_capacity(data_length as usize);
                decoder.read_to_end(&mut decompressed)
                    .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;

                let mut decomp_buf = BytesMut::from(&decompressed[..]);
                if let Some(packet_id) = read_varint_from_buf(&mut decomp_buf)? {
                    Ok(Some(RawPacket::new(packet_id, decomp_buf.freeze())))
                } else {
                    Err(ProtocolError::UnexpectedEof)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Nen va dong goi packet
    pub fn compress_packet(&self, packet: &RawPacket, dst: &mut BytesMut) -> Result<(), ProtocolError> {
        let uncompressed_len = varint_len(packet.id) + packet.payload.len();

        if (uncompressed_len as i32) < self.threshold {
            // data_length = 0
            let data_length = 0i32;
            let packet_len = varint_len(data_length) + uncompressed_len;
            write_varint_to_buf(packet_len as i32, dst);
            write_varint_to_buf(data_length, dst);
            write_varint_to_buf(packet.id, dst);
            dst.extend_from_slice(&packet.payload);
        } else {
            // Nen qua Zlib
            let mut raw_data = BytesMut::with_capacity(uncompressed_len);
            write_varint_to_buf(packet.id, &mut raw_data);
            raw_data.extend_from_slice(&packet.payload);

            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&raw_data)
                .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;
            let compressed_data = encoder.finish()
                .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;

            let data_length = uncompressed_len as i32;
            let packet_len = varint_len(data_length) + compressed_data.len();
            write_varint_to_buf(packet_len as i32, dst);
            write_varint_to_buf(data_length, dst);
            dst.extend_from_slice(&compressed_data);
        }
        Ok(())
    }
}
