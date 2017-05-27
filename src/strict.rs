use std::ops::{Deref, DerefMut};

use ::{LazyRef, LazyMut, Lazy};


/// A do-nothing, strict "thunk". This is intended for implementing structures which
/// are generic over strictness.
#[derive(Clone, Copy)]
pub struct Strict<T>(T);


impl<T> From<T> for Strict<T> {
    fn from(t: T) -> Strict<T> {
        Strict(t)
    }
}


impl<T> AsRef<T> for Strict<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}


impl<T> AsMut<T> for Strict<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}


impl<T> Deref for Strict<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}


impl<T> DerefMut for Strict<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}


impl<'a, T: 'a> LazyRef<'a> for Strict<T> {
    #[inline]
    fn defer<F: FnOnce() -> T + 'a>(f: F) -> Strict<T> {
        Strict(f())
    }


    #[inline]
    fn force(&self) {}
}


impl<'a, T: 'a> LazyMut<'a> for Strict<T> {}


impl<'a, T: 'a> Lazy<'a> for Strict<T> {
    #[inline]
    fn unwrap(self) -> T { self.0 }
}


#[cfg(test)]
mod test {
    use super::*;

    use test::{self, Bencher};

    #[test]
    fn thunk_computed() {
        let thunk = Strict::computed(1 + 1);

        assert_eq!(*thunk, 2);
    }

    #[test]
    fn thunk_deferred() {
        let thunk = Strict::defer(|| test::black_box(1) + 1);

        assert_eq!(*thunk, 2);
    }

    fn ten_thousand_xors_strict(n: usize) -> Strict<usize> {
        Strict::computed((0..test::black_box(10000)).fold(test::black_box(n), |old, new| old ^ new))
    }

    fn ten_thousand_xors_lazy(n: usize) -> Strict<usize> {
        Strict::defer(move || {
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
