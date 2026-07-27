// Forked from `once_cell v1.21.3` crate by @matklad
// Original code available at https://github.com/matklad/once_cell/tree/v1.21.3

//! # Overview
//!
//! `once_cell_no_std` provides a `no_std` [`OnceCell`] type that implements [`Sync`] and **can be used in
//! statics**. It does _not_ use spinlocks or any other form of blocking. Instead, concurrent
//! initialization is reported as an explicit `ConcurrentInitialization` error that the caller can
//! handle as it likes.
//!
//! The only thing this crate requires from the target is atomic compare-and-swap on `u8`. This
//! covers most `no_std` targets, but not all of them: CAS-less targets such as
//! `thumbv6m-none-eabi` (Cortex-M0/M0+), AVR, or RISC-V without the `A` extension are _not_
//! supported and fail to compile.
//!
//! `OnceCell` might store arbitrary non-`Copy` types, can be assigned to at most once and provide direct access
//! to the stored contents. In a nutshell, API looks *roughly* like this:
//!
//! ```rust,ignore
//! impl OnceCell<T> {
//!     fn new() -> OnceCell<T> { ... }
//!     fn set(&self, value: T) -> Result<(), SetError<T>> { ... }
//!     fn get(&self) -> Option<&T> { ... }
//! }
//! ```
//!
//! Note that the `set` method requires only a shared reference, so it can also be used in
//! non-mutable `static`s.
//!
//! ## Example
//!
//! ```rust
//! use std::{env, io};
//!
//! use once_cell_no_std::OnceCell;
//!
//! #[derive(Debug)]
//! pub struct Logger {
//!     // ...
//! }
//! static INSTANCE: OnceCell<Logger> = OnceCell::new();
//!
//! impl Logger {
//!     pub fn global() -> &'static Logger {
//!         INSTANCE.get().expect("logger is not initialized")
//!     }
//!
//!     fn from_cli(args: env::Args) -> Result<Logger, std::io::Error> {
//!        // ...
//! #      Ok(Logger {})
//!     }
//! }
//!
//! fn main() {
//!     let logger = Logger::from_cli(env::args()).unwrap();
//!     INSTANCE.set(logger).unwrap();
//!     // use `Logger::global()` from now on
//! }
//! ```
//!
//! # Implementation details
//!
//! The implementation is heavily based on the
//! [`once_cell`](https://github.com/matklad/once_cell) crate by @matklad, especially the
//! [implementation for parking-lot](https://github.com/matklad/once_cell/blob/master/src/imp_pl.rs).
//!
//! # Related crates
//!
//! This crate was forked from the great
//! [`once_cell` crate](https://docs.rs/once_cell/1.21.3/once_cell/). The original `once_cell` crate
//! provides two flavors of `OnceCell` types: [`unsync::OnceCell`][unsync-once-cell] and
//! [`sync::OnceCell`][sync-once-cell]. The following
//! table compares the types against `once_cell_no_std::OnceCell`:
//!
//! |                                    | `once_cell_no_std::OnceCell`              | [`once_cell::sync::OnceCell`][sync-once-cell]                                                                       | [`once_cell::unsync::OnceCell`][unsync-once-cell] |
//! | ---------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
//! | implements `Sync`                  | yes                                       | yes                                                                                                                 | no                                                |
//! | concurrent initialization leads to | `ConcurrentInitialization` error returned | thread blocked                                                                                                      | cannot happen                                     |
//! | `no_std` supported                 | yes                                       | partially (requires [`critical-section`](https://docs.rs/critical-section/latest/critical_section/) implementation) | yes                                               |
//!
//! Parts of `once_cell` API are included into `std`/`core` [as of Rust 1.70.0](https://github.com/rust-lang/rust/pull/105587).
//! The following table compares `once_cell_no_std::OnceCell` against the [`core::cell::OnceCell`] and [`std::sync::OnceLock`] types:
//!
//! |                                    | `once_cell_no_std::OnceCell`              | [`std::sync::OnceLock`] | [`core::cell::OnceCell`] |
//! | ---------------------------------- | ----------------------------------------- | ----------------------- | ------------------------ |
//! | implements `Sync`                  | yes                                       | yes                     | no                       |
//! | concurrent initialization leads to | `ConcurrentInitialization` error returned | thread blocked          | cannot happen            |
//! | `no_std` supported                 | yes                                       | no                      | yes                      |
//!
//! For more related crates, check out the [README of `once_cell`](https://github.com/matklad/once_cell?tab=readme-ov-file#related-crates).
//!
//! [unsync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/unsync/struct.OnceCell.html
//! [sync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/sync/struct.OnceCell.html
//! [`core::cell::OnceCell`]: https://doc.rust-lang.org/stable/core/cell/struct.OnceCell.html
//! [`std::sync::OnceLock`]: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html

