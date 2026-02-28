// SPDX-License-Identifier: ISC
use alloc::vec;
use alloc::vec::Vec;

use crate::decompress;
#[cfg(feature = "encoder")]
use crate::{CompressionMode, compress_with_mode};
use crate::{Error, Result};

const MAGIC_0: u8 = b'Z';
const MAGIC_1: u8 = b'V';
const TYPE_UNCOMPRESSED: u8 = 0;
const TYPE_COMPRESSED: u8 = 1;
const TYPE0_HDR_SIZE: usize = 5;
const TYPE1_HDR_SIZE: usize = 7;

/// Encodes input into `lzf` block stream format (`ZV\0`/`ZV\1` blocks).
///
/// `block_size` must be in `1..=65535`.
///
/// For each block, compressed payload is used when it fits in the framed
/// compressed form; otherwise an uncompressed block is emitted.
#[cfg(feature = "encoder")]
pub fn encode_blocks(input: &[u8], block_size: usize) -> Result<Vec<u8>> {
    encode_blocks_with_mode(input, block_size, CompressionMode::Normal)
}

/// Encodes input into `lzf` block stream format (`ZV\0`/`ZV\1` blocks),
/// selecting the raw compressor mode.
///
/// `block_size` must be in `1..=65535`.
///
/// This function is format-compatible with the historical `lzf` utility block
/// stream representation.
#[cfg(feature = "encoder")]
pub fn encode_blocks_with_mode(
    input: &[u8],
    block_size: usize,
    mode: CompressionMode,
) -> Result<Vec<u8>> {
    if block_size == 0 || block_size > usize::from(u16::MAX) {
        return Err(Error::InvalidParameter);
    }

    let mut output = Vec::new();

    for block in input.chunks(block_size) {
        let max_try = block.len().saturating_sub(4);
        let mut compressed = vec![0u8; max_try];

        let encoded_len = if max_try == 0 {
            Err(Error::OutputTooSmall)
        } else {
            compress_with_mode(block, &mut compressed, mode)
        };

        match encoded_len {
            Ok(cs) => {
                let cs_u16 = u16::try_from(cs).map_err(|_| Error::InvalidParameter)?;
                let us_u16 = u16::try_from(block.len()).map_err(|_| Error::InvalidParameter)?;

                output.push(MAGIC_0);
                output.push(MAGIC_1);
                output.push(TYPE_COMPRESSED);
                output.extend_from_slice(&cs_u16.to_be_bytes());
                output.extend_from_slice(&us_u16.to_be_bytes());
                output.extend_from_slice(&compressed[..cs]);
            }
            Err(Error::OutputTooSmall) => {
                let us_u16 = u16::try_from(block.len()).map_err(|_| Error::InvalidParameter)?;

                output.push(MAGIC_0);
                output.push(MAGIC_1);
                output.push(TYPE_UNCOMPRESSED);
                output.extend_from_slice(&us_u16.to_be_bytes());
                output.extend_from_slice(block);
            }
            Err(err) => return Err(err),
        }
    }

    Ok(output)
}

/// Decodes data encoded with `encode_blocks` or the `lzf` utility stream format.
///
/// Returns `Error::InvalidHeader` for malformed frame headers and
/// `Error::UnknownBlockType` for unsupported block type tags.
///
/// # Example
///
/// ```
/// use lzf_rust::{decode_blocks, encode_blocks};
///
/// let input = b"hello framed world";
/// let framed = encode_blocks(input, 4096).unwrap();
/// let decoded = decode_blocks(&framed).unwrap();
/// assert_eq!(decoded, input);
/// ```
pub fn decode_blocks(input: &[u8]) -> Result<Vec<u8>> {
    let mut ip = 0usize;
    let mut output = Vec::new();

    while ip < input.len() {
        if input[ip] == 0 {
            break;
        }

        if input.len() - ip < TYPE0_HDR_SIZE {
            return Err(Error::InvalidHeader);
        }
        if input[ip] != MAGIC_0 || input[ip + 1] != MAGIC_1 {
            return Err(Error::InvalidHeader);
        }

        let block_type = input[ip + 2];
        match block_type {
            TYPE_UNCOMPRESSED => {
                let uncompressed_len =
                    usize::from(u16::from_be_bytes([input[ip + 3], input[ip + 4]]));
                ip += TYPE0_HDR_SIZE;
                if input.len() - ip < uncompressed_len {
                    return Err(Error::InvalidData);
                }
                output.extend_from_slice(&input[ip..ip + uncompressed_len]);
                ip += uncompressed_len;
            }
            TYPE_COMPRESSED => {
                if input.len() - ip < TYPE1_HDR_SIZE {
                    return Err(Error::InvalidHeader);
                }
                let compressed_len =
                    usize::from(u16::from_be_bytes([input[ip + 3], input[ip + 4]]));
                let uncompressed_len =
                    usize::from(u16::from_be_bytes([input[ip + 5], input[ip + 6]]));
                ip += TYPE1_HDR_SIZE;

                if input.len() - ip < compressed_len {
                    return Err(Error::InvalidData);
                }

                let mut block = vec![0u8; uncompressed_len];
                let written = decompress(&input[ip..ip + compressed_len], &mut block)?;
                if written != uncompressed_len {
                    return Err(Error::InvalidData);
                }
                output.extend_from_slice(&block);
                ip += compressed_len;
            }
            other => return Err(Error::UnknownBlockType(other)),
        }
    }

    Ok(output)
}
