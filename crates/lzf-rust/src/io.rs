// SPDX-License-Identifier: ISC
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{Error, Result};

/// `no_std`-compatible read trait used by streaming interfaces.
///
/// This trait is intentionally close to `std::io::Read` so the same code can
/// be shared across `std` and `no_std` builds.
pub trait Read {
    /// Reads bytes into `buf`, returning the number of bytes read.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Reads exactly `buf.len()` bytes into `buf`.
    ///
    /// Returns:
    /// - `Ok(())` if the buffer was fully filled.
    /// - `Err(Error::Eof)` if input ended early.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        default_read_exact(self, buf)
    }
}

/// `no_std`-compatible write trait used by streaming interfaces.
///
/// This trait mirrors the core behavior of `std::io::Write`.
pub trait Write {
    /// Writes bytes from `buf`, returning the number of bytes written.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Flushes buffered output.
    fn flush(&mut self) -> Result<()>;

    /// Writes all bytes from `buf`.
    ///
    /// Returns `Err(Error::WriteZero)` if the writer reports successful writes
    /// of zero bytes before the full input is consumed.
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        default_write_all(self, &mut buf)
    }
}

#[inline]
fn default_read_exact<R: Read + ?Sized>(this: &mut R, mut buf: &mut [u8]) -> Result<()> {
    while !buf.is_empty() {
        match this.read(buf) {
            Ok(0) => break,
            Ok(n) => buf = &mut buf[n..],
            Err(Error::Interrupted) => {}
            Err(e) => return Err(e),
        }
    }

    if buf.is_empty() { Ok(()) } else { Err(Error::Eof) }
}

#[inline]
fn default_write_all<W: Write + ?Sized>(this: &mut W, buf: &mut &[u8]) -> Result<()> {
    while !buf.is_empty() {
        match this.write(buf) {
            Ok(0) => return Err(Error::WriteZero),
            Ok(n) => *buf = &buf[n..],
            Err(Error::Interrupted) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
impl<R: Read + ?Sized> Read for &mut R {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }

    #[inline(always)]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_exact(buf)
    }
}

#[cfg(not(feature = "std"))]
impl<W: Write + ?Sized> Write for &mut W {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }
}

#[cfg(not(feature = "std"))]
impl Read for &[u8] {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.len().min(buf.len());
        let (left, right) = self.split_at(n);
        buf[..n].copy_from_slice(left);
        *self = right;
        Ok(n)
    }
}

#[cfg(not(feature = "std"))]
impl Write for &mut [u8] {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.is_empty() {
            return Err(Error::WriteZero);
        }

        let n = buf.len().min(self.len());
        self[..n].copy_from_slice(&buf[..n]);
        let remaining = core::mem::take(self);
        *self = &mut remaining[n..];
        Ok(n)
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl Write for Vec<u8> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + ?Sized> Read for alloc::boxed::Box<R> {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }

    #[inline(always)]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_exact(buf)
    }
}

#[cfg(not(feature = "std"))]
impl<W: Write + ?Sized> Write for alloc::boxed::Box<W> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }
}

#[cfg(feature = "std")]
impl<R: std::io::Read + ?Sized> Read for R {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        std::io::Read::read(self, buf).map_err(Error::from)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        std::io::Read::read_exact(self, buf).map_err(Error::from)
    }
}

#[cfg(feature = "std")]
impl<W: std::io::Write + ?Sized> Write for W {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        std::io::Write::write(self, buf).map_err(Error::from)
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        std::io::Write::flush(self).map_err(Error::from)
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        std::io::Write::write_all(self, buf).map_err(Error::from)
    }
}
