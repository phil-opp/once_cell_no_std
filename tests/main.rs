use std::{
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Barrier,
    },
    thread::scope,
};

use once_cell_no_std::OnceCell;

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
    c.set(90).unwrap().unwrap();
    *c.get_mut().unwrap() += 2;
    assert_eq!(c.get_mut(), Some(&mut 92));
}

#[test]
fn once_cell_get_unchecked() {
    let c = OnceCell::new();
    c.set(92).unwrap().unwrap();
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

    s.set("hello".to_string()).unwrap().unwrap();
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

    assert_eq!(cell.get_or_try_init(|| Err(())).unwrap(), Err(()));

    assert_eq!(
        cell.get_or_try_init(|| Ok::<_, ()>("hello".to_string())).unwrap(),
        Ok(&"hello".to_string())
    );
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
    cell.set("hello".to_string()).unwrap().unwrap();
    assert_eq!(cell.into_inner(), Some("hello".to_string()));
}

#[test]
fn debug_impl() {
    let cell = OnceCell::new();
    assert_eq!(format!("{:#?}", cell), "OnceCell(Uninit)");
    cell.set(vec!["hello", "world"]).unwrap().unwrap();
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
                scope.spawn(|| loop {
                    if let Some(msg) = cell.get() {
                        assert_eq!(msg, MSG);
                        break;
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
// https://github.com/rust-lang/rust/issues/34761#issuecomment-256320669
fn arrrrrrrrrrrrrrrrrrrrrr() {
    let cell = OnceCell::new();
    {
        let s = String::new();
        cell.set(&s).unwrap().unwrap();
    }
}

#[test]
fn once_cell_is_sync_send() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<OnceCell<String>>();
}
