// SPDX-License-Identifier: ISC
use lzf_rust::{LzfReader, LzfWriter, Read, Write};

fn main() {
    let input = b"stream stream stream stream stream";

    let mut writer = LzfWriter::new(Vec::new(), 65535).expect("writer");
    writer.write_all(input).expect("write");
    let encoded = writer.finish().expect("finish");

    let mut reader = LzfReader::new(encoded.as_slice());
    let mut decoded = Vec::new();
    let mut chunk = [0u8; 64];
    loop {
        let n = reader.read(&mut chunk).expect("read");
        if n == 0 {
            break;
        }
        decoded.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(decoded, input);
    println!("encoded={} decoded={}", encoded.len(), decoded.len());
}
