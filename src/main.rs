use std::{error::Error, fs::read_to_string, io::stdout};

use clap::Parser as ClapParser;
use pulldown_cmark::{Options, Parser};
use writefmt::IoWriter;

mod bbcode;
mod writefmt;

/// dumb test
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File to parse
    file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let Args { file } = Args::parse();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let contents = read_to_string(file)?;

    let parser = Parser::new_ext(&contents, options);

    bbcode::write_bbcode(IoWriter(stdout()), parser)?;

    Ok(())
}
