pub struct Take<R> {
    pub(crate) inner: R,
    pub(crate) limit: u64,
}
