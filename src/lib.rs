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

#![cfg_attr(test, feature(test))]
#![feature(unsized_locals)]
#![feature(untagged_unions)]

extern crate unreachable;

#[cfg(test)]
extern crate test;

use std::ops::{Deref, DerefMut};

pub mod strict;
pub mod sync;
pub mod unsync;


pub use crate::strict::Strict;
pub use crate::sync::{AtomicThunk, ArcThunk};
pub use crate::unsync::{Thunk, RcThunk};


/// The `Lazy` trait abstracts thunks which have exactly the same lifetimes
/// as the types they defer computation of.
pub trait LazyRef
    : Deref + Sized
    where Self::Target: Into<Self> + Sized
{
    /// Construct a thunk with a precomputed value. This means
    /// forcing the thunk is a no-op.
    #[inline]
    fn computed(t: Self::Target) -> Self {
        t.into()
    }

    /// Defer a computation stored as a `FnOnce` closure. Unwrapping/dereferencing
    /// will force the computation of the closure. The supplied closure must live
    /// as long as the type which the thunk computes.
    fn defer<'a, F: FnOnce() -> Self::Target + 'a>(closure: F) -> Self where Self::Target: 'a;

    /// Manually force a thunk's computation.
    fn force(&self);
}


/// The `LazyMut` trait abstracts mutable references to lazily computed values.
pub trait LazyMut: From<<Self as Deref>::Target> + LazyRef + DerefMut
    where Self::Target: Sized
{
}


/// The `Lazy` trait abstracts owned, lazily computed values.
pub trait Lazy: LazyMut
    where Self::Target: Sized
{
    /// Unwrap a thunk into its inner value. This forces the thunk.
    fn unwrap(self) -> Self::Target;
}
