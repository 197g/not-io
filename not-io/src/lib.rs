//! Provides `Read` and `Write` alternatives on `no_std` while being compatible with the full
//! traits from `std` when allowed.
//!
//! ## Motivation
//!
//! The file parser ecosystem of Rust is more or less split across crates that use `no_std` and
//! crates that do not, as well as between crates using `alloc` and no-alloc (and the largely
//! overlapping zero-copy) crates. This has several reasons:
//!
//! * The `std::io::Read` and `std::io::Write` traits require an allocator due to their internal
//!   implementation and were not written to be OS independent.
//! * Before `1.36` it was not possible to depend on `alloc` without `std`.
//! * The lack of specialization makes it hard to be both generic over implementors of the standard
//!   traits while still allowing use when those traits are not available. This is in particular
//!   also since several types (e.g. `&[u8]`) implement those traits but would obviously be useful
//!   as byte sources and sinks even when they are unavailable.
//!
//! However, this is a problem for streaming decoding or selecting parts from very large data
//! structures. A crate that restricts itself to a strict `no_std` environment might accept its
//! data as a single slice or a series of slice. But it has the major limitation that all of the
//! data must be available in memory (or rather, in the address space if you utilize OS level
//! support for paging via mmap) at once. The disadvantages can range anywhere from cache
//! inefficiency, over uncontrolled latency spikes, to making the implementation actually
//! impossible.
//!
//! The goal of this crate is to allow other decoder crates to forward this implementation option
//! to the user.
//!
//! ## Usage guide
//!
//! This crate assumes you have a structure declared roughly as follows:
//!
//! ```rust
//! # struct SomeItem;
//! # use std::io::Read;
//!
//! struct Decoder<T> {
//!     reader: T,
//! }
//!
//! impl<T: std::io::Read> Decoder<T> {
//!     fn next(&mut self) -> Result<SomeItem, std::io::Error> {
//!         let mut buffer = vec![];
//!         self.reader.read_to_end(&mut buffer)?;
//! # unimplemented!()
//!     }
//! }
//! ```
//!
//! There is only one necessary change, be sure to keep the `std` feature enabled for now. This
//! should not break any code except if you relied on the precise type `T` in which case you will
//! need to use a few derefs and/or `into_inner`.
//!
//! ```
//! use not_io::AllowStd;
//! # use std::io::Read;
//!
//! struct Decoder<T> {
//!     reader: AllowStd<T>,
//! }
//!
//! # struct SomeItem;
//! # impl<T: std::io::Read> Decoder<T> {
//! #    fn next(&mut self) -> Result<SomeItem, std::io::Error> {
//! #        let mut buffer = vec![];
//! #        self.reader.0.read_to_end(&mut buffer)?;
//! # unimplemented!()
//! #    }
//! # }
//! ```
//!
//! And finally you can add to your crate a new default feature which enables the `std`/`alloc`
//! feature of this crate, and conditionally active your existing interfaces only when that feature
//! is active. Then add a few new impls that can be used even when the feature is inactive.
//!
//! ```
//! use not_io::AllowStd;
//! # struct SomeItem;
//!
//! struct Decoder<T> {
//!     reader: AllowStd<T>,
//! }
//!
//! /// The interface which lets the caller select which feature to turn on.
//! impl<T> Decoder<T>
//! where
//!     AllowStd<T>: not_io::Read
//! {
//!     fn no_std_next(&mut self) -> Result<SomeItem, not_io::Error> {
//! # unimplemented!()
//!     }
//! }
//!
//! /// An interface for pure no_std use with caller provide no_std reader.
//! impl<T> Decoder<T>
//! where
//!     T: not_io::Read
//! {
//!     fn not_io_next(&mut self) -> Result<SomeItem, not_io::Error> {
//!         let reader = &mut self.reader.0;
//! # unimplemented!()
//!     }
//! }
//! ```
//!
#![cfg_attr(all(not(feature = "std"), not(feature = "compat")), no_std)]

#[cfg(all(feature = "alloc"))]
extern crate alloc;

mod cursor;
mod empty;
mod read_adaptor;

pub use self::cursor::Cursor;
pub use self::empty::{Empty, Repeat, Sink};
pub use self::read_adaptor::Take;

/// An opaque error.
///
/// This is either equivalent to `std::io::Error` when the `std` feature is enabled, or it's a
/// stripped down version. In any case it is constructible from the non-exhaustive `ErrorKind` that
/// lists all the simple error conditions that do not depend on OS implementation.
#[derive(Debug)]
pub struct Error {
    #[allow(dead_code)]
    inner: ErrorInner,
}

/// A non-exhaustive enum of simple error kinds.
///
/// When the `compat` feature is selected this is instead implemented by a variant that you must
/// not match. However, it will not have any performance costs as the respective variant is
/// implemented in such a way that `rustc` is able to prove that it can never be constructed and
/// hence eliminates all branches matching it.
#[non_exhaustive]
#[derive(Debug)]
pub enum ErrorKind {
    /// No bytes of a buffer have been written.
    WriteZero,
    /// No bytes of a buffer have been read.
    UnexpectedEof,
    /// Event required spurious return in a blocking IO(-like) interface.
    /// Often best handled by retrying.
    Interrupted,
    /// Signals that a unfinished stream has no data, until refilled by some concurrent source.
    WouldBlock,
    /// Stream did not contain valid UTF-8 (or other encoding).
    InvalidData,
    /// Argument to an IO function was not valid.
    InvalidInput,
}

