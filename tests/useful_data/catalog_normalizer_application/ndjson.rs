//! Unchanged old framing is an independent whole-application control.
use super::{assert_batch_backend_matrix, copy_fixture, fixture, ScratchRoot};
#[test]
fn old_framing_and_new_structured_receiver_preserve_the_complete_normalizer_oracle() {
    let scratch = ScratchRoot::new();
    copy_fixture(&fixture(), scratch.path());
    std::fs::write(
        scratch.path().join("src/batch.spx"),
        include_str!("../catalog_normalizer_batch_original.txt"),
    )
    .unwrap();
    let manifest = std::fs::read_to_string(scratch.path().join("semaprax.toml"))
        .unwrap()
        .replace("std.data.ndjson = \"=0.1.0\"\n", "");
    std::fs::write(scratch.path().join("semaprax.toml"), manifest).unwrap();
    super::append_legacy_summary_adapter(scratch.path());
    // The same frozen published36/source14/max160M and all backend/fault observers run unchanged.
    assert_batch_backend_matrix(scratch.path());
    assert_batch_backend_matrix(&fixture());
}
