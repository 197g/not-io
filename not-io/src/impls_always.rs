use super::{BufRead, Cursor, Empty, Read, Repeat, Result, Seek, SeekFrom, Sink, Write};
use crate::{ErrorKind, Take};

impl<T> Read for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read = Read::read(&mut self.fill_buf()?, buf)?;
        self.consume(read);
        Ok(read)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        Read::read_exact(&mut self.fill_buf()?, buf)?;
        self.consume(buf.len());
        Ok(())
    }
}

impl<T> BufRead for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn fill_buf(&mut self) -> Result<&[u8]> {
        let buffer = self.inner.as_ref();
        let pos = self.pos.min(buffer.len() as u64) as usize;
        Ok(&buffer[pos..])
    }
    fn consume(&mut self, amt: usize) {
        self.pos += amt as u64;
    }
}

impl Write for Cursor<&mut [u8]> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let pos = self.pos.min(self.inner.len() as u64) as usize;
        let ref mut slice = &mut self.inner[pos..];
        let n = Write::write(slice, buf)?;
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<T> Seek for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let (base_pos, offset) = match pos {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            SeekFrom::End(n) => (self.inner.as_ref().len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };

        let new_pos = if offset >= 0 {
            base_pos.checked_add(offset as u64)
        } else {
            base_pos.checked_sub(offset.wrapping_neg() as u64)
        };

        self.pos = new_pos.ok_or_else(|| ErrorKind::InvalidInput)?;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.pos)
    }
}

impl Read for Empty {
    fn read(&mut self, _: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}

impl BufRead for Empty {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        Ok(&[])
    }

    fn consume(&mut self, _: usize) {}
}

impl Seek for Empty {
    fn seek(&mut self, _: SeekFrom) -> Result<u64> {
        Ok(0)
    }
    fn stream_position(&mut self) -> Result<u64> {
        Ok(0)
    }
}

impl Read for Repeat {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        buf.iter_mut().for_each(|b| *b = self.byte);
        Ok(buf.len())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        Read::read(self, buf)?;
        Ok(())
    }
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Write for &'_ Sink {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

// FIXME: move to impls_nostd and dispatch to std if available.
// FIXME: is there any way to do stack probing any other way?
pub fn stack_copy<R, W>(read: &mut R, write: &mut W) -> Result<u64>
where
    R: Read + ?Sized,
    W: Write + ?Sized,
{
    const DEFAULT_STACK_BUFFER_SIZE: usize = 512;

    let mut written = 0;
    let mut buffer = [0u8; DEFAULT_STACK_BUFFER_SIZE];

    loop {
        let len = match read.read(&mut buffer[..]) {
            Ok(0) => return Ok(written),
            Err(ref e) if e.is_interrupted() => continue,
            other => other?,
        };

        write.write_all(&buffer[..len])?;
        written += len as u64;
    }
}

#[inline(always)]
fn cap_min(limit: u64, len: usize) -> usize {
    usize::try_from(limit).unwrap_or(len).min(len)
}

// FIXME: in std this specializes `read_to_end` which would be done in impls_alloc.
impl<R: Read> Read for Take<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }

        let len = cap_min(self.limit, buf.len());
        let n = self.inner.read(&mut buf[..len])?;
        // This is an opinion. See <https://github.com/rust-lang/rust/issues/94981>
        self.limit = self.limit.saturating_sub(n as u64);
        Ok(n)
    }
}

impl<T: BufRead> BufRead for Take<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        // Don't call into inner reader at all at EOF because it may still block
        if self.limit == 0 {
            return Ok(&[]);
        }

        let buf = self.inner.fill_buf()?;
        let len = cap_min(self.limit, buf.len());
        Ok(&buf[..len])
    }

    fn consume(&mut self, amt: usize) {
        // Don't let callers reset the limit by passing an overlarge value
        let amt = cap_min(self.limit, amt);
        self.limit -= amt as u64;
        self.inner.consume(amt);
    }
}
