use crate::stable_with_metadata_of::WithMetadataOf;

use std::{
    any::Any,
    io::{Seek, Write},
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

/// A writer, which can dynamically provide IO traits.
///
/// The following traits may be optionally dynamically provided:
///
/// * [`Seek`]
///
/// The struct comes with a number of setter methods. The call to these requires proof to the
/// compiler that the bound is met, inserting the vtable from the impl instance. Afterward, the
/// bound is not required by any user. Using the (mutable) getters recombines the vtable with the
/// underlying value.
///
/// ## Usage
///
/// ```
/// # use flexible_io::Writer;
/// use std::io::SeekFrom;
///
/// let mut buffer: Vec<u8> = vec![];
/// let cursor = std::io::Cursor::new(&mut buffer);
/// let mut writer = Writer::new(cursor);
/// assert!(writer.as_seek().is_none());
///
/// writer
///     .as_write_mut()
///     .write_all(b"Hello, brain!")
///     .unwrap();
///
/// // But cursors are seekable, let's tell everyone.
/// writer.set_seek();
/// assert!(writer.as_seek().is_some());
///
/// // Now use the Seek implementation to undo our mistake
/// let seek = writer.as_seek_mut().unwrap();
/// seek.seek(SeekFrom::Start(7));
///
/// writer
///     .as_write_mut()
///     .write_all(b"world!")
///     .unwrap();
///
/// let contents: &Vec<u8> = writer.get_ref().get_ref();
/// assert_eq!(contents, b"Hello, world!");
/// ```
pub struct Writer<W: ?Sized> {
    write: *mut dyn Write,
    vtable: OptTable,
    inner: W,
}

#[derive(Clone, Copy, Default)]
struct OptTable {
    seek: Option<*mut dyn Seek>,
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

/// A box around a type-erased [`Writer`].
pub struct WriterBox<'lt> {
    inner: Box<dyn Write + 'lt>,
    vtable: OptTable,
}

impl<W: Write> Writer<W> {
    /// Wrap an underlying writer by-value.
    pub fn new(mut writer: W) -> Self {
        let write = lifetime_erase_trait_vtable!((&mut writer): '_ as Write);

        Writer {
            inner: writer,
            write,
            vtable: OptTable::default(),
        }
    }
}

impl<W: ?Sized> Writer<W> {
    /// Provide access to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Provide mutable access to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Get a view equivalent to very-fat mutable reference.
    ///
    /// This erases the concrete type `W` which allows consumers that intend to avoid polymorphic
    /// code that monomorphizes. The mutable reference has all accessors of a mutable reference
    /// except it doesn't offer access with the underlying writer's type itself.
    pub fn as_mut(&mut self) -> WriterMut<'_> {
        // Copy out all the vtable portions, we need a mutable reference to `self` for the
        // conversion into a dynamically typed `&mut dyn Read`.
        let Writer {
            inner: _,
            write: _,
            vtable,
        } = *self;

        WriterMut {
            inner: self.as_write_mut(),
            vtable,
        }
    }

    /// Get a view equivalent to very-fat mutable reference.
    ///
    /// This erases the concrete type `W` which allows consumers that intend to avoid polymorphic
    /// code that monomorphizes. The mutable reference has all accessors of a mutable reference
    /// except it doesn't offer access with the underlying reader's type itself.
    pub fn into_boxed<'lt>(self) -> WriterBox<'lt>
    where
        W: Sized + 'lt,
    {
        let Writer {
            inner,
            write,
            vtable,
        } = self;

        let ptr = Box::into_raw(Box::new(inner));
        let ptr = WithMetadataOf::with_metadata_of_on_stable(ptr, write);
        let inner = unsafe { Box::from_raw(ptr) };

        WriterBox { inner, vtable }
    }
}

dyn_setter! {
    impl<W> Writer<W> = self as that {
        /// Set the V-Table of [`Seek`].
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
    impl<W> Writer<W> = self as that {
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
    impl<W> Writer<W> = self as that {
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

impl<W: ?Sized> Writer<W> {
    /// Get the inner value as a dynamic `Write` reference.
    pub fn as_write(&self) -> &(dyn Write + '_) {
        let ptr = &self.inner as *const W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.write);
        unsafe { &*local }
    }

    /// Get the inner value as a mutable dynamic `Write` reference.
    pub fn as_write_mut(&mut self) -> &mut (dyn Write + '_) {
        let ptr = &mut self.inner as *mut W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.write);
        unsafe { &mut *local }
    }

    /// Unwrap the inner value at its original sized type.
    pub fn into_inner(self) -> W
    where
        W: Sized,
    {
        self.inner
    }
}

dyn_getter! {
    impl<W> Writer<W> = self as that {
        unsafe const ptr: &that.inner as *const W;
        unsafe mut ptr: &mut that.inner as *mut W;
    } {
        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Self::set_seek`] was executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Self::set_seek`] was executed, by any other caller.
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
    impl<W> Writer<W> = self as that {
        unsafe const ptr: &that.inner as *const W;
        unsafe mut ptr: &mut that.inner as *mut W;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsFd`] reference.
        fn as_fd -> AsFd = that.vtable.as_fd;

        /// Get the inner value as a dynamic [`AsRawFd`] reference.
        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl<W> Writer<W> = self as that {
        unsafe const ptr: &that.inner as *const W;
        unsafe mut ptr: &mut that.inner as *mut W;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsHandle`] reference.
        fn as_handle -> AsHandle = that.vtable.as_handle;

        /// Get the inner value as a dynamic [`AsRawHandle`] reference.
        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        /// Get the inner value as a dynamic [`AsSocket`] reference.
        fn as_socket -> AsSocket = that.vtable.as_socket;

        /// Get the inner value as a dynamic [`AsRawSocket`] reference.
        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

/// A mutable reference to a [`Writer`].
///
/// This type acts similar to a *very* fat mutable reference. It can be obtained by constructing a
/// concrete reader type and calling [`Writer::as_mut`].
///
/// Note: Any mutable reference to a `Writer` implements `Into<WriterMut>` for its lifetime. Use this
/// instead of coercion which would be available if this was a builtin kind of reference.
///
/// Note: Any `Writer` implements `Into<WriterBox>`, which can again be converted to `WriterMut`.
/// Use it for owning a writer without its specific type similar to `Box<dyn Read>`.
pub struct WriterMut<'lt> {
    inner: &'lt mut dyn Write,
    vtable: OptTable,
}

impl WriterMut<'_> {
    /// Get the inner value as a mutable dynamic `Write` reference.
    pub fn as_write_mut(&mut self) -> &mut (dyn Write + '_) {
        &mut *self.inner
    }
}

dyn_getter! {
    impl WriterMut<'_> = self as that {
        unsafe const ptr: that.inner as *const dyn Write;
        unsafe mut ptr: that.inner as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Writer::set_seek`] was executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Writer::set_seek`] was executed, by any other caller.
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
    impl WriterMut<'_> = self as that {
        unsafe const ptr: that.inner as *const dyn Write;
        unsafe mut ptr: that.inner as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsFd`] reference.
        fn as_fd -> AsFd = that.vtable.as_fd;

        /// Get the inner value as a dynamic [`AsRawFd`] reference.
        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl WriterMut<'_> = self as that {
        unsafe const ptr: that.inner as *const dyn Write;
        unsafe mut ptr: that.inner as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsHandle`] reference.
        fn as_handle -> AsHandle = that.vtable.as_handle;

        /// Get the inner value as a dynamic [`AsRawHandle`] reference.
        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        /// Get the inner value as a dynamic [`AsSocket`] reference.
        fn as_socket -> AsSocket = that.vtable.as_socket;

        /// Get the inner value as a dynamic [`AsRawSocket`] reference.
        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl WriterBox<'_> {
    /// Get a view equivalent to very-fat mutable reference.
    ///
    /// This erases the concrete type `W` which allows consumers that intend to avoid polymorphic
    /// code that monomorphizes. The mutable reference has all accessors of a mutable reference
    /// except it doesn't offer access with the underlying writer's type itself.
    pub fn as_mut(&mut self) -> WriterMut<'_> {
        WriterMut {
            vtable: self.vtable,
            inner: self.as_read_mut(),
        }
    }

    /// Provide mutable access to the underlying writer.
    pub fn as_read_mut(&mut self) -> &mut (dyn Write + '_) {
        &mut *self.inner
    }
}

