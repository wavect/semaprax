//! Whole current cache-compatible prefilter input before the unchanged constructor inverse.
use sha2::{Digest, Sha256};

// Closed final cost825 and retained847 bindings are added here. The entire original
// consumer and its immutable tables run before this adapter. Both the producer
// output and the fresh physical/compiled/archive source channels stay bound.
pub(super) fn after(domain: &str, source: &str, current: &str, projected: String) -> String {
    let (expected, compiled) = match (domain, source) {
        ("cost825", "src/workspace_graph/expected_projection/cost.rs") => (
            "ddf8dc2e74e4c0cc193e4e592671ffcfd9118b8bfe91cef2d0cf19eca3a635ce",
            include_str!("../workspace_graph/expected_projection/cost.rs"),
        ),
        ("cost825", "src/workspace_graph/expected_projection.rs") => (
            "974468e7035d1a7baa48b13078ce7ec7b0711c2c7610d64db7feea561b192f0c",
            include_str!("../workspace_graph/expected_projection.rs"),
        ),
        ("retained847", "src/ast/program_source/decl_slices.rs") => (
            "7b86d102694e1358c2a5ce00738a882df320d3e4860ba062c855cea56c198397",
            include_str!("../ast/program_source/decl_slices.rs"),
        ),
        ("retained847", "src/ast/program_source/decl_slices/alias_tests.rs") => (
            "5533f1fdf357b7b0ccfef93fe663599b6ecf2b1604f848d35f2f08e2479d38f8",
            include_str!("../ast/program_source/decl_slices/alias_tests.rs"),
        ),
        ("retained847", "src/ast/program_source/decl_slices/before_alias.rs") => (
            "2dbd704d16e38656ccd8b7aa035a1b2e2ddf6b5bfaf74ab7fe572cff858f7681",
            include_str!("../ast/program_source/decl_slices/before_alias.rs"),
        ),
        _ => return projected,
    };
    if super::source_sha(&projected) == expected {
        return projected;
    }
    assert_eq!(current, compiled);
    assert_eq!(
        projected, current,
        "the complete preceding cost producer remains unchanged"
    );
    let before = super::main_e177_final_inputs::value_direct(source, expected, current);
    assert_eq!(
        super::source_sha(&before),
        expected,
        "complete historical source-consumer input"
    );
    before
}

pub(super) fn retained_function847_input(source: &str, current: &str) -> String {
    let projected = super::retained_function847_previous(source, current);
    after("retained847", source, current, projected)
}

