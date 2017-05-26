[![Build Status](https://travis-ci.org/sdleffler/thunk-rs.svg?branch=master)](https://travis-ci.org/sdleffler/thunk-rs)
[![Docs Status](https://docs.rs/thunk/badge.svg)](https://docs.rs/thunk)
[![On crates.io](https://img.shields.io/crates/v/thunk.svg)](https://crates.io/crates/thunk)

N.B. this crate requires nightly, as it makes use of `FnBox`.

# `thunk`: Generic lazy evaluation for Rust

The `thunk` crate provides primitives for lazy evaluation in Rust.

At present, it provides three thunk types and a trait which encapsulates lazily
evaluated types, `Lazy`. The thunk types are as follows:

 * `unsync::Thunk`: non-`Send`, non-`Sync` thunks.
 * `sync::Thunk`: `Send + Sync` thunks which use atomic data internally. This is
   slower than `unsync::Thunk`, but `sync::Thunk` is thread-safe.
 * `strict::Thunk`: `Send + Sync`, paradoxically strict thunks. `strict::Thunk`
   doesn't actually defer anything, and is provided to make it simpler to write
   code which is generic over strictness.

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
