use std::{
    sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    thread::scope,
};

use once_cell_no_std::{
    CellState, Insertion, OnceCell,
    error::{InitError, SetError},
};

#[test]
fn once_cell() {
    let c = OnceCell::new();
    assert!(c.get().is_none());
    scope(|s| {
        s.spawn(|| {
            c.get_or_init(|| 92).unwrap();
            assert_eq!(c.get(), Some(&92));
        });
    });
    c.get_or_init(|| panic!("Kabom!")).unwrap();
    assert_eq!(c.get(), Some(&92));
}

#[test]
fn once_cell_with_value() {
    static CELL: OnceCell<i32> = OnceCell::with_value(12);
    assert_eq!(CELL.get(), Some(&12));
}

#[test]
fn once_cell_get_mut() {
    let mut c = OnceCell::new();
    assert!(c.get_mut().is_none());
    c.set(90).unwrap();
    *c.get_mut().unwrap() += 2;
    assert_eq!(c.get_mut(), Some(&mut 92));
}

#[test]
fn once_cell_get_unchecked() {
    let c = OnceCell::new();
    c.set(92).unwrap();
    unsafe {
        assert_eq!(c.get_unchecked(), &92);
    }
}

#[test]
fn once_cell_drop() {
    static DROP_CNT: AtomicUsize = AtomicUsize::new(0);
    struct Dropper;
    impl Drop for Dropper {
        fn drop(&mut self) {
            DROP_CNT.fetch_add(1, SeqCst);
        }
    }

    let x = OnceCell::new();
    scope(|s| {
        s.spawn(|| {
            x.get_or_init(|| Dropper).unwrap();
            assert_eq!(DROP_CNT.load(SeqCst), 0);
            drop(x);
        });
    });
    assert_eq!(DROP_CNT.load(SeqCst), 1);
}

#[test]
fn once_cell_drop_empty() {
    let x = OnceCell::<String>::new();
    drop(x);
}

#[test]
fn clone() {
    let s = OnceCell::new();
    let c = s.clone();
    assert!(c.get().is_none());

    s.set("hello".to_string()).unwrap();
    let c = s.clone();
    assert_eq!(c.get().map(String::as_str), Some("hello"));
}

#[test]
fn get_or_try_init() {
    let cell: OnceCell<String> = OnceCell::new();
    assert!(cell.get().is_none());

    let res = std::panic::catch_unwind(|| cell.get_or_try_init(|| -> Result<_, ()> { panic!() }));
    assert!(res.is_err());
    assert!(cell.get().is_none());

    assert_eq!(cell.get_or_try_init(|| Err(())), Err(InitError::InitFunctionFailed(())));

    assert_eq!(cell.get_or_try_init(|| Ok::<_, ()>("hello".to_string())), Ok(&"hello".to_string()));
    assert_eq!(cell.get(), Some(&"hello".to_string()));
}

#[test]
fn from_impl() {
    assert_eq!(OnceCell::from("value").get(), Some(&"value"));
    assert_ne!(OnceCell::from("foo").get(), Some(&"bar"));
}

#[test]
fn partialeq_impl() {
    assert!(OnceCell::from("value") == OnceCell::from("value"));
    assert!(OnceCell::from("foo") != OnceCell::from("bar"));

    assert!(OnceCell::<String>::new() == OnceCell::new());
    assert!(OnceCell::<String>::new() != OnceCell::from("value".to_owned()));
}

#[test]
fn into_inner() {
    let cell: OnceCell<String> = OnceCell::new();
    assert_eq!(cell.into_inner(), None);
    let cell = OnceCell::new();
    cell.set("hello".to_string()).unwrap();
    assert_eq!(cell.into_inner(), Some("hello".to_string()));
}

#[test]
fn debug_impl() {
    let cell = OnceCell::new();
    assert_eq!(format!("{:#?}", cell), "OnceCell(Uninit)");
    cell.set(vec!["hello", "world"]).unwrap();
    assert_eq!(
        format!("{:#?}", cell),
        r#"OnceCell(
    [
        "hello",
        "world",
    ],
)"#
    );
}

