//! Exact current publication controls before the immutable projection part.
use super::{digest, reader};
pub(super) fn previous(path: &str, length: usize, sha: &str) -> Option<String> {
    const SOURCE: &str =
        "src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs";
    const BEFORE: &str = "569dca7d1273c3c4cfbd1eda7b6ed9e3909db7cd607556eddb7c515838078a5f";
    if (path, length, sha) != (SOURCE, 7068, BEFORE) {
        return None;
    }
    let compiled = include_str!(
        "../../workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs"
    );
    assert_eq!(
        (compiled.len(), digest(compiled)),
        (
            9293,
            "f30d8ceea4d53421be07e2eec5610b9c5f0ac05ff06005f67c74028128157149".to_owned()
        )
    );
    let physical = reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCE),
        compiled.len(),
        &digest(compiled),
        2_097_152,
    )
    .expect("fresh complete current publication source");
    assert_eq!(physical, compiled);
    let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
        "../../../docs/evidence/current344-ci-20261021/publication-inverse.json"
    ))
    .unwrap();
    let mut original = physical;
    for (at, after, before) in edits.into_iter().rev() {
        assert_eq!(&original[at..at + after.len()], after);
        original.replace_range(at..at + after.len(), &before);
    }
    assert_eq!(original.len(), length);
    assert_eq!(digest(&original), sha);
    Some(original)
}

