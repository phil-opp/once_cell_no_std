//! The primitives that [`imp`](crate::imp) is built on, swapped for loom's models under
//! `--cfg loom`.
//!
//! Loom replaces atomics and `UnsafeCell` with instrumented versions in order to explore thread
//! interleavings exhaustively. Its `UnsafeCell` hands out pointers through closures so that it can
//! observe the whole access, rather than through a bare `get`.
//!
//! Following the layout `tokio` uses for the same purpose, the `cfg` switch lives here alone: the
//! state machine is written once, against loom's API, and the non-loom build gets a shim with the
//! same shape that compiles away.

#[cfg(not(all(test, loom)))]
pub(crate) use core::sync::atomic::{AtomicU8, Ordering};
#[cfg(all(test, loom))]
pub(crate) use loom::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, Ordering},
};

#[cfg(not(all(test, loom)))]
pub(crate) use shim::UnsafeCell;

#[cfg(not(all(test, loom)))]
mod shim {
    /// Mirrors the API of [`loom::cell::UnsafeCell`], compiling down to a plain
    /// [`core::cell::UnsafeCell`].
    #[derive(Debug)]
    pub(crate) struct UnsafeCell<T>(core::cell::UnsafeCell<T>);

    impl<T> UnsafeCell<T> {
        pub(crate) const fn new(data: T) -> UnsafeCell<T> {
            UnsafeCell(core::cell::UnsafeCell::new(data))
        }

        #[inline(always)]
        pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
            f(self.0.get())
        }

        #[inline(always)]
        pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
            f(self.0.get())
        }

        pub(crate) fn into_inner(self) -> T {
            self.0.into_inner()
        }
    }
}
