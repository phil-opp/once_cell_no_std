use core::{error::Error, fmt::Display};

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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
