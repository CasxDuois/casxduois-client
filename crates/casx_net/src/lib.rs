//! Asynchronous network transport, framing, and session management.

pub mod adapter;
pub mod codec;
pub mod session;

pub use adapter::{
    AdapterV340, AdapterV776, CanonicalClientCmd, CanonicalServerEvent, ProtocolAdapter,
};
pub use session::v340::SessionV340;
pub use session::v776::SessionV776;