#[test]
fn debug_impl_while_initializing() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                92
            })
            .unwrap();
        });
        barrier.wait();
        assert_eq!(format!("{:?}", cell), "OnceCell(Initializing)");
        barrier.wait();
    });
    assert_eq!(format!("{:?}", cell), "OnceCell(92)");
}

#[test]
fn init_error_is_reported_without_nesting() {
    let cell: OnceCell<i32> = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                92
            })
            .unwrap();
        });
        barrier.wait();
        // the init function of a concurrent call is not executed at all
        let err = cell.get_or_try_init(|| -> Result<i32, ()> { unreachable!() });
        assert_eq!(err, Err(InitError::ConcurrentInitialization));
        assert_eq!(err.unwrap_err().init_function_error(), None);
        barrier.wait();
    });
    assert_eq!(cell.get(), Some(&92));
}

#[test]
#[should_panic(expected = "concurrent initialization detected: ConcurrentInitialization")]
fn reentrant_init_no_std() {
    use std::cell::Cell;

    let x: OnceCell<Box<i32>> = OnceCell::new();
    let dangling_ref: Cell<Option<&i32>> = Cell::new(None);
    x.get_or_init(|| {
        let r = x.get_or_init(|| Box::new(92)).expect("concurrent initialization detected");
        dangling_ref.set(Some(r));
        Box::new(62)
    })
    .unwrap();
    eprintln!("use after free: {:?}", dangling_ref.get().unwrap());
}

#[test]
fn eval_once_macro() {
    macro_rules! eval_once {
        (|| -> $ty:ty {
            $($body:tt)*
        }) => {{
            static ONCE_CELL: OnceCell<$ty> = OnceCell::new();
            fn init() -> $ty {
                $($body)*
            }
            ONCE_CELL.get_or_init(init).unwrap()
        }};
    }

    let fib: &'static Vec<i32> = eval_once! {
        || -> Vec<i32> {
            let mut res = vec![1, 1];
            for i in 0..10 {
                let next = res[i] + res[i + 1];
                res.push(next);
            }
            res
        }
    };
    assert_eq!(fib[5], 8)
}

#[test]
fn once_cell_does_not_leak_partially_constructed_boxes() {
    let n_tries = if cfg!(miri) { 10 } else { 100 };
    let n_readers = 10;
    let n_writers = 3;
    const MSG: &str = "Hello, World";

    for _ in 0..n_tries {
        let cell: OnceCell<String> = OnceCell::new();
        scope(|scope| {
            for _ in 0..n_readers {
                scope.spawn(|| {
                    loop {
                        if let Some(msg) = cell.get() {
                            assert_eq!(msg, MSG);
                            break;
                        }
                    }
                });
            }
            for _ in 0..n_writers {
                let _ = scope.spawn(|| cell.set(MSG.to_owned()));
            }
        });
    }
}

#[test]
fn get_does_not_block() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                "hello".to_string()
            })
            .unwrap();
        });
        barrier.wait();
        assert_eq!(cell.get(), None);
        barrier.wait();
    });
    assert_eq!(cell.get(), Some(&"hello".to_string()));
}

#[test]
fn get_reports_a_cell_that_is_being_initialized_as_empty() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    assert_eq!(cell.get(), None);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                "hello".to_string()
            })
            .unwrap();
        });
        barrier.wait();
        // an initialization in progress is not distinguished from an empty cell
        assert_eq!(cell.get(), None);
        assert!(!cell.is_initialized());
        barrier.wait();
    });
    assert_eq!(cell.get(), Some(&"hello".to_string()));
    assert!(cell.is_initialized());
}

#[test]
fn get_after_failed_init() {
    let cell: OnceCell<String> = OnceCell::new();
    assert_eq!(cell.get_or_try_init(|| Err(())), Err(InitError::InitFunctionFailed(())));
    assert_eq!(cell.get(), None);
    assert!(!cell.is_initialized());

    let res = std::panic::catch_unwind(|| cell.get_or_try_init(|| -> Result<_, ()> { panic!() }));
    assert!(res.is_err());
    assert_eq!(cell.get(), None);
    assert!(!cell.is_initialized());
}

