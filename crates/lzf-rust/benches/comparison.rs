// SPDX-License-Identifier: ISC
use divan::{
    Bencher, black_box,
    counter::{BytesCount, ItemsCount},
    main,
};
use lzf_rust::{compress, decompress, max_compressed_size};

const SIZES: [usize; 3] = [1024, 8 * 1024, 64 * 1024];

fn gen_input(size: usize) -> Vec<u8> {
    let mut input = vec![0u8; size];
    for (i, b) in input.iter_mut().enumerate() {
        *b = ((i as u32).wrapping_mul(1103515245).wrapping_add(12345) >> 16) as u8;
    }
    input
}

#[divan::bench_group]
mod lzf {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn compress_rust(bencher: Bencher, size: usize) {
        let input = gen_input(size);
        let out_len = max_compressed_size(input.len());

        bencher
            .counter(BytesCount::new(input.len()))
            .counter(ItemsCount::new(1u64))
            .with_inputs(|| vec![0u8; out_len])
            .bench_refs(|out| {
                let written = compress(&input, out).expect("compress");
                black_box(written);
            });
    }

    #[divan::bench(args = SIZES)]
    fn decompress_rust_from_rust_stream(bencher: Bencher, size: usize) {
        let input = gen_input(size);
        let mut compressed = vec![0u8; max_compressed_size(input.len())];
        let compressed_len = compress(&input, &mut compressed).expect("compress baseline");
        compressed.truncate(compressed_len);

        bencher
            .counter(BytesCount::new(input.len()))
            .counter(ItemsCount::new(1u64))
            .with_inputs(|| vec![0u8; input.len()])
            .bench_refs(|out| {
                let written = decompress(&compressed, out).expect("decompress");
                black_box(written);
            });
    }

    #[divan::bench(args = SIZES)]
    fn decompress_rust_from_rust_buf(bencher: Bencher, size: usize) {
        let input = gen_input(size);
        let mut compressed = vec![0u8; max_compressed_size(input.len())];
        let compressed_len = compress(&input, &mut compressed).expect("compress baseline");
        compressed.truncate(compressed_len);

        bencher
            .counter(BytesCount::new(input.len()))
            .counter(ItemsCount::new(1u64))
            .with_inputs(|| vec![0u8; input.len()])
            .bench_refs(|out| {
                let written = decompress(&compressed, out).expect("decompress");
                black_box(written);
            });
    }
}
