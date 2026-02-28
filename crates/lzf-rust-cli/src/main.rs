// SPDX-License-Identifier: BSD-2-Clause
// Derived from the original liblzf command-line utility behavior.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use lzf_rust::{CompressionMode, LzfWriter, encode_blocks_with_mode};
use lzf_rust::{LzfReader, decode_blocks};

#[cfg(unix)]
use rustix::termios;
#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const BLOCKSIZE: usize = 1024 * 64 - 1;
const MAX_BLOCKSIZE: usize = BLOCKSIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Compress,
    Uncompress,
    Lzcat,
}

#[derive(Debug)]
struct Config {
    mode: Mode,
    force: bool,
    verbose: bool,
    best: bool,
    blocksize: usize,
    files: Vec<String>,
}

fn usage(rc: i32) -> ! {
    eprintln!();
    eprintln!("lzf-rust: LZF compression/decompression utility implemented in Rust.");
    eprintln!("Repository: https://github.com/xorgy/lzf-rust");
    eprintln!();
    eprintln!("usage: lzf [-dufhvb9] [file ...]");
    eprintln!("       unlzf [file ...]");
    eprintln!("       lzcat [file ...]");
    eprintln!();
    eprintln!("-c --compress    compress");
    eprintln!("-d --decompress  decompress");
    eprintln!("-9 --best        best compression");
    eprintln!("-f --force       force overwrite of output file");
    eprintln!("-h --help        give this help");
    eprintln!("-v --verbose     verbose mode");
    eprintln!("-b # --blocksize # set blocksize");
    eprintln!();
    std::process::exit(rc);
}

fn parse_u64_auto_radix(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(rest, 16).ok();
    }

    if s.len() > 1 && s.starts_with('0') {
        return u64::from_str_radix(&s[1..], 8).ok();
    }

    s.parse::<u64>().ok()
}

fn parse_block_size_compat(s: &str) -> usize {
    let Some(v) = parse_u64_auto_radix(s) else {
        return BLOCKSIZE;
    };
    if v == 0 || v > MAX_BLOCKSIZE as u64 { BLOCKSIZE } else { v as usize }
}

fn program_name(args0: Option<&str>) -> &str {
    args0.unwrap_or("lzf").rsplit('/').next().unwrap_or("lzf")
}

fn parse_args(args: &[String]) -> Config {
    let imagename = program_name(args.first().map(String::as_str));

    let mut mode = if imagename.starts_with("un") || imagename.starts_with("de") {
        Mode::Uncompress
    } else {
        Mode::Compress
    };
    if imagename.contains("cat") {
        mode = Mode::Lzcat;
    }

    let mut force = false;
    let mut verbose = false;
    let mut best = false;
    let mut blocksize =
        env::var("LZF_BLOCKSIZE").ok().map_or(BLOCKSIZE, |v| parse_block_size_compat(&v));

    let mut i = 1usize;
    let mut files = Vec::new();
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            files.extend(args[i + 1..].iter().cloned());
            break;
        }

        if !arg.starts_with('-') || arg == "-" {
            files.push(arg.clone());
            i += 1;
            continue;
        }

        if let Some(long) = arg.strip_prefix("--") {
            let (name, value) = long.split_once('=').map_or((long, None), |(n, v)| (n, Some(v)));
            match name {
                "compress" => mode = Mode::Compress,
                "decompress" | "uncompress" => mode = Mode::Uncompress,
                "best" => best = true,
                "force" => force = true,
                "help" => usage(0),
                "verbose" => verbose = true,
                "blocksize" => {
                    let val = if let Some(v) = value {
                        v
                    } else {
                        if i + 1 >= args.len() {
                            usage(1);
                        }
                        i += 1;
                        &args[i]
                    };
                    blocksize = parse_block_size_compat(val);
                }
                _ => usage(1),
            }
            i += 1;
            continue;
        }

        let mut chars = arg[1..].chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                'c' => mode = Mode::Compress,
                'd' => mode = Mode::Uncompress,
                '9' => best = true,
                'f' => force = true,
                'h' => usage(0),
                'v' => verbose = true,
                'b' => {
                    let inline: String = chars.collect();
                    if inline.is_empty() {
                        if i + 1 >= args.len() {
                            usage(1);
                        }
                        i += 1;
                        blocksize = parse_block_size_compat(&args[i]);
                    } else {
                        blocksize = parse_block_size_compat(&inline);
                    }
                    break;
                }
                _ => usage(1),
            }
        }

        i += 1;
    }

    Config { mode, force, verbose, best, blocksize, files }
}

#[cfg(unix)]
fn stdin_is_tty() -> bool {
    termios::isatty(io::stdin().as_fd())
}

#[cfg(not(unix))]
fn stdin_is_tty() -> bool {
    false
}

#[cfg(unix)]
fn stdout_is_tty() -> bool {
    termios::isatty(io::stdout().as_fd())
}

#[cfg(not(unix))]
fn stdout_is_tty() -> bool {
    false
}

fn compose_name(mode: Mode, input: &Path) -> Result<PathBuf, String> {
    let s = input.to_str().ok_or_else(|| format!("{}: invalid path", input.display()))?;
    match mode {
        Mode::Compress => Ok(PathBuf::from(format!("{s}.lzf"))),
        Mode::Uncompress => {
            if let Some(stripped) = s.strip_suffix(".lzf") {
                Ok(PathBuf::from(stripped))
            } else {
                Err(format!("{s}: unknown suffix"))
            }
        }
        Mode::Lzcat => Ok(PathBuf::new()),
    }
}

fn read_all(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

fn write_all(path: &Path, data: &[u8], force: bool) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    if !force {
        opts.create_new(true);
    }
    let mut f = opts.open(path)?;
    f.write_all(data)
}

