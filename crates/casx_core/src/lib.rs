//! Crate `casx_core` (Tang 2 trong do thi phu thuoc DAG).
//!
//! Trach nhiem:
//! - Khung giao thuc mang, ma hoa AES-128-CFB8 va nen Zlib.
//! - Bo phan tich Binary NBT da phien ban (Classic 1.12.2 va Nameless 26.2).
//! - Bo phan tich van ban chat (§ legacy va JSON Component).
//! - Bo nho dem the gioi (WorldCache), vat ly 20 TPS, do tia 3D DDA va giao tiep casx-ctl.

pub mod codec;
pub mod crypto;
pub mod compression;
pub mod nbt;
pub mod chat_parser;

use thiserror::Error;

/// Cac loi phat sinh trong qua trinh doc/ghi giao thuc mang Minecraft
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("[ProtocolError::VarIntTooBig] VarInt vuot qua gioi han 5 bytes")]
    VarIntTooBig,

    #[error("[ProtocolError::VarLongTooBig] VarLong vuot qua gioi han 10 bytes")]
    VarLongTooBig,

    #[error("[ProtocolError::UnexpectedEof] Du lieu luong mang bi thieu truoc khi doc xong")]
    UnexpectedEof,

    #[error("[ProtocolError::InvalidString] Chuoi khong phai UTF-8 hop le")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("[ProtocolError::StringTooLong] Chuoi vuot qua do dai toi da: {len} > {max}")]
    StringTooLong { len: usize, max: usize },

    #[error("[ProtocolError::CompressionError] Loi giai nen Zlib: {0}")]
    CompressionError(String),

    #[error("[ProtocolError::InvalidNbtTag] Ma Tag NBT khong hop le: {0}")]
    InvalidNbtTag(u8),

    #[error("[ProtocolError::NbtParseError] Loi phan tich Binary NBT: {0}")]
    NbtParseError(String),

    #[error("[ProtocolError::CryptoError] Loi ma hoa/giai ma: {0}")]
    CryptoError(String),

    #[error("[ProtocolError::Io] Loi I/O: {0}")]
    Io(#[from] std::io::Error),
}
