use std::{fmt, io};

pub trait WriteFmt {
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()>;
}

pub struct IoWriter<W>(pub W);

impl<W> WriteFmt for IoWriter<W>
where
    W: io::Write,
{
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        self.0.write_fmt(args)
    }
}

pub struct FmtWriter<W>(pub W);
impl<W> WriteFmt for FmtWriter<W>
where
    W: fmt::Write,
{
    fn write_fmt(&mut self, args: fmt::Arguments) -> io::Result<()> {
        // FIXME: translate fmt error to io error?
        self.0
            .write_fmt(args)
            .map_err(|_| io::ErrorKind::Other.into())
    }
}
