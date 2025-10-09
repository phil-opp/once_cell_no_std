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
