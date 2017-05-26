use std::ops::{Deref, DerefMut};

use ::Lazy;


/// A do-nothing, strict thunk. This is intended for implementing structures which
/// are generic over strictness.
pub struct Thunk<T>(T);


impl<T> From<T> for Thunk<T> {
    fn from(t: T) -> Thunk<T> {
        Thunk(t)
    }
}


impl<T> AsRef<T> for Thunk<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}


impl<T> AsMut<T> for Thunk<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}


impl<T> Deref for Thunk<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}


impl<T> DerefMut for Thunk<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}


impl<T> Lazy for Thunk<T> {
    #[inline]
    fn defer<F: FnOnce() -> T + 'static>(f: F) -> Thunk<T> {
        Thunk(f())
    }


    #[inline]
    fn force(&self) {}


    #[inline]
    fn unwrap(self) -> T { self.0 }
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
