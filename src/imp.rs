use core::{
    cell::UnsafeCell,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::atomic::{AtomicU8, Ordering},
};

use crate::{
    CellState,
    error::{ConcurrentInitialization, InitError},
};

pub(crate) struct OnceCell<T> {
    state: AtomicU8,
    value: UnsafeCell<Option<T>>,
}

const INCOMPLETE: u8 = 0x0;
const RUNNING: u8 = 0x1;
const COMPLETE: u8 = 0x2;

// SAFETY: the state machine hands write access to the value to at most one caller at a time, and
// the `Release` store that ends the `RUNNING` state synchronizes that write with every `Acquire`
// load that reads it. Sharing the cell across threads is therefore sound whenever `T` itself can
// be shared and sent.
//
// `T: Send` is needed on top of `T: Sync` because the value can cross threads without the cell
// itself moving: thread A creates a `OnceCell` and shares it with scoped thread B, which fills the
// cell, which is then destroyed by A. That is, A's destructor observes a value sent from B.
unsafe impl<T: Sync + Send> Sync for OnceCell<T> {}

// SAFETY: moving the cell to another thread moves the `T` it may hold, which `T: Send` allows. The
// state atomic can be sent unconditionally.
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

    /// Whether the cell holds a value.
    ///
    /// Safety: see [`state`](Self::state), which performs the load.
    #[inline]
    pub(crate) fn is_initialized(&self) -> bool {
        self.state() == CellState::Initialized
    }

    /// The current state of the cell.
    ///
    /// This is the only plain read of `state` outside of the initialization path, so it is the
    /// single place that maps the raw constants onto the state machine.
    ///
    /// Safety: synchronizes with store to value via Release/Acquire.
    #[inline]
    pub(crate) fn state(&self) -> CellState {
        match self.state.load(Ordering::Acquire) {
            COMPLETE => CellState::Initialized,
            RUNNING => CellState::Initializing,
            _ => CellState::Uninitialized,
        }
    }

    /// Safety: the `INCOMPLETE` -> `RUNNING` compare-exchange in
    /// [`try_initialize_inner`] acts as the exclusive claim on the value slot. Only the caller that
    /// wins it runs `f` and writes the slot, so the value is written at most once. The `Release`
    /// store that ends the `RUNNING` state synchronizes with the `Acquire` loads in
    /// [`is_initialized`](Self::is_initialized) and [`state`](Self::state).
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
            debug_assert!(f.is_some(), "init closure called twice");
            // SAFETY: `try_initialize_inner` runs this closure at most once, so `f` has not been
            // taken yet. See the contract documented on that function.
            let f = unsafe { f.take().unwrap_unchecked() };
            match f() {
                // SAFETY: winning the compare-exchange gave this caller exclusive access to the
                // slot, and no panic can happen between the write and the cell being marked as
                // initialized.
                Ok(value) => unsafe {
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
        // SAFETY: the caller guarantees that the cell is initialized, so the slot holds a `Some`
        // that no one is writing to, and that the write is synchronized to this thread.
        unsafe {
            let slot = &*self.value.get();
            slot.as_ref().unwrap_unchecked()
        }
    }

    /// Gets the mutable reference to the underlying value.
    /// Returns `None` if the cell is empty.
    pub(crate) fn get_mut(&mut self) -> Option<&mut T> {
        // SAFETY: `&mut self` rules out any other access to the slot, including an in-progress
        // initialization, which would need a shared reference to the cell.
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
///
/// # Contract relied upon by callers
///
/// Callers use `unsafe` to take values out of `Option`s that `init` is expected to have consumed
/// or left alone, so the following two properties are load-bearing for soundness. Both hold by
/// inspection of the loop below, which contains exactly one call to `init`:
///
/// 1. **`init` runs at most once.** The only call is in the arm that won the `INCOMPLETE` ->
///    `RUNNING` compare-exchange, and that arm returns immediately instead of looping again. Every
///    other arm either returns or retries the compare-exchange without calling `init`.
/// 2. **`init` never runs when `Err(ConcurrentInitialization)` is returned.** That error is
///    produced only by the arm that observed the cell in the `RUNNING` state, which does not call
///    `init` and returns straight away.
///
/// Keep both properties in mind when changing this function.
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
