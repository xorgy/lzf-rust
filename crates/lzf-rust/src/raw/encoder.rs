// SPDX-License-Identifier: BSD-2-Clause
// Derived from liblzf encoder logic by Stefan Traby and Marc Lehmann.
// See LICENSES/BSD-2-Clause-liblzf.txt for the preserved upstream notice.
use crate::{Error, MAX_LITERAL_LEN, MAX_MATCH_LEN, MAX_OFFSET, Result};

const HASH_LOG: usize = 16;
const HASH_SIZE: usize = 1 << HASH_LOG;
const HASH_BEST_SIZE: usize = 1 << HASH_LOG;

/// Encoder mode for raw LZF compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    /// Fast/liblzf default mode (`lzf_compress`).
    Normal,
    /// Best-compression mode (`lzf_compress_best`).
    Best,
}

#[inline]
fn hash3(input: &[u8], index: usize) -> usize {
    let v = (u32::from(input[index]) << 16)
        | (u32::from(input[index + 1]) << 8)
        | u32::from(input[index + 2]);
    ((v.wrapping_mul(0x1e35_a7bd) >> (32 - HASH_LOG - 8)) as usize) & (HASH_SIZE - 1)
}

#[inline]
fn hash_best3(input: &[u8], index: usize) -> usize {
    ((usize::from(input[index]) << 6)
        ^ (usize::from(input[index + 1]) << 3)
        ^ usize::from(input[index + 2]))
        & (HASH_BEST_SIZE - 1)
}

#[inline]
fn emit_literals(
    input: &[u8],
    out: &mut [u8],
    op: &mut usize,
    start: usize,
    end: usize,
) -> Result<()> {
    let len = end - start;
    if len == 0 {
        return Ok(());
    }
    if len <= MAX_LITERAL_LEN {
        let needed = 1 + len;
        if *op + needed > out.len() {
            return Err(Error::OutputTooSmall);
        }
        out[*op] = (len - 1) as u8;
        *op += 1;
        out[*op..*op + len].copy_from_slice(&input[start..end]);
        *op += len;
        return Ok(());
    }

    let mut cursor = start;
    while cursor < end {
        let chunk = (end - cursor).min(MAX_LITERAL_LEN);
        let needed = 1 + chunk;
        if *op + needed > out.len() {
            return Err(Error::OutputTooSmall);
        }

        out[*op] = (chunk - 1) as u8;
        *op += 1;
        out[*op..*op + chunk].copy_from_slice(&input[cursor..cursor + chunk]);
        *op += chunk;
        cursor += chunk;
    }
    Ok(())
}

#[inline]
fn emit_backref(out: &mut [u8], op: &mut usize, off: usize, len: usize) -> Result<()> {
    debug_assert!(off < MAX_OFFSET);
    debug_assert!((3..=MAX_MATCH_LEN).contains(&len));

    let l = len - 2;
    let needed = if l < 7 { 2 } else { 3 };
    if *op + needed > out.len() {
        return Err(Error::OutputTooSmall);
    }

    if l < 7 {
        out[*op] = ((l as u8) << 5) | ((off >> 8) as u8);
        *op += 1;
    } else {
        out[*op] = (7u8 << 5) | ((off >> 8) as u8);
        out[*op + 1] = (l - 7) as u8;
        *op += 2;
    }

    out[*op] = (off & 0xff) as u8;
    *op += 1;
    Ok(())
}

fn compress_normal(input: &[u8], output: &mut [u8]) -> Result<usize> {
    if input.is_empty() {
        return Ok(0);
    }

    let mut table = [0u32; HASH_SIZE];
    let mut op = 0usize;
    let mut anchor = 0usize;
    let mut pos = 0usize;

    while pos + 2 < input.len() {
        let h = hash3(input, pos);
        let prev = table[h] as usize;
        table[h] = (pos + 1) as u32;

        if prev != 0 {
            let candidate = prev - 1;
            if candidate < pos {
                let off = pos - candidate - 1;
                if off < MAX_OFFSET
                    && input[candidate] == input[pos]
                    && input[candidate + 1] == input[pos + 1]
                    && input[candidate + 2] == input[pos + 2]
                {
                    emit_literals(input, output, &mut op, anchor, pos)?;

                    let max_len = (input.len() - pos).min(MAX_MATCH_LEN);
                    let mut len = 3usize;
                    while len < max_len && input[candidate + len] == input[pos + len] {
                        len += 1;
                    }

                    emit_backref(output, &mut op, off, len)?;

                    let end = pos + len;
                    let mut scan = pos + 1;
                    while scan + 2 < end {
                        let hh = hash3(input, scan);
                        table[hh] = (scan + 1) as u32;
                        scan += 1;
                    }

                    pos = end;
                    anchor = pos;
                    continue;
                }
            }
        }

        pos += 1;
    }

    emit_literals(input, output, &mut op, anchor, input.len())?;
    Ok(op)
}

