//! Flexible IO allows you to choose seekable or buffered IO at runtime.
//!
//! The motivation of this is enabling APIs in which use of a reader (or writer) can be optimized
//! in case of it being buffered, but it's not required for correct functioning. Or, an API where
//! the Seek requirement is only determined at runtime, such as when a portion of the functionality
//! does not depend on it, and an error should be returned.
//!
//! Note that the wrapped type can not be unsized (`dyn` trait) itself. This may be fixed at a
//! later point to make the reader suitable for use in embedded. In particular, the double
//! indirection of instantiating with `R = &mut dyn Read` wouldn't make sense as the setters would
//! not be usable, their bounds can never be met. And combining traits into a large dyn-trait is
//! redundant as it trait-impls become part of the static validity requirement again.
#![cfg_attr(feature = "unstable_set_ptr_value", feature(set_ptr_value))]
#[deny(missing_docs)]

macro_rules! lifetime_erase_trait_vtable {
    ((&mut $r:expr): $lt:lifetime as $trait:path) => {{
        // Safety: Transmuting pointer-to-pointer, and they only differ by lifetime. Types must not
        // be specialized on lifetime parameters.
        let vtable = (&mut $r) as &mut (dyn $trait + $lt) as *mut (dyn $trait + $lt);
        unsafe { core::mem::transmute::<_, *mut (dyn $trait + 'static)>(vtable) }
    }};
}

// We solve this by a macro since it is actually more readable than spending ~4 lines of the same
// structure where we have potential copy-paste problems mentioning the trait name twice and the
// path of the value is not really obvious (a lot of noise around it).
//
// Also, it affords us the ability to make the syntax for all the getters similar.
macro_rules! dyn_setter {
    (
        impl $(<$R:ident>)? $tyfamily:path = self as $self:ident {
            $(
                $(#[$meta:meta])*
                fn $name:ident -> $trait:path = $lhs:expr;
            )*
        }
    ) => {
        impl$(<$R>)* $tyfamily {
            $(
                $(#[$meta])*
                pub fn $name(&mut self)
                    where R: $trait
                {
                    let $self = self;
                    $lhs = Some(lifetime_erase_trait_vtable!((&mut $self.inner): '_ as $trait));
                }
            )*
        }
    }
}

/// Macro to simply re-combination of pointer with vtable, such as for ReaderMut.
///
/// ```ignore
/// impl ReaderMut<'_> {
///     fn as_seek_mut(&mut self) -> Option<&'_ mut dyn Seek> {…}
/// }
/// ```
///
/// Requires that the type provide expressions to access the raw pointer (const and mutable) and
/// the expression to the vtable and then does all the plumbing for recombination. For curious
/// reasons we can not use `self` in these expressions so we first rename `self` value. Due to
/// macro cleanliness this new name must be passed in when the expressions want to refer to it.
macro_rules! dyn_getter {
    (
        impl$(<$R:ident>)? $tyfamily:path = self as $self:ident {
            // Safety requirement: must borrow and create a pointer that is valid for all the
            // vtable entries that are used in the body.
            unsafe const ptr: $constptr:expr;
            // Safety requirement: must mutably borrow and create a pointer that is valid for all
            // the vtable entries that are used in the body.
            unsafe mut ptr: $mutptr:expr;
        } {
            $(
                $(#[$meta:meta])*
                fn $name:ident $({ $(#[$mutmeta:meta])* mut: fn $mut:ident})? -> $trait:path = $lhs:expr;
            )*
        }
    ) => {
        impl$(<$R: ?Sized>)* $tyfamily {
            $(
                $(#[$meta])*
                pub fn $name(&self) -> Option<&'_ dyn $trait> {
                    let $self = self;
                    let raw = $constptr;
                    let local = WithMetadataOf::with_metadata_of_on_stable(raw, $lhs?);
                    Some(unsafe { &*local })
                }

                $(
                    $(#[$mutmeta])*
                    pub fn $mut(&mut self) -> Option<&'_ mut dyn $trait> {
                        let $self = self;
                        let raw = $mutptr;
                        let local = WithMetadataOf::with_metadata_of_on_stable(raw, $lhs?);
                        Some(unsafe { &mut *local })
                    }
                )*
            )*
        }
    }
}

/// Provides wrappers for values of [`Read`](std::io::Read) types.
pub mod reader;
/// Provides wrappers for values of [`Write`](std::io::Write) types.
pub mod writer;

mod stable_with_metadata_of;

pub use reader::Reader;
pub use writer::Writer;