enum ErrorInner {
    #[cfg(not(feature = "std"))]
    Kind(ErrorKind),
    #[cfg(feature = "std")]
    Error(std::io::Error),
}

/// Public interface block for `Error`, independent of features.
impl Error {
    pub(crate) fn is_interrupted(&self) -> bool {
        // Dispatch to feature combination.
        self.is_interrupted_impl()
    }

    pub(crate) fn from_kind(kind: ErrorKind) -> Self {
        // Dispatch to feature combination.
        Self::from_kind_impl(kind)
    }
}

impl From<ErrorKind> for Error {
    fn from(err: ErrorKind) -> Self {
        Error::from_kind(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// Replicates the standard `Read` trait, with a simpler error.
///
/// With `std`-feature enabled this is an actual duplicate. Note that it is implemented for the
/// generic `AllowStd<impl Read>` family if `std` is enabled, and on select instances such as
/// `AllowStd<&[u8]>` otherwise. Additionally, the trait is implemented for all select types
/// directly.
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => return Err(Error::from(ErrorKind::UnexpectedEof)),
                Ok(n) => buf = &mut buf[n..],
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    #[cfg(feature = "alloc")]
    fn read_to_end(&mut self, buf: &mut alloc::vec::Vec<u8>) -> Result<usize> {
        impls_alloc::read_to_end(self, buf)
    }

    #[cfg(feature = "alloc")]
    fn read_to_string(&mut self, buf: &mut alloc::string::String) -> Result<usize> {
        impls_alloc::read_to_string(self, buf)
    }

    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        Take { inner: self, limit }
    }
}

pub trait BufRead: Read {
    fn fill_buf(&mut self) -> Result<&[u8]>;

    fn consume(&mut self, amt: usize);

    #[cfg(feature = "alloc")]
    fn read_until(&mut self, byte: u8, buf: &mut alloc::vec::Vec<u8>) -> Result<usize> {
        impls_alloc::read_until(self, byte, buf)
    }

    #[cfg(feature = "alloc")]
    fn read_line(&mut self, buf: &mut alloc::string::String) -> Result<usize> {
        impls_alloc::read_line(self, buf)
    }
}

pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    fn stream_position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// Replicates the standard `Write` trait, with a simpler error.
///
/// With `std`-feature enabled this is an actual duplicate. Note that it is implemented for the
/// generic `AllowStd<impl Write>` family if `std` is enabled, and on select instances such as
/// `AllowStd<&mut [u8]>` otherwise. Additionally, the trait is implemented for all select types
/// directly.
///
/// FIXME: should proxy `write_vectored` and `write_fmt`.
pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    fn flush(&mut self) -> Result<()>;

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(Error::from(ErrorKind::WriteZero)),
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

pub fn copy<R, W>(read: &mut R, write: &mut W) -> Result<u64>
where
    R: Read + ?Sized,
    W: Write + ?Sized,
{
    impls_always::stack_copy(read, write)
}

pub fn empty() -> Empty {
    Empty { _private: () }
}

pub fn sink() -> Sink {
    Sink { _private: () }
}

pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

/// A simple new type wrapper holding a potential reader or writer.
///
/// This type allows the library to satisfy the compatibility across different features without
/// having to resort to specialization. Simply put, this struct implements `Read` and `Write`:
///
/// * for all types that implement the respective trait from `std` if the `std` feature is active.
/// * on a concrete subset of those types if the `alloc` feature but not the `std` feature has been
///   turned on.
/// * only for types from `core` when neither feature is turned on.
///
/// Note that without this type we couldn't safely introduce a conditionally active, generic impl
/// of our own traits. The reason is that features must only activate SemVer compatible changes.
/// These two sets of impls are not SemVer compatible due to the uncovered generic `T`. In
/// particular in the first case you'd be allowed to implement the trait for your own type that
/// also implements `std::io::Read` while in the second this is an impl conflict.
///
/// * `impl Read for &'_ [u8]`
/// * `impl<T> Read for T where std::io::Read`
///
/// By adding our own private struct as a layer of indirection, you are no longer allowed to make
/// such changes:
///
/// * `impl Read for AllowStd<&'_ [u8]>`
/// * `impl<T> Read for AllowStd<T> where T: std::io::Read`
///
/// This still means there is one impl which will never be added. Instead, the impls for
/// core/standard types are provided separately and individually.
///
/// * `impl<T> Read for AllowStd<T> where T: crate::Read`
pub struct AllowStd<T>(pub T);

/// A type that never implements any of the `std::io` traits.
///
/// This is the reverse escape hatch to `AllowStd`. It allows this crate to provide a generic impl
/// that Rust knows can never collide with another blanket impl bounded by `std::io::Read` or
/// `std::io::Write`.
pub struct NotIo<T>(pub T);

mod impls_always;

/// Impls that are special in `no_std`, no-`alloc` but also appear differently in `alloc`.
#[cfg(not(feature = "alloc"))]
mod impls_nostd_noalloc;

/// Impls that are implement on the `std` feature by the `io::Read`/`io::Write` bounds but
/// individually here.
#[cfg(not(feature = "std"))]
mod impls_nostd;

/// Impls that depend only on the ability to allocate.
/// These include all methods to read or write to containers, as well as buffers. It doesn't
/// include `AllowStd`-impls as those are either individual or generic, with feature `std`.
#[cfg(feature = "alloc")]
mod impls_alloc;

/// Impls that are generic with `std` but individual on `alloc`.
#[cfg(all(feature = "alloc", not(feature = "std")))]
mod impls_nostd_alloc;

#[cfg(feature = "std")]
mod impls_std;
