//! Model transport (live, record, replay), the Jev codecs and the Claude backends.
//!
//! Every model call goes through this crate, and the rest of the engine sees only its types,
//! never a provider's wire format. Its errors are registered codes with details this crate chose,
//! never a provider's text, and no credential reaches an error, a record or a log.

pub mod canonical;
pub mod claude;
pub mod error;
pub mod gate;
pub mod hosts;
pub mod replay;
pub mod spend;
pub mod transport;

pub use error::ModelError;
