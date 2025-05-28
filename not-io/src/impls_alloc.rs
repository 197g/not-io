// FIXME: specialize impls? Many are copies from `impls_nostd_noalloc.rs`
use super::{ErrorKind, Result};
use crate::alloc::{string::String, vec::Vec};

impl super::Read for &'_ [u8] {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = self.len().min(buf.len());
        buf[..len].copy_from_slice(&self[..len]);
        *self = &self[len..];
        Ok(len)
    }
}

impl super::BufRead for &'_ [u8] {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        Ok(*self)
    }

    fn consume(&mut self, n: usize) {
        *self = &self[n..];
    }
}

impl super::Write for &'_ mut [u8] {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.len().min(buf.len());
        let (head, tail) = core::mem::take(self).split_at_mut(len);
        *self = tail;
        head.copy_from_slice(buf);
        Ok(len)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl super::Write for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub(crate) fn read_to_end<R: super::Read + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize> {
    struct Guard<'vec> {
        buf: &'vec mut Vec<u8>,
        len: usize,
    }

    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.buf.truncate(self.len)
        }
    }

    let mut guard = Guard {
        len: buf.len(),
        buf,
    };
    let start_len = guard.len;

    loop {
        // Ensure room.
        if guard.buf.len() == guard.len {
            guard.buf.reserve(32);
            guard.buf.resize(guard.buf.capacity(), 0);
            // FIXME: once it's sound, use `initializer`.
        }

        let buf = &mut guard.buf[guard.len..];
        match r.read(buf) {
            Ok(0) => return Ok(guard.len - start_len),
            Ok(n) => {
                assert!(n <= buf.len());
                guard.len += n;
            }
            Err(e) if e.is_interrupted() => {}
            Err(e) => return Err(e),
        }
    }
}

pub(crate) fn read_to_string<R: super::Read + ?Sized>(
    r: &mut R,
    buf: &mut String,
) -> Result<usize> {
    append_to_string(r, buf, |r, buf| read_to_end(r, buf))
}

pub(crate) fn append_to_string<R: super::Read + ?Sized>(
    r: &mut R,
    buf: &mut String,
    mut reader: impl FnMut(&mut R, &mut Vec<u8>) -> Result<usize>,
) -> Result<usize> {
    struct Utf8Guard<'vec> {
        buf: &'vec mut Vec<u8>,
        len: usize,
    }

    let mut guard = unsafe {
        Utf8Guard {
            len: buf.len(),
            buf: buf.as_mut_vec(),
        }
    };

    let ret = reader(r, guard.buf);
    if core::str::from_utf8(&guard.buf[guard.len..]).is_err() {
        ret.and_then(|_| Err(super::Error::from(super::ErrorKind::InvalidData)))
    } else {
        guard.len = guard.buf.len();
        ret
    }
}

pub(crate) fn read_until<R: super::BufRead + ?Sized>(
    r: &mut R,
    byte: u8,
    buf: &mut Vec<u8>,
) -> Result<usize> {
    let mut read = 0;

    loop {
        let available = match r.fill_buf() {
            Ok(n) => n,
            Err(ref e) if e.is_interrupted() => continue,
            Err(e) => return Err(e),
        };

        let (done, used) = match available.iter().position(|&b| b == byte) {
            Some(n) => {
                buf.extend_from_slice(&available[..=n]);
                (true, n + 1)
            }
            None => {
                buf.extend_from_slice(available);
                (false, available.len())
            }
        };

        r.consume(used);
        read += used;

        if done || used == 0 {
            return Ok(read);
        }
    }
}

pub(crate) fn read_line<R: super::BufRead + ?Sized>(r: &mut R, buf: &mut String) -> Result<usize> {
    append_to_string(r, buf, |r, buf| read_until(r, b'\n', buf))
}

fn vec_write(pos_mut: &mut u64, vec: &mut Vec<u8>, buf: &[u8]) -> super::Result<usize> {
    let pos = usize::try_from(*pos_mut).map_err(|_| ErrorKind::InvalidInput)?;

    // Simulate zeroed content for all interior content.
    let newlen = vec.len().max(pos);
    vec.resize(newlen, 0);
    let (overwrite, fill) = buf.split_at(buf.len().min(newlen - pos));
    vec[pos..][..overwrite.len()].copy_from_slice(overwrite);
    vec.extend_from_slice(fill);

    *pos_mut = (pos + buf.len()) as u64;
    Ok(buf.len())
}

impl super::Write for super::Cursor<&mut Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> super::Result<usize> {
        vec_write(&mut self.pos, &mut *self.inner, buf)
    }

    fn flush(&mut self) -> super::Result<()> {
        Ok(())
    }
}

impl super::Write for super::Cursor<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> super::Result<usize> {
        vec_write(&mut self.pos, &mut self.inner, buf)
    }

    fn flush(&mut self) -> super::Result<()> {
        Ok(())
    }
}
