//! Minecraft Zlib packet compression and decompression (RFC 1950 Deflate).

use crate::codec::{read_varint, write_varint};
use crate::ProtocolError;
use bytes::{Buf, BufMut, BytesMut};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Decompresses Zlib byte slice into target uncompressed length.
pub fn decompress_zlib(
    compressed: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::with_capacity(uncompressed_len);
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;

    if decompressed.len() != uncompressed_len {
        return Err(ProtocolError::CompressionError(format!(
            "Decompressed length mismatch: got {} bytes, expected {} bytes",
            decompressed.len(),
            uncompressed_len
        )));
    }

    Ok(decompressed)
}

/// Compresses byte slice using Zlib.
pub fn compress_zlib(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(data.len() / 2), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))
}

/// Formats packet payload respecting compression threshold $T_{\text{threshold}}$.
pub fn format_packet_payload(
    payload: &[u8],
    threshold: Option<i32>,
) -> Result<BytesMut, ProtocolError> {
    let mut buf = BytesMut::new();
    match threshold {
        Some(th) if th >= 0 => {
            if payload.len() >= th as usize {
                let compressed = compress_zlib(payload)?;
                let data_len = payload.len() as i32;
                let mut data_len_buf = BytesMut::new();
                write_varint(&mut data_len_buf, data_len);

                let packet_len = (data_len_buf.len() + compressed.len()) as i32;
                write_varint(&mut buf, packet_len);
                buf.put(data_len_buf);
                buf.put_slice(&compressed);
            } else {
                // Below threshold: data_len = 0, uncompressed
                let mut data_len_buf = BytesMut::new();
                write_varint(&mut data_len_buf, 0);

                let packet_len = (data_len_buf.len() + payload.len()) as i32;
                write_varint(&mut buf, packet_len);
                buf.put(data_len_buf);
                buf.put_slice(payload);
            }
        }
        _ => {
            // Compression disabled
            write_varint(&mut buf, payload.len() as i32);
            buf.put_slice(payload);
        }
    }
    Ok(buf)
}

/// Parse packet payload from inbound buffer
pub fn parse_packet_payload<B: Buf>(
    buf: &mut B,
    threshold: Option<i32>,
) -> Result<Vec<u8>, ProtocolError> {
    match threshold {
        Some(th) if th >= 0 => {
            let data_len = read_varint(buf)?;
            if data_len == 0 {
                // Uncompressed
                let mut payload = vec![0u8; buf.remaining()];
                buf.copy_to_slice(&mut payload);
                Ok(payload)
            } else {
                if (data_len as usize) < th as usize {
                    return Err(ProtocolError::CompressionError(format!(
                        "Packet compressed below threshold: {} < {}",
                        data_len, th
                    )));
                }
                let mut compressed = vec![0u8; buf.remaining()];
                buf.copy_to_slice(&mut compressed);
                decompress_zlib(&compressed, data_len as usize)
            }
        }
        _ => {
            let mut payload = vec![0u8; buf.remaining()];
            buf.copy_to_slice(&mut payload);
            Ok(payload)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zlib_roundtrip() {
        let raw =
            b"Minecraft Protocol 340 & 770 Zlib Test Payload - repeated test pattern".repeat(10);
        let compressed = compress_zlib(&raw).expect("Zlib compression must succeed");
        assert!(
            compressed.len() < raw.len(),
            "Compressed size should be smaller than raw"
        );

        let decompressed =
            decompress_zlib(&compressed, raw.len()).expect("Zlib decompression must succeed");
        assert_eq!(
            raw,
            decompressed.as_slice(),
            "Decompressed data must match raw"
        );
    }

    #[test]
    fn test_packet_framing_with_threshold() {
        let threshold = 256;
        let small_payload = vec![0x01, 0x02, 0x03];
        let mut framed_small = format_packet_payload(&small_payload, Some(threshold))
            .expect("Packet framing must succeed");

        let packet_len = read_varint(&mut framed_small).expect("Read packet length");
        assert_eq!(packet_len as usize, framed_small.remaining());

        let parsed_small = parse_packet_payload(&mut framed_small, Some(threshold))
            .expect("Payload parsing must succeed");
        assert_eq!(small_payload, parsed_small);

        let large_payload = vec![0x7A; 1000];
        let mut framed_large = format_packet_payload(&large_payload, Some(threshold))
            .expect("Packet framing must succeed");

        let packet_len_lg = read_varint(&mut framed_large).expect("Read packet length");
        assert_eq!(packet_len_lg as usize, framed_large.remaining());

        let parsed_large = parse_packet_payload(&mut framed_large, Some(threshold))
            .expect("Payload parsing must succeed");
        assert_eq!(large_payload, parsed_large);
    }
}
