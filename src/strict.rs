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