// A complete historical cursor is DATA, not a replacement current-source
// channel. Reacquire the actual compiled current and authenticate both named
// views against that same current before entering the unchanged old projector.
pub(super) fn history(source: &str, expected: &str, supplied: &str) -> Option<String> {
    let (cursor_sha, current) = match (source, expected) {
        (
            "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs",
            "cb4c709ecd0d8966ec03c9cc9d666468182861ded8cb0c860ed20c35300064e2",
        ) => (
            "592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e",
            include_str!(
                "../workspace_graph/expected_projection/private_layout/capacity_tests/source.rs"
            ),
        ),
        (
            "src/ast/program_source/function_alias.rs",
            "67bc93a9e52e70a42ad53614b4475a5fa881e96b17859fa376205bbc558088b2",
        ) => (
            "14cfbf7b8e491e35e3d52496782eb24ddf5aa51de0d5db1804177531345bbe09",
            include_str!("../ast/program_source/function_alias.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs",
            "434640ce68332a1d96840ae10b8e4a500799a567f6d75cc903a6e272196a24b2",
        ) => (
            "6dafc416d72e22cc58ba07cfb63ca91cd727d03167a14de0e88f038a36628561",
            include_str!("../workspace_graph/expected_projection/borrowed_imports/typed/alias.rs"),
        ),
        (
            "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs",
            "fdd92e3f7dd1a2064a727f478bdc15f217c5c93edc9d4bb32d0d901d510ec1da",
        ) => (
            "b2191c03d4dd66a071d943fe235bb743bc00032340b4928a3313faed1b9bc570",
            include_str!(
                "../workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs"
            ),
        ),
        (
            "src/hir/identity_resolution/nominal.rs",
            "6ffc8c764bb2638cb174473f95eb88cc8dfc4894af95e08295dfafa4b50ff49a",
        ) => (
            "92360748e76da16e6302d5b4d45ae5cdc73a70a25f9b68f56a7589bb176d5c46",
            include_str!("../hir/identity_resolution/nominal.rs"),
        ),
        _ => return None,
    };
    if super::source_sha(supplied) != cursor_sha {
        return None;
    }
    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();
    let cursor = invocation.value_direct(source, cursor_sha, current);
    assert_eq!(
        cursor, supplied,
        "the historical cursor belongs to the complete live current"
    );
    let before = invocation.value_direct(source, expected, current);
    assert_eq!(
        super::source_sha(&before),
        expected,
        "complete earlier source input"
    );
    Some(before)
}

#[test]
fn final_cost_binding_keeps_the_complete_fixed_source_and_rejects_changed_producers() {
    let source = "src/workspace_graph/expected_projection/cost.rs";
    let current = include_str!("../workspace_graph/expected_projection/cost.rs");
    let expected = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/ddf8dc2e74e4c0cc193e4e592671ffcfd9118b8bfe91cef2d0cf19eca3a635ce"
    ));
    assert_eq!(
        after("cost825", source, current, current.to_owned()),
        expected
    );
    for bad in [
        format!("{current} "),
        current[..current.len() - 1].to_owned(),
        String::new(),
    ] {
        assert!(std::panic::catch_unwind(|| after("cost825", source, &bad, bad.clone())).is_err());
    }
    assert!(std::panic::catch_unwind(|| after(
        "cost825",
        source,
        current,
        "foreign producer".to_owned()
    ))
    .is_err());
    assert_eq!(
        after("foreign", source, current, "untouched".to_owned()),
        "untouched"
    );
    assert_eq!(
        after("cost825", "foreign", current, "untouched".to_owned()),
        "untouched"
    );
}

#[test]
fn capacity_cursor_reacquires_current_before_both_complete_views_and_refuses_foreign_tuples() {
    let source = "src/workspace_graph/expected_projection/private_layout/capacity_tests/source.rs";
    let cursor_sha = "592ff99b199000a529e68f39e91e986fc133871718965889231f0139ff6a575e";
    let expected = "cb4c709ecd0d8966ec03c9cc9d666468182861ded8cb0c860ed20c35300064e2";
    let cursor =
        super::main_e177_final_inputs::registered_file(source, cursor_sha, usize::MAX).unwrap();
    let before =
        super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).unwrap();
    assert_eq!(history(source, expected, &cursor), Some(before));
    for bad in [
        format!("{cursor} "),
        cursor[..cursor.len() - 1].to_owned(),
        String::new(),
    ] {
        assert!(history(source, expected, &bad).is_none());
    }
    assert!(history("foreign", expected, &cursor).is_none());
    assert!(history(source, "foreign", &cursor).is_none());
}

// Normalize only the constructor checkpoints that consume this complete stage.
// Other checkpoints retain their own original source and inverse validation.
pub(super) fn typed_for(source: &str, expected: &str, current: &str) -> Option<String> {
    match (source, expected) {
        (
            "src/workspace_graph/constructor_failure/typed.rs",
            "1482ad177d1d605a183cccc3ef185aaed36e7214b1bf4db8065a466303fc99f6",
        )
        | (
            "src/workspace_graph/constructor_failure/receiving/typed/tests.rs",
            "2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4",
        ) => typed(source, current),
        _ => None,
    }
}

#[test]
fn typed_stage_claims_only_its_owned_checkpoint_and_keeps_complete_current_checks() {
    let source = "src/workspace_graph/constructor_failure/typed.rs";
    let expected = "1482ad177d1d605a183cccc3ef185aaed36e7214b1bf4db8065a466303fc99f6";
    let current = include_str!("../workspace_graph/constructor_failure/typed.rs");
    assert_eq!(typed_for(source, expected, current), typed(source, current));
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/function-alias-graph-image-parity-847-20261016/source-inverses.json"
    ))
    .unwrap();
    let row = fixture["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"].as_str() == Some(source))
        .unwrap();
    let cursor = row["edits"][0]["new"].as_str().unwrap();
    let cursor_sha = "5532f0f3b6de8c36bc6b4f4eb66863273fdc0c09275daa1e6ebba706d44523e7";
    assert_eq!(super::source_sha(cursor), cursor_sha);
    assert!(typed_for(source, cursor_sha, cursor).is_none());
    assert!(typed_for("foreign", expected, current).is_none());
    assert!(typed_for(source, "foreign", current).is_none());
    for bad in [cursor.to_owned(), format!("{current} "), String::new()] {
        assert!(std::panic::catch_unwind(|| typed_for(source, expected, &bad)).is_err());
    }
}

