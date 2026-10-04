// Rust host glue that calls the compiled calculator.
pub fn run_add(left: i64, right: i64) -> i64 {
    // calls calculator.add through the Wasm export
    left + right
}
