# Changelog

## Unreleased

This release reworks how failures are reported. Every fallible method used to return a nested
`Result<Result<_, _>, ConcurrentInitialization>`, which forced callers to unwrap twice and dropped
the value they had passed in whenever the cell turned out to be busy. The nesting is gone: each
method returns a single `Result` with a dedicated error type, and every error that was handed a
value hands it back.

### Breaking

- `OnceCell::set` returns `Result<(), SetError<T>>` instead of
  `Result<Result<(), T>, ConcurrentInitialization>`. Both `SetError` variants carry the value that
  was not written, which `SetError::into_rejected_value` recovers. Previously a concurrent
  initialization dropped that value, making a retry impossible.
- `OnceCell::try_insert` was renamed to `OnceCell::get_or_insert` and returns
  `Result<Insertion<'_, T>, InsertError<T>>` instead of
  `Result<Result<&T, (&T, T)>, ConcurrentInitialization>`. An already initialized cell is no longer
  reported as an error, because the caller still ends up with a reference to a stored value; that
  is the `Insertion::AlreadyInitialized` outcome, which hands the rejected value back as well. The
  only remaining failure is a concurrent initialization, where there is no value to hand out at
  all. This makes `cell.get_or_insert(value)?.stored()` a `get_or_init` that takes a value instead
  of a closure.
- `OnceCell::get_or_try_init` returns `Result<&T, InitError<E>>` instead of
  `Result<Result<&T, E>, ConcurrentInitialization>`. `InitError` keeps the two failure reasons
  apart while composing with the `?` operator.
- The crate was migrated to the 2024 edition, which raises the MSRV to 1.85. Note that the
  previously declared 1.65 was never sufficient to build this crate — 1.81 stabilized the
  `core::error::Error` trait it relies on — so the effective MSRV bump is smaller than it looks. A
  CI job now verifies it.

### Added

- `OnceCell::state`, returning a `CellState` snapshot that distinguishes an empty cell from one
  that another caller is currently initializing. It is meant for reporting — logging, diagnostics,
  health checks — rather than for control flow, since only `get_or_init`, `get_or_insert`, and
  `set` resolve the race atomically.
- `OnceCell::is_initialized`, to check the cell state without borrowing the value.

### Changed

- The `Debug` implementation prints `OnceCell(Initializing)` for a cell that is currently being
  initialized, instead of reporting it as `OnceCell(Uninit)`.

### Documented

- Reentrant initialization is a guarantee rather than an unspecified case: calling back into the
  same cell from an init function returns a `ConcurrentInitialization` error.
- `Clone` and `PartialEq` treat a cell that is currently being initialized as empty. Both are
  therefore racy when used on a cell another caller is writing to, and `PartialEq` can report a
  cell as unequal to the very value it is about to hold.

### Internal

- The state machine is now verified with [`loom`](https://docs.rs/loom), which explores thread
  interleavings exhaustively rather than sampling them, and checked under Miri. Both run in CI.

## 0.1.1

- Remove unused dependencies

## 0.1.0

- Forked from [`once_cell v1.21.3`](https://docs.rs/once_cell/1.21.3/once_cell/)
