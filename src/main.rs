#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use std::path::PathBuf;

use anyhow::Result;
use structopt::clap::AppSettings;
use structopt::StructOpt;

/// Simple CLI tool to check CRC values in file names
#[derive(Debug, StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// Whether to update a CRC code if it didn't match
    #[structopt(short, long)]
    update: bool,

    /// The directory where to search for files
    #[structopt(parse(from_os_str), default_value = ".")]
    dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    crccheck::check(opt.dir, opt.update).await
}
