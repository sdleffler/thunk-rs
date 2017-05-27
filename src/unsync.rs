use std::boxed::FnBox;
use std::cell::{Cell, UnsafeCell};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use unreachable::{unreachable, UncheckedOptionExt};

use {LazyRef, LazyMut, Lazy};


/// A non-thread-safe `Thunk`, representing a lazily computed value.
pub struct Thunk<'a, T: 'a> {
    /// The `Flag` value is used to represent the state of the thunk. Ordinarily
    /// it would be idiomatic Rust to simply store the `Cache` value as an enum,
    /// and carry this `Flag` data as part of the enum discriminant; however,
    /// here, it remains simpler to use an untagged union for the enum value in
    /// order to avoid the need to check discriminants.
    flag: Cell<Flag>,

    /// Interior mutability is used here so that the fact that dereferencing a
    /// `Thunk` may cause a mutation is abstracted away.
    data: UnsafeCell<Cache<'a, T>>,
}


#[derive(Clone, Copy, Debug)]
enum Flag {
    Deferred,
    Evaluated,
    Empty,
}


#[allow(unions_with_drop_fields)]
union Cache<'a, T: 'a> {
    deferred: Box<FnBox() -> T + 'a>,
    evaluated: T,

    #[allow(dead_code)]
    evaluating: (),
}


impl<'a, T: 'a> Drop for Thunk<'a, T> {
    fn drop(&mut self) {
        match self.flag.get() {
            Flag::Deferred => mem::drop(unsafe { self.take_data().deferred }),
            Flag::Evaluated => mem::drop(unsafe { self.take_data().evaluated }),
            Flag::Empty => {}
        }
    }
}


impl<'a, T: 'a> Cache<'a, T> {
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


impl<'a, T: 'a> AsRef<T> for Thunk<'a, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}


impl<'a, T: 'a> AsMut<T> for Thunk<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}


impl<'a, T: 'a> Deref for Thunk<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.force();

        unsafe { &self.data.get().as_ref().unchecked_unwrap().evaluated }
    }
}


impl<'a, T: 'a> DerefMut for Thunk<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.force();

        unsafe { &mut self.data.get().as_mut().unchecked_unwrap().evaluated }
    }
}


impl<'a, T: 'a> From<T> for Thunk<'a, T> {
    #[inline]
    fn from(t: T) -> Thunk<'a, T> {
        Thunk {
            flag: Cell::new(Flag::Evaluated),
            data: UnsafeCell::new(Cache { evaluated: t }),
        }
    }
}


impl<'a, T: 'a> Thunk<'a, T> {
    #[inline]
    fn take_data(&mut self) -> Cache<'a, T> {
        self.flag.set(Flag::Empty);
        unsafe {
            mem::replace(&mut self.data, UnsafeCell::new(Cache { evaluating: () })).into_inner()
        }
    }
}


impl<'a, T: 'a> LazyRef<'a> for Thunk<'a, T> {
    #[inline]
    fn defer<F: FnBox() -> T + 'a>(f: F) -> Thunk<'a, T> {
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
}


impl<'a, T: 'a> LazyMut<'a> for Thunk<'a, T> {}


impl<'a, T: 'a> Lazy<'a> for Thunk<'a, T> {
    #[inline]
    fn unwrap(mut self) -> T {
        self.force();

        unsafe { self.take_data().evaluated }
    }
}


/// An `Rc`-wrapped `Thunk` which implements `LazyRef`.
pub struct RcThunk<'a, T: 'a>(Rc<Thunk<'a, T>>);


impl<'a, T: 'a> RcThunk<'a, T> {
    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk` is
    /// the sole, unique owner of the underlying thunk, this will return the forced
    /// value; otherwise, it will return an `Err` containing the original `RcThunk`.
    pub fn try_unwrap(this: RcThunk<'a, T>) -> Result<T, RcThunk<'a, T>> {
        match Rc::try_unwrap(this.0) {
            Ok(thunk) => Ok(thunk.unwrap()),
            Err(rc) => Err(RcThunk(rc)),
        }
    }


    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk` is
    /// the sole, unique owner of the underlying thunk, this will return a
    /// mutable reference to the forced value; otherwise, it will return `None`.
    pub fn get_mut<'b>(this: &'b mut RcThunk<'a, T>) -> Option<&'b mut T> {
        Rc::get_mut(&mut this.0).map(DerefMut::deref_mut)
    }


    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk`
    /// is the sole, unique owner of the underlying thunk, this will return a
    /// mutable reference to the forced value; if it is not, then it will clone
    /// the forced value and return a mutable reference to the newly cloned
    /// value. The `&mut RcThunk` passed in will be updated to reference the
    /// newly cloned value.
    pub fn make_mut<'b>(this: &'b mut RcThunk<'a, T>) -> &'b mut T
        where T: Clone
    {
        // No, moving it into a temp doesn't help. We just have to trust the CSE
        // pass here. This is a known borrowchecking issue.
        if Rc::get_mut(&mut this.0).is_some() {
            return &mut **Rc::get_mut(&mut this.0)
                .expect("We know it's `some` - this won't change.");
        }

        let new_rc = Rc::new(Thunk::computed((*this.0).clone()));
        this.0 = new_rc;
        RcThunk::get_mut(this).unwrap()
    }
}


impl<'a, T: 'a> Clone for RcThunk<'a, T> {
    fn clone(&self) -> RcThunk<'a, T> {
        RcThunk(self.0.clone())
    }
}


impl<'a, T: 'a> AsRef<T> for RcThunk<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}


impl<'a, T: 'a> Deref for RcThunk<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}


impl<'a, T: 'a> From<T> for RcThunk<'a, T> {
    fn from(t: T) -> RcThunk<'a, T> {
        RcThunk(Rc::new(Thunk::computed(t)))
    }
}


impl<'a, T: 'a> LazyRef<'a> for RcThunk<'a, T> {
    #[inline]
    fn defer<F: FnOnce() -> T + 'a>(f: F) -> RcThunk<'a, T> {
        RcThunk(Rc::new(Thunk::defer(f)))
    }


    #[inline]
    fn force(&self) {
        self.0.force();
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

    fn ten_thousand_xors_strict<'a>(n: usize) -> Thunk<'a, usize> {
        Thunk::computed((0..test::black_box(10000)).fold(test::black_box(n), |old, new| old ^ new))
    }

    fn ten_thousand_xors_lazy<'a>(n: usize) -> Thunk<'a, usize> {
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
