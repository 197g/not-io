//! Demonstrates the use case of non-monomorphizing decoder.
use flexible_io::{reader::ReaderMut, Reader};
use std::io::{BufRead, Read};

#[test]
fn finds_the_right_data() {
    let data = b"Hello\0World\0";

    {
        let cursor = std::io::Cursor::new(data);
        let reader = Reader::new(cursor);
        let mut string_list_file = StringListFile { inner: reader };
        StringListFile::skip_one_string(&mut string_list_file).unwrap();
        assert_eq!(string_list_file.inner.get_ref().position(), 6);
    }

    {
        let cursor = std::io::Cursor::new(data);
        let mut reader = Reader::new(cursor);
        reader.set_buf();

        let mut string_list_file = StringListFile { inner: reader };
        StringListFile::skip_one_string(&mut string_list_file).unwrap();
        assert_eq!(string_list_file.inner.get_ref().position(), 6);
    }

    {
        let cursor = std::io::Cursor::new(data);
        let mut reader = Reader::new(cursor);
        reader.set_seek();

        let mut string_list_file = StringListFile { inner: reader };
        StringListFile::skip_one_string(&mut string_list_file).unwrap();
        assert_eq!(string_list_file.inner.get_ref().position(), 6);
    }
}

pub struct StringListFile<R: ?Sized> {
    inner: Reader<R>,
}

impl StringListFile<dyn Read + '_> {
    fn skip_one_string(&mut self) -> std::io::Result<()> {
        skip_one_string(self.inner.as_mut())
    }
}

/// Reader a nul-terminated string, discard it, make sure the reader is then positioned at the next
/// byte behind it.
fn skip_one_string(mut reader: ReaderMut) -> std::io::Result<()> {
    if let Some(bufreader) = reader.as_buf_mut() {
        return skip_bufread(bufreader);
    } else if reader.as_seek_mut().is_some() {
        // Over-read and seek back.
        let mut buffer = std::io::BufReader::new(reader.as_read_mut());
        skip_bufread(&mut buffer)?;
        // Find out how much we have read to buffer that we should not have.
        let back = buffer.buffer().len() as i64;
        reader.as_seek_mut().unwrap().seek_relative(-back)?;
        Ok(())
    } else {
        // Oops, we must read byte-for-byte. Damn.
        let mut byte = [0u8; 1];
        let reader = reader.as_read_mut();
        loop {
            reader.read_exact(&mut byte)?;
            if byte[0] == b'\0' {
                return Ok(());
            }
        }
    }
}

fn skip_bufread(bufread: &mut dyn BufRead) -> std::io::Result<()> {
    // Use efficient string search for BufRead.
    loop {
        let buf = bufread.fill_buf()?;

        if buf.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "No nul-byte found",
            ));
        }

        // `split_once` may have better performance (definitely uses `memchr`).
        let pos = buf.iter().position(|&x| x == b'\0');
        let skip = pos.map_or(buf.len(), |n| n + 1);
        bufread.consume(skip);

        if pos.is_some() {
            return Ok(());
        }
    }
}
