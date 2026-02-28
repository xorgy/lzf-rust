// SPDX-License-Identifier: ISC
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use lzf_rust::{Error, decode_blocks, decompress};

fn regression_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join("regression").join(kind)
}

fn parse_expected_error(text: &str) -> Error {
    let trimmed = text.trim();
    match trimmed {
        "Eof" => Error::Eof,
        "Interrupted" => Error::Interrupted,
        "OutputTooSmall" => Error::OutputTooSmall,
        "WriteZero" => Error::WriteZero,
        "InvalidData" => Error::InvalidData,
        "InvalidHeader" => Error::InvalidHeader,
        "InvalidParameter" => Error::InvalidParameter,
        "Other" => Error::Other,
        _ if trimmed.starts_with("UnknownBlockType:") => {
            let suffix = &trimmed["UnknownBlockType:".len()..];
            let value = suffix
                .parse::<u8>()
                .unwrap_or_else(|_| panic!("invalid UnknownBlockType value: {suffix}"));
            Error::UnknownBlockType(value)
        }
        _ => panic!("unknown expected error '{trimmed}'"),
    }
}

fn load_case_stems(dir: &Path) -> Vec<String> {
    let mut stems = Vec::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("read_dir entry {}: {e}", dir.display()));
        let path = entry.path();

        if path.extension() != Some(OsStr::new("in")) {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_else(|| panic!("invalid fixture name: {}", path.display()))
            .to_owned();
        stems.push(stem);
    }

    stems.sort();
    stems
}

#[test]
fn regression_raw_fixtures() {
    let dir = regression_dir("raw");
    let stems = load_case_stems(&dir);
    assert!(!stems.is_empty(), "no regression fixtures found in {}", dir.display());

    for stem in stems {
        let input_path = dir.join(format!("{stem}.in"));
        let out_path = dir.join(format!("{stem}.out"));
        let err_path = dir.join(format!("{stem}.err"));

        let input =
            fs::read(&input_path).unwrap_or_else(|e| panic!("read {}: {e}", input_path.display()));
        let has_out = out_path.exists();
        let has_err = err_path.exists();
        assert!(has_out ^ has_err, "case {stem}: expected exactly one of .out/.err");

        if has_out {
            let expected =
                fs::read(&out_path).unwrap_or_else(|e| panic!("read {}: {e}", out_path.display()));
            let mut output = vec![0u8; expected.len()];
            let written =
                decompress(&input, &mut output).unwrap_or_else(|e| panic!("case {stem}: {e}"));
            assert_eq!(written, expected.len(), "case {stem}: output len");
            assert_eq!(output, expected, "case {stem}: output mismatch");
        } else {
            let expected_err = parse_expected_error(
                &fs::read_to_string(&err_path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", err_path.display())),
            );
            let mut output = vec![0u8; 1 << 20];
            let err =
                decompress(&input, &mut output).expect_err(&format!("case {stem}: expected error"));
            assert_eq!(err, expected_err, "case {stem}: error mismatch");
        }
    }
}

#[test]
fn regression_framed_fixtures() {
    let dir = regression_dir("framed");
    let stems = load_case_stems(&dir);
    assert!(!stems.is_empty(), "no regression fixtures found in {}", dir.display());

    for stem in stems {
        let input_path = dir.join(format!("{stem}.in"));
        let out_path = dir.join(format!("{stem}.out"));
        let err_path = dir.join(format!("{stem}.err"));

        let input =
            fs::read(&input_path).unwrap_or_else(|e| panic!("read {}: {e}", input_path.display()));
        let has_out = out_path.exists();
        let has_err = err_path.exists();
        assert!(has_out ^ has_err, "case {stem}: expected exactly one of .out/.err");

        if has_out {
            let expected =
                fs::read(&out_path).unwrap_or_else(|e| panic!("read {}: {e}", out_path.display()));
            let got = decode_blocks(&input).unwrap_or_else(|e| panic!("case {stem}: {e}"));
            assert_eq!(got, expected, "case {stem}: output mismatch");
        } else {
            let expected_err = parse_expected_error(
                &fs::read_to_string(&err_path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", err_path.display())),
            );
            let err = decode_blocks(&input).expect_err(&format!("case {stem}: expected error"));
            assert_eq!(err, expected_err, "case {stem}: error mismatch");
        }
    }
}
