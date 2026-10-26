//! Whole current cache-compatible prefilter input before the unchanged constructor inverse.
use sha2::{Digest, Sha256};
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
