// SPDX-License-Identifier: ISC
//! Pure Rust LZF compression and decompression.
//!
//! # Overview
//!
//! This crate provides:
//!
//! - Raw LZF token encode/decode (`compress`/`decompress`).
//! - `lzf` block framing support (`ZV\0`/`ZV\1`) via `encode_blocks`/`decode_blocks`.
//! - Streaming adapters (`LzfReader`, `LzfWriter`) for framed streams.
//! - `no_std`-compatible I/O traits (`LzfRead`, `LzfWrite`).
//!
//! Raw token compatibility matches `liblzf` (`lzf_compress`, `lzf_compress_best`,
//! and `lzf_decompress` behavior for valid inputs).
//!
//! # Features
//!
//! - `std` (default): integrates with `std::io::{Read, Write}`.
//! - `encoder` (default): enables compression APIs and `LzfWriter`.
//!
//! # no_std
//!
//! Disable default features to use in `no_std + alloc` environments:
//!
//! ```toml
//! [dependencies]
//! lzf-rust = { version = "0.1", default-features = false, features = ["encoder"] }
//! ```
//!
//! In this mode, use crate-level `LzfRead`/`LzfWrite` traits.
//!
//! # Examples
//!
//! Raw token roundtrip:
//!
//! ```
//! use lzf_rust::{compress, decompress, max_compressed_size};
//!
//! let input = b"hello hello hello hello";
//! let mut compressed = vec![0u8; max_compressed_size(input.len())];
//! let n = compress(input, &mut compressed).unwrap();
//! compressed.truncate(n);
//!
//! let mut out = vec![0u8; input.len()];
//! let m = decompress(&compressed, &mut out).unwrap();
//! assert_eq!(m, input.len());
//! assert_eq!(&out, input);
//! ```
//!
//! Framed block roundtrip:
//!
//! ```
//! use lzf_rust::{decode_blocks, encode_blocks};
//!
//! let input = b"framed lzf data";
//! let framed = encode_blocks(input, 32 * 1024).unwrap();
//! let decoded = decode_blocks(&framed).unwrap();
//! assert_eq!(decoded, input);
//! ```
//!
//! # Safety
//!
//! This crate forbids `unsafe` code.
//!
//! # License
//!
//! This repository uses file-level licensing:
//!
//! - `src/raw/encoder.rs`: `BSD-2-Clause` (derived from liblzf encoder logic).
//! - Remaining from-scratch Rust sources: `ISC`.

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod error;
mod framed;
mod io;
mod raw;
mod stream;

/// Crate error and result types.
pub use error::{Error, Result};
/// Decodes `lzf` framed block streams (`ZV\0`/`ZV\1`).
pub use framed::decode_blocks;
#[cfg(feature = "encoder")]
#[cfg_attr(docsrs, doc(cfg(feature = "encoder")))]
/// Encodes bytes into `lzf` framed block streams (`ZV\0`/`ZV\1`).
pub use framed::encode_blocks;
#[cfg(feature = "encoder")]
#[cfg_attr(docsrs, doc(cfg(feature = "encoder")))]
/// Encodes bytes into framed block streams with an explicit compression mode.
pub use framed::encode_blocks_with_mode;
/// `no_std`-compatible read/write traits used by streaming APIs.
pub use io::{Read, Write};
/// Alias for `Read` to mirror naming used by related compression crates.
pub use io::{Read as LzfRead, Write as LzfWrite};
#[cfg(feature = "encoder")]
#[cfg_attr(docsrs, doc(cfg(feature = "encoder")))]
/// Raw LZF encoder APIs.
pub use raw::{CompressionMode, compress, compress_best, compress_with_mode};
/// Raw LZF decoder APIs.
pub use raw::{decompress, decompress_into_vec};
/// Framed LZF stream reader.
pub use stream::LzfReader;
#[cfg(feature = "encoder")]
#[cfg_attr(docsrs, doc(cfg(feature = "encoder")))]
/// Framed LZF stream writer.
pub use stream::LzfWriter;

/// Maximum literal run size in the LZF format.
pub const MAX_LITERAL_LEN: usize = 1 << 5;

/// Maximum backwards offset in the LZF format.
pub const MAX_OFFSET: usize = 1 << 13;

/// Maximum match length in the LZF format.
pub const MAX_MATCH_LEN: usize = (1 << 8) + (1 << 3);

/// Computes a guaranteed upper bound for compressed output size.
#[inline]
pub const fn max_compressed_size(input_len: usize) -> usize {
    ((input_len * 33) >> 5) + 1
}

/// Internal trait used by [`AutoFinisher`] to finalize streams on drop.
#[doc(hidden)]
pub trait AutoFinish {
    /// Finalizes the wrapped stream and ignores any returned error.
    fn finish_ignore_error(self);
}

/// Wrapper that attempts to finish the wrapped writer on drop.
///
/// This is useful when you want best-effort stream finalization even when
/// early returns or panics bypass an explicit `finish()` call.
pub struct AutoFinisher<T: AutoFinish>(pub(crate) Option<T>);

impl<T: AutoFinish> Drop for AutoFinisher<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            inner.finish_ignore_error();
        }
    }
}

impl<T: AutoFinish> core::ops::Deref for AutoFinisher<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("AutoFinisher: inner value missing")
    }
}

impl<T: AutoFinish> core::ops::DerefMut for AutoFinisher<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("AutoFinisher: inner value missing")
    }
}

impl<T: AutoFinish + Write> Write for AutoFinisher<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.as_mut().expect("AutoFinisher: inner value missing").write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.as_mut().expect("AutoFinisher: inner value missing").flush()
    }
}
