use not_io::{AllowStd, BufRead, Cursor, Read, Seek, SeekFrom, Write};

fn is_read<R: Read>() {}
fn is_write<W: Write>() {}
fn is_buf_read<R: BufRead>() {}

const XXX: () = {
    let _ = is_read::<&'static [u8]>;
    let _ = is_write::<&'static mut [u8]>;
    let _ = is_read::<AllowStd<&'static [u8]>>;
    let _ = is_write::<AllowStd<&'static mut [u8]>>;
    let _ = is_buf_read::<&'static [u8]>;
    let _ = is_read::<Cursor<&'static [u8]>>;
    let _ = is_read::<Cursor<&'static str>>; // From AsRef<[u8]>.
    let _ = is_write::<Cursor<&'static mut [u8]>>;
    let _ = is_buf_read::<Cursor<&'static [u8]>>;
};

#[test]
fn evaluate_consts() {
    let _: () = XXX;
}

#[test]
fn cursor() {
    const SOURCE: &str = "Hello, world!";
    let mut stream: Cursor<&'static str> = Cursor::new(SOURCE);
    assert!(matches!(stream.fill_buf(), Ok(src) if src == SOURCE.as_bytes()));
}

#[test]
fn cursor_seek_end() {
    const SOURCE: &str = "Hello, world!";
    let mut stream: Cursor<&'static str> = Cursor::new(SOURCE);

    stream.seek(SeekFrom::End(-1)).expect("allowed");
    assert!(matches!(stream.fill_buf(), Ok(b"!")));
}

#[test]
fn copy() {
    const SOURCE: &[u8] = b"Hello, world!";
    assert!(
        matches!(not_io::copy(&mut &SOURCE[..], &mut not_io::sink()), Ok(len) if len as usize == SOURCE.len())
    );
}
