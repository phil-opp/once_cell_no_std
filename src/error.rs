use core::{
    error::Error,
    fmt::{self, Display},
};

/// There is another init function running concurrently for the same `OnceCell`.
///
/// Initialization functions write to the value wrapped by the `OnceCell` directly. Multiple
/// concurrent write operations would result in undefined behavior, so the `OnceCell` type guards
/// against this.
///
/// This error means that there is already another concurrent initialization function running that
/// has exclusive access to the wrapped value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcurrentInitialization;

impl Display for ConcurrentInitialization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "another init function is running concurrently")
    }
}

impl Error for ConcurrentInitialization {}

/// The reason why the value of a `OnceCell` is not available.
///
/// Returned by [`OnceCell::try_get`](crate::OnceCell::try_get), which is the fallible counterpart
/// of [`OnceCell::get`](crate::OnceCell::get).
///
/// Both variants are a _snapshot_ of the cell state at the time of the call. The state might
/// already have changed again when this error is handled, so the variants are best treated as
/// hints, not as guarantees. In particular, an [`Uninitialized`](Self::Uninitialized) error is no
/// promise that a subsequent initialization attempt will succeed, and a
/// [`ConcurrentInitialization`](Self::ConcurrentInitialization) error is no promise that the
/// concurrent initialization will complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetError {
    /// The cell is empty and no initialization function is currently running.
    ///
    /// Note that the cell also returns to this state when an initialization function returns an
    /// error or panics, so this variant does not mean that no initialization was attempted yet.
    Uninitialized,
    /// There is another init function running concurrently for the same `OnceCell`.
    ///
    /// See [`ConcurrentInitialization`] for details.
    ConcurrentInitialization,
}

impl Display for GetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GetError::Uninitialized => write!(f, "the cell is not initialized"),
            GetError::ConcurrentInitialization => ConcurrentInitialization.fmt(f),
        }
    }
}

impl Error for GetError {}

impl From<ConcurrentInitialization> for GetError {
    fn from(_: ConcurrentInitialization) -> Self {
        GetError::ConcurrentInitialization
    }
}

/// The cell could not be initialized.
///
/// Returned by [`OnceCell::get_or_try_init`](crate::OnceCell::get_or_try_init), which fails either
/// because the init function itself failed, or because another caller is already initializing the
/// cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError<E> {
    /// The init function returned an error.
    ///
    /// The cell is left uninitialized, so the operation can be retried.
    InitFunctionFailed(E),
    /// There is another init function running concurrently for the same `OnceCell`.
    ///
    /// The init function of this call was _not_ executed. It is dropped together with everything
    /// that it captured, so it cannot be reused for a retry. See
    /// [`OnceCell::get_or_try_init`](crate::OnceCell::get_or_try_init) for how to keep ownership
    /// of captured resources, and [`ConcurrentInitialization`] for details on the error itself.
    ConcurrentInitialization,
}

impl<E> InitError<E> {
    /// Returns the error of the init function, or `None` for a
    /// [`ConcurrentInitialization`](Self::ConcurrentInitialization) error.
    pub fn init_function_error(self) -> Option<E> {
        match self {
            InitError::InitFunctionFailed(error) => Some(error),
            InitError::ConcurrentInitialization => None,
        }
    }
}

impl<E: fmt::Display> fmt::Display for InitError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitError::InitFunctionFailed(error) => write!(f, "the init function failed: {error}"),
            InitError::ConcurrentInitialization => ConcurrentInitialization.fmt(f),
        }
    }
}

impl<E: Error + 'static> Error for InitError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InitError::InitFunctionFailed(error) => Some(error),
            InitError::ConcurrentInitialization => None,
        }
    }
}

impl<E> From<ConcurrentInitialization> for InitError<E> {
    fn from(_: ConcurrentInitialization) -> Self {
        InitError::ConcurrentInitialization
    }
}

/// The value could not be written to the `OnceCell`.
///
/// Returned by [`OnceCell::set`](crate::OnceCell::set). Every variant carries the value that was
/// _not_ written back to the caller, so that it can be reused (e.g. to retry after a
/// [`ConcurrentInitialization`](Self::ConcurrentInitialization) error) instead of being dropped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetError<T> {
    /// The cell was already initialized with a different value.
    ///
    /// Contains the value that was not written to the cell.
    AlreadyInitialized(T),
    /// There is another init function running concurrently for the same `OnceCell`.
    ///
    /// Contains the value that was not written to the cell, so that the operation can be retried
    /// once the concurrent initialization is finished. See [`ConcurrentInitialization`] for
    /// details.
    ConcurrentInitialization(T),
}

impl<T> SetError<T> {
    /// Returns the value that was not written to the cell.
    pub fn into_inner(self) -> T {
        match self {
            SetError::AlreadyInitialized(value) | SetError::ConcurrentInitialization(value) => {
                value
            }
        }
    }
}

// Manual impl to avoid a `T: Debug` bound, which would make `set(..).unwrap()` unusable for
// non-`Debug` types.
impl<T> fmt::Debug for SetError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetError::AlreadyInitialized(_) => f.write_str("SetError::AlreadyInitialized(..)"),
            SetError::ConcurrentInitialization(_) => {
                f.write_str("SetError::ConcurrentInitialization(..)")
            }
        }
    }
}

impl<T> Display for SetError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetError::AlreadyInitialized(_) => write!(f, "the cell is already initialized"),
            SetError::ConcurrentInitialization(_) => ConcurrentInitialization.fmt(f),
        }
    }
}

impl<T> Error for SetError<T> {}

impl<T> From<InsertError<'_, T>> for SetError<T> {
    fn from(error: InsertError<'_, T>) -> Self {
        match error {
            InsertError::AlreadyInitialized { value, .. } => SetError::AlreadyInitialized(value),
            InsertError::ConcurrentInitialization(value) => {
                SetError::ConcurrentInitialization(value)
            }
        }
    }
}

/// The value could not be inserted into the `OnceCell`.
///
/// Returned by [`OnceCell::insert`](crate::OnceCell::insert). Like [`SetError`], every
/// variant carries the value that was _not_ inserted back to the caller, so that it can be reused
/// instead of being dropped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InsertError<'a, T> {
    /// The cell was already initialized with a different value.
    AlreadyInitialized {
        /// A reference to the value that is stored in the cell.
        stored: &'a T,
        /// The value that was not inserted.
        value: T,
    },
    /// There is another init function running concurrently for the same `OnceCell`.
    ///
    /// Contains the value that was not inserted, so that the operation can be retried once the
    /// concurrent initialization is finished. See [`ConcurrentInitialization`] for details.
    ConcurrentInitialization(T),
}

impl<T> InsertError<'_, T> {
    /// Returns the value that was not inserted into the cell.
    pub fn into_inner(self) -> T {
        match self {
            InsertError::AlreadyInitialized { value, .. }
            | InsertError::ConcurrentInitialization(value) => value,
        }
    }
}

// Manual impl to avoid a `T: Debug` bound, see [`SetError`].
impl<T> fmt::Debug for InsertError<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::AlreadyInitialized { .. } => {
                f.write_str("InsertError::AlreadyInitialized { .. }")
            }
            InsertError::ConcurrentInitialization(_) => {
                f.write_str("InsertError::ConcurrentInitialization(..)")
            }
        }
    }
}

impl<T> Display for InsertError<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::AlreadyInitialized { .. } => write!(f, "the cell is already initialized"),
            InsertError::ConcurrentInitialization(_) => ConcurrentInitialization.fmt(f),
        }
    }
}

impl<T> Error for InsertError<'_, T> {}
