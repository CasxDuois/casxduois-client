//! Binary NBT parser (Classic 1.12.2 and Nameless Network NBT 26.2).

use crate::ProtocolError;
use bytes::{Buf, BufMut};
use std::collections::HashMap;

/// All 12 NBT Tag types according to Minecraft specification
#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    /// Returns NBT tag ID according to specification
    pub fn tag_id(&self) -> u8 {
        match self {
            NbtTag::End => 0,
            NbtTag::Byte(_) => 1,
            NbtTag::Short(_) => 2,
            NbtTag::Int(_) => 3,
            NbtTag::Long(_) => 4,
            NbtTag::Float(_) => 5,
            NbtTag::Double(_) => 6,
            NbtTag::ByteArray(_) => 7,
            NbtTag::String(_) => 8,
            NbtTag::List(_) => 9,
            NbtTag::Compound(_) => 10,
            NbtTag::IntArray(_) => 11,
            NbtTag::LongArray(_) => 12,
        }
    }

    pub fn as_compound(&self) -> Option<&HashMap<String, NbtTag>> {
        match self {
            NbtTag::Compound(map) => Some(map),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.as_compound().and_then(|m| m.get(key))
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(NbtTag::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn get_int(&self, key: &str) -> Option<i32> {
        match self.get(key) {
            Some(NbtTag::Int(v)) => Some(*v),
            Some(NbtTag::Byte(v)) => Some(*v as i32),
            Some(NbtTag::Short(v)) => Some(*v as i32),
            _ => None,
        }
    }
}

/// Read Classic NBT (Minecraft 1.12.2: Tag 10 + 2-byte name + Payload)
pub fn read_classic_nbt<B: Buf>(buf: &mut B) -> Result<(String, NbtTag), ProtocolError> {
    if !buf.has_remaining() {
        return Err(ProtocolError::UnexpectedEof);
    }
    let root_type = buf.get_u8();
    if root_type == 0 {
        return Ok((String::new(), NbtTag::End));
    }
    if root_type != 10 {
        return Err(ProtocolError::NbtParseError(format!(
            "Root Tag of Classic NBT must be Compound (10), got: {}",
            root_type
        )));
    }
    let root_name = read_nbt_string_raw(buf)?;
    let payload = read_compound_payload(buf)?;
    Ok((root_name, NbtTag::Compound(payload)))
}

/// Read Nameless Network NBT (Minecraft 26.2: Tag 10 + Payload, omitting root name)
pub fn read_nameless_nbt<B: Buf>(buf: &mut B) -> Result<NbtTag, ProtocolError> {
    if !buf.has_remaining() {
        return Err(ProtocolError::UnexpectedEof);
    }
    let root_type = buf.get_u8();
    if root_type == 0 {
        return Ok(NbtTag::End);
    }
    if root_type != 10 {
        return Err(ProtocolError::NbtParseError(format!(
            "Root Tag of Nameless NBT must be Compound (10), got: {}",
            root_type
        )));
    }
    let payload = read_compound_payload(buf)?;
    Ok(NbtTag::Compound(payload))
}

/// Write Classic NBT (1.12.2) into buffer
pub fn write_classic_nbt<B: BufMut>(
    buf: &mut B,
    root_name: &str,
    tag: &NbtTag,
) -> Result<(), ProtocolError> {
    buf.put_u8(tag.tag_id());
    if tag.tag_id() == 0 {
        return Ok(());
    }
    write_nbt_string_raw(buf, root_name);
    write_tag_payload(buf, tag)
}

/// Write Nameless NBT (26.2) into buffer
pub fn write_nameless_nbt<B: BufMut>(buf: &mut B, tag: &NbtTag) -> Result<(), ProtocolError> {
    buf.put_u8(tag.tag_id());
    if tag.tag_id() == 0 {
        return Ok(());
    }
    write_tag_payload(buf, tag)
}

fn read_tag_payload<B: Buf>(buf: &mut B, tag_type: u8) -> Result<NbtTag, ProtocolError> {
    match tag_type {
        0 => Ok(NbtTag::End),
        1 => {
            if !buf.has_remaining() {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Byte(buf.get_i8()))
        }
        2 => {
            if buf.remaining() < 2 {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Short(buf.get_i16()))
        }
        3 => {
            if buf.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Int(buf.get_i32()))
        }
        4 => {
            if buf.remaining() < 8 {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Long(buf.get_i64()))
        }
        5 => {
            if buf.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Float(buf.get_f32()))
        }
        6 => {
            if buf.remaining() < 8 {
                return Err(ProtocolError::UnexpectedEof);
            }
            Ok(NbtTag::Double(buf.get_f64()))
        }
        7 => {
            if buf.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            let len = buf.get_i32();
            if len < 0 || buf.remaining() < len as usize {
                return Err(ProtocolError::UnexpectedEof);
            }
            let mut data = vec![0u8; len as usize];
            buf.copy_to_slice(&mut data);
            Ok(NbtTag::ByteArray(data))
        }
        8 => Ok(NbtTag::String(read_nbt_string_raw(buf)?)),
        9 => {
            if buf.remaining() < 5 {
                return Err(ProtocolError::UnexpectedEof);
            }
            let elem_type = buf.get_u8();
            let len = buf.get_i32();
            if len <= 0 {
                return Ok(NbtTag::List(Vec::new()));
            }
            let mut list = Vec::with_capacity(len as usize);
            for _ in 0..len {
                list.push(read_tag_payload(buf, elem_type)?);
            }
            Ok(NbtTag::List(list))
        }
        10 => Ok(NbtTag::Compound(read_compound_payload(buf)?)),
        11 => {
            if buf.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            let len = buf.get_i32();
            if len < 0 || buf.remaining() < (len as usize * 4) {
                return Err(ProtocolError::UnexpectedEof);
            }
            let mut list = Vec::with_capacity(len as usize);
            for _ in 0..len {
                list.push(buf.get_i32());
            }
            Ok(NbtTag::IntArray(list))
        }
        12 => {
            if buf.remaining() < 4 {
                return Err(ProtocolError::UnexpectedEof);
            }
            let len = buf.get_i32();
            if len < 0 || buf.remaining() < (len as usize * 8) {
                return Err(ProtocolError::UnexpectedEof);
            }
            let mut list = Vec::with_capacity(len as usize);
            for _ in 0..len {
                list.push(buf.get_i64());
            }
            Ok(NbtTag::LongArray(list))
        }
        unknown => Err(ProtocolError::InvalidNbtTag(unknown)),
    }
}