#[test]
fn state_distinguishes_empty_from_initializing() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    assert_eq!(cell.state(), CellState::Uninitialized);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                "hello".to_string()
            })
            .unwrap();
        });
        barrier.wait();
        assert_eq!(cell.state(), CellState::Initializing);
        barrier.wait();
    });
    assert_eq!(cell.state(), CellState::Initialized);
}

#[test]
fn state_returns_to_uninitialized_after_a_failed_init() {
    let cell: OnceCell<String> = OnceCell::new();
    assert_eq!(cell.get_or_try_init(|| Err(())), Err(InitError::InitFunctionFailed(())));
    // a failed init leaves no trace: the cell is indistinguishable from one never written to
    assert_eq!(cell.state(), CellState::Uninitialized);
}

#[test]
fn set_hands_the_value_back_on_error() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                "hello".to_string()
            })
            .unwrap();
        });
        barrier.wait();
        let err = cell.set("world".to_string()).unwrap_err();
        assert_eq!(err, SetError::ConcurrentInitialization("world".to_string()));
        assert_eq!(err.into_rejected_value(), "world");
        barrier.wait();
    });
    let err = cell.set("world".to_string()).unwrap_err();
    assert_eq!(err, SetError::AlreadyInitialized("world".to_string()));
    assert_eq!(err.into_rejected_value(), "world");
    assert_eq!(cell.get(), Some(&"hello".to_string()));
}

#[test]
fn get_or_insert_hands_the_value_back_when_it_is_not_inserted() {
    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                "hello".to_string()
            })
            .unwrap();
        });
        barrier.wait();
        // a concurrent initialization is the only failure: there is no value to hand out
        let err = cell.get_or_insert("world".to_string()).unwrap_err();
        assert_eq!(err.into_rejected_value(), "world");
        barrier.wait();
    });
    // an already initialized cell is not an error, because a stored value is still available
    let insertion = cell.get_or_insert("world".to_string()).unwrap();
    assert_eq!(
        insertion,
        Insertion::AlreadyInitialized {
            stored: &"hello".to_string(),
            rejected: "world".to_string()
        }
    );
    assert!(!insertion.was_inserted());
    assert_eq!(insertion.stored(), "hello");
    assert_eq!(insertion.into_rejected_value(), Some("world".to_string()));
}

#[test]
fn get_or_insert_reports_an_inserted_value() {
    let cell = OnceCell::new();
    let insertion = cell.get_or_insert("hello".to_string()).unwrap();
    assert_eq!(insertion, Insertion::Inserted(&"hello".to_string()));
    assert!(insertion.was_inserted());
    assert_eq!(insertion.stored(), "hello");
    assert_eq!(insertion.into_rejected_value(), None);
}

/// Drives the contention exit of `get_or_insert` without any threads: a reentrant call sees the
/// cell in the initializing state, which is the one situation where the init closure must not have
/// run. Being single-threaded, this covers that `unwrap_unchecked` deterministically, including
/// under Miri.
#[test]
fn get_or_insert_reports_contention_when_called_reentrantly() {
    let cell: OnceCell<String> = OnceCell::new();
    let reentrant = cell
        .get_or_init(|| {
            let error = cell
                .get_or_insert("inner".to_string())
                .expect_err("a reentrant insert must report contention");
            // the closure never ran, so the value comes back intact
            assert_eq!(error.into_rejected_value(), "inner");
            "outer".to_string()
        })
        .unwrap();
    assert_eq!(reentrant, "outer");
    assert_eq!(cell.get(), Some(&"outer".to_string()));
}

