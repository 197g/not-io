use not_io::{AllowStd, BufRead, Cursor, Empty, Read, Repeat, Seek, Sink, Take, Write};

extern crate alloc;
use alloc::{string::String, vec::Vec};

// Make sure that this includes the no-`alloc` subset of tests.
#[path = "no_alloc.rs"]
mod _alloc;

fn is_read<R: Read>() {}
fn is_write<W: Write>() {}
fn is_buf_read<R: BufRead>() {}
fn is_seek<R: Seek>() {}

const XXX: () = {
    let _ = is_read::<&'static [u8]>;
    let _ = is_write::<&'static mut [u8]>;
    let _ = is_write::<Vec<u8>>;
    let _ = is_read::<AllowStd<&'static [u8]>>;
    let _ = is_write::<AllowStd<&'static mut [u8]>>;
    let _ = is_write::<AllowStd<Vec<u8>>>;
    let _ = is_write::<Cursor<Vec<u8>>>;
    let _ = is_seek::<Cursor<Vec<u8>>>;
    let _ = is_write::<Cursor<&'static mut Vec<u8>>>;
    let _ = is_seek::<Cursor<&'static mut Vec<u8>>>;
    let _ = is_write::<Cursor<Vec<u8>>>;
    let _ = is_seek::<Cursor<Vec<u8>>>;
    let _ = is_read::<Empty>;
    let _ = is_buf_read::<Empty>;
    let _ = is_seek::<Empty>;
    let _ = is_write::<Sink>;
    let _ = is_read::<Repeat>;
    let _ = is_write::<&'static Sink>;
    let _ = is_read::<Take<Empty>>;
    let _ = is_buf_read::<Take<Empty>>;
};

#[test]
fn evaluate_consts() {
    let _: () = XXX;
}

#[test]
fn read_to_buffer() {
    const SOURCE: &str = "Hello, world";
    let ref mut source = SOURCE.as_bytes();
    let elen = source.len();
    let mut buffer = Vec::new();

    assert!(matches!(Read::read_to_end(source, &mut buffer), Ok(rlen) if rlen == elen));
    assert_eq!(buffer, SOURCE.as_bytes());
}

#[test]
fn read_to_string() {
    const SOURCE: &str = "Hello, world";
    let ref mut source = SOURCE.as_bytes();
    let elen = source.len();
    let mut buffer = String::new();

    assert!(matches!(Read::read_to_string(source, &mut buffer), Ok(rlen) if rlen == elen));
    assert_eq!(buffer, SOURCE);
}

#[test]
fn read_to_fail() {
    const SOURCE: &[u8] = b"Hello, \xfeworld";
    let ref mut source = &SOURCE[..];
    let mut buffer = String::new();

    assert!(matches!(Read::read_to_string(source, &mut buffer), Err(_)));
}

#[test]
fn read_buf() {
    const SOURCE: &str = "Hello, world";
    let ref mut source = SOURCE.as_bytes();
    assert!(matches!(source.fill_buf(), Ok(src) if src == SOURCE.as_bytes()));

    let mut buffer = Vec::new();
    assert!(matches!(source.read_until(b',', &mut buffer), Ok(6)));
    assert_eq!(buffer, b"Hello,");
}

#[test]
fn read_buf_to_string() {
    const SOURCE: &[u8] = b"Hello,\n\xfeworld";
    let ref mut source = &SOURCE[..];
    let mut buffer = String::new();

    assert!(matches!(source.read_line(&mut buffer), Ok(7)));
    assert_eq!(buffer, "Hello,\n");

    assert!(matches!(source.read_line(&mut buffer), Err(_)));
}

#[test]
fn buf_writer_cursor() {
    const SOURCE: &[u8] = b"Hello, world";
    let mut buffer = vec![0u8; 0];

    let mut cursor = Cursor::new(&mut buffer);
    cursor.write_all(SOURCE).unwrap();

    assert_eq!(cursor.position(), SOURCE.len() as u64);
    assert_eq!(buffer, SOURCE);
}

#[test]
fn buf_writer_cursor_mid() {
    const SOURCE: &[u8] = b"Hello, world";
    let mut buffer = vec![0u8; 0];

    let mut cursor = Cursor::new(&mut buffer);
    cursor.set_position(7);
    cursor.write_all(&SOURCE[7..]).unwrap();

    assert_eq!(cursor.position(), SOURCE.len() as u64);
    assert_eq!(buffer.len(), SOURCE.len());
    assert_eq!(buffer[..7], [0; 7]);
}

#[test]
fn take_short() {
    const SOURCE: &[u8] = b"Hello, world";
    let mut reader = Read::take(&SOURCE[..], 5);
    let mut buffer = vec![0u8; 0];

    assert!(matches!(reader.fill_buf(), Ok(b"Hello")));
    assert!(matches!(reader.read_to_end(&mut buffer), Ok(5)));
    assert!(matches!(reader.fill_buf(), Ok(&[])));
    assert_eq!(buffer, b"Hello");
}

#[test]
fn take_long() {
    const SOURCE: &[u8] = b"Hello, world";
    let mut reader = Read::take(&SOURCE[..], 0x42);
    let mut buffer = vec![0u8; 0];

    assert!(matches!(reader.fill_buf(), Ok(SOURCE)));
    assert!(matches!(reader.read_to_end(&mut buffer), Ok(len) if len == SOURCE.len()));
    assert!(matches!(reader.fill_buf(), Ok(&[])));
    assert_eq!(buffer, SOURCE);
}
