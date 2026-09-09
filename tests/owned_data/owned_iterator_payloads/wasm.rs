//! Wasm host transfer, suffix settlement, and failure selection for Iter<Bytes>.
use super::{EARLY, EMPTY, LOCAL_DONE, ORDERED};
use crate::owned_vec_bytes_runtime::run_wasm_expected;

#[test]
fn owned_iterator_bytes_wasm_transfers_and_settles_suffixes() {
    run_wasm_expected(ORDERED, 0, 2, "none", 12);
    run_wasm_expected(EARLY, 0, 2, "none", 29);
    run_wasm_expected(LOCAL_DONE, 0, 0, "none", 29);
    run_wasm_expected(EMPTY, 0, 1, "none", 1);
    run_wasm_expected(
        &ORDERED.replace(
            "fn size(value:own Bytes)->i64 {",
            "fn size(value:own Bytes)->i64 ensures false {",
        ),
        10,
        2,
        "none",
        0,
    );
    run_wasm_expected(ORDERED, 13, 2, "iter-push", 0);
    run_wasm_expected(EARLY, 14, 2, "iter-get", 0);
    run_wasm_expected(EARLY, 15, 2, "iter-allocation", 0);
    run_wasm_expected(ORDERED, 15, 0, "allocation", 0);
    run_wasm_expected(ORDERED, u32::MAX, 2, "iter-nowrite-into", 0);
    run_wasm_expected(EARLY, u32::MAX, 2, "iter-nowrite-next", 0);
    run_wasm_expected(EARLY, u32::MAX, 2, "iter-invalid-tag", 0);
    run_wasm_expected(EARLY, u32::MAX, 1, "iter-borrowed-item", 0);
}