/// `get_or_insert` takes values out of an `Option` with `unwrap_unchecked` on both of its exits,
/// relying on the init closure running exactly when the cell was empty and never when the call
/// loses the race. This hammers all three outcomes concurrently, so that a broken invariant shows
/// up as a `debug_assert` failure here (and as undefined behavior under Miri) rather than silently.
#[test]
fn get_or_insert_upholds_its_init_closure_invariants_under_contention() {
    let n_tries = if cfg!(miri) { 3 } else { 100 };
    let n_rivals = 7;

    for round in 0..n_tries {
        let cell: OnceCell<String> = OnceCell::new();
        let winner = format!("winner-{round}");
        // the rivals may only attempt while the winner is inside its init closure, and the winner
        // may only leave it once every rival has attempted
        let init_entered = Barrier::new(n_rivals + 1);
        let rivals_done = Barrier::new(n_rivals + 1);
        // the winner leaving its closure is not the same as the cell being initialized: the state
        // only becomes `Initialized` once `get_or_init` has returned
        let winner_done = Barrier::new(n_rivals + 1);
        let contended = AtomicUsize::new(0);
        let already = AtomicUsize::new(0);

        scope(|scope| {
            let (winner, cell) = (&winner, &cell);
            let (init_entered, rivals_done) = (&init_entered, &rivals_done);
            let winner_done = &winner_done;
            scope.spawn(move || {
                let inserted = cell
                    .get_or_init(|| {
                        init_entered.wait();
                        rivals_done.wait();
                        winner.clone()
                    })
                    .unwrap();
                assert_eq!(inserted, winner);
                winner_done.wait();
            });

            for rival in 0..n_rivals {
                let (contended, already) = (&contended, &already);
                scope.spawn(move || {
                    let mine = format!("rival-{round}-{rival}");
                    init_entered.wait();
                    // the cell is guaranteed to be mid-initialization here
                    match cell.get_or_insert(mine.clone()) {
                        Err(error) => {
                            // the closure never ran, so the value comes back intact
                            assert_eq!(error.into_rejected_value(), mine);
                            contended.fetch_add(1, SeqCst);
                        }
                        other => panic!("expected a contention error, got {other:?}"),
                    }
                    rivals_done.wait();
                    winner_done.wait();

                    // and again once the winner is done, which must now take the other exit
                    match cell.get_or_insert(mine.clone()) {
                        Ok(Insertion::AlreadyInitialized { stored, rejected }) => {
                            assert_eq!(rejected, mine);
                            assert_eq!(stored, winner);
                            already.fetch_add(1, SeqCst);
                        }
                        other => panic!("expected an already-initialized insertion, got {other:?}"),
                    }
                });
            }
        });

        // every rival must have driven both `unwrap_unchecked` sites exactly once
        assert_eq!(contended.load(SeqCst), n_rivals);
        assert_eq!(already.load(SeqCst), n_rivals);
        assert_eq!(cell.get(), Some(&winner));
    }
}

#[test]
fn get_or_insert_works_like_get_or_init_without_a_closure() {
    let cell = OnceCell::new();
    assert_eq!(cell.get_or_insert(92).unwrap().stored(), &92);
    // the cell keeps the first value, and later callers still get a reference to it
    assert_eq!(cell.get_or_insert(62).unwrap().stored(), &92);
    assert_eq!(cell.get(), Some(&92));
}

#[test]
fn concurrent_set_does_not_drop_the_value() {
    static DROP_CNT: AtomicUsize = AtomicUsize::new(0);
    struct Dropper;
    impl Drop for Dropper {
        fn drop(&mut self) {
            DROP_CNT.fetch_add(1, SeqCst);
        }
    }

    let cell = OnceCell::new();
    let barrier = Barrier::new(2);
    scope(|scope| {
        scope.spawn(|| {
            cell.get_or_init(|| {
                barrier.wait();
                barrier.wait();
                Dropper
            })
            .unwrap();
        });
        barrier.wait();
        let value = cell.set(Dropper).unwrap_err().into_rejected_value();
        assert_eq!(DROP_CNT.load(SeqCst), 0);
        drop(value);
        assert_eq!(DROP_CNT.load(SeqCst), 1);
        barrier.wait();
    });
}

#[test]
// See:
// https://github.com/rust-lang/rust/issues/34761#issuecomment-256320669
// https://github.com/matklad/once_cell/pull/72
// https://forge.rust-lang.org/libs/maintaining-std.html#is-there-a-manual-drop-implementation
fn arrrrrrrrrrrrrrrrrrrrrr() {
    let cell = OnceCell::new();
    {
        let s = String::new();
        cell.set(&s).unwrap();
    }
}

#[test]
fn once_cell_is_sync_send() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<OnceCell<String>>();
}
