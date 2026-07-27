fn main() {
    let cell = once_cell_no_std::OnceCell::<u32>::new();
    cell.get_or_init(|| {
        cell.get_or_init(|| 1).unwrap(); // this unwrap will panic because of `ConcurrentInitialization`
        2
    })
    .unwrap();
}
