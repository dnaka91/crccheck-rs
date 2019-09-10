use std::path::PathBuf;

use structopt::clap::AppSettings;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
about = "An example of StructOpt usage.",
setting = AppSettings::ColoredHelp,
)]
struct Opt {
    #[structopt(short, long, help = "Add a CRC code to files that don't contain one")]
    add: bool,

    #[structopt(short, long, help = "Whether to update a CRC code if it didn't match")]
    update: bool,

    #[structopt(
    parse(from_os_str),
    help = "The directory where to search for files",
    default_value = "."
    )]
    dir: PathBuf,
}

fn main() {
    let opt: Opt = Opt::from_args();
    crccheck::check(opt.dir, opt.update).unwrap();
}
