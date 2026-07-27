//! Exhaustive interleaving checks of the `OnceCell` state machine.
//!
//! Run with `RUSTFLAGS="--cfg loom" cargo test --test loom_model`.
#![cfg(loom)]

use loom::sync::Arc;
use loom::thread;
use once_cell_no_std::{CellState, Insertion, OnceCell};

/// Two callers race to initialize. Whatever the schedule: at most one init function runs, the
/// cell ends up holding exactly one of the two values, and nobody observes a torn or absent value
/// after a successful init.
#[test]
fn concurrent_get_or_init_runs_one_init_function() {
    loom::model(|| {
        let cell = Arc::new(OnceCell::new());
        let ran = Arc::new(loom::sync::atomic::AtomicUsize::new(0));

        let handles: Vec<_> = [1u32, 2u32]
            .into_iter()
            .map(|v| {
                let (cell, ran) = (cell.clone(), ran.clone());
                thread::spawn(move || {
                    let outcome = cell.get_or_init(|| {
                        ran.fetch_add(1, loom::sync::atomic::Ordering::SeqCst);
                        v
                    });
                    if let Ok(stored) = outcome {
                        assert!(*stored == 1 || *stored == 2, "observed a value nobody stored");
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        assert!(ran.load(loom::sync::atomic::Ordering::SeqCst) <= 1, "more than one init ran");
        let stored = cell.get().expect("one thread must have succeeded");
        assert!(*stored == 1 || *stored == 2);
    });
}

/// A writer and a reader. The reader must never see a value that is not fully published: reading
/// `Some` has to imply the writer's init function completed (the Release/Acquire pair).
#[test]
fn a_reader_never_observes_an_unpublished_value() {
    loom::model(|| {
        let cell = Arc::new(OnceCell::new());

        let writer = {
            let cell = cell.clone();
            thread::spawn(move || {
                let _ = cell.set(vec![1u8, 2, 3]);
            })
        };
        let reader = {
            let cell = cell.clone();
            thread::spawn(move || {
                if let Some(v) = cell.get() {
                    // if the value is visible at all, it must be visible in full
                    assert_eq!(v, &[1, 2, 3]);
                }
                // `state` must agree with `get`
                match cell.state() {
                    CellState::Initialized => assert!(cell.get().is_some()),
                    _ => {}
                }
            })
        };
        writer.join().unwrap();
        reader.join().unwrap();

        assert_eq!(cell.get(), Some(&vec![1, 2, 3]));
    });
}

/// The two `unwrap_unchecked` sites in `get_or_insert`: under every schedule, a caller either
/// inserts its value, is told the cell was already initialized, or is told there is contention —
/// and in the latter two cases its value must come back intact rather than having been consumed.
#[test]
fn get_or_insert_never_loses_or_duplicates_a_value() {
    loom::model(|| {
        let cell = Arc::new(OnceCell::new());

        let handles: Vec<_> = [10u32, 20u32]
            .into_iter()
            .map(|mine| {
                let cell = cell.clone();
                thread::spawn(move || match cell.get_or_insert(mine) {
                    Ok(Insertion::Inserted(stored)) => {
                        assert_eq!(*stored, mine, "inserted a value that was not ours");
                        true
                    }
                    Ok(Insertion::AlreadyInitialized { stored, rejected }) => {
                        assert_eq!(rejected, mine, "the rejected value was not ours");
                        assert_ne!(*stored, mine, "our value was both stored and rejected");
                        false
                    }
                    Err(error) => {
                        assert_eq!(error.into_rejected_value(), mine, "contention ate our value");
                        false
                    }
                })
            })
            .collect();

        let inserts: usize = handles.into_iter().map(|h| h.join().unwrap() as usize).sum();
        assert!(inserts <= 1, "two threads both claimed to have inserted");
        let stored = cell.get().expect("one thread must have inserted");
        assert!(*stored == 10 || *stored == 20);
    });
}

/// A failing init must leave the cell reusable: another caller has to be able to initialize it
/// afterwards, under every interleaving.
#[test]
fn a_failed_init_leaves_the_cell_usable() {
    loom::model(|| {
        let cell = Arc::new(OnceCell::new());

        let failer = {
            let cell = cell.clone();
            thread::spawn(move || {
                let _ = cell.get_or_try_init(|| Err::<u32, ()>(()));
            })
        };
        let setter = {
            let cell = cell.clone();
            thread::spawn(move || cell.set(7u32).is_ok())
        };

        failer.join().unwrap();
        let set_succeeded = setter.join().unwrap();

        // the failing init must never be the one that publishes a value
        if set_succeeded {
            assert_eq!(cell.get(), Some(&7));
        } else {
            // the setter lost to the failing init being in progress; the cell must be empty and
            // still initializable
            assert_eq!(cell.get(), None);
            assert_eq!(cell.state(), CellState::Uninitialized);
            assert!(cell.set(9u32).is_ok(), "the cell was left permanently unusable");
        }
    });
}
