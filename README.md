[![Build Status](https://travis-ci.org/sdleffler/thunk-rs.svg?branch=master)](https://travis-ci.org/sdleffler/thunk-rs)
[![Docs Status](https://docs.rs/thunk/badge.svg)](https://docs.rs/thunk)
[![On crates.io](https://img.shields.io/crates/v/thunk.svg)](https://crates.io/crates/thunk)

N.B. this crate requires nightly, as it makes use of `FnBox` and untagged unions.

# `thunk`: Generic lazy evaluation for Rust

The `thunk` crate provides primitives for lazy evaluation in Rust.

At present, it provides five thunk types and three traits which abstract lazily
evaluated types, `LazyRef`, `LazyMut`, and `Lazy`. The thunk types are as follows:

 * `Thunk`: non-`Send`, non-`Sync` thunks.
 * `RcThunk`: a reference-counted, cloneable thunk. An `RcThunk<T>` is essentially
   an `Rc<Thunk<T>>`; however, it implements `Lazy`.
 * `AtomicThunk`: `Send + Sync` thunks which use atomic data internally. This is
   slower than `Thunk`, but `AtomicThunk` is thread-safe.
 * `ArcThunk`: the `Arc` equivalent to `RcThunk`. Essentially an
   `Arc<AtomicThunk<T>>`.
 * `Strict`: `Send + Sync`, paradoxically strict thunks. `Strict`
   doesn't actually defer anything, and is provided to make it simpler to write
   code which is generic over strictness.

The provided traits - `LazyRef`, `LazyMut`, and `Lazy` - abstract immutable
references to lazy values, mutable references to lazy values, and owned lazy
values, respectively. For example, `Thunk` implements `LazyRef` and `LazyMut` and
`Lazy`; however, `RcThunk` only implements `LazyRef`. All traits take a lifetime
parameter; this is a bandaid to cover Rust's current lack of associated lifetimes.
This parameter bounds the lifetime of the closure object representing a deferred
computation.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
