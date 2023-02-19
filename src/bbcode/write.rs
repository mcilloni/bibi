use std::{
    fmt,
    io::{self, Write},
};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

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
                    write!(self, "[c=inline]")?;
                    write!(self, "{text}")?;
                    write!(self, "[/c]")?;
                }
                SoftBreak => {
                    write!(self, "\n")?;
                }
                HardBreak => {
                    write!(self, "\n\n")?;
                }
                Rule => {
                    write!(self, "[hr]\n")?;
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
            BlockQuote => write!(self, "[quote]\n"),
            CodeBlock(info) => {
                use CodeBlockKind::*;

                match info {
                    Fenced(info) => {
                        let lang = info.split(' ').next().unwrap();
                        let lang = if lang.is_empty() { "code" } else { lang };

                        write!(self, "[code={lang}]\n")
                    }
                    Indented => write!(self, "[code=code]\n"),
                }
            }
            List(Some(1)) => write!(self, "[list type=\"1\"]\n"),
            List(Some(start)) => {
                write!(self, "[list start=\"{start}\"]\n")
            }
            List(None) => write!(self, "[list]\n"),
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
                write!(self, "[/quote]\n")?;
            }
            CodeBlock(_) => {
                self.ensure_newline()?;
                write!(self, "[/code]\n")?;
            }
            List(_) => {
                self.ensure_newline()?;
                write!(self, "[/list]\n")?;
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

pub fn dump_bbcode<'a, W>(writer: W, contents: &str) -> io::Result<()>
where
    W: io::Write,
{
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(contents, options);

    BBCode::new(parser, writer).run()
}