fn compress_best_impl(input: &[u8], output: &mut [u8]) -> Result<usize> {
    if input.is_empty() {
        return Ok(0);
    }

    // liblzf stores pointers; we store index+1 (0 == null).
    let mut first = [0usize; HASH_BEST_SIZE];
    let mut prev = [0u16; MAX_OFFSET];

    let in_len = input.len();
    let mut op = 0usize;
    let mut anchor = 0usize;
    let mut pos = 0usize;

    while pos + 2 < in_len {
        let hash = hash_best3(input, pos);
        let prev_head = first[hash];
        let slot = pos & (MAX_OFFSET - 1);

        prev[slot] = if prev_head == 0 {
            0
        } else {
            let p = prev_head - 1;
            (pos - p).min(usize::from(u16::MAX)) as u16
        };
        first[hash] = pos + 1;

        let mut best_len = 0usize;
        let mut best_pos = 0usize;
        let max_len = (in_len - pos).min(MAX_MATCH_LEN);
        let lower_bound = pos.saturating_sub(MAX_OFFSET);

        if prev_head != 0 {
            let mut p = prev_head - 1;
            let pos0 = input[pos];
            let pos1 = input[pos + 1];
            let pos2 = input[pos + 2];

            while p >= lower_bound {
                if input[p] == pos0
                    && input[p + 1] == pos1
                    && input[p + 2] == pos2
                    && (best_len == 0 || input[p + best_len] == input[pos + best_len])
                {
                    let mut l = 3usize;
                    while l < max_len && input[p + l] == input[pos + l] {
                        l += 1;
                    }

                    if l >= best_len {
                        best_len = l;
                        best_pos = p;
                        if l == max_len {
                            break;
                        }
                    }
                }

                let diff = usize::from(prev[p & (MAX_OFFSET - 1)]);
                if diff == 0 || p < diff {
                    break;
                }
                p -= diff;
            }
        }

        if best_len >= 3 {
            emit_literals(input, output, &mut op, anchor, pos)?;

            let off = pos - best_pos - 1;
            emit_backref(output, &mut op, off, best_len)?;

            let end = pos + best_len;
            let mut scan = pos + 1;
            while scan + 2 < end {
                let h = hash_best3(input, scan);
                let s = scan & (MAX_OFFSET - 1);
                let head = first[h];

                prev[s] = if head == 0 {
                    0
                } else {
                    let p = head - 1;
                    (scan - p).min(usize::from(u16::MAX)) as u16
                };
                first[h] = scan + 1;
                scan += 1;
            }

            pos = end;
            anchor = pos;
        } else {
            pos += 1;
        }
    }

    emit_literals(input, output, &mut op, anchor, input.len())?;
    Ok(op)
}

/// Compresses `input` into `output` using raw LZF format.
///
/// Uses the default liblzf mode (`lzf_compress`).
///
/// Returns `Error::OutputTooSmall` if `output` cannot hold the encoded stream.
///
/// For a guaranteed-capacity buffer, use `max_compressed_size(input.len())`.
pub fn compress(input: &[u8], output: &mut [u8]) -> Result<usize> {
    compress_with_mode(input, output, CompressionMode::Normal)
}

/// Compresses `input` into `output` using liblzf best-compression mode.
///
/// This mirrors `lzf_compress_best` semantics.
///
/// Returns `Error::OutputTooSmall` if `output` cannot hold the encoded stream.
pub fn compress_best(input: &[u8], output: &mut [u8]) -> Result<usize> {
    compress_best_impl(input, output)
}

/// Compresses `input` into `output` using the given encoder mode.
///
/// - `CompressionMode::Normal` tracks `liblzf` default compressor behavior.
/// - `CompressionMode::Best` tracks `liblzf` best-ratio compressor behavior.
pub fn compress_with_mode(input: &[u8], output: &mut [u8], mode: CompressionMode) -> Result<usize> {
    match mode {
        CompressionMode::Normal => compress_normal(input, output),
        CompressionMode::Best => compress_best_impl(input, output),
    }
}
