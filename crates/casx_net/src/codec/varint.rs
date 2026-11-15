// crates/casx_net/src/codec/varint.rs
// Asynchronous and synchronous VarInt / VarLong read/write codec

use bytes::{Buf, BufMut, BytesMut};
use casx_core::ProtocolError;

pub fn read_varint_from_buf(src: &mut BytesMut) -> Result<Option<i32>, ProtocolError> {
    let mut value: i32 = 0;
    let mut position = 0;
    let mut bytes_read = 0;

    for &byte in src.iter() {
        bytes_read += 1;
        value |= ((byte & 0x7F) as i32) << position;

        if (byte & 0x80) == 0 {
            src.advance(bytes_read);
            return Ok(Some(value));
        }

        position += 7;
        if position >= 35 {
            return Err(ProtocolError::VarIntTooBig);
        }
    }

    // Insufficient bytes in buffer
    Ok(None)
}

pub fn write_varint_to_buf(mut value: i32, dst: &mut BytesMut) {
    loop {
        if (value & !0x7F) == 0 {
            dst.put_u8(value as u8);
            return;
        } else {
            dst.put_u8(((value & 0x7F) | 0x80) as u8);
            value = ((value as u32) >> 7) as i32;
        }
    }
}

pub fn varint_len(mut value: i32) -> usize {
    let mut len = 0;
    loop {
        len += 1;
        if (value & !0x7F) == 0 {
            break;
        }
        value = ((value as u32) >> 7) as i32;
    }
    len
}
