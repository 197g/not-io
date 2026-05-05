use crate::stable_with_metadata_of::WithMetadataOf;

use std::{
    any::Any,
    io::{BufRead, Read, Seek},
};

#[cfg(target_os = "windows")]
use std::os::windows::{
    fs::FileExt,
    io::{AsHandle, AsRawHandle, AsRawSocket, AsSocket},
};

#[cfg(target_family = "unix")]
use std::os::{
    fd::{AsFd, AsRawFd},
    unix::fs::FileExt,
};

/// A reader, which can dynamically provide IO traits.
///
/// The following traits may be optionally dynamically provided:
///
/// * [`Seek`]
/// * [`BufRead`]
/// * [`Any`]
///
/// The struct comes with a number of setter methods. The call to these requires proof to the
/// compiler that the bound is met, inserting the vtable from the impl instance. Afterward, the
/// bound is not required by any user. Using the (mutable) getters recombines the vtable with the
/// underlying value.
///
/// Note that the value can not be unsized (`dyn` trait) itself. This may be fixed at a later point
/// to make the reader suitable for use in embedded. In particular, the double indirection of
/// instantiating with `R = &mut dyn Read` wouldn't make sense as the setters would not be usable,
/// their bounds can never be met. And combining traits into a large dyn-trait is redundant as it
/// trait-impls become part of the static validity requirement again.
///
/// ## Usage
///
/// ```
/// # use flexible_io::Reader;
/// let mut buffer: &[u8] = b"Hello, world!";
/// let mut reader = Reader::new(&mut buffer);
/// assert!(reader.as_buf().is_none());
///
/// // But slices are buffered readers, let's tell everyone.
/// reader.set_buf();
/// assert!(reader.as_buf().is_some());
///
/// // Now use the ReadBuf implementation directly
/// let buffered = reader.as_buf_mut().unwrap();
/// buffered.consume(7);
/// assert_eq!(buffered.fill_buf().unwrap(), b"world!");
/// ```
pub struct Reader<R: ?Sized> {
    read: *mut dyn Read,
    vtable: OptTable,
    inner: R,
}

#[derive(Clone, Copy, Default)]
struct OptTable {
    seek: Option<*mut dyn Seek>,
    buf: Option<*mut dyn BufRead>,
    any: Option<*mut dyn Any>,

    // Unix family traits:
    #[cfg(target_family = "unix")]
    file_ext: Option<*mut dyn FileExt>,
    #[cfg(target_family = "unix")]
    as_fd: Option<*mut dyn AsFd>,
    #[cfg(target_family = "unix")]
    as_raw_fd: Option<*mut dyn AsRawFd>,

    // Windows only traits:
    #[cfg(target_os = "windows")]
    file_ext: Option<*mut dyn FileExt>,
    #[cfg(target_os = "windows")]
    as_handle: Option<*mut dyn AsHandle>,
    #[cfg(target_os = "windows")]
    as_raw_handle: Option<*mut dyn AsRawHandle>,
    #[cfg(target_os = "windows")]
    as_socket: Option<*mut dyn AsSocket>,
    #[cfg(target_os = "windows")]
    as_raw_socket: Option<*mut dyn AsRawSocket>,
}

/// A box around a type-erased [`Reader`].
pub struct ReaderBox<'lt> {
    inner: Box<dyn Read + 'lt>,
    vtable: OptTable,
}

impl<R: Read> Reader<R> {
    /// Wrap an underlying reader by-value.
    pub fn new(mut reader: R) -> Self {
        let read = lifetime_erase_trait_vtable!((&mut reader): '_ as Read);

        Reader {
            inner: reader,
            read,
            vtable: OptTable::default(),
        }
    }
}

impl<R: ?Sized> Reader<R> {
    /// Provide access to the underlying reader.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Provide mutable access to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Get a view equivalent to very-fat mutable reference.
    ///
    /// This erases the concrete type `R` which allows consumers that intend to avoid polymorphic
    /// code that monomorphizes. The mutable reference has all accessors of a mutable reference
    /// except it doesn't offer access with the underlying reader's type itself.
    pub fn as_mut(&mut self) -> ReaderMut<'_> {
        // Copy out all the vtable portions, we need a mutable reference to `self` for the
        // conversion into a dynamically typed `&mut dyn Read`.
        let Reader {
            inner: _,
            read: _,
            vtable,
        } = *self;

        ReaderMut {
            inner: self.as_read_mut(),
            vtable,
        }
    }

