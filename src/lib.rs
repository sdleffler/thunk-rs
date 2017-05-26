//! This crate provides functionality for thread-safe and non-thread-safe lazy
//! evaluation in Rust. It also provides functionality for generically working
//! with thunks of a given type.
//!
//! Three different thunk types are provided, all implementing `Lazy`:
//! * `unsync::Thunk`: a non thread-safe thunk.
//! * `sync::Thunk`: a thread-safe thunk, which implements `Send + Sync`.
//! * `strict::Thunk`: a strict, non-deferred thunk which always immediately
//!   evaluates whatever computation it's given, intended for genericity over
//!   strictness.

#![feature(fnbox)]
#![feature(untagged_unions)]
#![cfg_attr(test, feature(test))]

extern crate unreachable;

#[cfg(test)]
extern crate test;

use std::ops::{Deref, DerefMut};

pub mod strict;
pub mod sync;
pub mod unsync;


pub use strict::Strict;
pub use sync::AtomicThunk;
pub use unsync::Thunk;


/// The `Lazy` trait abstracts lazily computed values, also known as "thunks".
pub trait Lazy
    where Self: AsRef<<Self as Deref>::Target> + AsMut<<Self as Deref>::Target>,
          Self: Deref + DerefMut + From<<Self as Deref>::Target>,
          Self::Target: Sized
{
    /// Construct a thunk with a precomputed value. This means
    /// unwrapping/dereferencing is effectively a no-op.
    #[inline]
    fn computed(t: Self::Target) -> Self {
        t.into()
    }

    /// Defer a computation stored as a `FnOnce` closure. Unwrapping/dereferencing
    /// will force the computation of the closure.
    fn defer<F: FnOnce() -> Self::Target + 'static>(F) -> Self;

    /// Manually force a thunk's computation.
    fn force(&self);

    /// Unwrap a thunk into its inner value. This forces the thunk.
    fn unwrap(self) -> Self::Target;
}
