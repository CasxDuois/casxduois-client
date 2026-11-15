//! Nen va giai nen goi tin Minecraft theo thuat toan Zlib (RFC 1950 Deflate).

use crate::codec::{read_varint, write_varint};
use crate::ProtocolError;
use bytes::{Buf, BufMut, BytesMut};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Giai nen mang byte du lieu Zlib voi dung luong dich mong muon
pub fn decompress_zlib(compressed: &[u8], uncompressed_len: usize) -> Result<Vec<u8>, ProtocolError> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::with_capacity(uncompressed_len);
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;

    if decompressed.len() != uncompressed_len {
        return Err(ProtocolError::CompressionError(format!(
            "Dung luong giai nen khong khop: thuc te {} bytes, mong doi {} bytes",
            decompressed.len(),
            uncompressed_len
        )));
    }

    Ok(decompressed)
}

/// Nen mang byte du lieu bang Zlib voi cap do toi uu toc do/kich thuoc
pub fn compress_zlib(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(data.len() / 2), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| ProtocolError::CompressionError(e.to_string()))
}

/// Dong goi du lieu packet voi nguong nen $T_{\text{threshold}}$
pub fn format_packet_payload(payload: &[u8], threshold: Option<i32>) -> Result<BytesMut, ProtocolError> {
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
                // Duoi nguong: data_len = 0, khong nen
                let mut data_len_buf = BytesMut::new();
                write_varint(&mut data_len_buf, 0);

                let packet_len = (data_len_buf.len() + payload.len()) as i32;
                write_varint(&mut buf, packet_len);
                buf.put(data_len_buf);
                buf.put_slice(payload);
            }
        }
        _ => {
            // Khong bat nen
            write_varint(&mut buf, payload.len() as i32);
            buf.put_slice(payload);
        }
    }
    Ok(buf)
}

/// Bo tach khung goi tin tu du lieu nhan duoc
pub fn parse_packet_payload<B: Buf>(buf: &mut B, threshold: Option<i32>) -> Result<Vec<u8>, ProtocolError> {
    match threshold {
        Some(th) if th >= 0 => {
            let data_len = read_varint(buf)?;
            if data_len == 0 {
                // Khong nen
                let mut payload = vec![0u8; buf.remaining()];
                buf.copy_to_slice(&mut payload);
                Ok(payload)
            } else {
                if (data_len as usize) < th as usize {
                    return Err(ProtocolError::CompressionError(format!(
                        "Goi tin bi nen duoi nguong: {} < {}",
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
        let raw = b"Minecraft Protocol 340 & 770 Zlib Test Payload - Lap lai nhieu lan de nen!".repeat(10);
        let compressed = compress_zlib(&raw).expect("Nen Zlib phai thanh cong");
        assert!(compressed.len() < raw.len(), "Du lieu nen phai nho hon du lieu goc");

        let decompressed = decompress_zlib(&compressed, raw.len()).expect("Giai nen phai thanh cong");
        assert_eq!(raw, decompressed.as_slice(), "Du lieu giai nen phai khop voi ban dau");
    }

    #[test]
    fn test_packet_framing_with_threshold() {
        let threshold = 256;
        let small_payload = vec![0x01, 0x02, 0x03];
        let mut framed_small = format_packet_payload(&small_payload, Some(threshold)).expect("Dong goi phai thanh cong");

        let packet_len = read_varint(&mut framed_small).expect("Doc do dai packet");
        assert_eq!(packet_len as usize, framed_small.remaining());

        let parsed_small = parse_packet_payload(&mut framed_small, Some(threshold)).expect("Giai ma phai thanh cong");
        assert_eq!(small_payload, parsed_small);

        let large_payload = vec![0x7A; 1000];
        let mut framed_large = format_packet_payload(&large_payload, Some(threshold)).expect("Dong goi phai thanh cong");

        let packet_len_lg = read_varint(&mut framed_large).expect("Doc do dai packet");
        assert_eq!(packet_len_lg as usize, framed_large.remaining());

        let parsed_large = parse_packet_payload(&mut framed_large, Some(threshold)).expect("Giai ma packet lon");
        assert_eq!(large_payload, parsed_large);
    }
}
