// SPDX-License-Identifier: ISC
use lzf_rust::{LzfReader, LzfWriter, Read, Write};
use std::cell::RefCell;
use std::rc::Rc;

fn pattern_data(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size];
    for (i, b) in out.iter_mut().enumerate() {
        *b = ((i * 17) ^ (i >> 3) ^ 0x5a) as u8;
    }
    out
}

fn read_all<R: Read>(reader: &mut R) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = reader.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    out
}

#[derive(Clone)]
struct SharedVecWriter(Rc<RefCell<Vec<u8>>>);

impl Write for SharedVecWriter {
    fn write(&mut self, buf: &[u8]) -> lzf_rust::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> lzf_rust::Result<()> {
        Ok(())
    }
}

#[test]
fn writer_reader_roundtrip() {
    let input = pattern_data(180_000);

    let mut writer = LzfWriter::new(Vec::new(), 65535).expect("writer");
    writer.write_all(&input[..10_000]).expect("write 1");
    writer.write_all(&input[10_000..]).expect("write 2");
    let encoded = writer.finish().expect("finish");

    let mut reader = LzfReader::new(encoded.as_slice());
    let output = read_all(&mut reader);

    assert_eq!(output, input);
}

#[test]
fn writer_with_eof_marker_roundtrip() {
    let input = pattern_data(4097);

    let mut writer = LzfWriter::new_with_eof_marker(Vec::new(), 4096).expect("writer");
    writer.write_all(&input).expect("write");
    let encoded = writer.finish().expect("finish");

    assert_eq!(encoded.last().copied(), Some(0));

    let mut reader = LzfReader::new(encoded.as_slice());
    let output = read_all(&mut reader);

    assert_eq!(output, input);
}

#[test]
fn reader_handles_small_buffers() {
    let input = pattern_data(30_000);

    let mut writer = LzfWriter::new(Vec::new(), 8192).expect("writer");
    writer.write_all(&input).expect("write");
    let encoded = writer.finish().expect("finish");

    let mut reader = LzfReader::new(encoded.as_slice());
    let mut output = Vec::new();
    let mut chunk = [0u8; 7];

    loop {
        let n = reader.read(&mut chunk).expect("read");
        if n == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(output, input);
}

#[test]
fn auto_finish_flushes_on_drop() {
    let input = pattern_data(20_000);
    let shared = Rc::new(RefCell::new(Vec::<u8>::new()));

    {
        let sink = SharedVecWriter(shared.clone());
        let mut writer = LzfWriter::new(sink, 4096).expect("writer").auto_finish();
        writer.write_all(&input).expect("write");
        // no explicit finish()
    }

    let encoded = shared.borrow().clone();
    let mut reader = LzfReader::new(encoded.as_slice());
    let output = read_all(&mut reader);
    assert_eq!(output, input);
}