fn encode_bytes(imagename: &str, input: &[u8], cfg: &Config) -> Result<Vec<u8>, ()> {
    let mode = if cfg.best { CompressionMode::Best } else { CompressionMode::Normal };
    encode_blocks_with_mode(input, cfg.blocksize, mode).map_err(|_| {
        eprintln!("{imagename}: compress failed");
    })
}

fn decode_bytes(imagename: &str, input: &[u8]) -> Result<Vec<u8>, ()> {
    decode_blocks(input).map_err(|_| {
        eprintln!("{imagename}: decompress: invalid stream - data corrupted");
    })
}

fn print_verbose(mode: Mode, src: &Path, dst: &Path, nr_read: usize, nr_written: usize) {
    let pct = match mode {
        Mode::Compress => {
            if nr_read == 0 {
                0.0
            } else {
                100.0 - (nr_written as f64 / (nr_read as f64 / 100.0))
            }
        }
        Mode::Uncompress | Mode::Lzcat => {
            if nr_written == 0 {
                0.0
            } else {
                100.0 - (nr_read as f64 / (nr_written as f64 / 100.0))
            }
        }
    };

    eprintln!("{}:  {:5.1}% -- replaced with {}", src.display(), pct, dst.display());
}

fn run_file(imagename: &str, cfg: &Config, file: &str) -> i32 {
    let input = Path::new(file);

    let in_meta = match fs::symlink_metadata(input) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{imagename}: {}: {e}", input.display());
            return 1;
        }
    };

    if !in_meta.file_type().is_file() {
        eprintln!("{imagename}: {}: not a regular file.", input.display());
        return 1;
    }

    let out_path = if cfg.mode == Mode::Lzcat {
        PathBuf::new()
    } else {
        match compose_name(cfg.mode, input) {
            Ok(p) => p,
            Err(msg) => {
                eprintln!("{imagename}: {msg}");
                return 1;
            }
        }
    };

    let in_bytes = match read_all(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{imagename}: {}: {e}", input.display());
            return 1;
        }
    };

    let out_bytes = match cfg.mode {
        Mode::Compress => match encode_bytes(imagename, &in_bytes, cfg) {
            Ok(o) => o,
            Err(()) => return 1,
        },
        Mode::Uncompress | Mode::Lzcat => match decode_bytes(imagename, &in_bytes) {
            Ok(o) => o,
            Err(()) => return 1,
        },
    };

    if cfg.mode == Mode::Lzcat {
        if io::stdout().write_all(&out_bytes).is_err() {
            eprintln!("{imagename}: write error");
            return 1;
        }
        return 0;
    }

    if let Err(e) = write_all(&out_path, &out_bytes, cfg.force) {
        eprintln!("{imagename}: {}: {e}", out_path.display());
        return 1;
    }

    #[cfg(unix)]
    {
        let mode = in_meta.permissions().mode();
        let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
    }

    if cfg.verbose {
        print_verbose(cfg.mode, input, &out_path, in_bytes.len(), out_bytes.len());
    }

    if let Err(e) = fs::remove_file(input) {
        eprintln!("{imagename}: {}: {e}", input.display());
        return 1;
    }

    0
}

fn run_stdio(imagename: &str, cfg: &Config) -> i32 {
    if !cfg.force {
        if matches!(cfg.mode, Mode::Uncompress | Mode::Lzcat) && stdin_is_tty() {
            eprintln!(
                "{imagename}: compressed data not read from a terminal. Use -f to force decompression."
            );
            return 1;
        }
        if cfg.mode == Mode::Compress && stdout_is_tty() {
            eprintln!(
                "{imagename}: compressed data not written to a terminal. Use -f to force compression."
            );
            return 1;
        }
    }

    let mut in_lock = io::stdin().lock();
    let mut out_lock = io::stdout().lock();

    match cfg.mode {
        Mode::Compress => {
            let mode = if cfg.best { CompressionMode::Best } else { CompressionMode::Normal };
            let mut writer = match LzfWriter::new_with_mode(&mut out_lock, cfg.blocksize, mode) {
                Ok(w) => w,
                Err(_) => {
                    eprintln!("{imagename}: compress failed");
                    return 1;
                }
            };

            let read_chunk = cfg.blocksize.saturating_mul(16).clamp(1, 1 << 20);
            let mut buf = vec![0u8; read_chunk];
            loop {
                let n = match io::Read::read(&mut in_lock, &mut buf[..]) {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("{imagename}: read error");
                        return 1;
                    }
                };
                if n == 0 {
                    break;
                }
                if lzf_rust::Write::write_all(&mut writer, &buf[..n]).is_err() {
                    eprintln!("{imagename}: write error");
                    return 1;
                }
            }

            if writer.finish().is_err() {
                eprintln!("{imagename}: write error");
                return 1;
            }
            0
        }
        Mode::Uncompress | Mode::Lzcat => {
            let mut reader = LzfReader::new(&mut in_lock);
            let mut buf = vec![0u8; 1024 * 1024];

            loop {
                let n = match lzf_rust::Read::read(&mut reader, &mut buf[..]) {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("{imagename}: decompress: invalid stream - data corrupted");
                        return 1;
                    }
                };
                if n == 0 {
                    break;
                }
                if io::Write::write_all(&mut out_lock, &buf[..n]).is_err() {
                    eprintln!("{imagename}: write error");
                    return 1;
                }
            }

            0
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cfg = parse_args(&args);
    let imagename = program_name(args.first().map(String::as_str));

    let mut rc = 0i32;
    if cfg.files.is_empty() {
        rc |= run_stdio(imagename, &cfg);
    } else {
        for f in &cfg.files {
            rc |= run_file(imagename, &cfg, f);
        }
    }

    std::process::exit(if rc == 0 { 0 } else { 1 });
}
