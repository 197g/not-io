use super::{AllowStd, Result};
use crate::alloc::vec::Vec;

impl super::Read for AllowStd<&'_ [u8]> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
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

impl super::Write for AllowStd<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
