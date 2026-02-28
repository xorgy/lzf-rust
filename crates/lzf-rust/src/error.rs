// SPDX-License-Identifier: ISC
use core::fmt;

/// Result type used by this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Error type for LZF encode/decode operations.
///
/// The variants are shared by raw token APIs, framed block APIs, and streaming
/// reader/writer adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// End of input reached unexpectedly.
    Eof,
    /// Operation was interrupted.
    Interrupted,
    /// Output buffer is too small for the requested operation.
    OutputTooSmall,
    /// Could not write any bytes.
    WriteZero,
    /// Input stream is malformed.
    InvalidData,
    /// Framed input has an invalid header.
    InvalidHeader,
    /// Framed input contains an unsupported block type.
    ///
    /// The contained byte is the unknown `ZV` block type value.
    UnknownBlockType(u8),
    /// Configuration is invalid.
    InvalidParameter,
    /// Other I/O error.
    Other,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => f.write_str("unexpected end of input"),
            Self::Interrupted => f.write_str("operation interrupted"),
            Self::OutputTooSmall => f.write_str("output buffer too small"),
            Self::WriteZero => f.write_str("failed to write data"),
            Self::InvalidData => f.write_str("invalid compressed data"),
            Self::InvalidHeader => f.write_str("invalid LZF block header"),
            Self::UnknownBlockType(kind) => write!(f, "unknown LZF block type: {kind}"),
            Self::InvalidParameter => f.write_str("invalid parameter"),
            Self::Other => f.write_str("I/O error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::UnexpectedEof => Self::Eof,
            std::io::ErrorKind::Interrupted => Self::Interrupted,
            std::io::ErrorKind::InvalidData => Self::InvalidData,
            std::io::ErrorKind::InvalidInput => Self::InvalidParameter,
            std::io::ErrorKind::WriteZero => Self::WriteZero,
            _ => Self::Other,
        }
    }
}
