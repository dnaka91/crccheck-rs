use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;

use colored::*;
use crc32fast::Hasher;
use crossbeam_utils::sync::WaitGroup;
use failure::{err_msg, Error};
use threadpool::ThreadPool;

pub fn check<P: AsRef<Path>>(files: &Vec<P>, update: bool, add: bool) -> Result<(), Error> {
    let pool = ThreadPool::new(num_cpus::get() * 4);
    let wg = WaitGroup::new();

    for file in files {
        let file = file.as_ref();
        if file.is_dir() {
            continue;
        }

        let wg = wg.clone();
        let file = file.to_path_buf();
        pool.execute(move || {
            check_crc(file.as_path(), update, add).unwrap();
            drop(wg);
        });
    }

    wg.wait();
    Ok(())
}

fn check_crc(file: &Path, update: bool, add: bool) -> Result<(), Error> {
    let name = file.file_name().unwrap().to_str().unwrap();
    let hash_bytes = extract_hash(name)?;
    let calc_bytes = match calculate_hash(file) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    let result = match hash_bytes {
        None => {
            if add {
                add_file_hash(file, calc_bytes)?;
                "ADDED".blue()
            } else {
                "SKIPPED".magenta()
            }
        }
        Some(hash_bytes) => {
            if hash_bytes == calc_bytes {
                "OK".green()
            } else if update {
                update_file_hash(file, hash_bytes, calc_bytes)?;
                "UPDATED".yellow()
            } else {
                "MISMATCH".red()
            }
        }
    };

    println!("{:>8} - {}", result, name);
    Ok(())
}

fn extract_hash(name: &str) -> Result<Option<u32>, Error> {
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

fn calculate_hash(file: &Path) -> Result<u32, Error> {
    let mut file = File::open(file)?;
    let mut buf = [0u8; 8192];
    let mut hasher = Hasher::new();

    loop {
        match file.read(&mut buf) {
            Ok(0) => return Ok(hasher.finalize()),
            Ok(len) => hasher.update(&buf[..len]),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
    }
}

fn add_file_hash(file: &Path, hash_bytes: u32) -> Result<(), Error> {
    if let Some(name) = file.to_str() {
        let mut name = name.to_owned();
        if let Some(i) = name.rfind(".") {
            name.insert_str(i, &format!("[{:08X}]", hash_bytes));
            fs::rename(file, name)?;
            return Ok(());
        }
    }
    Err(err_msg("can't add hash to file name"))
}

fn update_file_hash(file: &Path, hash_bytes: u32, calc_bytes: u32) -> Result<(), Error> {
    let crc_hash = format!("[{:08X}]", hash_bytes);
    let crc_calc = format!("[{:08X}]", calc_bytes);
    let new_name = file
        .to_str()
        .ok_or(err_msg("can't update hash of file"))?
        .replace(&crc_hash, &crc_calc);
    fs::rename(file, new_name)?;
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
