use super::{AllowStd, Result};
impl super::Read for AllowStd<&'_ [u8]> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

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

impl super::Write for AllowStd<&'_ mut [u8]> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
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
