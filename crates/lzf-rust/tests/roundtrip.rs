// SPDX-License-Identifier: ISC
use lzf_rust::{
    Error, compress, decode_blocks, decompress, decompress_into_vec, encode_blocks,
    max_compressed_size,
};

fn lcg_data(size: usize) -> Vec<u8> {
    let mut x = 0x1234_5678u32;
    let mut out = vec![0u8; size];
    for b in &mut out {
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (x >> 24) as u8;
    }
    out
}

#[test]
fn raw_roundtrip_small_cases() {
    let cases: [&[u8]; 5] = [
        b"",
        b"a",
        b"aaaaaa",
        b"abcabcabcabcabcabc",
        b"the quick brown fox jumps over the lazy dog",
    ];

    for input in cases {
        let mut compressed = vec![0u8; max_compressed_size(input.len())];
        let compressed_len = compress(input, &mut compressed).expect("compress");
        compressed.truncate(compressed_len);

        let mut output = vec![0u8; input.len()];
        let restored_len = decompress(&compressed, &mut output).expect("decompress");
        assert_eq!(restored_len, input.len());
        assert_eq!(&output, input);
    }
}

#[test]
fn raw_roundtrip_random_data() {
    for size in [1usize, 3, 32, 257, 4096, 16384] {
        let input = lcg_data(size);

        let mut compressed = vec![0u8; max_compressed_size(input.len())];
        let compressed_len = compress(&input, &mut compressed).expect("compress");
        compressed.truncate(compressed_len);

        let restored = decompress_into_vec(&compressed, input.len()).expect("decompress_into_vec");
        assert_eq!(restored, input);
    }
}

#[test]
fn framed_roundtrip() {
    let input = lcg_data(200_000);
    let encoded = encode_blocks(&input, 65535).expect("encode_blocks");
    let decoded = decode_blocks(&encoded).expect("decode_blocks");
    assert_eq!(decoded, input);
}

#[test]
fn invalid_back_reference_is_rejected() {
    let mut out = [0u8; 16];
    let err = decompress(&[0b0010_0000, 0x00], &mut out).expect_err("expected invalid backref");
    assert_eq!(err, Error::InvalidData);
}

#[test]
fn too_small_output_fails() {
    let input = b"aaaaabaaaaabaaaaabaaaaab";
    let mut compressed = vec![0u8; max_compressed_size(input.len())];
    let compressed_len = compress(input, &mut compressed).expect("compress");
    compressed.truncate(compressed_len);

    let mut out = vec![0u8; input.len() - 1];
    let err = decompress(&compressed, &mut out).expect_err("expected output-too-small");
    assert_eq!(err, Error::OutputTooSmall);
}
