# not-io

A collection of IO crates based on experience with image decoding. At the time
of writing, the Rust ecosystem is split into three camps for the purpose of
representing data inputs: `std::io`, `tokio`, embedded. These are incompatible
with each other, fundamentally as with `no_std` and `std::io` types/traits, or
on a SemVer level such as supporting `std::io` as an optional feature while
still consuming `&[u8]` streams at `no_std`.

These are the result of years of experience in failing to bridge these two
camps.
