//! Pure-rust H.263 decoder

mod decoder;
mod error;
pub mod parser;
mod traits;
mod types;

pub use decoder::{DecoderOption, H263Decoder};
pub use error::{Error, Result};