    /// Get an allocated, type-erased very-fat mutable box.
    ///
    /// This erases the concrete type `R` which allows consumers that intend to avoid polymorphic
    /// code that monomorphizes. The mutable reference has all accessors of a mutable reference
    /// except it doesn't offer access with the underlying reader's type itself.
    pub fn into_boxed<'lt>(self) -> ReaderBox<'lt>
    where
        R: Sized + 'lt,
    {
        let Reader {
            inner,
            read,
            vtable,
        } = self;

        let ptr = Box::into_raw(Box::new(inner));
        let ptr = WithMetadataOf::with_metadata_of_on_stable(ptr, read);
        let inner = unsafe { Box::from_raw(ptr) };

        ReaderBox { inner, vtable }
    }
}

dyn_setter! {
    impl<R> Reader<R> = self as that {
        /// Set the V-Table for [`BufRead`].
        ///
        /// After this call, the methods [`Self::as_buf`] and [`Self::as_buf_mut`] will return values.
        fn set_buf -> BufRead = that.vtable.buf;

        /// Set the V-Table for [`Seek`].
        ///
        /// After this call, the methods [`Self::as_seek`] and [`Self::as_seek_mut`] will return values.
        fn set_seek -> Seek = that.vtable.seek;

        /// Set the V-Table for [`Any`].
        ///
        /// After this call, the methods [`Self::as_any`] and [`Self::as_any_mut`] will return values.
        fn set_any -> Any = that.vtable.any;
    }
}

#[cfg(target_family = "unix")]
dyn_setter! {
    impl<R> Reader<R> = self as that {
        /// Set the V-Table for [`FileExt`].
        ///
        /// After this call, the methods [`Self::as_file_ext`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_file_ext -> FileExt = that.vtable.file_ext;

        /// Set the V-Table for [`AsRawFd`].
        ///
        /// After this call, the methods [`Self::as_fd`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_fd -> AsFd = that.vtable.as_fd;

        /// Set the V-Table for [`AsRawFd`].
        ///
        /// After this call, the methods [`Self::as_raw_fd`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_setter! {
    impl<R> Reader<R> = self as that {
        /// Set the V-Table for [`FileExt`].
        ///
        /// After this call, the methods [`Self::as_file_ext`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_file_ext -> FileExt = that.vtable.file_ext;

        /// Set the V-Table for [`AsHandle`].
        ///
        /// After this call, the methods [`Self::as_handle`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_handle -> AsHandle = that.vtable.as_handle;

        /// Set the V-Table for [`AsRawHandle`].
        ///
        /// After this call, the methods [`Self::as_raw_handle`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        /// Set the V-Table for [`AsSocket`].
        ///
        /// After this call, the methods [`Self::as_socket`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_socket -> AsSocket = that.vtable.as_socket;

        /// Set the V-Table for [`AsRawSocket`].
        ///
        /// After this call, the methods [`Self::as_raw_socket`] will return a value. (This trait only has
        /// methods with a `&self` receiver).
        fn set_as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl<R: ?Sized> Reader<R> {
    pub fn as_read(&self) -> &(dyn Read + '_) {
        let ptr = &self.inner as *const R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.read);
        unsafe { &*local }
    }

    /// Get the inner value as a mutable dynamic `Read` reference.
    pub fn as_read_mut(&mut self) -> &mut (dyn Read + '_) {
        let ptr = &mut self.inner as *mut R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.read);
        unsafe { &mut *local }
    }

    /// Unwrap the inner value at its original sized type.
    pub fn into_inner(self) -> R
    where
        R: Sized,
    {
        self.inner
    }
}

dyn_getter! {
    impl<R> Reader<R> = self as that {
        unsafe const ptr: &that.inner as *const R;
        unsafe mut ptr: &mut that.inner as *mut R;
    } {
        /// Get the inner value as a dynamic `BufRead` reference.
        ///
        /// This returns `None` unless a previous call to [`Self::set_buf`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_buf {
            /// Get the inner value as a mutable dynamic `BufRead` reference.
            ///
            /// This returns `None` unless a previous call to [`Self::set_buf`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_buf_mut
        } -> BufRead = that.vtable.buf;

        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_seek_mut
        } -> Seek = that.vtable.seek;

        /// Get the inner value as a dynamic `Any` reference.
        fn as_any {
            /// Get the inner value as a dynamic `Any` reference.
            mut: fn as_any_mut
        } -> Any = that.vtable.any;
    }
}

