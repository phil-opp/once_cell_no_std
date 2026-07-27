//! Instantiates the public API with a concrete type, so that `ci/no-panic.sh` can look for
//! panicking paths in the compiled library.
//!
//! Everything on `OnceCell<T>` is generic, so an ordinary build emits no code for it — only MIR
//! for a later monomorphization. This module forces one concrete instantiation into the rlib. It
//! is compiled only under `--cfg no_panic_check` and is not part of the public API.
//!
//! A method that is not called below contributes no code, so the check is only as good as the
//! coverage here.

use crate::{CellState, Insertion, OnceCell};

static CELL: OnceCell<u64> = OnceCell::new();

/// Exercises every method that takes `&self`.
#[unsafe(no_mangle)]
pub extern "C" fn __no_panic_check_shared(value: u64) -> u64 {
    let mut total = CELL.is_initialized() as u64;

    total += match CELL.state() {
        CellState::Uninitialized => 1,
        CellState::Initializing => 2,
        CellState::Initialized => 3,
    };
    if let Some(stored) = CELL.get() {
        total += *stored;
    }

    match CELL.get_or_insert(value) {
        Ok(insertion) => {
            total += *insertion.stored() + insertion.was_inserted() as u64;
            total += insertion.into_rejected_value().unwrap_or(0);
        }
        Err(error) => total += error.into_rejected_value(),
    }
    match CELL.get_or_insert(value) {
        Ok(Insertion::Inserted(stored)) => total += *stored,
        Ok(Insertion::AlreadyInitialized { stored, rejected }) => total += *stored + rejected,
        Err(error) => total += error.into_rejected_value(),
    }

    match CELL.get_or_init(|| value) {
        Ok(stored) => total += *stored,
        Err(_) => total += 1,
    }
    match CELL.get_or_try_init(|| if value > 0 { Ok(value) } else { Err(()) }) {
        Ok(stored) => total += *stored,
        Err(error) => total += error.init_function_error().is_some() as u64,
    }
    if let Err(error) = CELL.set(value) {
        total += error.into_rejected_value();
    }

    total
}

/// Exercises every method that takes `self` or `&mut self`.
#[unsafe(no_mangle)]
pub extern "C" fn __no_panic_check_owned(value: u64) -> u64 {
    let mut cell = OnceCell::with_value(value);
    let mut total = 0;

    if let Some(stored) = cell.get_mut() {
        total += *stored;
    }
    if let Some(taken) = cell.take() {
        total += taken;
    }

    let other = OnceCell::from(value);
    let clone = other.clone();
    total += (clone == other) as u64;
    total += OnceCell::<u64>::default().into_inner().unwrap_or(0);
    total += other.into_inner().unwrap_or(0);
    // SAFETY: `clone` was cloned from an initialized cell.
    total += unsafe { *clone.get_unchecked() };

    total
}
