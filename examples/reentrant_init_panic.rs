fn main() {
    let cell = once_cell_no_std::OnceCell::<u32>::new();
    cell.get_or_init(|| {
        cell.get_or_init(|| 1).unwrap(); // this unwrap will panic because of `ConcurrentInitialization`
        2
    })
    .unwrap();
}

/// Dummy test to make it seem hang when compiled as `--test`
/// See https://github.com/matklad/once_cell/issues/79
#[test]
fn dummy_test() {
    std::thread::sleep(std::time::Duration::from_secs(4));
}