pub(super) fn typed(source: &str, current: &str) -> Option<String> {
    if source == "src/workspace_graph/constructor_failure/receiving/typed/tests.rs" {
        assert_eq!(
            current,
            include_str!("../workspace_graph/constructor_failure/receiving/typed/tests.rs")
        );
        let expected = "ddaa2f3c3d54bab731acb976de3a9482d71fa65a4d409a83c2036ed8f9ad1251";
        let before = super::main_e177_final_inputs::value_direct(source, expected, current);
        assert_eq!(before.len(), 27662);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
            ),
            expected
        );
        return Some(before);
    }
    if source != "src/workspace_graph/constructor_failure/typed.rs" {
        return None;
    }
    assert_eq!(
        current,
        include_str!("../workspace_graph/constructor_failure/typed.rs")
    );
    assert_eq!(
        current,
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
            .unwrap()
    );
    assert_eq!(current.len(), 11766);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "2d51c6b69300d23950a780837f0ba865df703cc50a0b035192de2f86e1ef157b"
    );
    let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
        "../../docs/evidence/cache-dispatch-20261021/typed-inverse.json"
    ))
    .unwrap();
    let mut before = current.to_owned();
    for (at, after, prior) in edits.into_iter().rev() {
        assert_eq!(&before[at..at + after.len()], after);
        before.replace_range(at..at + after.len(), &prior);
    }
    assert_eq!(before.len(), 11670);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7"
    );
    Some(before)
}

pub(super) fn owner(source: &str, current: &str) -> Option<String> {
    let (compiled, expected) = match source {
        "src/bounded_output.rs" => (
            include_str!("../bounded_output.rs"),
            "fa91c8eca4825931211d066d6ebde8c0ff157de1ec78b5950abe867c295e481f",
        ),
        "src/workspace_graph/initial_report/build.rs" => (
            include_str!("../workspace_graph/initial_report/build.rs"),
            "48b56f916c25d00004bf6d8bc13e56a0972d07f531fceda2c7c94b57d2af20e1",
        ),
        "src/workspace_graph/initial_report/tests.rs" => (
            include_str!("../workspace_graph/initial_report/tests.rs"),
            "0f530933b3964478e93a13bd7065cda6a97ab121b7c77508db45e72aff401ad7",
        ),
        _ => return None,
    };
    assert_eq!(current, compiled);
    assert_eq!(
        current,
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
            .unwrap()
    );
    let before = if source == "src/workspace_graph/initial_report/tests.rs" {
        let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
            "../../docs/evidence/runtime-history-20261021/owner-inverse.json"
        ))
        .unwrap();
        let mut before = current.to_owned();
        for (at, after, prior) in edits.into_iter().rev() {
            assert_eq!(&before[at..at + after.len()], after);
            before.replace_range(at..at + after.len(), &prior);
        }
        before
    } else {
        super::main_e177_final_inputs::value_direct(source, expected, current)
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        expected
    );
    Some(before)
}

