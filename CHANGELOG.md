# Changelog

## Unreleased

- **Breaking:** `OnceCell::set` now returns `Result<(), SetError<T>>` and `OnceCell::try_insert`
  now returns `Result<&T, InsertError<'_, T>>`. Both error types hand the value that was not
  written back to the caller. Previously the value was silently dropped when the call failed with
  a `ConcurrentInitialization` error, which made it impossible to retry.
- Add `OnceCell::try_get`, which distinguishes an empty cell from a concurrently initializing one
  through the new `error::GetError` type

## 0.1.1

- Remove unused dependencies

## 0.1.0

- Forked from [`once_cell v1.21.3`](https://docs.rs/once_cell/1.21.3/once_cell/)