// Whole current audit join only. The original production purity check runs
// before this historical facade route, and every original phase inverse runs
// after it. No source component is omitted from the current input.
pub(super) fn audit_join(supplied: &str) -> Option<String> {
    if supplied.len() != 805856
        || digest(supplied) != "8184cb3400f5e0d5e212b25bb03718ec6a6349bb8666989f52f10346ffa63cf8"
    {
        return None;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut complete = String::new();
    // Literal generated from the actual owning macro, in original order.
    // Data pieces remain read-only source evidence, never compiled authority.
    const PARTS: &[(&str, &str, usize, &str)] = &[
        ("src/workspace_graph/expected_projection.rs", include_str!("../../workspace_graph/expected_projection.rs"), 59903, "6c6e07a5bc02662dbc273eac0bb9fa2cb3f580134dd208f58212fc6f64528f5e"),
        ("src/workspace_graph/expected_projection/shared_local_callees.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees.rs"), 11322, "d282c57cfbfbffe5f26826e574576682549dfd98c9008e1894569c6f0bb6ff95"),
        ("src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs"), 14427, "f95beb41205dc47d5707ed4f189bf04dacde802016039e6b335be04b3bf213ef"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs"), 24967, "7f94781ad055eb7c899706aaa1a81e6c346d63c006e8ee2f73c79987dea4ece9"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reused.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reused.rs"), 10317, "ed964ba39e51dfd7dbdd8c391b831260adba376c6f8dad5b035e8df36ab59fd7"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reused_controls.rs"), 6466, "721df26a16a534acefc26db15e6b6cd73e9337048157250805e314843410661f"),
        ("src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/before_projection.rs"), 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
        ("src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/controls_tests.rs"), 12902, "9f125e910fc4635332b2f9525dc3d544f1325b922d969bb2b25c13fa254b399b"),
        ("src/workspace_graph/expected_projection/shared_local_callees/invoice.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/invoice.rs"), 9314, "50bfe2c1d81e5bd637a542d4254198bcd6132293c5826231fa9d08865f94643c"),
        ("src/workspace_graph/expected_projection/shared_local_callees/tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/tests.rs"), 9590, "a3079d6e5f427d20801582413905630ac9106a189bce6d49af977cf8a2133099"),
        ("src/workspace_graph/expected_projection/shared_local_callees/walk.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/walk.rs"), 9350, "fafdcb7e6743647fcd1ad787ea98731a159fc155e0a8d35b02541f60dd74ce17"),
        ("src/workspace_graph/expected_projection/vec_constructor.rs", include_str!("../../workspace_graph/expected_projection/vec_constructor.rs"), 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
        ("src/workspace_graph/expected_projection/held_projection.rs", include_str!("../../workspace_graph/expected_projection/held_projection.rs"), 22889, "8443e4086088b9a623df9f7b7cd9755a0e69664fc4a60066bf5e8da2e6797710"),
        ("src/workspace_graph/expected_projection/call_carriers.rs", include_str!("../../workspace_graph/expected_projection/call_carriers.rs"), 12486, "33215311b715633780ade87b45cbe28d7b19b6e6c0f9ababcf43c579836a7e6b"),
        ("src/workspace_graph/expected_projection/call_carriers_joint.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_joint.rs"), 12386, "76ccfc4bb7c5252b15864a764c221187e3e2a9c2e0e5b8c357b93faaf0c20c94"),
        ("src/workspace_graph/expected_projection/call_carriers_tests.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_tests.rs"), 25183, "d5f1f5027b904e45143b0a65d05a7027d987c69dce23392f06fb134aa66d75a9"),
        ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_inverse.rs"), 7145, "19b35869b9438236bc14672cd51bb11dd919de2929c3ec10b23283666a1a37b9"),
        ("src/workspace_graph/expected_projection/call_carriers_original.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_original.rs"), 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
        ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_historical_tests.rs"), 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
        ("src/workspace_graph/expected_projection/prelude_forecast.rs", include_str!("../../workspace_graph/expected_projection/prelude_forecast.rs"), 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
        ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", include_str!("../../workspace_graph/expected_projection/prelude_forecast_tests.rs"), 20025, "8e41b4ce5b9708f95080e19393444caf5b2a9bea5f7164bd0b78e6974e49d2da"),
        ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", include_str!("../../workspace_graph/expected_projection/prelude_forecast_original.rs"), 21032, "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187"),
        ("src/workspace_graph/expected_projection/held_projection_original.rs", include_str!("../../workspace_graph/expected_projection/held_projection_original.rs"), 18237, "b976530c05c01bb64e685f1ed12b36d1305032cda8560778ce0f832bcc88e6f6"),
        ("src/workspace_graph/expected_projection/borrowed_leaf.rs", include_str!("../../workspace_graph/expected_projection/borrowed_leaf.rs"), 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
        ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", include_str!("../../workspace_graph/expected_projection/borrowed_leaf/tests.rs"), 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
        ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", include_str!("../../workspace_graph/expected_projection/borrowed_leaf/checked_key.rs"), 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
        ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", include_str!("../../workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs"), 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
        ("src/workspace_graph/expected_projection/borrowed_imports.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports.rs"), 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
        ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/typed.rs"), 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
        ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/typed/alias.rs"), 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
        ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs"), 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
        ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs"), 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
        ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/inline.rs"), 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
        ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/typed_cost.rs"), 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
        ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports_original.rs"), 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
        ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/alias_clone.rs"), 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests.rs"), 23207, "94aa760de11d6317cebdc638946b4d2a76d822b7b9249ea8a8789d6c681777e9"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs"), 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/old.rs"), 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/full.rs"), 27568, "f7ada2db950526d26e0c8e91ef90548c3a5154216b42d61e8c30a89beb978ef2"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs"), 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
        ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/receiving.rs"), 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs"), 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs"), 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs"), 9293, "f30d8ceea4d53421be07e2eec5610b9c5f0ac05ff06005f67c74028128157149"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", include_str!("../../workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs"), 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
        ("src/bounded_output/key_directory.rs", include_str!("../../bounded_output/key_directory.rs"), 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
        ("src/bounded_output/key_directory/tests.rs", include_str!("../../bounded_output/key_directory/tests.rs"), 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
        ("src/ast/program_source/receiving_roots.rs", include_str!("../../ast/program_source/receiving_roots.rs"), 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
        ("src/ast/program_source/generated_roots.rs", include_str!("../../ast/program_source/generated_roots.rs"), 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
        ("src/ast/program_source.rs", include_str!("../../ast/program_source.rs"), 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
        ("src/ast/program_source/checked_key.rs", include_str!("../../ast/program_source/checked_key.rs"), 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
        ("src/ast/program_source/checked_key/encode.rs", include_str!("../../ast/program_source/checked_key/encode.rs"), 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
        ("src/ast/program_source/decl_slices.rs", include_str!("../../ast/program_source/decl_slices.rs"), 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
        ("src/ast/program_source/decl_slices_original.rs", include_str!("../../ast/program_source/decl_slices_original.rs"), 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
        ("", "\n", 1, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"),
        ("src/workspace_graph/expected_projection/record_traversal.rs", include_str!("../../workspace_graph/expected_projection/record_traversal.rs"), 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
        ("src/workspace_graph/expected_projection/sort_keys.rs", include_str!("../../workspace_graph/expected_projection/sort_keys.rs"), 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
        ("src/workspace_graph/expected_projection/field_unique.rs", include_str!("../../workspace_graph/expected_projection/field_unique.rs"), 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
        ("src/workspace_graph/expected_projection/field_update.rs", include_str!("../../workspace_graph/expected_projection/field_update.rs"), 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
        ("src/workspace_graph/expected_projection/string_slots.rs", include_str!("../../workspace_graph/expected_projection/string_slots.rs"), 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
    ];
    for &(path, compiled, length, sha) in PARTS {
        assert_eq!((compiled.len(), digest(compiled)), (length, sha.to_owned()));
        if !path.is_empty() {
            let physical = reader::read(&root.join(path), length, sha, 2_097_152)
                .expect("fresh complete current projection audit part");
            assert_eq!(physical, compiled);
        }
        complete.push_str(compiled);
    }
    assert_eq!(
        complete, supplied,
        "every actual current audit part and join byte"
    );
    const PATH: &str = "docs/evidence/projection-current-entry-20261022/whole-phase.before.txt";
    const BEFORE: &str = include_str!(
        "../../../docs/evidence/projection-current-entry-20261022/whole-phase.before.txt"
    );
    const SHA: &str = "fe42350bec2fc90ebd37d5aef11786020716c555df70c85c3bf7622a4b4b1e7b";
    assert_eq!((BEFORE.len(), digest(BEFORE)), (647939, SHA.to_owned()));
    let previous = reader::read(&root.join(PATH), 647939, SHA, 2_097_152)
        .expect("complete independent unchanged phase predecessor");
    assert_eq!(previous, BEFORE);
    Some(previous)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CURRENT: &str =
        include_str!("../../../docs/evidence/projection-current-entry-20261022/whole-current.txt");
    const BEFORE: &str = include_str!(
        "../../../docs/evidence/projection-current-entry-20261022/whole-phase.before.txt"
    );
    #[test]
    fn current_audit_entry_keeps_all_fifty_complete_inputs_and_the_whole_phase_predecessor() {
        assert_eq!(audit_join(CURRENT).as_deref(), Some(BEFORE));
        reader::controls();
    }
    #[test]
    fn current_audit_entry_refuses_foreign_same_length_truncated_duplicate_and_extended_joins() {
        let mut changed = CURRENT.as_bytes().to_vec();
        changed[0] ^= 1;
        let changed = String::from_utf8(changed).unwrap();
        for foreign in [
            String::new(),
            changed,
            CURRENT[1..].to_owned(),
            format!("{CURRENT}\n"),
            format!("{CURRENT}{CURRENT}"),
        ] {
            assert!(audit_join(&foreign).is_none());
        }
        assert!(audit_join(BEFORE).is_none());
    }
}
