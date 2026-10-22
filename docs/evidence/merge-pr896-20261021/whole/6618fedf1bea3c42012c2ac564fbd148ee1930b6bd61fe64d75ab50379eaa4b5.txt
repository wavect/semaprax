#[test]
fn legacy_nominal_complete_original_sources_and_semantic_constructor_stay_exact() {
    let rows: serde_json::Value =
        serde_json::from_str(super::metadata(super::Fixture::LegacyNominal)).unwrap();
    let files=[
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/hir/declaration_index/identity_payer.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/identity_payer.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__declaration_index__identity_payer.rs.before.txt"))),
        ("src/hir/declaration_index/nominal.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__declaration_index__nominal.rs.before.txt"))),
        ("src/hir/identity_resolution/nominal_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_tests.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__identity_resolution__nominal_tests.rs.before.txt"))),
        ("src/hir/nominal_payload.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__nominal_payload.rs.before.txt"))),
        ("src/hir/nominal_payload/elements.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/elements.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__nominal_payload__elements.rs.before.txt"))),
        ("src/hir/nominal_payload/map.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/map.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__nominal_payload__map.rs.before.txt"))),
        ("tests/hir_module_boundaries.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/tests__hir_module_boundaries.rs.before.txt"))),
        ("tests/workspace/semantic_graph/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/tests__workspace__semantic_graph__source_inventory.rs.before.txt"))),
        ("src/hir/nominal_payload/legacy.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy.rs")),""),
        ("src/hir/nominal_payload/legacy/clone.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/clone.rs")),""),
        ("src/hir/nominal_payload/legacy/tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/tests.rs")),""),
        ("src/hir/declaration_index/nominal/legacy.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal/legacy.rs")),""),
        ("src/hir/identity_resolution/nominal_tests/legacy.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_tests/legacy.rs")),""),
        ("src/hir/nominal_payload/legacy/source_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/nominal_payload/legacy/source_tests.rs")),""),
        ("src/hir/declaration_index/nominal_original.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/declaration_index/nominal_original.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__declaration_index__nominal_original.rs.before.txt"))),
        ("src/hir/identity_resolution/nominal_original.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/identity_resolution/nominal_original.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__identity_resolution__nominal_original.rs.before.txt"))),
    ];
    assert_eq!(rows.as_array().unwrap().len(), files.len());
    for ((path, current, old), row) in files.into_iter().zip(rows.as_array().unwrap()) {
        assert_eq!(row["source"].as_str().unwrap(), path);
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(restored.matches(new).count(), 1, "{path}");
            restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, old, "{path}");
    }
    let current = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/hir/declaration_index/identity_payer.rs"
    ));
    let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__declaration_index__identity_payer.rs.before.txt"));
    assert_eq!(
        current.split_once("    fn build_nominal_index").unwrap().1,
        old.split_once("    fn build_nominal_index").unwrap().1
    );
    let current = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/hir/nominal_payload.rs"
    ));
    let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/legacy-nominal-sharing-827/original/src__hir__nominal_payload.rs.before.txt"));
    fn section<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
        let rest = text.split_once(start).unwrap().1;
        rest.split_once(end).unwrap().0
    }
    for (start, end) in [
        (
            "impl<T: NominalElement> Clone",
            "impl<T: NominalElement> Deref for",
        ),
        ("    pub fn try_make_mut", "    /// Moves recursive roots"),
        (
            "impl<T: NominalElement> Drop for NominalPayload",
            "pub(crate) fn dispose",
        ),
    ] {
        assert_eq!(section(current, start, end), section(old, start, end));
    }
}
