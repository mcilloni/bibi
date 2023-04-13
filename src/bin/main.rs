use std::{error::Error, ffi::OsStr, fs::read_to_string, io::stdout, path::Path};

use clap::Parser as ClapParser;

use bibi::{dump_bbcode, dump_markdown};

/// dumb test
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File to parse
    file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let Args { file } = Args::parse();

    let contents = read_to_string(&file)?;

    if let Some("md") = Path::new(&file).extension().and_then(OsStr::to_str) {
        dump_bbcode(stdout(), &contents)?;
    } else {
        dump_markdown(stdout(), &contents)?;
    }

    Ok(())
}