#[cfg(target_family = "unix")]
dyn_getter! {
    impl<R> Reader<R> = self as that {
        unsafe const ptr: &that.inner as *const R;
        unsafe mut ptr: &mut that.inner as *mut R;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_fd -> AsFd = that.vtable.as_fd;

        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl<R> Reader<R> = self as that {
        unsafe const ptr: &that.inner as *const R;
        unsafe mut ptr: &mut that.inner as *mut R;
    } {
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_handle -> AsHandle = that.vtable.as_handle;

        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        fn as_socket -> AsSocket = that.vtable.as_socket;

        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl<R: Read> Read for Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.inner.read_exact(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        self.inner.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        self.inner.read_to_string(buf)
    }
}

/// A mutable reference to a [`Reader`].
///
/// This type acts similar to a *very* fat mutable reference. It can be obtained by constructing a
/// concrete reader type and calling [`Reader::as_mut`].
///
/// Note: Any mutable reference to a `Reader` implements `Into<ReaderMut>` for its lifetime. Use
/// this instead of coercion which would be available if this was a builtin kind of reference.
///
/// Note: Any `Reader` implements `Into<ReaderBox>`, which can again be converted to [`ReaderMut`].
/// Use it for owning a writer without its specific type similar to `Box<dyn Write>`.
pub struct ReaderMut<'lt> {
    inner: &'lt mut dyn Read,
    vtable: OptTable,
}

impl ReaderMut<'_> {
    pub fn as_read_mut(&mut self) -> &mut (dyn Read + '_) {
        &mut *self.inner
    }
}

dyn_getter! {
    impl ReaderMut<'_> = self as that {
        unsafe const ptr: that.inner as *const dyn Read;
        unsafe mut ptr: that.inner as *mut dyn Read;
    } {
        /// Get the inner value as a dynamic `BufRead` reference.
        ///
        /// This returns `None` unless a previous call to [`Reader::set_buf`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_buf {
            /// Get the inner value as a mutable dynamic `BufRead` reference.
            ///
            /// This returns `None` unless a previous call to [`Reader::set_buf`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_buf_mut
        } -> BufRead = that.vtable.buf;

        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Reader::set_seek`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Reader::set_seek`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_seek_mut
        } -> Seek = that.vtable.seek;

        /// Get the inner value as a dynamic `Any` reference.
        fn as_any {
            /// Get the inner value as a dynamic `Any` reference.
            mut: fn as_any_mut
        } -> Any = that.vtable.any;
    }
}

#[cfg(target_family = "unix")]
dyn_getter! {
    impl ReaderMut<'_> = self as that {
        unsafe const ptr: that.inner as *const dyn Read;
        unsafe mut ptr: that.inner as *mut dyn Read;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_fd -> AsFd = that.vtable.as_fd;

        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl ReaderMut<'_> = self as that {
        unsafe const ptr: &that.inner as *const dyn Read;
        unsafe mut ptr: &mut that.inner as *mut dyn Read;
    } {
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_handle -> AsHandle = that.vtable.as_handle;

        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        fn as_socket -> AsSocket = that.vtable.as_socket;

        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl ReaderBox<'_> {
    pub fn as_mut(&mut self) -> ReaderMut<'_> {
        ReaderMut {
            vtable: self.vtable,
            inner: self.as_read_mut(),
        }
    }

    pub fn as_read_mut(&mut self) -> &mut (dyn Read + '_) {
        &mut *self.inner
    }
}

dyn_getter! {
    impl ReaderBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const dyn Read;
        unsafe mut ptr: that.inner.as_mut() as *mut dyn Read;
    } {
        /// Get the inner value as a dynamic `BufRead` reference.
        ///
        /// This returns `None` unless a previous call to [`Reader::set_buf`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_buf {
            /// Get the inner value as a mutable dynamic `BufRead` reference.
            ///
            /// This returns `None` unless a previous call to [`Reader::set_buf`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_buf_mut
        } -> BufRead = that.vtable.buf;

        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Reader::set_seek`] as executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Reader::set_seek`] as executed, by any other caller.
            /// The value can be moved after such call arbitrarily.
            mut: fn as_seek_mut
        } -> Seek = that.vtable.seek;

        /// Get the inner value as a dynamic `Any` reference.
        fn as_any {
            /// Get the inner value as a dynamic `Any` reference.
            mut: fn as_any_mut
        } -> Any = that.vtable.any;
    }
}

#[cfg(target_family = "unix")]
dyn_getter! {
    impl ReaderBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const _;
        unsafe mut ptr: that.inner.as_mut() as *mut _;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_fd -> AsFd = that.vtable.as_fd;

        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl ReaderBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const dyn Read;
        unsafe mut ptr: that.inner.as_mut() as *mut dyn Read;
    } {
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        fn as_handle -> AsHandle = that.vtable.as_handle;

        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        fn as_socket -> AsSocket = that.vtable.as_socket;

        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl<'lt, R> From<&'lt mut Reader<R>> for ReaderMut<'lt> {
    fn from(value: &'lt mut Reader<R>) -> Self {
        value.as_mut()
    }
}

impl<'lt, R: 'lt> From<Reader<R>> for ReaderBox<'lt> {
    fn from(value: Reader<R>) -> Self {
        value.into_boxed()
    }
}

impl<'lt> From<&'lt mut ReaderBox<'_>> for ReaderMut<'lt> {
    fn from(value: &'lt mut ReaderBox<'_>) -> Self {
        value.as_mut()
    }
}