#![no_std]

use core::{fmt, mem};

mod imp;
pub mod error;

use imp::OnceCell as Imp;

use crate::error::{ConcurrentInitialization, InitError, InsertError, SetError};

/// The outcome of a successful [`OnceCell::get_or_insert`] call.
///
/// Either variant means the cell holds a value and that [`stored`](Self::stored) hands out a
/// reference to it. They differ only in _whose_ value that is: the one passed to `get_or_insert`,
/// or one that an earlier caller had already put there.
///
/// # Example
///
/// ```
/// use once_cell_no_std::{Insertion, OnceCell};
///
/// let cell = OnceCell::new();
///
/// // the cell was empty, so the value went in
/// let insertion = cell.get_or_insert(92).unwrap();
/// assert_eq!(insertion, Insertion::Inserted(&92));
/// assert!(insertion.was_inserted());
///
/// // the cell was full, so the value is handed back instead of being dropped
/// let insertion = cell.get_or_insert(62).unwrap();
/// assert_eq!(insertion, Insertion::AlreadyInitialized { stored: &92, rejected: 62 });
/// assert_eq!(insertion.into_rejected_value(), Some(62));
///
/// // either way, `stored` is the value that is in the cell
/// assert_eq!(cell.get_or_insert(17).unwrap().stored(), &92);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insertion<'a, T> {
    /// The cell was empty, so the value was inserted into it.
    Inserted(&'a T),
    /// The cell already held a value, which was left untouched.
    AlreadyInitialized {
        /// A reference to the value that is stored in the cell.
        stored: &'a T,
        /// The value that was not inserted.
        rejected: T,
    },
}

impl<'a, T> Insertion<'a, T> {
    /// Returns a reference to the value that is stored in the cell.
    ///
    /// This is the value passed to [`get_or_insert`](OnceCell::get_or_insert) for
    /// [`Inserted`](Self::Inserted), and the value an earlier caller stored for
    /// [`AlreadyInitialized`](Self::AlreadyInitialized). In both cases it is the value that
    /// [`OnceCell::get`] returns from now on, since an initialized cell keeps its value.
    pub fn stored(&self) -> &'a T {
        match self {
            Insertion::Inserted(stored) | Insertion::AlreadyInitialized { stored, .. } => stored,
        }
    }

    /// Returns whether the value was inserted into the cell.
    pub fn was_inserted(&self) -> bool {
        matches!(self, Insertion::Inserted(_))
    }

    /// Returns the value that was not inserted, or `None` if it was.
    pub fn into_rejected_value(self) -> Option<T> {
        match self {
            Insertion::Inserted(_) => None,
            Insertion::AlreadyInitialized { rejected, .. } => Some(rejected),
        }
    }
}

/// A snapshot of the state of a [`OnceCell`], returned by [`OnceCell::state`].
///
/// # This is an observation, not a decision
///
/// The returned state describes the cell at the moment of the call and may have changed again by
/// the time it is inspected. It is meant for reporting: logging, diagnostics, health checks, and
/// tests. Driving control flow from it is a mistake, because neither of the two "not available"
/// states supports the conclusion it seems to invite:
///
/// - [`Uninitialized`](Self::Uninitialized) does not mean an initialization will succeed. Another
///   caller may start one before you do.
/// - [`Initializing`](Self::Initializing) does not mean an initialization will complete. The init
///   function may fail or panic and return the cell to `Uninitialized`, so a caller that waits for
///   it to finish may wait forever.
///
/// Use [`get_or_init`](OnceCell::get_or_init), [`set`](OnceCell::set), or
/// [`get_or_insert`](OnceCell::get_or_insert) when the answer has to be acted upon: they resolve the race
/// atomically and report contention as of the instant of the attempt.
///
/// [`Initialized`](Self::Initialized) is the one state that is stable: a cell only leaves it
/// through `&mut` access, which no other caller can hold at the same time. Observing it therefore
/// does guarantee that a subsequent [`get`](OnceCell::get) returns `Some`.
///
/// # Example
///
/// ```
/// use once_cell_no_std::{CellState, OnceCell};
///
/// let cell = OnceCell::new();
/// assert_eq!(cell.state(), CellState::Uninitialized);
///
/// cell.set(92).unwrap();
/// assert_eq!(cell.state(), CellState::Initialized);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellState {
    /// The cell is empty and no initialization function is currently running.
    ///
    /// Note that the cell also returns to this state when an initialization function fails or
    /// panics, so this state does not mean that no initialization was attempted yet.
    Uninitialized,
    /// Another caller is currently running an initialization function for this cell.
    Initializing,
    /// The cell holds a value.
    Initialized,
}

