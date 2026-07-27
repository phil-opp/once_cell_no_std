# `once_cell_no_std`

The `once_cell_no_std` crate provides a `no_std` `OnceCell` type that implements `Sync` and **can be used in
statics**. It does _not_ use spinlocks or any other form of blocking. Instead, concurrent
initialization is reported as an explicit `ConcurrentInitialization` error that the caller can
handle as it likes.

The only thing this crate requires from the target is atomic compare-and-swap on `u8`. This covers
most `no_std` targets, but not all of them: CAS-less targets such as `thumbv6m-none-eabi`
(Cortex-M0/M0+), AVR, or RISC-V without the `A` extension are _not_ supported and fail to compile.

`OnceCell` might store arbitrary non-`Copy` types, can be assigned to at most once and provide direct access
to the stored contents. In a nutshell, the API looks like this:

```rust
use once_cell_no_std::OnceCell;

// `new` is a `const fn`, so a cell can live in a `static`
static CELL: OnceCell<u32> = OnceCell::new();
assert_eq!(CELL.get(), None);

// `set` takes `&self`, so it works on a non-mutable `static`
CELL.set(92).unwrap();
assert_eq!(CELL.get(), Some(&92));

// the cell keeps its first value, later updates are rejected. The value that was not written is
// handed back to the caller instead of being dropped.
assert_eq!(CELL.set(1).unwrap_err().into_rejected_value(), 1);
```

## Initializing on first use

`get_or_insert` combines the two steps above: it writes the value if the cell is still empty, and
either way hands out a reference to whatever ends up stored. `get_or_init` does the same for a value
that has to be computed by a closure.

```rust
use once_cell_no_std::OnceCell;

static CELL: OnceCell<u32> = OnceCell::new();

// `get_or_insert` returns an `Err` on concurrent initialization by another caller. This crate
// never blocks/spins, so you have to decide yourself how to handle this (e.g. panic or retry in
// a loop).
let stored = match CELL.get_or_insert(92) {
    Ok(insertion) => insertion.stored(),
    Err(_) => panic!("concurrent initialization"),
};
assert_eq!(stored, &92);

// the cell still keeps its first value, so a later one is handed back
let insertion = CELL.get_or_insert(1).unwrap();
assert_eq!(insertion.into_rejected_value(), Some(1));
assert_eq!(insertion.stored(), &92); // still the previous value

// the closure only runs if the cell is still empty
assert_eq!(CELL.get_or_init(|| unreachable!()), Ok(&92));
```

## Waiting for a concurrent initialization

Since this crate never blocks, waiting is something the caller opts into rather than something that
happens by default. Every method that can run into a concurrent initialization reports it as an
explicit error, so a caller that wants to wait can simply retry:

```rust
use once_cell_no_std::{OnceCell, error::ConcurrentInitialization};

/// Returns the value of the cell, initializing it with `init` if it is still empty.
///
/// Spins until the value is available if another caller is initializing the cell.
fn get_or_spin<T>(cell: &OnceCell<T>, mut init: impl FnMut() -> T) -> &T {
    loop {
        // `&mut init` is passed instead of `init` so that it survives for the next attempt
        match cell.get_or_init(&mut init) {
            Ok(value) => return value,
            // another caller is initializing the cell: retry in a busy loop
            Err(ConcurrentInitialization) => core::hint::spin_loop(),
        }
    }
}

let cell = OnceCell::new();
assert_eq!(get_or_spin(&cell, || "Hello, World!"), &"Hello, World!");
// the init function is not called for an already initialized cell
assert_eq!(get_or_spin(&cell, || unreachable!()), &"Hello, World!");
```

This is the lazy initialization that [`spin::Once`] and the [`lazy_static`] crate (with its
`spin_no_std` feature) provide for `no_std` environments. The advantage of building it yourself is
that the strategy stays yours, and spinning is not always the right one.

A spinning caller is hard to tell apart from one doing useful work, so a scheduler may keep running
it in place of the very caller it is waiting for. [Spinlocks Considered Harmful][spinharm] makes the
case in full. Waiting for an interrupt, yielding to the scheduler, or reporting the error upwards
may suit your system better. This crate enables you to switch to one of those (later) without
changing your cell type.

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

[spinharm]: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html
[`spin::Once`]: https://docs.rs/spin/latest/spin/once/struct.Once.html
[`lazy_static`]: https://docs.rs/lazy_static/latest/lazy_static/
[unsync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/unsync/struct.OnceCell.html
[sync-once-cell]: https://docs.rs/once_cell/1.21.3/once_cell/sync/struct.OnceCell.html
[`core::cell::OnceCell`]: https://doc.rust-lang.org/stable/core/cell/struct.OnceCell.html
[`std::sync::OnceLock`]: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html
