// Forked from `once_cell v1.21.3` crate by @matklad
// Original code available at https://github.com/matklad/once_cell/tree/v1.21.3

//! # Overview
//!
//! `once_cell_no_std` provides a `no_std` [`OnceCell`] type that implements [`Sync`] and can be used in
//! statics. It does _not_ use spinlocks or any other form of blocking. Instead, concurrent
//! initialization is reported as an explicit `ConcurrentInitialization` error that the caller can
//! handle as it likes.
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

use crate::error::{ConcurrentInitialization, GetError, InsertError, SetError};

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
pub struct OnceCell<T>(Imp<T>);

impl<T> Default for OnceCell<T> {
    fn default() -> OnceCell<T> {
        OnceCell::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.get() {
            Some(v) => f.debug_tuple("OnceCell").field(v).finish(),
            None => f.write_str("OnceCell(Uninit)"),
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

    /// Gets the reference to the underlying value.
    ///
    /// Returns `None` if the cell is empty, or being initialized. This
    /// method never blocks.
    ///
    /// Use [`try_get`](Self::try_get) if you need to distinguish these two cases.
    pub fn get(&self) -> Option<&T> {
        self.try_get().ok()
    }

    /// Gets the reference to the underlying value or report why it is not available.
    ///
    /// This method is similar [`get`](Self::get), but it returns a detailed [`GetError`] instead
    /// of `None` if the value is not available.
    ///
    /// The method returns [`GetError::Uninitialized`] if the cell is empty and
    /// [`GetError::ConcurrentInitialization`] if another caller is currently initializing it.
    /// Like `get`, this method never blocks.
    ///
    /// The returned error is a snapshot of the cell state, which might have changed again by the
    /// time the error is handled. See [`GetError`] for details.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::{OnceCell, error::GetError};
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.try_get(), Err(GetError::Uninitialized));
    ///
    /// cell.set(92).unwrap();
    /// assert_eq!(cell.try_get(), Ok(&92));
    /// ```
    pub fn try_get(&self) -> Result<&T, GetError> {
        self.0.check_initialized()?;
        // Safe b/c value is initialized.
        Ok(unsafe { self.get_unchecked() })
    }

    /// Gets the mutable reference to the underlying value.
    ///
    /// Returns `None` if the cell is empty.
    ///
    /// Unlike [`get`](Self::get), this is unambiguous: a `None` return value always means that
    /// the cell is empty, never that an initialization is in progress. Since this method requires
    /// `&mut` access, no other caller can hold the shared reference that a concurrent
    /// initialization needs, so there is no `try_get_mut` counterpart.
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
        match self.try_insert(value) {
            Ok(_) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Like [`set`](Self::set), but also returns a reference to the final cell value.
    ///
    /// # Example
    ///
    /// ```
    /// use once_cell_no_std::{OnceCell, error::InsertError};
    ///
    /// let cell = OnceCell::new();
    /// assert!(cell.get().is_none());
    ///
    /// assert_eq!(cell.try_insert(92), Ok(&92));
    /// assert_eq!(
    ///     cell.try_insert(62),
    ///     Err(InsertError::AlreadyInitialized { stored: &92, value: 62 })
    /// );
    ///
    /// assert!(cell.get().is_some());
    /// ```
    pub fn try_insert(&self, value: T) -> Result<&T, InsertError<'_, T>> {
        let mut value = Some(value);
        let res = match self.get_or_init(|| unsafe { value.take().unwrap_unchecked() }) {
            Ok(res) => res,
            Err(ConcurrentInitialization) => {
                // The init closure is only called after the cell was exclusively acquired, so a
                // `ConcurrentInitialization` error means that it never ran and `value` is still
                // there.
                let value = value.take().expect("init closure ran despite a concurrent init");
                return Err(InsertError::ConcurrentInitialization(value));
            }
        };
        match value {
            None => Ok(res),
            Some(value) => Err(InsertError::AlreadyInitialized { stored: res, value }),
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell
    /// was empty.
    ///
    /// Many threads may call `get_or_init` concurrently with different
    /// initializing functions, but it is guaranteed that only one function
    /// will be executed.
    ///
    /// # Panics
    ///
    /// If `f` panics, the panic is propagated to the caller, and the cell
    /// remains uninitialized.
    ///
    /// It is an error to reentrantly initialize the cell from `f`. The
    /// exact outcome is unspecified. Current implementation deadlocks, but
    /// this may be changed to a panic in the future.
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
        match self.get_or_try_init(|| Ok::<T, Void>(f()))? {
            Ok(val) => Ok(val),
            Err(void) => match void {},
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if
    /// the cell was empty. If the cell was empty and `f` failed, an
    /// `Ok(Err(_))` value is returned.
    ///
    /// If the cell is concurrently being initialized by another caller, an
    /// `Err(ConcurrentInitialization)` is returned.
    ///
    /// # Panics
    ///
    /// If `f` panics, the panic is propagated to the caller, and
    /// the cell remains uninitialized.
    ///
    /// It is an error to reentrantly initialize the cell from `f`.
    /// The exact outcome is unspecified. Current implementation
    /// deadlocks, but this may be changed to a panic in the future.
    ///
    /// # Example
    /// ```
    /// use once_cell_no_std::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.get_or_try_init(|| Err(())).unwrap(), Err(()));
    /// assert!(cell.get().is_none());
    /// let value = cell.get_or_try_init(|| -> Result<i32, ()> {
    ///     Ok(92)
    /// }).unwrap();
    /// assert_eq!(value, Ok(&92));
    /// assert_eq!(cell.get(), Some(&92))
    /// ```
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<Result<&T, E>, ConcurrentInitialization>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // Fast path check
        if let Some(value) = self.get() {
            return Ok(Ok(value));
        }

        match self.0.try_initialize(f)? {
            Ok(()) => {}
            Err(error) => return Ok(Err(error)),
        }

        // Safe b/c value is initialized.
        debug_assert!(self.0.is_initialized());
        Ok(Ok(unsafe { self.get_unchecked() }))
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