// Exact already-registered whole inputs required by the existing metadata inverses.
pub(super) fn entry(source: &str, expected: &str) -> bool {
    const STAGES: &[(&str, &str)] = &[
        (
            "src/cleanup_plan/replay/branch_clone/before_inline.rs",
            "56b26aa261b2523329fb1d2a84e62c8fb8ac7697e93a8efdd7849408c9a1c413",
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            "db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7",
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs",
            "a8422b646b93710ae1803f0c32ae81c5c45b7de3fa50a0344d37afee6f96bbb3",
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs",
            "a25ce9f229ff208bcd02915e3495f15b0139087dfc9ce584a348b9092678b5dc",
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs",
            "4f2e45b389447291464f818cf8834880f7935e6578087e9bef7f96c89a16c8af",
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs",
            "969a66f6a53585c67165c9b7eba481cc8528bf93e7e8e9a797b47c99352199a7",
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",
            "531399a55c2d96fa86f44fc2170918dbc84e826c2b47a2db8d26c7e26a03ba03",
        ),
        (
            "src/cleanup_plan/replay/factored/oracle_hook.rs",
            "0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae",
        ),
        (
            "src/cleanup_plan/replay/final_output/before.rs",
            "7e5c2e493e1058dee41eb42922aedcc7c657acb75e4acfe745c2d4229e5c5a17",
        ),
        (
            "src/cleanup_plan/replay/path_array/tests.rs",
            "b641a21d3e5ba8f517f79b3184f4bc3496fa32c74e3b36951877fd145b528619",
        ),
        (
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "ff4af4658d4f51db9a3652316bcbdd5c13c79fc5bcde2fbb4b7f1a011472aaec",
        ),
    ];
    STAGES.contains(&(source, expected))
        || super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).is_some()
}

#[test]
fn complete_metadata_inverses_keep_original_outputs_and_refuse_changed_inputs() {
    use super::{metadata_json, runtime_json};
    let cases = [
        (
            runtime_json::metadata(metadata_json::Fixture::M20),
            "src/cleanup_plan/replay/path_array/tests.rs",
            "9b31375f7ee10af00217bd4caa212cbe21a95ac6391fb0849083e01ac7d3f0e5",
        ),
        (
            runtime_json::metadata(metadata_json::Fixture::M23),
            "src/cleanup_plan/replay/path_array/tests.rs",
            "9b31375f7ee10af00217bd4caa212cbe21a95ac6391fb0849083e01ac7d3f0e5",
        ),
        (
            runtime_json::metadata(metadata_json::Fixture::M26),
            "src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs",
            "22358b445b982ce13d5a55dfff11df4eb0d1ec0133ad9fae9008c66c26efe029",
        ),
        (
            runtime_json::metadata(metadata_json::Fixture::M28),
            "src/cleanup_plan/replay/factored/oracle_hook.rs",
            "aefe0d06e3f7a6b4ae36bc11160a9c7aed2bd9b30b6041308f619ada3266f707",
        ),
        (
            runtime_json::metadata(metadata_json::Fixture::M31),
            "src/cleanup_plan/replay/branch_clone/before_inline.rs",
            "c273939c59b7d0d36012b2d2dd4347a7e4dcc85bd087a7c110e4a10a4b34496d",
        ),
        (
            runtime_json::metadata(metadata_json::Fixture::M31),
            "src/cleanup_plan/replay/final_output/before.rs",
            "f7be016b8050ba2c7113915daece93cc671fa3065afd04e7b8b9821ba498ec33",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F03),
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            "a179fa71398494ef6f96fdf14713d17a563c0649c883197dbc9c1840eb2cadde",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F08),
            "src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",
            "2797ecdf2618961555d76d228fbd03b2c5cf345d538ef0d8a8bcb7e87456eb3c",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F09),
            "src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",
            "c4c006a456f6d1a9a370e52d720aa4d679e499b6e25a8253b614bb44875b78e7",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F10),
            "src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",
            "c14eed7c27913cdfec7ff0ccd94b82ecf851f9e5ab64fc6356bcbcd346094a97",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F12),
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            "a179fa71398494ef6f96fdf14713d17a563c0649c883197dbc9c1840eb2cadde",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F13),
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            "a179fa71398494ef6f96fdf14713d17a563c0649c883197dbc9c1840eb2cadde",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F16),
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs",
            "75cb7782397334be2bd59b1b9cc5615f995c20271aec086911c2c92e5ef10ad1",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F29),
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs",
            "75cb7782397334be2bd59b1b9cc5615f995c20271aec086911c2c92e5ef10ad1",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F38),
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            "a179fa71398494ef6f96fdf14713d17a563c0649c883197dbc9c1840eb2cadde",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F64),
            "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs",
            "f231ca22435f360e55efda50a6381350130968967932a349fc790cff1664659e",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F64),
            "src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs",
            "486badc82f654435111acc2b40be6ed7d8a86019e7ee67ea16f92ae6382e75eb",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F68),
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F69),
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F70),
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "43384a448492f20e042f02fe4de5a23241331bae3aa7501a3844dd5a5f72a587",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F72),
            "src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs",
            "f231ca22435f360e55efda50a6381350130968967932a349fc790cff1664659e",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F72),
            "src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs",
            "486badc82f654435111acc2b40be6ed7d8a86019e7ee67ea16f92ae6382e75eb",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F79),
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000",
        ),
        (
            runtime_json::load(runtime_json::Fixture::F86),
            "src/cleanup_plan/replay/structural_control/calls.rs",
            "17f757fd22a55b7aaaca952c0ab84b7c8d45bf9285fa15a0a4032ecfbdaa5000",
        ),
    ];
    for (payload, source, original) in cases {
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source))
                .unwrap();
        let before = super::project(&payload, source, &raw);
        assert_eq!(
            super::source_sha(&before),
            original,
            "complete original inverse: {source}"
        );
        for hostile in [
            format!("{raw}\n"),
            raw[..raw.len() - 1].to_owned(),
            format!(" {raw}"),
        ] {
            assert!(
                std::panic::catch_unwind(|| super::project(&payload, source, &hostile)).is_err(),
                "changed complete input: {source}"
            );
        }
    }
}

