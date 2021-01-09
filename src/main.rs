#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

use std::path::{Path, PathBuf};

use anyhow::Result;
use async_stream::try_stream;
use clap::{AppSettings, Clap};
use colored::Colorize;
use crc32fast::Hasher;
use futures_util::stream::{Stream, StreamExt, TryStreamExt};
use tokio::{
    fs::{self, DirEntry, File},
    io::{AsyncReadExt, ErrorKind},
};

/// Simple CLI tool to check CRC values in file names
#[derive(Debug, Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// Whether to update a CRC code if it didn't match
    #[clap(short, long)]
    update: bool,

    /// The directory where to search for files
    #[clap(parse(from_os_str), default_value = ".")]
    dir: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let opt: Opt = Opt::parse();
    check(opt.dir, opt.update).await
}

pub async fn check<P: AsRef<Path> + Send>(dir: P, update: bool) -> Result<()> {
    let files = read_dir(dir).await?.collect::<Vec<_>>().await;

    let temp = futures_util::stream::iter(files)
        .try_filter_map(|file: DirEntry| async move {
            if file.metadata().await?.is_file() {
                Ok(Some(check_crc(file.path(), update)))
            } else {
                Ok(None)
            }
        })
        .try_buffer_unordered(num_cpus::get() * 2);

    Box::pin(temp).try_collect::<()>().await
}

async fn read_dir(dir: impl AsRef<Path> + Send) -> Result<impl Stream<Item = Result<DirEntry>>> {
    let dir = dir.as_ref().to_owned();
    let mut files = fs::read_dir(dir).await?;

    Ok(try_stream! {
        while let Some(entry) = files.next_entry().await? {
            yield entry;
        }
    })
}

async fn check_crc(file: PathBuf, update: bool) -> Result<()> {
    let name = file.file_name().unwrap().to_str().unwrap();
    let hash_bytes = match extract_hash(name)? {
        Some(v) => v,
        None => return Ok(()),
    };
    let calc_bytes = match calculate_hash(&file).await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    let result = if hash_bytes == calc_bytes {
        "OK".green()
    } else if update {
        rename_file(&file, hash_bytes, calc_bytes).await?;
        "UPDATED".yellow()
    } else {
        "MISMATCH".red()
    };

    println!("{:>8} - {}", result, name);
    Ok(())
}

fn extract_hash(name: &str) -> Result<Option<u32>> {
    let mut sub = &name[..];
    while let Some((l, r)) = find_surrounded(sub, '[', ']') {
        let hex = &sub[l + 1..r];
        if is_u32_hex(hex) {
            return Ok(Some(u32::from_str_radix(hex, 16)?));
        }
        sub = &sub[..l];
    }
    Ok(None)
}

#[inline]
fn find_surrounded(text: &str, left: char, right: char) -> Option<(usize, usize)> {
    if let Some(r) = text.rfind(right) {
        if let Some(l) = text[..r].rfind(left) {
            return Some((l, r));
        }
    }
    None
}

#[inline]
fn is_u32_hex(text: &str) -> bool {
    text.len() == 8 && text.chars().all(|c| "0123456789abcdefABCDEF".contains(c))
}

async fn calculate_hash(file: &PathBuf) -> Result<u32> {
    let mut file = File::open(file).await?;
    let mut buf = [0_u8; 8192];
    let mut hasher = Hasher::new();

    loop {
        match file.read(&mut buf).await {
            Ok(0) => return Ok(hasher.finalize()),
            Ok(len) => hasher.update(&buf[..len]),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
    }
}

async fn rename_file(file: &PathBuf, hash_bytes: u32, calc_bytes: u32) -> Result<()> {
    let crc_hash = format!("[{:08X}]", hash_bytes);
    let crc_calc = format!("[{:08X}]", calc_bytes);
    let new_name = file
        .to_str()
        .unwrap_or_default()
        .replace(&crc_hash, &crc_calc);
    fs::rename(file, new_name).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_hash_works() {
        let cases = [
            ("[11111111]", "11111111"),
            ("[aabbccdd]", "AABBCCDD"),
            ("[11111111]aa[bbb].txt", "11111111"),
            ("[11111111][22222222]", "22222222"),
        ];

        for (input, expect) in &cases {
            let result = extract_hash(input);
            if let Ok(Some(i)) = result {
                assert_eq!(expect, &format!("{:08X}", i));
            } else {
                panic!("Expected {} but got {:?}", expect, result);
            }
        }
    }

    #[test]
    fn extract_hash_fails() {
        let cases = [
            "[111]",
            "[1111111122]",
            "[aabbccdd",
            "aabbccdd]",
            "aabbccdd",
        ];

        for input in &cases {
            let result = extract_hash(input);
            if let Ok(Some(i)) = result {
                panic!("No valued expected but got {}", format!("{:08X}", i));
            }
        }
    }
}
