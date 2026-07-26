# Changelog

## Unreleased

- Add `OnceCell::try_get`, which distinguishes an empty cell from a concurrently initializing one
  through the new `error::GetError` type

## 0.1.1

- Remove unused dependencies

## 0.1.0

- Forked from [`once_cell v1.21.3`](https://docs.rs/once_cell/1.21.3/once_cell/)
