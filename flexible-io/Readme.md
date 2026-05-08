# Flexible IO

Packs a reader or writer with its io-trait implementations, allowing the
omission of the traits in the static trait bounds.

The motivation of this is enabling APIs in which use of a reader (or writer)
can be optimized in case of it being buffered, but it's not required for
correct functioning. Or, an API where the Seek requirement is only determined
at runtime, such as when a portion of the functionality does not depend on it,
and an error should be returned.

The crate implements, as a proof of concept, a reader which can optionally
`Seek` or `BufRead`; and a writer which can optionally `Seek`. Its up to a
caller to provide the traits by calling setter methods, in a local _scope_
where the traits are statically shown to be implemented.

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

fn with_seek_strategy(_: &mut dyn std::io::Seek) {}

fn with_read_strategy(_: &mut dyn std::io::Read) {}
```

## Known issues

Due to lifetime issues, it is not possible to preserve the knowledge of a trait
being implemented. That is, each conversion from `Reader` to a concrete value
of either type `&mut dyn {Read,BufRead,Seek}` mutably borrows the whole reader.
Therefore you can't have to such references at the same time. As a workaround,
unwrap the `Option` returned from `as_*` methods where appropriate.
