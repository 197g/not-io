# Flexible IO

Make internal use of _optional_ IO capabilities such as `Seek`, `AsRawFd`, or
`read_at` on Unix without requiring a bound mentioning them in your interfaces.
Provides more options around `io::{Read, Write}` by, essentially, *very fat*
pointer types that can also be boxed and type-erased. This makes trait usage a
runtime choice.

The motivation of this is enabling APIs in which use of a reader
can be optimized differently depending on its how it can be buffered and the
manner in which it is most efficiently read into memory. For instance the
capability to Seek can determined at runtime to avoid buffering when data can
be reread while still supporting a pure stream input with buffering in another
path. This pattern comes up when processing zip-like files.

## Usage

```rust
use flexible_io::reader::ReaderMut;

fn read_from(mut file: ReaderMut<'_>) {
    if let Some(seekable) = file.as_seek_mut() {
        with_seek_strategy(seekable);
    } else {
        with_read_strategy(file.as_read_mut());
    }
}

fn with_seek_strategy(_: &mut dyn std::io::Seek) {
    // 1. Seek to central directory
    // 2. Find file entry
    // 3. Seek to file stream
}

fn with_read_strategy(_: &mut dyn std::io::Read) {
    // 1. Buffer until central directory
    // 2. Find file entry
    // 3. Discard non-file stream
}
```

## Known issues

Due to lifetime issues, it is not possible to combine multiple queries of
traits being implemented at the same time. That is, each conversion from
`Reader` to a concrete value of either type `&mut dyn {Read,BufRead,Seek}`
mutably borrows the whole reader. Therefore you can't have to such references
at the same time. As a workaround, unwrap the `Option` returned from `as_*`
methods where appropriate.
