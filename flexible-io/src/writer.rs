use crate::stable_with_metadata_of::WithMetadataOf;

use std::{
    any::Any,
    io::{Seek, Write},
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

#[derive(Clone, Copy)]
struct OptTable {
    seek: Option<*mut dyn Seek>,
    any: Option<*mut dyn Any>,
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
            vtable: OptTable {
                seek: None,
                any: None,
            },
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

    /// Set the V-Table of [`Seek`].
    ///
    /// After this call, the methods [`Self::as_seek`] and [`Self::as_seek_mut`] will return values.
    pub fn set_seek(&mut self)
    where
        W: Sized + Seek,
    {
        self.vtable.seek = Some(lifetime_erase_trait_vtable!((&mut self.inner): '_ as Seek));
    }

    /// Set the V-Table for [`Any`].
    ///
    /// After this call, the methods [`Self::as_any`] and [`Self::as_any_mut`] will return values.
    pub fn set_any(&mut self)
    where
        W: Sized + Any,
    {
        self.vtable.any = Some(lifetime_erase_trait_vtable!((&mut self.inner): '_ as Any));
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

    /// Get the inner value as a dynamic `Seek` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_seek(&self) -> Option<&(dyn Seek + '_)> {
        let ptr = &self.inner as *const W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a mutable dynamic `Seek` reference.
    ///
    /// This returns `None` unless a previous call to [`Self::set_seek`] as executed, by any other caller.
    /// The value can be moved after such call arbitrarily.
    pub fn as_seek_mut(&mut self) -> Option<&mut (dyn Seek + '_)> {
        let ptr = &mut self.inner as *mut W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any(&self) -> Option<&'_ dyn Any> {
        let ptr = &self.inner as *const W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any_mut(&mut self) -> Option<&'_ mut dyn Any> {
        let ptr = &mut self.inner as *mut W;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &mut *local })
    }

    /// Unwrap the inner value at its original sized type.
    pub fn into_inner(self) -> W
    where
        W: Sized,
    {
        self.inner
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
    pub fn as_write_mut(&mut self) -> &mut (dyn Write + '_) {
        &mut *self.inner
    }

    pub fn as_seek_mut(&mut self) -> Option<&mut (dyn Seek + '_)> {
        let ptr = self.inner as *mut dyn Write;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.seek?);
        Some(unsafe { &mut *local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any(&self) -> Option<&'_ dyn Any> {
        let ptr = self.inner as *const dyn Write;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &*local })
    }

    /// Get the inner value as a dynamic `Any` reference.
    pub fn as_any_mut(&mut self) -> Option<&'_ mut dyn Any> {
        let ptr = self.inner as *mut dyn Write;
        let local = WithMetadataOf::with_metadata_of_on_stable(ptr, self.vtable.any?);
        Some(unsafe { &mut *local })
    }
}

impl WriterBox<'_> {
    pub fn as_mut(&mut self) -> WriterMut<'_> {
        WriterMut {
            vtable: self.vtable,
            inner: self.as_read_mut(),
        }
    }

    pub fn as_read_mut(&mut self) -> &mut (dyn Write + '_) {
        &mut *self.inner
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
