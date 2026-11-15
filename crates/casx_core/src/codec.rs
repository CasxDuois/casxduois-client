//! Ma hoa va giai ma cac kieu du lieu nguyen thuy cua giao thuc Minecraft.

use crate::ProtocolError;
use bytes::{Buf, BufMut};

/// Doc mot so nguyen bien thien 32-bit (`VarInt`) tu bo dem (toi da $5\text{ bytes}$)
pub fn read_varint<B: Buf>(buf: &mut B) -> Result<i32, ProtocolError> {
    let mut value = 0i32;
    let mut position = 0;

    loop {
        if !buf.has_remaining() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let byte = buf.get_u8();
        value |= ((byte & 0x7F) as i32) << position;

        if (byte & 0x80) == 0 {
            break;
        }

        position += 7;
        if position >= 35 {
            return Err(ProtocolError::VarIntTooBig);
        }
    }

    Ok(value)
}

/// Ghi mot so nguyen bien thien 32-bit (`VarInt`) vao bo dem
pub fn write_varint<B: BufMut>(buf: &mut B, mut value: i32) {
    loop {
        if (value as u32 & !0x7F) == 0 {
            buf.put_u8(value as u8);
            return;
        }
        buf.put_u8(((value & 0x7F) | 0x80) as u8);
        value = (value as u32 >> 7) as i32;
    }
}

/// Doc mot so nguyen bien thien 64-bit (`VarLong`) tu bo dem (toi da $10\text{ bytes}$)
pub fn read_varlong<B: Buf>(buf: &mut B) -> Result<i64, ProtocolError> {
    let mut value = 0i64;
    let mut position = 0;

    loop {
        if !buf.has_remaining() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let byte = buf.get_u8();
        value |= ((byte & 0x7F) as i64) << position;

        if (byte & 0x80) == 0 {
            break;
        }

        position += 7;
        if position >= 70 {
            return Err(ProtocolError::VarLongTooBig);
        }
    }

    Ok(value)
}

/// Ghi mot so nguyen bien thien 64-bit (`VarLong`) vao bo dem
pub fn write_varlong<B: BufMut>(buf: &mut B, mut value: i64) {
    loop {
        if (value as u64 & !0x7F) == 0 {
            buf.put_u8(value as u8);
            return;
        }
        buf.put_u8(((value & 0x7F) | 0x80) as u8);
        value = (value as u64 >> 7) as i64;
    }
}

/// Doc chuoi UTF-8 di kem do dai VarInt tu bo dem
pub fn read_string<B: Buf>(buf: &mut B, max_bytes: usize) -> Result<String, ProtocolError> {
    let len = read_varint(buf)? as usize;
    if len > max_bytes {
        return Err(ProtocolError::StringTooLong { len, max: max_bytes });
    }
    if buf.remaining() < len {
        return Err(ProtocolError::UnexpectedEof);
    }

    let mut bytes = vec![0u8; len];
    buf.copy_to_slice(&mut bytes);
    String::from_utf8(bytes).map_err(ProtocolError::InvalidString)
}

/// Ghi chuoi UTF-8 di kem do dai VarInt vao bo dem
pub fn write_string<B: BufMut>(buf: &mut B, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as i32);
    buf.put_slice(bytes);
}

/// Dong goi toa do khoi 3 chieu theo chuan Minecraft 1.12.2:
/// $$X (26\text{b}) \parallel Y (12\text{b}) \parallel Z (26\text{b})$$
pub fn encode_block_pos_1_12_2(x: i32, y: i32, z: i32) -> u64 {
    let packed = (((x as i64) & 0x3FFFFFF) << 38)
        | (((y as i64) & 0xFFF) << 26)
        | ((z as i64) & 0x3FFFFFF);
    packed as u64
}

/// Giai ma toa do khoi 3 chieu theo chuan Minecraft 1.12.2 (co xu ly mo rong dau Two's Complement):
pub fn decode_block_pos_1_12_2(packed: u64) -> (i32, i32, i32) {
    let val = packed as i64;
    let x = (val >> 38) as i32;
    let y = ((val << 26) >> 52) as i32;
    let z = ((val << 38) >> 38) as i32;
    (x, y, z)
}

/// Dong goi toa do khoi 3 chieu theo chuan Minecraft 26.2:
/// $$X (26\text{b}) \parallel Z (26\text{b}) \parallel Y (12\text{b})$$
pub fn encode_block_pos_26_2(x: i32, y: i32, z: i32) -> u64 {
    let packed = (((x as i64) & 0x3FFFFFF) << 38)
        | (((z as i64) & 0x3FFFFFF) << 12)
        | ((y as i64) & 0xFFF);
    packed as u64
}

/// Giai ma toa do khoi 3 chieu theo chuan Minecraft 26.2 (co xu ly mo rong dau Two's Complement):
pub fn decode_block_pos_26_2(packed: u64) -> (i32, i32, i32) {
    let val = packed as i64;
    let x = (val >> 38) as i32;
    let z = ((val << 26) >> 38) as i32;
    let y = ((val << 52) >> 52) as i32;
    (x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_varint_roundtrip() {
        let test_cases = [0, 1, 2, 127, 128, 255, 256, 2147483647, -1, -2147483648];
        for &val in &test_cases {
            let mut buf = BytesMut::new();
            write_varint(&mut buf, val);
            let mut reader = buf.freeze();
            let decoded = read_varint(&mut reader).expect("Giai ma VarInt phai thanh cong");
            assert_eq!(val, decoded, "Gia tri VarInt khong khop");
            assert!(!reader.has_remaining(), "Bo dem phai duoc doc het");
        }
    }

    #[test]
    fn test_varlong_roundtrip() {
        let test_cases = [0i64, 1, 127, 128, 2147483647, 9223372036854775807, -1, -9223372036854775808];
        for &val in &test_cases {
            let mut buf = BytesMut::new();
            write_varlong(&mut buf, val);
            let mut reader = buf.freeze();
            let decoded = read_varlong(&mut reader).expect("Giai ma VarLong phai thanh cong");
            assert_eq!(val, decoded, "Gia tri VarLong khong khop");
            assert!(!reader.has_remaining(), "Bo dem phai duoc doc het");
        }
    }

    #[test]
    fn test_block_pos_parity_both_versions() {
        let coords = [
            (0, 64, 0),
            (100, 255, -200),
            (-15420, 120, 8912),
            (-30_000_000, -64, 30_000_000),
            (29_999_984, 2047, -29_999_984),
            (-29_999_984, -2048, 29_999_984),
        ];

        for (x, y, z) in coords {
            let packed_112 = encode_block_pos_1_12_2(x, y, z);
            assert_eq!(decode_block_pos_1_12_2(packed_112), (x, y, z), "1.12.2 block pos codec loi tai {:?}", (x, y, z));

            let packed_262 = encode_block_pos_26_2(x, y, z);
            assert_eq!(decode_block_pos_26_2(packed_262), (x, y, z), "26.2 block pos codec loi tai {:?}", (x, y, z));
        }
    }
}
