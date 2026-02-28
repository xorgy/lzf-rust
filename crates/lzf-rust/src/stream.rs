// SPDX-License-Identifier: ISC
use alloc::vec;
use alloc::vec::Vec;

use crate::decompress;
#[cfg(feature = "encoder")]
use crate::{AutoFinish, AutoFinisher, Error, Result, Write};
#[cfg(feature = "encoder")]
use crate::{CompressionMode, compress_with_mode};
use crate::{Read, Result as DecodeResult};

const MAGIC_0: u8 = b'Z';
const MAGIC_1: u8 = b'V';
const TYPE_UNCOMPRESSED: u8 = 0;
const TYPE_COMPRESSED: u8 = 1;

/// Reader that decodes framed LZF (`ZV` block stream).
///
/// The reader consumes blocks lazily and yields decompressed bytes through the
/// crate's `Read` trait.
///
/// # Example
///
/// ```
/// use lzf_rust::{LzfRead, LzfReader, encode_blocks};
///
/// let input = b"stream reader example";
/// let framed = encode_blocks(input, 4096).unwrap();
/// let mut src: &[u8] = &framed;
/// let mut reader = LzfReader::new(&mut src);
///
/// let mut out = vec![0u8; input.len()];
/// reader.read_exact(&mut out).unwrap();
/// assert_eq!(out, input);
/// ```
pub struct LzfReader<R: Read> {
    inner: R,
    in_buf: Vec<u8>,
    out_buf: Vec<u8>,
    out_pos: usize,
    finished: bool,
}

impl<R: Read> LzfReader<R> {
    /// Creates a new framed LZF reader.
    pub fn new(inner: R) -> Self {
        Self { inner, in_buf: Vec::new(), out_buf: Vec::new(), out_pos: 0, finished: false }
    }

    /// Unwraps the reader and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns a shared reference to the underlying reader.
    pub fn inner(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the underlying reader.
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    fn load_next_block(&mut self) -> DecodeResult<bool> {
        if self.finished {
            return Ok(false);
        }

        let mut first = [0u8; 1];
        let n = self.inner.read(&mut first)?;
        if n == 0 || first[0] == 0 {
            self.finished = true;
            return Ok(false);
        }

        let mut rest = [0u8; 4];
        self.inner.read_exact(&mut rest)?;

        if first[0] != MAGIC_0 || rest[0] != MAGIC_1 {
            return Err(crate::Error::InvalidHeader);
        }

        let block_type = rest[1];
        match block_type {
            TYPE_UNCOMPRESSED => {
                let us = usize::from(u16::from_be_bytes([rest[2], rest[3]]));
                self.out_buf.resize(us, 0);
                self.inner.read_exact(&mut self.out_buf)?;
                self.out_pos = 0;
                Ok(true)
            }
            TYPE_COMPRESSED => {
                let cs = usize::from(u16::from_be_bytes([rest[2], rest[3]]));
                let mut us_buf = [0u8; 2];
                self.inner.read_exact(&mut us_buf)?;
                let us = usize::from(u16::from_be_bytes(us_buf));

                self.in_buf.resize(cs, 0);
                self.inner.read_exact(&mut self.in_buf)?;

                self.out_buf.resize(us, 0);
                let written = decompress(&self.in_buf, &mut self.out_buf)?;
                if written != us {
                    return Err(crate::Error::InvalidData);
                }
                self.out_pos = 0;
                Ok(true)
            }
            other => Err(crate::Error::UnknownBlockType(other)),
        }
    }
}

impl<R: Read> Read for LzfReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> DecodeResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut written = 0usize;
        while written < buf.len() {
            if self.out_pos < self.out_buf.len() {
                let avail = self.out_buf.len() - self.out_pos;
                let take = (buf.len() - written).min(avail);
                buf[written..written + take]
                    .copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + take]);
                self.out_pos += take;
                written += take;
                continue;
            }

            self.out_buf.clear();
            self.out_pos = 0;
            if !self.load_next_block()? {
                break;
            }
        }

        Ok(written)
    }
}

/// Writer that encodes framed LZF (`ZV` block stream).
///
/// Data written into this adapter is chunked into blocks and emitted as either
/// compressed or uncompressed `ZV` blocks.
#[cfg(feature = "encoder")]
pub struct LzfWriter<W: Write> {
    inner: W,
    block_size: usize,
    mode: CompressionMode,
    in_buf: Vec<u8>,
    comp_buf: Vec<u8>,
    write_eof_marker: bool,
}

#[cfg(feature = "encoder")]
impl<W: Write> LzfWriter<W> {
    /// Creates a new framed LZF writer with the given block size (`1..=65535`).
    pub fn new(inner: W, block_size: usize) -> Result<Self> {
        Self::new_with_mode(inner, block_size, CompressionMode::Normal)
    }