dyn_getter! {
    impl WriterBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const dyn Write;
        unsafe mut ptr: that.inner.as_mut() as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic `Seek` reference.
        ///
        /// This returns `None` unless a previous call to [`Writer::set_seek`] was executed, by any other caller.
        /// The value can be moved after such call arbitrarily.
        fn as_seek {
            /// Get the inner value as a mutable dynamic `Seek` reference.
            ///
            /// This returns `None` unless a previous call to [`Writer::set_seek`] was executed, by any other caller.
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
    impl WriterBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const dyn Write;
        unsafe mut ptr: that.inner.as_mut() as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsFd`] reference.
        fn as_fd -> AsFd = that.vtable.as_fd;

        /// Get the inner value as a dynamic [`AsRawFd`] reference.
        fn as_raw_fd -> AsRawFd = that.vtable.as_raw_fd;
    }
}

#[cfg(target_os = "windows")]
dyn_getter! {
    impl WriterBox<'_> = self as that {
        unsafe const ptr: that.inner.as_ref() as *const dyn Write;
        unsafe mut ptr: that.inner.as_mut() as *mut dyn Write;
    } {
        /// Get the inner value as a dynamic [`FileExt`] reference.
        fn as_file_ext -> FileExt = that.vtable.file_ext;

        /// Get the inner value as a dynamic [`AsHandle`] reference.
        fn as_handle -> AsHandle = that.vtable.as_handle;

        /// Get the inner value as a dynamic [`AsRawHandle`] reference.
        fn as_raw_handle -> AsRawHandle = that.vtable.as_raw_handle;

        /// Get the inner value as a dynamic [`AsSocket`] reference.
        fn as_socket -> AsSocket = that.vtable.as_socket;

        /// Get the inner value as a dynamic [`AsRawSocket`] reference.
        fn as_raw_socket -> AsRawSocket = that.vtable.as_raw_socket;
    }
}

impl<'lt, R> From<&'lt mut Writer<R>> for WriterMut<'lt> {
    fn from(value: &'lt mut Writer<R>) -> Self {
        value.as_mut()
    }
}

impl<'lt, R: 'lt> From<Writer<R>> for WriterBox<'lt> {
    fn from(value: Writer<R>) -> Self {
        value.into_boxed()
    }
}

impl<'lt> From<&'lt mut WriterBox<'_>> for WriterMut<'lt> {
    fn from(value: &'lt mut WriterBox) -> Self {
        value.as_mut()
    }
}