#[test]
fn existing_registered_oracle_stage_keeps_four_complete_inverses_and_refuses_foreign_tuples() {
    use super::runtime_json;
    const SOURCE: &str = "src/cleanup_plan/replay/allocation/oracle.rs";
    const EXPECTED: &str = "fac362691f9d979a58f9274596f07027ee5ce5d14550695e6db47c481a1532ee";
    assert!(entry(SOURCE, EXPECTED));
    assert!(!entry("foreign.rs", EXPECTED));
    assert!(!entry(SOURCE, &"0".repeat(64)));
    let raw = include_str!("../cleanup_plan/replay/allocation/oracle.rs");
    for fixture in [
        runtime_json::Fixture::F03,
        runtime_json::Fixture::F12,
        runtime_json::Fixture::F13,
        runtime_json::Fixture::F38,
    ] {
        let payload = runtime_json::load(fixture);
        let before = super::project(&payload, SOURCE, raw);
        assert_ne!(before, raw);
        for hostile in [format!("{raw}\n"), raw[..raw.len() - 1].to_owned()] {
            assert!(
                std::panic::catch_unwind(|| super::project(&payload, SOURCE, &hostile)).is_err()
            );
        }
    }
}

#[test]
fn every_fixed_metadata_stage_has_its_complete_registered_input() {
    use super::main_e177_final_inputs::ProjectionInvocation;
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/metadata-views-20261024/new-views.json"
    ))
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 204);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut invocation = ProjectionInvocation::new();
    for row in rows {
        let source = row["source"].as_str().unwrap();
        let view = &row["view"];
        let expected = view["sha256"].as_str().unwrap();
        let current = std::fs::read_to_string(root.join(source)).unwrap();
        let prior = invocation.value_direct(source, expected, &current);
        assert_eq!(prior.len() as u64, view["length"].as_u64().unwrap());
        assert_eq!(super::source_sha(&prior), expected, "{source}");
        assert_eq!(
            prior,
            std::fs::read_to_string(root.join(view["archive"].as_str().unwrap())).unwrap()
        );
    }
    let row = &rows[0];
    let source = row["source"].as_str().unwrap();
    let expected = row["view"]["sha256"].as_str().unwrap();
    let current = std::fs::read_to_string(root.join(source)).unwrap();
    assert!(entry(source, expected));
    assert!(!entry("foreign.rs", expected));
    assert!(!entry(source, &"0".repeat(64)));
    for hostile in [
        format!("{current}\n"),
        current[..current.len() - 1].to_owned(),
    ] {
        let value = invocation.value_direct(source, expected, &hostile);
        assert_ne!(super::source_sha(&value), expected);
    }
}

