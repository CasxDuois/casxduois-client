//! Crate `casx_core` (Layer 2 in DAG).
//!
//! Responsibilities:
//! - Core network codec, AES-128-CFB8 cryptography, and Zlib compression.
//! - Multi-version Binary NBT parser (Classic 1.12.2 and Nameless 26.2).
//! - Chat text parsing (§ legacy formatting and JSON Text Components).
//! - Exact lookup tables (jmath 65536 sin/cos), physics, and canonical protocol types.

pub mod adapter;
pub mod chat_parser;
pub mod codec;
pub mod compression;
pub mod crypto;
pub mod ctl_server;
pub mod inventory;
pub mod jmath;
pub mod native_mods;
pub mod nbt;
pub mod palette;
pub mod physics;
pub mod raycast;
pub mod world;

pub use adapter::{ClientAction, ConnectionState, GameVersion, ProtocolAdapter, ServerEvent};
pub use inventory::{CanonicalItemStack, ItemStack};
pub use jmath::{DVec3, IVec3, AABB};
pub use world::{CanonicalBlockState, CanonicalBlockStateId, ChunkPos, ChunkSection};

use thiserror::Error;

/// Protocol codec and decoding errors.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("VarInt exceeds 5 bytes limit")]
    VarIntTooBig,

    #[error("VarLong exceeds 10 bytes limit")]
    VarLongTooBig,

    #[error("Unexpected end of file while reading buffer")]
    UnexpectedEof,

    #[error("Invalid UTF-8 string: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("String exceeds maximum allowed length: {len} > {max}")]
    StringTooLong { len: usize, max: usize },

    #[error("Zlib compression error: {0}")]
    CompressionError(String),

    #[error("Invalid NBT tag type: {0}")]
    InvalidNbtTag(u8),

    #[error("Binary NBT parse error: {0}")]
    NbtParseError(String),

    #[error("Cryptography error: {0}")]
    CryptoError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
