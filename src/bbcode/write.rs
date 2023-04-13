use std::{
    fmt,
    io::{self, Write},
};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

pub const DEFAULT_ANON_CODELANG: &str = "code";
pub const DEFAULT_ANON_ICODELANG: &str = "inline";

struct BBCode<I, W: io::Write> {
    iter: I,

    writer: W,

    at_newline: bool,
    buf: Vec<u8>,
}

impl<'a, I, W> BBCode<I, W>
where
    I: Iterator<Item = Event<'a>> + 'a,
    W: io::Write,
{
    fn new(iter: I, writer: W) -> Self {
        Self {
            iter,
            writer,
            at_newline: true,
            buf: vec![],
        }
    }

    fn ensure_newline(&mut self) -> io::Result<()> {
        if !self.at_newline {
            #[cfg(windows)]
            const LINE_END: &str = "\r\n";

            #[cfg(not(windows))]
            const LINE_END: &str = "\n";

            write!(self, "{}", LINE_END)?;
        }

        Ok(())
    }

    fn write_buf(&mut self) -> io::Result<()> {
        self.writer.write_all(&self.buf)
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        self.buf.clear();

        self.buf.write_fmt(args)?;

        self.at_newline = self.buf.last().map(|&b| b == b'\n').unwrap_or_default();

        self.write_buf()
    }

    fn run(mut self) -> io::Result<()> {
        while let Some(event) = self.iter.next() {
            use Event::*;

            match event {
                Start(tag) => {
                    self.start_tag(tag)?;
                }
                End(tag) => {
                    self.end_tag(tag)?;
                }
                Text(text) => {
                    write!(self, "{text}")?;
                }
                Code(text) => {
                    write!(self, "[c={DEFAULT_ANON_ICODELANG}]")?;
                    write!(self, "{text}")?;
                    write!(self, "[/c]")?;
                }
                SoftBreak => {
                    writeln!(self)?;
                }
                HardBreak => {
                    write!(self, "\n\n")?;
                }
                Rule => {
                    writeln!(self, "[hr]")?;
                }
                _ => continue,
            }
        }

        Ok(())
    }

    /// Writes the start of an HTML tag.
    fn start_tag(&mut self, tag: Tag<'a>) -> io::Result<()> {
        use Tag::*;

        match tag {
            Paragraph => Ok(()),
            Heading(..) => write!(self, "[big]"),
            BlockQuote => writeln!(self, "[quote]"),
            CodeBlock(info) => {
                use CodeBlockKind::*;

                let lang = match &info {
                    Fenced(info) => {
                        let lang = info.split(' ').next().unwrap();

                        if lang.is_empty() {
                            "code"
                        } else {
                            lang
                        }
                    }
                    Indented => DEFAULT_ANON_CODELANG,
                };

                writeln!(self, "[code={lang}]")
            }
            List(Some(1)) => writeln!(self, "[list type=\"1\"]"),
            List(Some(start)) => {
                writeln!(self, "[list start=\"{start}\"]")
            }
            List(None) => writeln!(self, "[list]"),
            Item => {
                self.ensure_newline()?;

                write!(self, "[*]")
            }
            Emphasis => write!(self, "[cur]"),
            Strong => write!(self, "[b]"),
            Strikethrough => write!(self, "[del]"),
            Link(_, dest, _) => {
                write!(self, "[url={dest}]")
            }
            Image(_, dest, _) => {
                write!(self, "[img]{dest}[/img]")
            }
            _ => Ok(()),
        }
    }

    fn end_tag(&mut self, tag: Tag) -> io::Result<()> {
        use Tag::*;

        match tag {
            Paragraph => {
                write!(self, "\n\n")?;
            }
            Heading(..) => {
                write!(self, "[/big]\n\n")?;
            }
            BlockQuote => {
                writeln!(self, "[/quote]")?;
            }
            CodeBlock(_) => {
                self.ensure_newline()?;
                writeln!(self, "[/code]")?;
            }
            List(_) => {
                self.ensure_newline()?;
                writeln!(self, "[/list]")?;
            }
            Item => {}
            Emphasis => {
                write!(self, "[/cur]")?;
            }
            Strong => {
                write!(self, "[/b]")?;
            }
            Strikethrough => {
                write!(self, "[/del]")?;
            }
            Link(_, _, _) => {
                write!(self, "[/url]")?;
            }
            Image(_, _, _) => {} // do nothing, the image has already been closed in the start function
            _ => {}
        }
        Ok(())
    }
}

pub fn dump_bbcode(writer: impl io::Write, contents: &str) -> io::Result<()> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(contents, options);

    BBCode::new(parser, writer).run()
}
