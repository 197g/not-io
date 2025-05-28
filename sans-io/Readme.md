Enable a sans-IO model by an internal state machine.

# Motivation

Coding sans-IO decoders is a weird affair. You have an internal state machine
that should be fed input bytes and will write to output bytes (or an output
state) but in a function interface you will need to do all this yourself. The
resulting code is far from simple to read, and crucially trust its correctness
due to the non-local control flow. Meanwhile Rust offers a mechanism for that
transformation: `async`. This crate enables utilizing `async` and interacting
with input/output buffers. Then internally it drives the resulting state
machine within a highly specific execution context.

# Future Development

The implementation is far from nice. We move an attribute into a coroutine,
then do tremendous workarounds with pinning and `task::Context` to mutably
access that attribute both outside and while running. But such moved data is
just an attribute on the synthesized coroutine type! If we could access it as
such, the implementation would be much cleaner,simpler, and less scary. I think
we could get rid of all `unsafe`.