fn read_compound_payload<B: Buf>(buf: &mut B) -> Result<HashMap<String, NbtTag>, ProtocolError> {
    let mut map = HashMap::new();
    loop {
        if !buf.has_remaining() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let tag_type = buf.get_u8();
        if tag_type == 0 {
            break;
        }
        let name = read_nbt_string_raw(buf)?;
        let tag = read_tag_payload(buf, tag_type)?;
        map.insert(name, tag);
    }
    Ok(map)
}

fn read_nbt_string_raw<B: Buf>(buf: &mut B) -> Result<String, ProtocolError> {
    if buf.remaining() < 2 {
        return Err(ProtocolError::UnexpectedEof);
    }
    let len = buf.get_u16() as usize;
    if buf.remaining() < len {
        return Err(ProtocolError::UnexpectedEof);
    }
    let mut bytes = vec![0u8; len];
    buf.copy_to_slice(&mut bytes);
    String::from_utf8(bytes).map_err(ProtocolError::InvalidString)
}

fn write_nbt_string_raw<B: BufMut>(buf: &mut B, s: &str) {
    let bytes = s.as_bytes();
    buf.put_u16(bytes.len() as u16);
    buf.put_slice(bytes);
}

fn write_tag_payload<B: BufMut>(buf: &mut B, tag: &NbtTag) -> Result<(), ProtocolError> {
    match tag {
        NbtTag::End => Ok(()),
        NbtTag::Byte(v) => {
            buf.put_i8(*v);
            Ok(())
        }
        NbtTag::Short(v) => {
            buf.put_i16(*v);
            Ok(())
        }
        NbtTag::Int(v) => {
            buf.put_i32(*v);
            Ok(())
        }
        NbtTag::Long(v) => {
            buf.put_i64(*v);
            Ok(())
        }
        NbtTag::Float(v) => {
            buf.put_f32(*v);
            Ok(())
        }
        NbtTag::Double(v) => {
            buf.put_f64(*v);
            Ok(())
        }
        NbtTag::ByteArray(v) => {
            buf.put_i32(v.len() as i32);
            buf.put_slice(v);
            Ok(())
        }
        NbtTag::String(s) => {
            write_nbt_string_raw(buf, s);
            Ok(())
        }
        NbtTag::List(list) => {
            if list.is_empty() {
                buf.put_u8(0);
                buf.put_i32(0);
            } else {
                let elem_type = list[0].tag_id();
                buf.put_u8(elem_type);
                buf.put_i32(list.len() as i32);
                for item in list {
                    write_tag_payload(buf, item)?;
                }
            }
            Ok(())
        }
        NbtTag::Compound(map) => {
            for (name, item) in map {
                buf.put_u8(item.tag_id());
                write_nbt_string_raw(buf, name);
                write_tag_payload(buf, item)?;
            }
            buf.put_u8(0); // TAG_End
            Ok(())
        }
        NbtTag::IntArray(v) => {
            buf.put_i32(v.len() as i32);
            for &item in v {
                buf.put_i32(item);
            }
            Ok(())
        }
        NbtTag::LongArray(v) => {
            buf.put_i32(v.len() as i32);
            for &item in v {
                buf.put_i64(item);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_classic_nbt_roundtrip() {
        let mut compound = HashMap::new();
        compound.insert("Health".to_string(), NbtTag::Float(20.0));
        compound.insert("Score".to_string(), NbtTag::Int(1500));
        compound.insert(
            "PlayerName".to_string(),
            NbtTag::String("CasxTester".to_string()),
        );

        let original_tag = NbtTag::Compound(compound);
        let mut buf = BytesMut::new();
        write_classic_nbt(&mut buf, "PlayerStats", &original_tag).expect("Ghi Classic NBT");

        let mut reader = buf.freeze();
        let (root_name, decoded) = read_classic_nbt(&mut reader).expect("Doc Classic NBT");
        assert_eq!(root_name, "PlayerStats");
        assert_eq!(decoded.get_int("Score"), Some(1500));
        assert_eq!(decoded.get_str("PlayerName"), Some("CasxTester"));
    }

    #[test]
    fn test_nameless_nbt_roundtrip() {
        let mut compound = HashMap::new();
        compound.insert(
            "Dimension".to_string(),
            NbtTag::String("minecraft:overworld".to_string()),
        );
        compound.insert("SeaLevel".to_string(), NbtTag::Int(63));

        let original_tag = NbtTag::Compound(compound);
        let mut buf = BytesMut::new();
        write_nameless_nbt(&mut buf, &original_tag).expect("Ghi Nameless NBT");

        let mut reader = buf.freeze();
        let decoded = read_nameless_nbt(&mut reader).expect("Doc Nameless NBT");
        assert_eq!(decoded.get_str("Dimension"), Some("minecraft:overworld"));
        assert_eq!(decoded.get_int("SeaLevel"), Some(63));
    }
}