    /// Creates a new framed LZF writer with an explicit compression mode.
    pub fn new_with_mode(inner: W, block_size: usize, mode: CompressionMode) -> Result<Self> {
        if block_size == 0 || block_size > usize::from(u16::MAX) {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            inner,
            block_size,
            mode,
            in_buf: Vec::with_capacity(block_size),
            comp_buf: vec![0u8; block_size.saturating_sub(4)],
            write_eof_marker: false,
        })
    }

    /// Creates a writer and enables writing a trailing zero byte EOF marker on finish.
    ///
    /// The marker matches the historical `lzf` utility stream behavior.
    pub fn new_with_eof_marker(inner: W, block_size: usize) -> Result<Self> {
        Self::new_with_eof_marker_and_mode(inner, block_size, CompressionMode::Normal)
    }

    /// Creates a writer and enables writing a trailing zero byte EOF marker on finish.
    ///
    /// Compression mode is explicitly selected.
    pub fn new_with_eof_marker_and_mode(
        inner: W,
        block_size: usize,
        mode: CompressionMode,
    ) -> Result<Self> {
        let mut this = Self::new_with_mode(inner, block_size, mode)?;
        this.write_eof_marker = true;
        Ok(this)
    }

    /// Unwraps the writer and returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns a shared reference to the underlying writer.
    pub fn inner(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the underlying writer.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Finishes the stream and returns the underlying writer.
    ///
    /// This flushes any pending input block. If EOF marker mode is enabled, a
    /// trailing zero byte is appended after the final block.
    pub fn finish(mut self) -> Result<W> {
        self.flush_pending()?;
        if self.write_eof_marker {
            self.inner.write_all(&[0])?;
        }
        self.inner.flush()?;
        Ok(self.inner)
    }

    /// Returns a wrapper that will call `finish()` on drop.
    ///
    /// This is useful for best-effort stream finalization in scopes with early
    /// returns.
    pub fn auto_finish(self) -> AutoFinisher<Self> {
        AutoFinisher(Some(self))
    }

    fn flush_pending(&mut self) -> Result<()> {
        if !self.in_buf.is_empty() {
            Self::write_block_into(&mut self.inner, self.mode, &mut self.comp_buf, &self.in_buf)?;
            self.in_buf.clear();
        }
        Ok(())
    }

    fn write_block_into(
        inner: &mut W,
        mode: CompressionMode,
        comp_buf: &mut Vec<u8>,
        block: &[u8],
    ) -> Result<()> {
        let max_try = block.len().saturating_sub(4);
        if max_try > 0 {
            if comp_buf.len() < max_try {
                comp_buf.resize(max_try, 0);
            }
            match compress_with_mode(block, &mut comp_buf[..max_try], mode) {
                Ok(cs) => {
                    let cs_u16 =
                        u16::try_from(cs).map_err(|_| Error::InvalidParameter)?.to_be_bytes();
                    let us_u16 = u16::try_from(block.len())
                        .map_err(|_| Error::InvalidParameter)?
                        .to_be_bytes();
                    inner.write_all(&[MAGIC_0, MAGIC_1, TYPE_COMPRESSED])?;
                    inner.write_all(&cs_u16)?;
                    inner.write_all(&us_u16)?;
                    inner.write_all(&comp_buf[..cs])?;
                    return Ok(());
                }
                Err(Error::OutputTooSmall) => {}
                Err(err) => return Err(err),
            }
        }

        let us_u16 = u16::try_from(block.len()).map_err(|_| Error::InvalidParameter)?.to_be_bytes();
        inner.write_all(&[MAGIC_0, MAGIC_1, TYPE_UNCOMPRESSED])?;
        inner.write_all(&us_u16)?;
        inner.write_all(block)?;
        Ok(())
    }
}

#[cfg(feature = "encoder")]
impl<W: Write> AutoFinish for LzfWriter<W> {
    fn finish_ignore_error(self) {
        let _ = self.finish();
    }
}

#[cfg(feature = "encoder")]
impl<W: Write> Write for LzfWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut input = buf;

        if !self.in_buf.is_empty() {
            let need = self.block_size - self.in_buf.len();
            let take = need.min(input.len());
            self.in_buf.extend_from_slice(&input[..take]);
            input = &input[take..];

            if self.in_buf.len() == self.block_size {
                Self::write_block_into(
                    &mut self.inner,
                    self.mode,
                    &mut self.comp_buf,
                    &self.in_buf,
                )?;
                self.in_buf.clear();
            }
        }

        let mut consumed = 0usize;
        while input.len() - consumed >= self.block_size {
            let block = &input[consumed..consumed + self.block_size];
            Self::write_block_into(&mut self.inner, self.mode, &mut self.comp_buf, block)?;
            consumed += self.block_size;
        }

        if consumed < input.len() {
            self.in_buf.extend_from_slice(&input[consumed..]);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.flush_pending()?;
        self.inner.flush()
    }
}
