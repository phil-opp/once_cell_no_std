# Changelog

## Unreleased

- **Breaking:** `OnceCell::set` now returns `Result<(), SetError<T>>` and `OnceCell::try_insert`
  now returns `Result<&T, InsertError<'_, T>>`. Both error types hand the value that was not
  written back to the caller. Previously the value was silently dropped when the call failed with
  a `ConcurrentInitialization` error, which made it impossible to retry.
- **Breaking:** `OnceCell::try_insert` was renamed to `OnceCell::insert`. The `try_` prefix no
  longer carried any information, since the method is fallible in exactly the same cases as
  `set`.
- **Breaking:** `OnceCell::get_or_try_init` now returns `Result<&T, InitError<E>>` instead of the
  nested `Result<Result<&T, E>, ConcurrentInitialization>`. The new `error::InitError` type keeps
  the two failure reasons apart, but composes with the `?` operator.
- **Breaking:** The declared MSRV is now 1.81, which is the version that stabilized
  `core::error::Error`. The previously declared 1.65 was never sufficient to build this crate; a
  CI job now verifies the MSRV.
- Add `OnceCell::try_get`, which distinguishes an empty cell from a concurrently initializing one
  through the new `error::GetError` type
- Add `OnceCell::is_initialized` to check the cell state without borrowing the value
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