#[test]
fn declared_cursor_epoch_binds_all_current_channels_before_the_complete_history() {
    let source = "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs";
    let expected = "fdb672ec67714810ba990a306ec8b282ce7c628d627b272b322d21e51293da34";
    let current_sha = "2c91db9d0da06846703b41d7221a5daf16b56aa73933f0a88b714d23d08552a2";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let current = std::fs::read_to_string(root.join(source)).unwrap();
    let previous = std::fs::read_to_string(root.join("e").join(expected)).unwrap();
    assert_eq!(current.len(), 8182);
    assert_eq!(previous.len(), 8127);
    assert_eq!(super::source_sha(&current), current_sha);
    assert_eq!(super::source_sha(&previous), expected);
    let mut invocation = super::main_e177_final_inputs::ProjectionInvocation::new();
    assert_eq!(
        super::main_e177_final_inputs::registered_file(source, current_sha, 8182).unwrap(),
        current
    );
    assert_eq!(
        invocation.file(&root.join(source), 8127, expected).unwrap(),
        previous
    );
    for supplied in [&current, &previous] {
        assert_eq!(
            invocation.value_direct(source, expected, supplied),
            previous
        );
    }
    for bad in [
        String::new(),
        format!("{current} "),
        format!("{previous} "),
        current[..current.len() - 1].to_owned(),
    ] {
        assert_ne!(
            super::source_sha(&invocation.value_direct(source, expected, &bad)),
            expected
        );
    }
    assert!(super::main_e177_final_inputs::registered_file(source, expected, 8126).is_none());
    assert!(
        super::main_e177_final_inputs::registered_file(source, "foreign", usize::MAX).is_none()
    );
    assert!(
        super::main_e177_final_inputs::registered_file("foreign", expected, usize::MAX).is_none()
    );
    assert_eq!(
        invocation.value_direct("foreign", expected, &current),
        current
    );
}

#[test]
fn retained_alias_cursors_authenticate_both_complete_views_against_actual_current() {
    {
        let source = "src/ast/program_source/function_alias.rs";
        let expected = "67bc93a9e52e70a42ad53614b4475a5fa881e96b17859fa376205bbc558088b2";
        let cursor = super::main_e177_final_inputs::registered_file(
            source,
            "14cfbf7b8e491e35e3d52496782eb24ddf5aa51de0d5db1804177531345bbe09",
            usize::MAX,
        )
        .unwrap();
        let before =
            super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).unwrap();
        assert_eq!(history(source, expected, &cursor), Some(before));
        assert!(history(source, expected, &format!("{cursor} ")).is_none());
        assert!(history(source, "foreign", &cursor).is_none());
    }
    {
        let source = "src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs";
        let expected = "434640ce68332a1d96840ae10b8e4a500799a567f6d75cc903a6e272196a24b2";
        let cursor = super::main_e177_final_inputs::registered_file(
            source,
            "6dafc416d72e22cc58ba07cfb63ca91cd727d03167a14de0e88f038a36628561",
            usize::MAX,
        )
        .unwrap();
        let before =
            super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).unwrap();
        assert_eq!(history(source, expected, &cursor), Some(before));
        assert!(history(source, expected, &format!("{cursor} ")).is_none());
        assert!(history(source, "foreign", &cursor).is_none());
    }
    {
        let source =
            "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs";
        let expected = "fdd92e3f7dd1a2064a727f478bdc15f217c5c93edc9d4bb32d0d901d510ec1da";
        let cursor = super::main_e177_final_inputs::registered_file(
            source,
            "b2191c03d4dd66a071d943fe235bb743bc00032340b4928a3313faed1b9bc570",
            usize::MAX,
        )
        .unwrap();
        let before =
            super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).unwrap();
        assert_eq!(history(source, expected, &cursor), Some(before));
        assert!(history(source, expected, &format!("{cursor} ")).is_none());
        assert!(history(source, "foreign", &cursor).is_none());
    }
    {
        let source = "src/hir/identity_resolution/nominal.rs";
        let expected = "6ffc8c764bb2638cb174473f95eb88cc8dfc4894af95e08295dfafa4b50ff49a";
        let cursor = super::main_e177_final_inputs::registered_file(
            source,
            "92360748e76da16e6302d5b4d45ae5cdc73a70a25f9b68f56a7589bb176d5c46",
            usize::MAX,
        )
        .unwrap();
        let before =
            super::main_e177_final_inputs::registered_file(source, expected, usize::MAX).unwrap();
        assert_eq!(history(source, expected, &cursor), Some(before));
        assert!(history(source, expected, &format!("{cursor} ")).is_none());
        assert!(history(source, "foreign", &cursor).is_none());
    }
}
