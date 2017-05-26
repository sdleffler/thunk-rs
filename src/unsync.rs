use std::boxed::FnBox;
use std::cell::{Cell, UnsafeCell};
use std::mem;
use std::ops::{Deref, DerefMut};

use unreachable::{unreachable, UncheckedOptionExt};

use ::Lazy;


/// A non-thread-safe `Thunk`, representing a lazily computed value.
pub struct Thunk<T> {
    /// The `Flag` value is used to represent the state of the thunk. Ordinarily
    /// it would be idiomatic Rust to simply store the `Cache` value as an enum,
    /// and carry this `Flag` data as part of the enum discriminant; however,
    /// here, it remains simpler to use an untagged union for the enum value in
    /// order to avoid the need to check discriminants.
    flag: Cell<Flag>,

    /// Interior mutability is used here so that the fact that dereferencing a
    /// `Thunk` may cause a mutation is abstracted away.
    data: UnsafeCell<Cache<T>>,
}


#[derive(Clone, Copy, Debug)]
enum Flag {
    Deferred,
    Evaluated,
    Empty,
}


#[allow(unions_with_drop_fields)]
union Cache<T> {
    deferred: Box<FnBox() -> T>,
    evaluated: T,

    #[allow(dead_code)]
    evaluating: (),
}


impl<T> Drop for Thunk<T> {
    fn drop(&mut self) {
        match self.flag.get() {
            Flag::Deferred => mem::drop(unsafe { self.take_data().deferred }),
            Flag::Evaluated => mem::drop(unsafe { self.take_data().evaluated }),
            Flag::Empty => {}
        }
    }
}


impl<T> Cache<T> {
    /// PRECONDITION: `Cache` must be `Deferred`! UB results otherwise.
    ///
    /// Evaluate the thunk and replace the `Cache` with an `Evaluated` value
    /// containing the computed result.
    #[inline]
    unsafe fn evaluate_thunk(&mut self) {
        let Cache { deferred: thunk } = mem::replace(self, Cache { evaluating: () });
        mem::replace(self, Cache { evaluated: thunk() });
    }
}


impl<T> AsRef<T> for Thunk<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}


impl<T> AsMut<T> for Thunk<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}


impl<T> Deref for Thunk<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.force();

        unsafe { &self.data.get().as_ref().unchecked_unwrap().evaluated }
    }
}


impl<T> DerefMut for Thunk<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.force();

        unsafe { &mut self.data.get().as_mut().unchecked_unwrap().evaluated }
    }
}


impl<T> From<T> for Thunk<T> {
    #[inline]
    fn from(t: T) -> Thunk<T> {
        Thunk {
            flag: Cell::new(Flag::Evaluated),
            data: UnsafeCell::new(Cache { evaluated: t }),
        }
    }
}


impl<T> Thunk<T> {
    #[inline]
    fn take_data(&mut self) -> Cache<T> {
        self.flag.set(Flag::Empty);
        unsafe {
            mem::replace(&mut self.data, UnsafeCell::new(Cache { evaluating: () })).into_inner()
        }
    }
}


impl<T> Lazy for Thunk<T> {
    #[inline]
    fn defer<F: FnBox() -> T + 'static>(f: F) -> Thunk<T> {
        Thunk {
            flag: Cell::new(Flag::Deferred),
            data: UnsafeCell::new(Cache { deferred: Box::new(f) }),
        }
    }


    #[inline]
    fn force(&self) {
        match self.flag.get() {
            Flag::Deferred => {
                unsafe {
                    (*self.data.get()).evaluate_thunk();
                }

                self.flag.set(Flag::Evaluated);
            }
            Flag::Evaluated => {}
            Flag::Empty => unsafe { unreachable() },
        }
    }


    #[inline]
    fn unwrap(mut self) -> T {
        self.force();

        unsafe { self.take_data().evaluated }
    }
}


#[cfg(test)]
mod test {
    use super::*;

    use test::{self, Bencher};

    #[test]
    fn thunk_computed() {
        let thunk = Thunk::computed(1 + 1);

        assert_eq!(*thunk, 2);
    }

    #[test]
    fn thunk_deferred() {
        let thunk = Thunk::defer(|| test::black_box(1) + 1);

        assert_eq!(*thunk, 2);
    }

    fn ten_thousand_xors_strict(n: usize) -> Thunk<usize> {
        Thunk::computed((0..test::black_box(10000)).fold(test::black_box(n), |old, new| old ^ new))
    }

    fn ten_thousand_xors_lazy(n: usize) -> Thunk<usize> {
        Thunk::defer(move || {
                         (0..test::black_box(10000)).fold(test::black_box(n), |old, new| old ^ new)
                     })
    }

    #[bench]
    fn ten_thousand_xors_threadsafe_strict(b: &mut Bencher) {
        b.iter(|| {
                   let mut things: Vec<_> = (0..1000).map(ten_thousand_xors_strict).collect();
                   test::black_box(things.pop())
               })
    }

    #[bench]
    fn ten_thousand_xors_threadsafe_lazy(b: &mut Bencher) {
        b.iter(|| {
            let mut things: Vec<_> = (0..1000).map(ten_thousand_xors_lazy).collect();
            test::black_box(things.pop())
        })
    }
}
