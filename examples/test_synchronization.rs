//! Test if the OnceCell properly synchronizes.
//! Needs to be run in release mode.
//!
//! We create a `Vec` with `N_ROUNDS` of `OnceCell`s. All threads will walk the `Vec`, and race to
//! be the first one to initialize a cell.
//! Every thread adds the results of the cells it sees to an accumulator, which is compared at the
//! end.
//! All threads should end up with the same result.

use std::hint::spin_loop;

use once_cell_no_std::{OnceCell, error::ConcurrentInitialization};

const N_THREADS: usize = 32;
const N_ROUNDS: usize = 1_000_000;

static CELLS: OnceCell<Vec<OnceCell<usize>>> = OnceCell::new();
static RESULT: OnceCell<usize> = OnceCell::new();

fn main() {
    let start = std::time::Instant::now();
    CELLS.get_or_init(|| vec![OnceCell::new(); N_ROUNDS]).unwrap();
    let threads =
        (0..N_THREADS).map(|i| std::thread::spawn(move || thread_main(i))).collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    println!("{:?}", start.elapsed());
    println!("No races detected");
}

#[allow(clippy::single_match)]
fn thread_main(i: usize) {
    let cells = CELLS.get().unwrap();
    let mut accum = 0;
    for cell in cells.iter() {
        let &value = loop {
            match cell.get_or_init(|| i) {
                Ok(value) => break value,
                Err(ConcurrentInitialization) => {
                    spin_loop(); // retry
                }
            }
        };
        accum += value;
    }
    let result = loop {
        match RESULT.get_or_init(|| accum) {
            Ok(value) => break value,
            Err(ConcurrentInitialization) => {
                spin_loop(); // retry
            }
        }
    };
    assert_eq!(result, &accum);
}
