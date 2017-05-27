//! This crate provides functionality for thread-safe and non-thread-safe lazy
//! evaluation in Rust. It also provides functionality for generically working
//! with thunks of a given type.
//!
//! Three different owned thunk types are provided, implementing `Lazy`,
//! `LazyRef`, and `LazyMut`:
//! * `Thunk`: a non thread-safe thunk.
//! * `AtomicThunk`: a thread-safe thunk, which implements `Send + Sync`.
//! * `Strict`: a strict, non-deferred thunk which always immediately
//!   evaluates whatever computation it's given, intended for genericity over
//!   strictness.
//!
//! In addition, two shared thunk types are provided, implementing `LazyRef`
//! and `LazyShared`:
//! * `RcThunk`: a reference-counted thunk type. This is a wrapper over `Thunk`.
//! * `ArcThunk`: an atomically reference-counted thunk type. This is a wrapper
//!   over `AtomicThunk`.

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
pub use sync::{AtomicThunk, ArcThunk};
pub use unsync::{Thunk, RcThunk};


/// The `LazyRef` trait abstracts immutable references to lazily computed values.
pub trait LazyRef<'a>
    : Deref + AsRef<<Self as Deref>::Target> + From<<Self as Deref>::Target> + 'a
    where Self::Target: Sized
{
    /// Construct a thunk with a precomputed value. This means
    /// unwrapping/dereferencing is effectively a no-op.
    #[inline]
    fn computed(t: Self::Target) -> Self {
        t.into()
    }

    /// Defer a computation stored as a `FnOnce` closure. Unwrapping/dereferencing
    /// will force the computation of the closure.
    fn defer<F: FnOnce() -> Self::Target + 'a>(F) -> Self;

    /// Manually force a thunk's computation.
    fn force(&self);
}


/// The `LazyMut` trait abstracts mutable references to lazily computed values.
pub trait LazyMut<'a>: LazyRef<'a> + DerefMut + AsMut<<Self as Deref>::Target>
    where Self::Target: Sized
{
}


/// The `Lazy` trait abstracts owned, lazily computed values.
pub trait Lazy<'a>: LazyRef<'a> + LazyMut<'a>
    where Self::Target: Sized
{
    /// Unwrap a thunk into its inner value. This forces the thunk.
    fn unwrap(self) -> Self::Target;
}
