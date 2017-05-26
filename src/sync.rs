use std::boxed::FnBox;
use std::cell::UnsafeCell;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use unreachable::{unreachable, UncheckedOptionExt};

use ::Lazy;

/// A thread-safe `AtomicThunk`, representing a lazily computed value.
///
/// TODO: Test `Option<UnsafeCell<Cache<T>>>` instead of storing thunk
/// invalidation in the atomic `flag`.
pub struct AtomicThunk<T> {
    /// The `lock` mutex is used for preventing other threads from accessing the
    /// thunk's internals when a thunk is evaluating.
    lock: Mutex<()>,

    /// The `flag` represents the current state of the thunk - deferred, evaluated,
    /// locking, or locked.
    flag: AtomicUsize,

    /// The thunk and/or its computed result are stored in an `UnsafeCell` so that
    /// the fact that a `AtomicThunk` is either computed *or* non-computed can be made
    /// opaque to the user. This way, an immutable reference can have its thunk
    /// forced.
    data: UnsafeCell<Cache<T>>,
}


unsafe impl<T: Send> Send for AtomicThunk<T> {}
unsafe impl<T: Sync> Sync for AtomicThunk<T> {}


/// The `AtomicThunk` is not yet evaluated. We can try to lock it and evaluate.
const THUNK_DEFERRED: usize = 0;

/// The `AtomicThunk` is evaluated, and can be safely accessed.
const THUNK_EVALUATED: usize = 1;

/// The `AtomicThunk` is currently *locking* - the `Mutex` is not yet locked but will
/// be very soon.
const THUNK_LOCKING: usize = 2;

/// The thread which is going to evaluate the `AtomicThunk` has a lock on the `Mutex`.
/// When the `Mutex` becomes unlocked, the computed result may be accessed.
const THUNK_LOCKED: usize = 3;

/// There is no data in the `AtomicThunk` - it has been removed and dealt with. Thus,
/// the thunk is invalidated and should only be dropped. Any function which can
/// put the thunk in this state is already marked unsafe.
const THUNK_INVALIDATED: usize = 4;


/// The storage for a possibly deferred, thread-safe thunk. A thunk is either
/// deferred - in which case it contains a boxed closure which holds necessary
/// data to run the deferred computation; or, it holds the already computed
/// result.
#[allow(unions_with_drop_fields)]
union Cache<T> {
    deferred: Box<FnBox() -> T>,
    evaluated: T,

    #[allow(dead_code)]
    evaluating: (),
}


impl<T> Drop for AtomicThunk<T> {
    fn drop(&mut self) {
        match unsafe { ptr::read(&self.flag) }.into_inner() {
            THUNK_DEFERRED => mem::drop(unsafe { self.take_data().deferred }),
            THUNK_EVALUATED => mem::drop(unsafe { self.take_data().evaluated }),
            THUNK_INVALIDATED => {},
            THUNK_LOCKING | THUNK_LOCKED => unreachable!("thunks should never be dropped while locking or locked!"),
            _ => unsafe { unreachable() },
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


impl<T> AsRef<T> for AtomicThunk<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self
    }
}


impl<T> AsMut<T> for AtomicThunk<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self
    }
}


impl<T> Deref for AtomicThunk<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.force();

        unsafe {
            &self.data
                .get()
                .as_ref()
                .unchecked_unwrap()
                .evaluated
        }
    }
}


impl<T> DerefMut for AtomicThunk<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.force();

        unsafe {
            &mut self.data
                .get()
                .as_mut()
                .unchecked_unwrap()
                .evaluated
        }
    }
}


impl<T> From<T> for AtomicThunk<T> {
    #[inline]
    fn from(t: T) -> AtomicThunk<T> {
        AtomicThunk {
            lock: Mutex::new(()),
            flag: AtomicUsize::new(THUNK_EVALUATED),
            data: UnsafeCell::new(Cache { evaluated: t }),
        }
    }
}


impl<T> AtomicThunk<T> {
    #[inline]
    fn take_data(&mut self) -> Cache<T> {
        self.flag.store(THUNK_INVALIDATED, Ordering::Relaxed);
        unsafe { mem::replace(&mut self.data, UnsafeCell::new(Cache { evaluating: () })).into_inner() }
    }


