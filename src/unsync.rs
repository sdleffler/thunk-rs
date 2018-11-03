use std::borrow::{Borrow, BorrowMut};
use std::boxed::FnBox;
use std::cell::{Cell, UnsafeCell};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use unreachable::{unreachable, UncheckedOptionExt};

use {LazyRef, LazyMut, Lazy};


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


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flag {
    Deferred,
    Evaluated,
    Empty,
}


#[allow(unions_with_drop_fields)]
union Cache<T> {
    deferred: Box<FnBox() -> ()>,
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

        let thunk_cast = Box::from_raw(Box::into_raw(thunk) as *mut FnBox() -> T);

        mem::replace(self, Cache { evaluated: thunk_cast() });
    }
}


impl<T> Borrow<T> for Thunk<T> {
    #[inline]
    fn borrow(&self) -> &T {
        self
    }
}


impl<T> BorrowMut<T> for Thunk<T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self
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
        mem::replace(&mut self.data, UnsafeCell::new(Cache { evaluating: () })).into_inner()
    }
}


impl<T> LazyRef for Thunk<T> {
    #[inline]
    fn defer<'a, F: FnBox() -> T + 'a>(f: F) -> Thunk<T>
        where T: 'a
    {
        let thunk = {
            unsafe {
                let thunk_raw: *mut FnBox() -> T = Box::into_raw(Box::new(f));
                Box::from_raw(thunk_raw as *mut (FnBox() -> () + 'static))
            }
        };

        Thunk {
            flag: Cell::new(Flag::Deferred),
            data: UnsafeCell::new(Cache { deferred: thunk }),
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


impl<T> LazyMut for Thunk<T> {}


impl<T> Lazy for Thunk<T> {
    #[inline]
    fn unwrap(mut self) -> T {
        self.force();

        unsafe { self.take_data().evaluated }
    }
}


/// An `Rc`-wrapped `Thunk` which implements `LazyRef`.
pub struct RcThunk<T>(Rc<Thunk<T>>);


impl<T> RcThunk<T> {
    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk` is
    /// the sole, unique owner of the underlying thunk, this will return the forced
    /// value; otherwise, it will return an `Err` containing the original `RcThunk`.
    pub fn try_unwrap(this: RcThunk<T>) -> Result<T, RcThunk<T>> {
        match Rc::try_unwrap(this.0) {
            Ok(thunk) => Ok(thunk.unwrap()),
            Err(rc) => Err(RcThunk(rc)),
        }
    }


    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk` is
    /// the sole, unique owner of the underlying thunk, this will return a
    /// mutable reference to the forced value; otherwise, it will return `None`.
    pub fn get_mut(this: &mut RcThunk<T>) -> Option<&mut T> {
        Rc::get_mut(&mut this.0).map(DerefMut::deref_mut)
    }


    /// If the `RcThunk` is unevaluated, this will force it. If the `RcThunk`
    /// is the sole, unique owner of the underlying thunk, this will return a
    /// mutable reference to the forced value; if it is not, then it will clone
    /// the forced value and return a mutable reference to the newly cloned
    /// value. The `&mut RcThunk` passed in will be updated to reference the
    /// newly cloned value.
    pub fn make_mut(this: &mut RcThunk<T>) -> &mut T
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


impl<T> Clone for RcThunk<T> {
    fn clone(&self) -> RcThunk<T> {
        RcThunk(self.0.clone())
    }
}


impl<T> AsRef<T> for RcThunk<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}


impl<T> Deref for RcThunk<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}


impl<T> From<T> for RcThunk<T> {
    fn from(t: T) -> RcThunk<T> {
        RcThunk(Rc::new(Thunk::computed(t)))
    }
}


impl<T> LazyRef for RcThunk<T> {
    #[inline]
    fn defer<'a, F: FnOnce() -> T + 'a>(f: F) -> RcThunk<T> where T: 'a {
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


    #[test]
    fn rc_thunk_computed() {
        let rc_thunk0 = RcThunk::computed(1 + 1);
        let rc_thunk1 = rc_thunk0.clone();

        assert_eq!(rc_thunk0.0.flag.get(), Flag::Evaluated);
        assert_eq!(&*rc_thunk1, &2);
        assert_eq!(rc_thunk0.0.flag.get(), Flag::Evaluated);
        assert_eq!(&*rc_thunk0, &2);
    }

    #[test]
    fn rc_thunk_deferred() {
        let rc_thunk0 = RcThunk::defer(move || test::black_box(1) + 1);
        let rc_thunk1 = rc_thunk0.clone();

        assert_eq!(rc_thunk0.0.flag.get(), Flag::Deferred);
        assert_eq!(&*rc_thunk1, &2);
        assert_eq!(rc_thunk0.0.flag.get(), Flag::Evaluated);
        assert_eq!(&*rc_thunk0, &2);
    }
}
