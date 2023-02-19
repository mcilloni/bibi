use std::{error::Error, ffi::OsStr, fs::read_to_string, io::stdout, path::Path};

use clap::Parser as ClapParser;

mod bbcode;

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
        bbcode::dump_bbcode(stdout(), &contents)?;
    } else {
        bbcode::dump_markdown(stdout(), &contents)?;
    }

    Ok(())
}
