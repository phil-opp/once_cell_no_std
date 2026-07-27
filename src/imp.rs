use core::{
    cell::UnsafeCell,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::atomic::{AtomicU8, Ordering},
};

use crate::error::{ConcurrentInitialization, GetError, InitError};

pub(crate) struct OnceCell<T> {
    state: AtomicU8,
    value: UnsafeCell<Option<T>>,
}

const INCOMPLETE: u8 = 0x0;
const RUNNING: u8 = 0x1;
const COMPLETE: u8 = 0x2;

// Why do we need `T: Send`?
// Thread A creates a `OnceCell` and shares it with
// scoped thread B, which fills the cell, which is
// then destroyed by A. That is, destructor observes
// a sent value.
unsafe impl<T: Sync + Send> Sync for OnceCell<T> {}
unsafe impl<T: Send> Send for OnceCell<T> {}

impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceCell<T> {}
impl<T: UnwindSafe> UnwindSafe for OnceCell<T> {}

impl<T> OnceCell<T> {
    pub(crate) const fn new() -> OnceCell<T> {
        OnceCell { state: AtomicU8::new(INCOMPLETE), value: UnsafeCell::new(None) }
    }

    pub(crate) const fn with_value(value: T) -> OnceCell<T> {
        OnceCell { state: AtomicU8::new(COMPLETE), value: UnsafeCell::new(Some(value)) }
    }

    /// Safety: synchronizes with store to value via Release/Acquire.
    #[inline]
    pub(crate) fn is_initialized(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }

    /// Like [`is_initialized`](Self::is_initialized), but reports _why_ the cell is not
    /// initialized.
    ///
    /// Safety: synchronizes with store to value via Release/Acquire.
    #[inline]
    pub(crate) fn check_initialized(&self) -> Result<(), GetError> {
        match self.state.load(Ordering::Acquire) {
            COMPLETE => Ok(()),
            RUNNING => Err(GetError::ConcurrentInitialization),
            _ => Err(GetError::Uninitialized),
        }
    }

    /// Safety: the `INCOMPLETE` -> `RUNNING` compare-exchange in
    /// [`try_initialize_inner`] acts as the exclusive claim on the value slot. Only the caller that
    /// wins it runs `f` and writes the slot, so the value is written at most once. The `Release`
    /// store that ends the `RUNNING` state synchronizes with the `Acquire` loads in
    /// [`is_initialized`](Self::is_initialized) and
    /// [`check_initialized`](Self::check_initialized).
    #[cold]
    pub(crate) fn try_initialize<F, E>(&self, f: F) -> Result<(), InitError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut f = Some(f);
        let mut res: Result<(), E> = Ok(());
        let slot: *mut Option<T> = self.value.get();
        try_initialize_inner(&self.state, &mut || {
            // We are calling a user-supplied function and need to be careful.
            // - if it returns Err, the `Guard` resets the state to `INCOMPLETE` and we return
            //   without ever touching the slot
            // - if it panics, the `Guard` resets the state to `INCOMPLETE` and the panic
            //   propagates without the slot having been touched
            // - if it calls `set` or `get_or_try_init` re-entrantly, that nested call finds the
            //   state to be `RUNNING` and fails with `ConcurrentInitialization` without touching
            //   the slot. This is what keeps a second writer from aliasing the slot, so it is
            //   important for safety.
            // - finally, if it returns Ok, we store the value and the `Guard` then stores
            //   `COMPLETE` with `Release`, which synchronizes with the `Acquire` loads.
            let f = unsafe { f.take().unwrap_unchecked() };
            match f() {
                Ok(value) => unsafe {
                    // Safe b/c we have a unique access and no panic may happen
                    // until the cell is marked as initialized.
                    debug_assert!((*slot).is_none());
                    *slot = Some(value);
                    true
                },
                Err(err) => {
                    res = Err(err);
                    false
                }
            }
        })?;
        res.map_err(InitError::InitFunctionFailed)
    }

    /// Get the reference to the underlying value, without checking if the cell
    /// is initialized.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the cell is in initialized state, and that
    /// the contents are acquired by (synchronized to) this thread.
    pub(crate) unsafe fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_initialized());
        let slot = &*self.value.get();
        slot.as_ref().unwrap_unchecked()
    }

    /// Gets the mutable reference to the underlying value.
    /// Returns `None` if the cell is empty.
    pub(crate) fn get_mut(&mut self) -> Option<&mut T> {
        // Safe b/c we have an exclusive access
        let slot: &mut Option<T> = unsafe { &mut *self.value.get() };
        slot.as_mut()
    }

    /// Consumes this `OnceCell`, returning the wrapped value.
    /// Returns `None` if the cell was empty.
    pub(crate) fn into_inner(self) -> Option<T> {
        self.value.into_inner()
    }
}

struct Guard<'a> {
    state: &'a AtomicU8,
    new_state: u8,
}

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        self.state.store(self.new_state, Ordering::Release);
    }
}

// Note: this is intentionally monomorphic
/// Tries to run the given `init` function, returns `Err(ConcurrentInitialization)` when there is a
/// concurrent init function running.
///
/// If the `state` is already `COMPLETE` (i.e. already initialized), the given `init` function is
/// _not_ executed and `Ok(())` is returned directly.
#[inline(never)]
fn try_initialize_inner(
    state: &AtomicU8,
    init: &mut dyn FnMut() -> bool,
) -> Result<(), ConcurrentInitialization> {
    loop {
        let exchange =
            state.compare_exchange_weak(INCOMPLETE, RUNNING, Ordering::Acquire, Ordering::Acquire);
        match exchange {
            Ok(_) => {
                let mut guard = Guard { state, new_state: INCOMPLETE };
                if init() {
                    guard.new_state = COMPLETE;
                }
                return Ok(());
            }
            Err(COMPLETE) => return Ok(()),
            Err(RUNNING) => return Err(ConcurrentInitialization),
            Err(INCOMPLETE) => (),
            Err(_) => debug_assert!(false),
        }
    }
}

#[test]
fn test_size() {
    use core::mem::size_of;

    assert_eq!(size_of::<OnceCell<bool>>(), size_of::<bool>() + size_of::<u8>());
}
