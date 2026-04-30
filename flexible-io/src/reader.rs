use crate::stable_with_metadata_of::WithMetadataOf;

use std::{
    any::Any,
    io::{BufRead, Read, Seek},
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

#[derive(Clone, Copy)]
struct OptTable {
    seek: Option<*mut dyn Seek>,
    buf: Option<*mut dyn BufRead>,
    any: Option<*mut dyn Any>,
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
            vtable: OptTable {
                seek: None,
                buf: None,
                any: None,
            },
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

    /// Set the V-Table for [`BufRead`].
    ///
    /// After this call, the methods [`Self::as_buf`] and [`Self::as_buf_mut`] will return values.
    pub fn set_buf(&mut self)
    where
        R: Sized + BufRead,
    {
        self.vtable.buf = Some(lifetime_erase_trait_vtable!((&mut self.inner): '_ as BufRead));
    }

    /// Set the V-Table for [`Seek`].
    ///
    /// After this call, the methods [`Self::as_seek`] and [`Self::as_seek_mut`] will return values.
    pub fn set_seek(&mut self)
    where
        R: Sized + Seek,
    {
        self.vtable.seek = Some(lifetime_erase_trait_vtable!((&mut self.inner): '_ as Seek));
    }

    /// Set the V-Table for [`Any`].
    ///
    /// After this call, the methods [`Self::as_any`] and [`Self::as_any_mut`] will return values.
    pub fn set_any(&mut self)
    where
        R: Sized + Any,
    {
        self.vtable.any = Some(lifetime_erase_trait_vtable!((&mut self.inner): '_ as Any));
    }
}

impl<R: ?Sized> Reader<R> {
    /// Get the inner value as a dynamic `Read` reference.
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

    /// Get the inner value as a dynamic `BufRead` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_buf`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_buf(&self) -> Option<&(dyn BufRead + '_)> {
        let ptr = &self.inner as *const R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.buf?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a mutable dynamic `BufRead` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_buf`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_buf_mut(&mut self) -> Option<&mut (dyn BufRead + '_)> {
        let ptr = &mut self.inner as *mut R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.buf?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Seek` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_seek(&self) -> Option<&(dyn Seek + '_)> {
        let ptr = &self.inner as *const R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a mutable dynamic `Seek` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_seek_mut(&mut self) -> Option<&mut (dyn Seek + '_)> {
        let ptr = &mut self.inner as *mut R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any(&self) -> Option<&'_ dyn Any> {
        let ptr = &self.inner as *const R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any_mut(&mut self) -> Option<&'_ mut dyn Any> {
        let ptr = &mut self.inner as *mut R;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &mut *local })
    }

    /// Unwrap the inner value at its original sized type.
    pub fn into_inner(self) -> R
    where
        R: Sized,
    {
        self.inner
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

    pub fn as_buf_mut(&mut self) -> Option<&mut (dyn BufRead + '_)> {
        let ptr = self.inner as *mut dyn Read;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.buf?);
        Some(unsafe { &mut *local })
    }

    pub fn as_seek_mut(&mut self) -> Option<&mut (dyn Seek + '_)> {
        let ptr = self.inner as *mut dyn Read;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any(&self) -> Option<&'_ dyn Any> {
        let ptr = self.inner as *const dyn Read;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any_mut(&mut self) -> Option<&'_ mut dyn Any> {
        let ptr = self.inner as *mut dyn Read;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &mut *local })
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

    pub fn as_buf_mut(&mut self) -> Option<&mut (dyn BufRead + '_)> {
        let ptr = self.inner.as_mut() as *mut _;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.buf?);
        Some(unsafe { &mut *local })
    }

    pub fn as_seek_mut(&mut self) -> Option<&mut (dyn Seek + '_)> {
        let ptr = self.inner.as_mut() as *mut _;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any(&self) -> Option<&'_ dyn Any> {
        let ptr = self.inner.as_ref() as *const _;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any_mut(&mut self) -> Option<&'_ mut dyn Any> {
        let ptr = self.inner.as_mut() as *mut _;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &mut *local })
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
