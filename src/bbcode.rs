use std::{io::{self, Write}, fmt};

use pulldown_cmark::{CodeBlockKind, Event, Tag};

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
        Self { iter, writer, at_newline: true, buf: vec![] }
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        self.buf.clear();

        self.buf.write_fmt(args)?;

        self.at_newline = self.buf.last().map(|&b| b == b'\n').unwrap_or_default();

        self.writer.write_all(&self.buf)
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
                    write!(self.writer, "{text}")?;
                }
                Code(text) => {
                    write!(self, "[c=inline]")?;
                    write!(self.writer, "{text}")?;
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

                        write!(self.writer, "[code={lang}]\n")
                    }
                    Indented => write!(self, "[code=code]\n"),
                }
            }
            List(Some(1)) => write!(self, "[list type=\"1\"]\n"),
            List(Some(start)) => {
                write!(self.writer, "[list start=\"{start}\"]\n")
            }
            List(None) => write!(self, "[list]\n"),
            Item => {
                if self.at_newline {
                    write!(self, "[*]")
                } else {
                    write!(self, "\n[*]")
                }
            }
            Emphasis => write!(self, "[cur]"),
            Strong => write!(self, "[b]"),
            Strikethrough => write!(self, "[del]"),
            Link(_, dest, _) => {
                write!(self.writer, "[url={dest}]")
            }
            Image(_, dest, _) => {
                write!(self.writer, "[img]{dest}[/img]")
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
                write!(self, "[/code]\n")?;
            }
            List(_) => {
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
                write!(self, "[/a]")?;
            }
            Image(_, _, _) => {} // do nothing, the image has already been closed in the start function
            _ => {}
        }
        Ok(())
    }
}

pub fn write_bbcode<'a, I, W>(writer: W, iter: I) -> io::Result<()>
where
    I: Iterator<Item = Event<'a>> + 'a,
    W: io::Write,
{
    BBCode::new(iter, writer).run()
}
