//! Demonstrates an actual use case.
//!
//! In many cases, IO can be performed in ultimately equivalent ways. Seeking forward for instance
//! can be emulated by simply reading as many bytes as need to be skipped. The library address that
//! case, which is implemented in `read_with_skip`.
//!
//! Similarly, `BufRead` can be more efficient yet requiring it will force some callers into
//! double-buffering and rob them of the *choice* of buffering. This is demonstrated in
//! `read_TODO`.
use flexible_io::{reader::ReaderMut, Reader};
use std::io::{Read, SeekFrom};

#[test]
fn motivating_case() {
    {
        let mut untapped: &[u8] = b"Hello, world!";
        let mut reader = Reader::new(&mut untapped);
        let mut buffer = vec![];
        let report = read_with_skip(reader.as_mut(), 7, &mut buffer).unwrap();
        assert_eq!(buffer, b"world!");
        assert_eq!(
            report.num_seek, 0,
            "Obvious can't seek, we didn't tell that it can"
        );

        assert!(
            report.num_read >= 3,
            "Seek and read took two reads. We know slices fulfill the whole request if possible. Then a third read zeros to tell the reader that the slice is EOF."
        );
    }

    {
        let mut untapped: &[u8] = b"Hello, world!";
        let tapped = std::io::Cursor::new(&mut untapped);
        let mut reader = Reader::new(tapped);
        reader.set_seek();
        let mut buffer = vec![];
        let report = read_with_skip(reader.as_mut(), 7, &mut buffer).unwrap();
        assert_eq!(buffer, b"world!");
        assert_eq!(
            report.num_seek, 1,
            "Offset was resolved via seek, we didn't tell that it can"
        );

        assert!(
            report.num_read >= 2,
            "Read took two reads. We know slices fulfill the whole request if possible. Then a third read zeros to tell the reader that the slice is EOF."
        );
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct IoReport {
    pub num_seek: u32,
    pub num_read: u32,
}

/// This operation skips N bytes from the stream, returns the rest read to end.
///
/// It works on *all* kinds of streams, but is optimized for seekable streams. However, the
/// direct caller need not worry about this in the interface. Only `Read` is a hard
/// requirement. Another third party can make the choice whether there is a vtable for `Seek`
/// or not with our caller just passing this encapsulation on.
pub fn read_with_skip(
    // Extra swag: no generic, no `Read` bound either! Wat?
    mut file: ReaderMut,
    skip: u64,
    buffer: &mut Vec<u8>,
) -> Result<IoReport, std::io::Error> {
    let mut report = IoReport::default();

    if let Some(seekable) = file.as_seek_mut() {
        let seekable: &mut dyn std::io::Seek = seekable;

        let mut skip: u64 = skip;
        while skip > 0 {
            let offset = i64::try_from(skip).unwrap_or(i64::MAX);
            seekable.seek(SeekFrom::Current(skip as i64))?;
            report.num_seek += 1;
            skip -= offset as u64;
        }
    } else {
        // No optimization. Use a loop to throw away all these bytes.
        let mut skip: u64 = skip;

        let mut buffer = Box::new([0; 1 << 14]);
        while skip > 0 {
            let bound = usize::try_from(skip).unwrap_or(usize::MAX);
            let exact_read = buffer.len().min(bound);
            let actual = file.as_read_mut().read(&mut buffer[..exact_read])?;
            report.num_read += 1;

            if actual == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof)?;
            }

            skip -= actual as u64;
        }
    }

    let reader = file.as_read_mut();
    let inner = read_to_end(reader, buffer)?;
    report.num_seek += inner.num_seek;
    report.num_read += inner.num_read;

    Ok(report)
}

// Poly-fill for std::io::Read::read_to_end (default), with a report on the number of actual reads
// that were used; for the purpose of IO-accounting.
fn read_to_end<R: Read + ?Sized>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> Result<IoReport, std::io::Error> {
    struct Plug<R>(pub R, IoReport);

    impl<R: Read> std::io::Read for Plug<R> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
            self.1.num_read += 1;
            self.0.read(buf)
        }
    }

    let mut plug = Plug(reader, IoReport::default());
    plug.read_to_end(buffer)?;
    Ok(plug.1)
}
