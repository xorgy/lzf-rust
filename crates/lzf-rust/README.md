# lzf-rust

Pure Rust LZF compression/decompression library.

`lzf-rust` supports raw LZF token streams, framed `ZV` block streams, and
streaming adapters for both `std` and `no_std + alloc` environments.

## Features

- Safe Rust raw LZF encoder/decoder (`liblzf` compatible token format)
- `ZV` block framing support compatible with the `lzf` utility stream format
- `std::io` adapters: `LzfReader` and `LzfWriter` for framed streaming I/O
- crate-level `LzfRead`/`LzfWrite` traits for `no_std` streaming
- `no_std` support (with `alloc`)

## Installation

```toml
[dependencies]
lzf-rust = "0.1"
```

`no_std` usage:

```toml
[dependencies]
lzf-rust = { version = "0.1", default-features = false, features = ["encoder"] }
```

## Usage

Raw LZF roundtrip:

```rust
use lzf_rust::{compress, decompress, max_compressed_size};

let input = b"hello hello hello hello";
let mut compressed = vec![0u8; max_compressed_size(input.len())];
let n = compress(input, &mut compressed).unwrap();
compressed.truncate(n);

let mut decompressed = vec![0u8; input.len()];
let m = decompress(&compressed, &mut decompressed).unwrap();
assert_eq!(m, input.len());
assert_eq!(&decompressed, input);
```

Framed block API:

```rust
use lzf_rust::{decode_blocks, encode_blocks};

let input = b"hello framed world";
let framed = encode_blocks(input, 32 * 1024).unwrap();
let decoded = decode_blocks(&framed).unwrap();
assert_eq!(decoded, input);
```

Streaming API:

```rust
use lzf_rust::{LzfRead, LzfReader, encode_blocks};

let input = b"streaming example";
let framed = encode_blocks(input, 4096).unwrap();
let mut src: &[u8] = &framed;
let mut reader = LzfReader::new(&mut src);
let mut out = vec![0u8; input.len()];
reader.read_exact(&mut out).unwrap();
assert_eq!(out, input);
```

## License

This crate uses file-level licensing:

- `src/raw/encoder.rs` is derived from `liblzf` encoder logic and is licensed
  under BSD-2-Clause.
- The from-scratch Rust implementation files are licensed under ISC.

License texts are provided in:

- `LICENSES/BSD-2-Clause-liblzf.txt`
- `LICENSES/ISC.txt`
