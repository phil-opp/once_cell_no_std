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
    ///
    /// This is the value that was handed to [`OnceCell::set`](crate::OnceCell::set), never the one
    /// stored in the cell: a `OnceCell` does not give up a value it has accepted.
    pub fn into_rejected_value(self) -> T {
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

impl<T> From<InsertError<T>> for SetError<T> {
    fn from(error: InsertError<T>) -> Self {
        SetError::ConcurrentInitialization(error.into_rejected_value())
    }
}

/// The value could not be inserted into the `OnceCell`.
///
/// Returned by [`OnceCell::get_or_insert`](crate::OnceCell::get_or_insert), which fails only
/// because another caller is initializing the cell. An already initialized cell is _not_ an error
/// there: the caller still ends up with a reference to a stored value, which
/// [`Insertion`](crate::Insertion) reports instead.
///
/// Carries the value that was not inserted back to the caller, so that it can be reused to retry
/// once the concurrent initialization has finished, instead of being dropped. See
/// [`ConcurrentInitialization`] for details on the error condition itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InsertError<T>(pub(crate) T);

impl<T> InsertError<T> {
    /// Returns the value that was not inserted into the cell.
    pub fn into_rejected_value(self) -> T {
        self.0
    }
}

// Manual impl to avoid a `T: Debug` bound, see [`SetError`].
impl<T> fmt::Debug for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InsertError(..)")
    }
}

impl<T> Display for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ConcurrentInitialization.fmt(f)
    }
}

impl<T> Error for InsertError<T> {}