/// A thread-safe cell which can be written to only once.
///
/// `OnceCell` provides `&` references to the contents without RAII guards.
///
/// Reading a non-`None` value out of `OnceCell` establishes a
/// happens-before relationship with a corresponding write. For example, if
/// thread A initializes the cell with `get_or_init(f)`, and thread B
/// subsequently reads the result of this call, B also observes all the side
/// effects of `f`.
///
/// `OnceCell` guarantees that at most one initialization function will be called to compute the
/// value. If two threads of execution call [`get_or_init`](Self::get_or_init) (or similar) concurrently, one of them
/// will return a [ConcurrentInitialization] error. It's up to the caller to decide how to handle
/// this error (e.g. wait and retry until the value is initialized by the other thread or panic if
/// this situation is unexpected).
///
/// The alternative to returning the [ConcurrentInitialization] error would be to let one of the
/// threads wait. If this is what you prefer, check out the original
/// [`once_cell::OnceCell`](https://docs.rs/once_cell/1.21.3/once_cell/sync/struct.OnceCell.html)
/// type that this crate is forked from. Note that waiting requires some form of OS support, but
/// also supports `no_std` use cases through its `critical-section` feature.
///
/// # Example
/// ```
/// use once_cell_no_std::OnceCell;
///
/// static CELL: OnceCell<String> = OnceCell::new();
/// assert!(CELL.get().is_none());
///
/// std::thread::spawn(|| {
///     let value: &String = CELL.get_or_init(|| {
///         "Hello, World!".to_string()
///     }).unwrap();
///     assert_eq!(value, "Hello, World!");
/// }).join().unwrap();
///
/// let value: Option<&String> = CELL.get();
/// assert!(value.is_some());
/// assert_eq!(value.unwrap().as_str(), "Hello, World!");
/// ```
///
/// # Handling concurrent initialization
///
/// Since this type never blocks, it is up to the caller to decide what to do when it runs into a
/// concurrent initialization. Every method that can run into it reports it as an explicit error,
/// so a caller that wants to wait can simply retry:
///
/// ```
/// use once_cell_no_std::{error::ConcurrentInitialization, OnceCell};
///
/// /// Returns the value of the cell, initializing it with `init` if it is still empty.
/// ///
/// /// Spins until the value is available if another caller is initializing the cell.
/// fn get_or_spin<T>(cell: &OnceCell<T>, mut init: impl FnMut() -> T) -> &T {
///     loop {
///         // `&mut init` is passed instead of `init` so that it survives for the next attempt
///         match cell.get_or_init(&mut init) {
///             Ok(value) => return value,
///             // another caller is initializing the cell: retry in a busy loop
///             Err(ConcurrentInitialization) => core::hint::spin_loop(),
///         }
///     }
/// }
///
/// let cell = OnceCell::new();
/// assert_eq!(get_or_spin(&cell, || "Hello, World!"), &"Hello, World!");
/// // the init function is not called for an already initialized cell
/// assert_eq!(get_or_spin(&cell, || unreachable!()), &"Hello, World!");
/// ```
///
/// Note that this spins, which is only appropriate if the initialization is short and the
/// execution context allows it. Depending on the system, waiting for an interrupt, yielding to a
/// scheduler, or reporting the error to the caller might be the better choice.
///
/// The above spinning helper is similar to the lazy initialization that
/// [`spin::Once`] and the [`lazy_static`] crate (with its `spin_no_std` feature) provide for
/// `no_std` environments. The advantage of using this crate is that you can easily switch to a
/// custom wait strategy later (e.g. wait for next interrupt instead of busy-looping).
///
/// If the value already exists instead of being computed by an init function, use
/// [`set`](Self::set) or [`get_or_insert`](Self::get_or_insert) in the same way: their errors hand the value
/// back, so the retry does not need to clone it.
///
/// [`spin::Once`]: https://docs.rs/spin/latest/spin/once/struct.Once.html
/// [`lazy_static`]: https://docs.rs/lazy_static/latest/lazy_static/
pub struct OnceCell<T>(Imp<T>);

