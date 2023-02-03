use std::io;

use pulldown_cmark::{CodeBlockKind, Event, Tag};

use crate::writefmt::WriteFmt;

struct BBCode<I, W: WriteFmt> {
    /// Iterator supplying events.
    iter: I,

    /// Writer to write to.
    writer: W,
}

impl<'a, I, W> BBCode<I, W>
where
    I: Iterator<Item = Event<'a>> + 'a,
    W: WriteFmt,
{
    fn new(iter: I, writer: W) -> Self {
        Self { iter, writer }
    }

    /// Writes a buffer, and tracks whether or not a newline was written.
    #[inline]
    fn write(&mut self, s: &str) -> io::Result<()> {
        write!(self.writer, "{}", s)
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
                    self.write("[c=inline]")?;
                    write!(self.writer, "{text}")?;
                    self.write("[/c]")?;
                }
                SoftBreak => {
                    self.write("\n")?;
                }
                HardBreak => {
                    self.write("\n\n")?;
                }
                Rule => {
                    self.write("[hr]\n")?;
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
            Heading(..) => self.write("[big]"),
            BlockQuote => self.write("[quote]\n"),
            CodeBlock(info) => {
                use CodeBlockKind::*;

                match info {
                    Fenced(info) => {
                        let lang = info.split(' ').next().unwrap();
                        let lang = if lang.is_empty() { "code" } else { lang };

                        write!(self.writer, "[code={lang}]\n")
                    }
                    Indented => self.write("[code=code]\n"),
                }
            }
            List(Some(1)) => self.write("[list type=\"1\"]\n"),
            List(Some(start)) => {
                write!(self.writer, "[list start=\"{start}\"]\n")
            }
            List(None) => self.write("[list]\n"),
            Item => self.write("[*]"),
            Emphasis => self.write("[cur]"),
            Strong => self.write("[b]"),
            Strikethrough => self.write("[del]"),
            Link(_, dest, _) => {
                write!(self.writer, "[url={dest}]")
            }
            Image(_, dest, _) => {
                write!(self.writer, "[img]{dest}")
            }
            _ => Ok(()),
        }
    }

    fn end_tag(&mut self, tag: Tag) -> io::Result<()> {
        use Tag::*;

        match tag {
            Paragraph => {
                self.write("\n\n")?;
            }
            Heading(..) => {
                self.write("[/big]\n")?;
            }
            BlockQuote => {
                self.write("[/quote]\n")?;
            }
            CodeBlock(_) => {
                self.write("[/code]\n")?;
            }
            List(_) => {
                self.write("[/list]\n")?;
            }
            Item => {
                self.write("\n")?;
            }
            Emphasis => {
                self.write("[/cur]")?;
            }
            Strong => {
                self.write("[/b]")?;
            }
            Strikethrough => {
                self.write("[/del]")?;
            }
            Link(_, _, _) => {
                self.write("[/a]")?;
            }
            Image(_, _, _) => {
                self.write("[/img]")?;
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn write_bbcode<'a, I, W>(writer: W, iter: I) -> io::Result<()>
where
    I: Iterator<Item = Event<'a>> + 'a,
    W: WriteFmt,
{
    BBCode::new(iter, writer).run()
}
