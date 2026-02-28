// SPDX-License-Identifier: ISC
use lzf_rust::{compress, decompress, max_compressed_size};

fn main() {
    let input = b"LZF LZF LZF LZF LZF";
    let mut compressed = vec![0u8; max_compressed_size(input.len())];
    let compressed_len = compress(input, &mut compressed).expect("compression failed");
    compressed.truncate(compressed_len);

    let mut restored = vec![0u8; input.len()];
    let restored_len = decompress(&compressed, &mut restored).expect("decompression failed");

    println!("in={} compressed={} out={}", input.len(), compressed_len, restored_len);
    assert_eq!(&restored, input);
}