impl<T> Default for OnceCell<T> {
    fn default() -> OnceCell<T> {
        OnceCell::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.get_or_reason() {
            Ok(value) => f.debug_tuple("OnceCell").field(value).finish(),
            Err(CellState::Initializing) => f.write_str("OnceCell(Initializing)"),
            Err(_) => f.write_str("OnceCell(Uninit)"),
        }
    }
}

impl<T: Clone> Clone for OnceCell<T> {
    fn clone(&self) -> OnceCell<T> {
        match self.get() {
            Some(value) => Self::with_value(value.clone()),
            None => Self::new(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (self.get_mut(), source.get()) {
            (Some(this), Some(source)) => this.clone_from(source),
            _ => *self = source.clone(),
        }
    }
}

impl<T> From<T> for OnceCell<T> {
    fn from(value: T) -> Self {
        Self::with_value(value)
    }
}

impl<T: PartialEq> PartialEq for OnceCell<T> {
    fn eq(&self, other: &OnceCell<T>) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for OnceCell<T> {}

impl<T> OnceCell<T> {
    /// Creates a new empty cell.
    pub const fn new() -> OnceCell<T> {
        OnceCell(Imp::new())
    }

    /// Creates a new initialized cell.
    pub const fn with_value(value: T) -> OnceCell<T> {
        OnceCell(Imp::with_value(value))
    }

    /// Returns whether the cell is initialized.
    ///
    /// This method never blocks. It only reports a snapshot of the cell state, which might have
    /// changed again by the time the returned value is used.
    ///
    /// Prefer [`get`](Self::get) if you need the value itself: it performs the same check, but
    /// hands out a reference in the same step. Use [`state`](Self::state) if you also need to know
    /// whether an initialization is currently in progress.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert!(!cell.is_initialized());
    ///
    /// cell.set(92).unwrap();
    /// assert!(cell.is_initialized());
    /// ```
    pub fn is_initialized(&self) -> bool {
        self.0.is_initialized()
    }

    /// Gets the reference to the underlying value.
    ///
    /// Returns `None` if the cell is empty, or being initialized. This
    /// method never blocks.
    ///
    /// The two cases are not distinguished here, because neither one supports a decision: an empty
    /// cell might be initialized by the time the caller acts on the `None`, and an initialization
    /// in progress might fail and leave the cell empty again.
    ///
    /// Use [`get_or_init`](Self::get_or_init), [`set`](Self::set), or [`get_or_insert`](Self::get_or_insert)
    /// when the difference has to be acted upon. They resolve the race atomically and report
    /// contention as a [`ConcurrentInitialization`] error that describes the cell at the instant
    /// of the attempt, rather than at some earlier point in time. Use [`state`](Self::state) when
    /// the difference only needs to be reported, as in logging or a health check.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.get(), None);
    ///
    /// cell.set(92).unwrap();
    /// assert_eq!(cell.get(), Some(&92));
    /// ```
    pub fn get(&self) -> Option<&T> {
        self.get_or_reason().ok()
    }

    /// Gets the value, or the state that explains why it is not available.
    ///
    /// This is the primitive that [`get`](Self::get) and the [`Debug`](fmt::Debug) implementation
    /// are built from. It exists to keep the `unsafe` read behind a single safe interface, and to
    /// answer from one atomic load, so that the reported state and the value cannot disagree.
    ///
    /// The `Err` value is never [`CellState::Initialized`].
    ///
    /// This is deliberately not public: [`state`](Self::state) already exposes the state, and
    /// because `Initialized` is stable, `state` followed by `get` observes the same thing without
    /// needing a combined accessor.
    fn get_or_reason(&self) -> Result<&T, CellState> {
        match self.0.state() {
            // Safe b/c the `Acquire` load in `state` reported the value as initialized.
            CellState::Initialized => Ok(unsafe { self.get_unchecked() }),
            state => Err(state),
        }
    }

    /// Returns a snapshot of the cell state.
    ///
    /// Unlike [`is_initialized`](Self::is_initialized), this distinguishes an empty cell from one
    /// that another caller is currently initializing. This method never blocks.
    ///
    /// The result describes the cell at the moment of the call and is intended for reporting, not
    /// for deciding what to do next. See [`CellState`] for why, and for which of the three states
    /// can be relied upon afterwards.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::{CellState, OnceCell};
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.state(), CellState::Uninitialized);
    ///
    /// cell.set("hello").unwrap();
    /// assert_eq!(cell.state(), CellState::Initialized);
    /// assert_eq!(cell.get(), Some(&"hello"));
    /// ```
    pub fn state(&self) -> CellState {
        self.0.state()
    }

    /// Gets the mutable reference to the underlying value.
    ///
    /// Returns `None` if the cell is empty.
    ///
    /// Unlike [`get`](Self::get), this is unambiguous: a `None` return value always means that
    /// the cell is empty, never that an initialization is in progress. Since this method requires
    /// `&mut` access, no other caller can hold the shared reference that a concurrent
    /// initialization needs, so the ambiguity cannot arise in the first place.
    ///
    /// This method is allowed to violate the invariant of writing to a `OnceCell`
    /// at most once because it requires `&mut` access to `self`. As with all
    /// interior mutability, `&mut` access permits arbitrary modification:
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let mut cell: OnceCell<u32> = OnceCell::new();
    /// cell.set(92).unwrap();
    /// cell = OnceCell::new();
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.0.get_mut()
    }

    /// Get the reference to the underlying value, without checking if the
    /// cell is initialized.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the cell is in initialized state, and that
    /// the contents are acquired by (synchronized to) this thread.
    #[inline]
    pub unsafe fn get_unchecked(&self) -> &T {
        self.0.get_unchecked()
    }

    /// Sets the contents of this cell to `value`.
    ///
    /// Returns `Ok(())` if the cell was empty. If the cell was already full, a
    /// [`SetError::AlreadyInitialized`] error is returned. If the cell is concurrently being
    /// initialized by another caller, a [`SetError::ConcurrentInitialization`] error is returned.
    ///
    /// Both error variants give `value` back to the caller, so that it can be reused, e.g. to
    /// retry after a concurrent initialization has finished.
    ///
    /// Use [`get_or_insert`](Self::get_or_insert) if you also need a reference to the value that ends up in the
    /// cell.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::{OnceCell, error::SetError};
    ///
    /// static CELL: OnceCell<i32> = OnceCell::new();
    ///
    /// fn main() {
    ///     assert!(CELL.get().is_none());
    ///
    ///     std::thread::spawn(|| {
    ///         assert_eq!(CELL.set(92), Ok(()));
    ///     }).join().unwrap();
    ///
    ///     assert_eq!(CELL.set(62), Err(SetError::AlreadyInitialized(62)));
    ///     assert_eq!(CELL.get(), Some(&92));
    /// }
    /// ```
    pub fn set(&self, value: T) -> Result<(), SetError<T>> {
        match self.get_or_insert(value)? {
            Insertion::Inserted(_) => Ok(()),
            Insertion::AlreadyInitialized { rejected, .. } => {
                Err(SetError::AlreadyInitialized(rejected))
            }
        }
    }

    /// Gets the contents of the cell, initializing it with `value` if the cell was empty.
    ///
    /// This is [`get_or_init`](Self::get_or_init) for a value that already exists, instead of one
    /// computed by a closure. Whether or not `value` is the one that ends up in the cell, the
    /// returned [`Insertion`] hands out a reference to whatever is stored:
    ///
    /// ```
    /// # use once_cell_no_std::OnceCell;
    /// # let cell = OnceCell::new();
    /// # let value = 92;
    /// let stored: &i32 = cell.get_or_insert(value)?.stored();
    /// # Ok::<(), once_cell_no_std::error::InsertError<i32>>(())
    /// ```
    ///
    /// An already initialized cell is therefore not an error. The only failure is a concurrent
    /// initialization by another caller, which leaves no value to hand out at all. Its
    /// [`InsertError`] gives `value` back, so it can be reused for a retry rather than dropped.
    ///
    /// Use [`set`](Self::set) instead when being the caller that initializes the cell is the point,
    /// rather than obtaining the value: it reports an already initialized cell as an error, and its
    /// [`SetError`] does not borrow the cell, so it can be propagated independently.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::{Insertion, OnceCell};
    ///
    /// let cell = OnceCell::new();
    /// assert!(cell.get().is_none());
    ///
    /// assert_eq!(cell.get_or_insert(92), Ok(Insertion::Inserted(&92)));
    /// assert_eq!(
    ///     cell.get_or_insert(62),
    ///     Ok(Insertion::AlreadyInitialized { stored: &92, rejected: 62 })
    /// );
    ///
    /// assert_eq!(cell.get(), Some(&92));
    /// ```
    pub fn get_or_insert(&self, value: T) -> Result<Insertion<'_, T>, InsertError<T>> {
        let mut value = Some(value);
        let stored = match self.get_or_init(|| unsafe { value.take().unwrap_unchecked() }) {
            Ok(stored) => stored,
            Err(ConcurrentInitialization) => {
                // The init closure is only called after the cell was exclusively acquired, so a
                // `ConcurrentInitialization` error means that it never ran and `value` is still
                // there.
                let value = value.take().expect("init closure ran despite a concurrent init");
                return Err(InsertError(value));
            }
        };
        Ok(match value {
            None => Insertion::Inserted(stored),
            Some(rejected) => Insertion::AlreadyInitialized { stored, rejected },
        })
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell
    /// was empty.
    ///
    /// Many callers may invoke `get_or_init` concurrently with different initializing functions,
    /// but it is guaranteed that at most one of them is executed. The other callers receive a
    /// [`ConcurrentInitialization`] error and their `f` is dropped without ever being called.
    ///
    /// # Panics
    ///
    /// If `f` panics, the panic is propagated to the caller, and the cell
    /// remains uninitialized.
    ///
    /// # Reentrancy
    ///
    /// Calling back into the same cell from `f` is safe and never deadlocks, because this type
    /// never blocks. The cell counts as concurrently initializing while `f` runs, so a nested
    /// [`get_or_init`](Self::get_or_init), [`set`](Self::set), or [`get_or_insert`](Self::get_or_insert) on the
    /// same cell returns a [`ConcurrentInitialization`] error.
    ///
    /// Note that such a nested call can never succeed, so `f` must be able to make progress
    /// without it. In particular, retrying in a loop like the one shown in the
    /// [type documentation](Self#handling-concurrent-initialization) does hang when used
    /// reentrantly, since the initialization it waits for is the one that is blocked on the loop.
    ///
    /// # Example
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// let value = cell.get_or_init(|| 92).unwrap();
    /// assert_eq!(value, &92);
    /// let value = cell.get_or_init(|| unreachable!()).unwrap();
    /// assert_eq!(value, &92);
    /// ```
    pub fn get_or_init<F>(&self, f: F) -> Result<&T, ConcurrentInitialization>
    where
        F: FnOnce() -> T,
    {
        enum Void {}
        self.get_or_try_init(|| Ok::<T, Void>(f())).map_err(|error| match error {
            InitError::InitFunctionFailed(void) => match void {},
            InitError::ConcurrentInitialization => ConcurrentInitialization,
        })
    }

    /// Gets the contents of the cell, initializing it with `f` if
    /// the cell was empty. If the cell was empty and `f` failed, an
    /// [`InitError::InitFunctionFailed`] error is returned.
    ///
    /// If the cell is concurrently being initialized by another caller, an
    /// [`InitError::ConcurrentInitialization`] error is returned. In that case `f` was _not_
    /// executed.
    ///
    /// # Retrying after a concurrent initialization
    ///
    /// An `f` that is not executed is dropped, together with everything that it captured. If `f`
    /// owns a resource that is needed for a retry, keep the ownership in the surrounding scope and
    /// let `f` borrow it:
    ///
    /// ```
    /// use once_cell_no_std::{error::InitError, OnceCell};
    ///
    /// # struct Uart;
    /// # struct Driver;
    /// # impl Driver { fn new(_uart: Uart) -> Driver { Driver } }
    /// let cell = OnceCell::new();
    /// let mut uart = Some(Uart);
    ///
    /// let result = cell.get_or_try_init(|| -> Result<_, ()> {
    ///     // `f` is called at most once, so the `Option` is always `Some` here
    ///     Ok(Driver::new(uart.take().expect("init function called twice")))
    /// });
    ///
    /// match result {
    ///     // the cell is initialized now, either by `f` or by an earlier caller
    ///     Ok(_driver) => {}
    ///     // `f` never ran, so `uart` is still available for another attempt
    ///     Err(InitError::ConcurrentInitialization) => assert!(uart.is_some()),
    ///     // `f` never returns an error in this example
    ///     Err(InitError::InitFunctionFailed(())) => unreachable!(),
    /// }
    /// ```
    ///
    /// Note that this is only needed for resources that cannot be recreated. If the value itself
    /// already exists, prefer [`set`](Self::set) or [`get_or_insert`](Self::get_or_insert), whose errors
    /// hand it back directly.
    ///
    /// # Panics
    ///
    /// If `f` panics, the panic is propagated to the caller, and
    /// the cell remains uninitialized.
    ///
    /// # Reentrancy
    ///
    /// Calling back into the same cell from `f` is safe and never deadlocks, because this type
    /// never blocks. The cell counts as concurrently initializing while `f` runs, so a nested
    /// [`get_or_try_init`](Self::get_or_try_init), [`set`](Self::set), or
    /// [`get_or_insert`](Self::get_or_insert) on the same cell returns a
    /// [`InitError::ConcurrentInitialization`] error.
    ///
    /// Note that such a nested call can never succeed, so `f` must be able to make progress
    /// without it. In particular, retrying in a loop like the one shown in the
    /// [type documentation](Self#handling-concurrent-initialization) does hang when used
    /// reentrantly, since the initialization it waits for is the one that is blocked on the loop.
    ///
    /// # Example
    /// ```
    /// use once_cell_no_std::{OnceCell, error::InitError};
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(
    ///     cell.get_or_try_init(|| Err(())),
    ///     Err(InitError::InitFunctionFailed(()))
    /// );
    /// assert!(cell.get().is_none());
    /// let value = cell.get_or_try_init(|| -> Result<i32, ()> {
    ///     Ok(92)
    /// });
    /// assert_eq!(value, Ok(&92));
    /// assert_eq!(cell.get(), Some(&92))
    /// ```
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, InitError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // Fast path check
        if let Some(value) = self.get() {
            return Ok(value);
        }

        self.0.try_initialize(f)?;

        // Safe b/c value is initialized.
        debug_assert!(self.0.is_initialized());
        Ok(unsafe { self.get_unchecked() })
    }

    /// Takes the value out of this `OnceCell`, moving it back to an uninitialized state.
    ///
    /// Has no effect and returns `None` if the `OnceCell` hasn't been initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let mut cell: OnceCell<String> = OnceCell::new();
    /// assert_eq!(cell.take(), None);
    ///
    /// let mut cell = OnceCell::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.take(), Some("hello".to_string()));
    /// assert_eq!(cell.get(), None);
    /// ```
    ///
    /// This method is allowed to violate the invariant of writing to a `OnceCell`
    /// at most once because it requires `&mut` access to `self`. As with all
    /// interior mutability, `&mut` access permits arbitrary modification:
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let mut cell: OnceCell<u32> = OnceCell::new();
    /// cell.set(92).unwrap();
    /// cell = OnceCell::new();
    /// ```
    pub fn take(&mut self) -> Option<T> {
        mem::take(self).into_inner()
    }

    /// Consumes the `OnceCell`, returning the wrapped value. Returns
    /// `None` if the cell was empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let cell: OnceCell<String> = OnceCell::new();
    /// assert_eq!(cell.into_inner(), None);
    ///
    /// let cell = OnceCell::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.into_inner(), Some("hello".to_string()));
    /// ```
    #[inline]
    pub fn into_inner(self) -> Option<T> {
        self.0.into_inner()
    }
}
