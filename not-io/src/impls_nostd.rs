impl core::fmt::Debug for super::ErrorInner {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let super::ErrorInner::Kind(inner) = self;
        core::fmt::Debug::fmt(inner, f)
    }
}

impl super::Error {
    pub(crate) fn is_interrupted_impl(&self) -> bool {
        false
    }

    pub(crate) fn from_kind_impl(kind: super::ErrorKind) -> Self {
        super::Error {
            inner: super::ErrorInner::Kind(kind),
        }
    }
}
