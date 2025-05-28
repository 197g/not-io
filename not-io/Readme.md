# not-io

Not the `std::io` module, but a very close compatibility interface for
no-std/no-alloc environments. This dissolves into a shim-layer when std
features are enabled, with a trick to _guarantee_ compatibility.

This approach works because, luckily, no `std::io` trait has any _non-default_
function that consumes an allocated container. Only some extension methods
exist which may be implemented sub-optimally if the implementer crate provides
no opt-in to the features.

This crate aims to stay as close to `std` and `alloc` as possible. No
extensions of the traits are planned and the traits will strictly _tail_
stabilized interface. No exceptions. For example, `no_std` may rely more
heavily on `EWOULDBLOCK`/`WouldBlock` to 'refill' streams by an outer loop.
However, we will _not_ add any helpers here. Propose those as an official RFC
(or libs-PR if small enough), or simply provide an extension crate, which is
what a stable, compatible version policy (`1.0`) enables.

## Comparison to similar crates

There are several alternatives. Why should you choose this one? Simply put, no
other crate I found is *stable*, *compatible*, and well behaved for feature
combinations. Most either target _purely_ no-std or are extensions. All other
crates get the SemVer/feature compatibility wrong, that is they permit adding
incompatible impls in some configuration.

- [`genio`](https://docs.rs/genio), extension.
- [`acid_io`](https://docs.rs/acid_io), extension.
- [`bare-io`](https://docs.rs/bare-io), extension,
- [`core_io`](https://crates.io/crates/core_io), nightly
- [`ciborium-io`](https://docs.rs/ciborium-io), feature incompatible, extension
- [`zorio`](https://crates.io/crates/zorio), no std compatibility
