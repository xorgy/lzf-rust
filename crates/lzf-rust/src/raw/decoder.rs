// SPDX-License-Identifier: ISC
use alloc::vec;
use alloc::vec::Vec;

use crate::{Error, Result};

/// Decompresses raw LZF `input` into `output`.
///
/// Returns the number of bytes written to `output`.
///
/// Returns:
/// - `Error::InvalidData` when the token stream is malformed.
/// - `Error::OutputTooSmall` when `output` is too small for the decoded data.
///
/// # Example
///
/// ```
/// use lzf_rust::{compress, decompress, max_compressed_size};
///
/// let input = b"raw decoder example";
/// let mut compressed = vec![0u8; max_compressed_size(input.len())];
/// let clen = compress(input, &mut compressed).unwrap();
/// compressed.truncate(clen);
///
/// let mut out = vec![0u8; input.len()];
/// let written = decompress(&compressed, &mut out).unwrap();
/// assert_eq!(written, input.len());
/// assert_eq!(out, input);
/// ```
pub fn decompress(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut ip = 0usize;
    let mut op = 0usize;

    while ip < input.len() {
        let ctrl = input[ip];
        ip += 1;

        if ctrl < 32 {
            let len = usize::from(ctrl) + 1;
            if ip + len > input.len() || op + len > output.len() {
                return Err(Error::InvalidData);
            }
            output[op..op + len].copy_from_slice(&input[ip..ip + len]);
            ip += len;
            op += len;
            continue;
        }

        let mut len = usize::from(ctrl >> 5);
        let off_hi = usize::from(ctrl & 0x1f) << 8;
        if len == 7 {
            if ip >= input.len() {
                return Err(Error::InvalidData);
            }
            len += usize::from(input[ip]);
            ip += 1;
        }

        if ip >= input.len() {
            return Err(Error::InvalidData);
        }

        let off = off_hi | usize::from(input[ip]);
        ip += 1;

        let copy_len = len + 2;
        if op + copy_len > output.len() {
            return Err(Error::OutputTooSmall);
        }
        if off >= op {
            return Err(Error::InvalidData);
        }

        let ref_pos = op - off - 1;
        if copy_len <= 8 {
            let mut dst = op;
            let mut src = ref_pos;
            let end = dst + copy_len;
            while dst < end {
                output[dst] = output[src];
                dst += 1;
                src += 1;
            }
            op += copy_len;
        } else if ref_pos + copy_len <= op {
            let (head, tail) = output.split_at_mut(op);
            tail[..copy_len].copy_from_slice(&head[ref_pos..ref_pos + copy_len]);
            op += copy_len;
        } else {
            let mut dst = op;
            let mut src = ref_pos;
            let end = dst + copy_len;
            while dst < end {
                output[dst] = output[src];
                dst += 1;
                src += 1;
            }
            op += copy_len;
        }
    }

    Ok(op)
}

/// Decompresses raw LZF `input` into a fresh `Vec<u8>` of `output_len` bytes.
///
/// Returns `Error::InvalidData` if the stream decodes to a length different
/// from `output_len`.
pub fn decompress_into_vec(input: &[u8], output_len: usize) -> Result<Vec<u8>> {
    let mut output = vec![0u8; output_len];
    let written = decompress(input, &mut output)?;
    if written != output_len {
        return Err(Error::InvalidData);
    }
    Ok(output)
}
