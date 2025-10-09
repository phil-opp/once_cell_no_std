# `once_cell_no_std`

The `once_cell_no_std` crate provides a `no_std` `OnceCell` type that implements `Sync` and can be used in
statics. It does _not_ use spinlocks or any other form of blocking. Instead, concurrent
initialization is reported as an explicit `ConcurrentInitialization` error that the caller can
handle as it likes.

`OnceCell` might store arbitrary non-`Copy` types, can be assigned to at most once and provide direct access
to the stored contents. In a nutshell, API looks *roughly* like this:

```rust
impl OnceCell<T> {
    fn new() -> OnceCell<T> { ... }
    fn set(&self, value: T) -> Result<Result<(), T>, ConcurrentInitialization> { ... }
    fn get(&self) -> Option<&T> { ... }
}
```

Note that, like with `RefCell` and `Mutex`, the `set` method requires only a shared reference.
Because of the single assignment restriction `get` can return an `&T` instead of `Ref<T>`
or `MutexGuard<T>`.

More patterns and use-cases are in the [docs](https://docs.rs/once_cell_no_std/)!

## Related crates

This crate was forked from the great
[`once_cell` crate](https://docs.rs/once_cell/1.21.3/once_cell/). The original `once_cell` crate
provides two flavors of `OnceCell` types: [`unsync::OnceCell`][unsync-once-cell] and
[`sync::OnceCell`][sync-once-cell]. The following
table compares the types against `once_cell_no_std::OnceCell`:

|                                    | `OnceCell` (this crate)                   | [`once_cell::sync::OnceCell`][sync-once-cell]                                                                       | [`once_cell::unsync::OnceCell`][unsync-once-cell] |
| ---------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| implements `Sync`                  | yes                                       | yes                                                                                                                 | no                                                |
| concurrent initialization leads to | `ConcurrentInitialization` error returned | thread blocked                                                                                                      | cannot happen                                     |
| `no_std` supported                 | yes                                       | partially (requires [`critical-section`](https://docs.rs/critical-section/latest/critical_section/) implementation) | yes                                               |

Parts of `once_cell` API are included into `std`/`core` [as of Rust 1.70.0](https://github.com/rust-lang/rust/pull/105587).
The following table compares `once_cell_no_std::OnceCell` against the [`core::cell::OnceCell`] and [`std::sync::OnceLock`] types:

|                                    | `OnceCell` (this crate)                   | [`std::sync::OnceLock`] | [`core::cell::OnceCell`] |
| ---------------------------------- | ----------------------------------------- | ----------------------- | ------------------------ |
| implements `Sync`                  | yes                                       | yes                     | no                       |
| concurrent initialization leads to | `ConcurrentInitialization` error returned | thread blocked          | cannot happen            |
| `no_std` supported                 | yes                                       | no                      | yes                      |

For more related crates, check out the [README of `once_cell`](https://github.com/matklad/once_cell?tab=readme-ov-file#related-crates).

[unsync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/unsync/struct.OnceCell.html
[sync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/sync/struct.OnceCell.html
[`core::cell::OnceCell`]: https://doc.rust-lang.org/stable/core/cell/struct.OnceCell.html
[`std::sync::OnceLock`]: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html
