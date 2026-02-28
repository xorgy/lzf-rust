// SPDX-License-Identifier: ISC
//! Raw (unframed) LZF token encoder/decoder.
//!
//! Raw mode operates on the token stream itself (no `ZV` block headers).
mod decoder;
#[cfg(feature = "encoder")]
mod encoder;

pub use decoder::{decompress, decompress_into_vec};
#[cfg(feature = "encoder")]
pub use encoder::{CompressionMode, compress, compress_best, compress_with_mode};
