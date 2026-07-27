# Changelog

## Unreleased

- **Breaking:** `OnceCell::set` now returns `Result<(), SetError<T>>`, whose error hands the value
  that was not written back to the caller. Previously the value was silently dropped when the call
  failed with a `ConcurrentInitialization` error, which made it impossible to retry.
- **Breaking:** `OnceCell::try_insert` was renamed to `OnceCell::get_or_insert` and now returns
  `Result<Insertion<'_, T>, InsertError<T>>`. An already initialized cell is no longer reported as
  an error, because the caller still ends up with a reference to a stored value; it is the
  `Insertion::AlreadyInitialized` outcome instead, which hands the rejected value back as well. The
  only remaining failure is a concurrent initialization, where there is no value to hand out at
  all. This makes `cell.get_or_insert(value)?.stored()` a `get_or_init` that takes a value instead
  of a closure.
- **Breaking:** `OnceCell::get_or_try_init` now returns `Result<&T, InitError<E>>` instead of the
  nested `Result<Result<&T, E>, ConcurrentInitialization>`. The new `error::InitError` type keeps
  the two failure reasons apart, but composes with the `?` operator.
- **Breaking:** The declared MSRV is now 1.81, which is the version that stabilized
  `core::error::Error`. The previously declared 1.65 was never sufficient to build this crate; a
  CI job now verifies the MSRV.
- Add `OnceCell::is_initialized` to check the cell state without borrowing the value
- Add `OnceCell::state`, which returns a `CellState` snapshot that distinguishes an empty cell from
  one that another caller is currently initializing. This is meant for reporting (logging,
  diagnostics, health checks); `get_or_init`, `set`, and `insert` remain the way to act on the
  difference, since only they resolve the race atomically.
- The `Debug` implementation of `OnceCell` now prints `OnceCell(Initializing)` for a cell that is
  currently being initialized, instead of reporting it as `OnceCell(Uninit)`
- Reentrant initialization is now a documented guarantee: calling back into the same cell from an
  init function returns a `ConcurrentInitialization` error. The docs inherited from `once_cell`
  still declared this case unspecified and claimed the implementation deadlocks, which was never
  true for this crate since it never blocks.

## 0.1.1

- Remove unused dependencies

## 0.1.0

- Forked from [`once_cell v1.21.3`](https://docs.rs/once_cell/1.21.3/once_cell/)