    /// PRECONDITIONS: flag must not be THUNK_DEFERRED or THUNK_INVALIDATED.
    ///
    /// `.besiege()` expects an evaluated or locked `AtomicThunk`.
    /// - If the `AtomicThunk` is locking, it will spin until the `AtomicThunk` is locked and
    ///   then wait to acquire and summarily release the mutex.
    /// - If the `AtomicThunk` is locked, it will wait for a lock on the mutex before
    ///   immediately releasing it and returning.
    /// - If the `AtomicThunk` is evaluated, it will immediately return.
    #[inline]
    unsafe fn besiege(&self) {
        loop {
            match self.flag.load(Ordering::Acquire) {
                // If the AtomicThunk has been evaluated, unwrap it and return it.
                THUNK_EVALUATED => return,

                // If we're waiting for the lock to become available, then spin.
                THUNK_LOCKING => {}

                // If the lock is available, lock it so that we can stop
                // spinning in place.
                THUNK_LOCKED => {
                    let _ = self.lock.lock().unwrap();
                    return;
                }

                THUNK_DEFERRED | THUNK_INVALIDATED | _ => unreachable(),
            }
        }
    }
}


impl<T> Lazy for AtomicThunk<T> {
    #[inline]
    fn defer<F: FnBox() -> T + 'static>(f: F) -> AtomicThunk<T> {
        AtomicThunk {
            lock: Mutex::new(()),
            flag: AtomicUsize::new(THUNK_DEFERRED),
            data: UnsafeCell::new(Cache { deferred: Box::new(f) }),
        }
    }


    #[inline]
    fn force(&self) {
        match self.flag
                  .compare_and_swap(THUNK_DEFERRED, THUNK_LOCKING, Ordering::Acquire) {
            // If we've successfully taken control of the AtomicThunk:
            THUNK_DEFERRED => {
                // Lock the mutex, and then set the flag to THUNK_LOCKED so that
                // other threads know that they can stop spinning and instead
                // lock the mutex. This lets them consume less resources by
                // relying on the scheduler to wake them up, rather than spin
                // until the mutex is released. (??? is this true?)
                let _mutex_lock = self.lock.lock().unwrap();
                self.flag.store(THUNK_LOCKED, Ordering::Release);

                unsafe {
                    (*self.data.get()).evaluate_thunk();

                    // The mutex will be unlocked at the end of the scope - first
                    // though, we store THUNK_EVALUATED into the flag so that
                    // threads released from the mutex see the correct "EVALUATED"
                    // flag and threads which did not see THUNK_LOCKING or
                    // THUNK_LOCKED and have not acquired the mutex are allowed
                    // to grab the value.
                    self.flag.store(THUNK_EVALUATED, Ordering::Release);
                }
            }

            /// If the `AtomicThunk` is evaluated, do nothing.
            THUNK_EVALUATED => {}

            /// If the `AtomicThunk` is `LOCKING` or `LOCKED`, wait until the thunk is
            /// done evaluating and then return a reference to the inner value.
            THUNK_LOCKING | THUNK_LOCKED => unsafe { self.besiege() },

            /// Only `THUNK_DEFERRED`, `THUNK_EVALUATED`, `THUNK_LOCKING`, and
            /// `THUNK_LOCKED` are valid values of the flag.
            THUNK_INVALIDATED | _ => unsafe { unreachable() },
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
        let thunk = AtomicThunk::computed(1 + 1);

        assert_eq!(*thunk, 2);
    }

    #[test]
    fn thunk_deferred() {
        let thunk = AtomicThunk::defer(|| test::black_box(1) + 1);

        assert_eq!(*thunk, 2);
    }

    fn ten_thousand_xors_strict(n: usize) -> AtomicThunk<usize> {
        AtomicThunk::computed((0..test::black_box(10000)).fold(test::black_box(n), |old, new| old ^ new))
    }

    fn ten_thousand_xors_lazy(n: usize) -> AtomicThunk<usize> {
        AtomicThunk::defer(move || {
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
