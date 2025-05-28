use super::{AllowStd, Error, ErrorInner, Result};
use std::io;
use std::io::{IoSlice, IoSliceMut};

impl<R: io::Read> super::Read for AllowStd<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        io::Read::read(&mut self.0, buf).map_err(Error::from)
    }
}

impl<R: io::Read> io::Read for AllowStd<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }
}

impl<W: io::Write> super::Write for AllowStd<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        io::Write::write(&mut self.0, buf).map_err(Error::from)
    }
    fn flush(&mut self) -> Result<()> {
        io::Write::flush(&mut self.0).map_err(Error::from)
    }
}

impl<W: io::Write> io::Write for AllowStd<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
    fn write_vectored(&mut self, bufs: &[IoSlice]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error {
            inner: ErrorInner::Error(err),
        }
    }
}

impl From<Error> for std::io::Error {
    fn from(err: Error) -> std::io::Error {
        let ErrorInner::Error(io) = err.inner;
        io
    }
}

impl core::fmt::Debug for super::ErrorInner {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let super::ErrorInner::Error(io) = self;
        core::fmt::Debug::fmt(io, f)
    }
}

impl super::Error {
    pub(crate) fn is_interrupted_impl(&self) -> bool {
        match &self.inner {
            ErrorInner::Error(err) => err.kind() == io::ErrorKind::Interrupted,
        }
    }

    pub(crate) fn from_kind_impl(kind: super::ErrorKind) -> Self {
        use super::ErrorKind::*;
        io::Error::from(match kind {
            WriteZero => io::ErrorKind::WriteZero,
            UnexpectedEof => io::ErrorKind::UnexpectedEof,
            Interrupted => io::ErrorKind::Interrupted,
            WouldBlock => io::ErrorKind::WouldBlock,
            InvalidData => io::ErrorKind::InvalidData,
            InvalidInput => io::ErrorKind::InvalidInput,
        })
        .into()
    }
}
