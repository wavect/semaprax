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

// Exact current component fronts precede the unchanged complete ordered join.
// Exact post83 current components acquire the complete successful852 input.
fn post83_part_before(path: &str, supplied: &str) -> String {
    let (length, sha, edits): (usize, &str, &[(usize, &str, &str)]) = match path {
        "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs" => (31169, "39b4b81288ffce7460fa764ea526348258d4546ca8f22b24d93d26bba4cd2ebb", &[
            (29032, "            (8110, \"        drop(scope);\\n        // Classification also lends b to work's private query scratch. End\\n        // that owner before testing exclusive access to the fixture's Rc.\\n        drop(work);\\n        assert!(Rc::get_mut(b.prefix.as_mut().unwrap()).is_some());\", \"        drop(scope);\\n        assert!(Rc::get_mut(b.prefix.as_mut().unwrap()).is_some());\"),\n", ""),
            (28325, "        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\")).as_slice(), 18414, \"074d4939f7c6a2c278f482219eb7e66419b3dfe66929602c8ab6cc942c826165\", 18010, \"066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41\", &[\n", "        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\")).as_slice(), 18243, \"f2219248dd29ce8f0b9ce4c1a545b545f6a740a9d0e09c8dded8c903e9585090\", 18010, \"066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41\", &[\n"),
            (24907, "    if path == \"src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs\" {\n        let compiled = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs\")).as_slice();\n        let physical = std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(path)).unwrap();\n        assert_eq!(physical.as_slice(), compiled);\n        assert_eq!(current, compiled);\n        assert_eq!(current.len(), 31154);\n        assert_eq!(digest(current), \"5e0da04b5e6373c30f7b31799f36a484477ccc0f9d79c98af4f91cd6a4d0e335\");\n        // Entire already-retained main42 input, before its unchanged old edits.\n        let before = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/e/fc7a3fe35db9670affa2d479fea6e0a9eadf50afed410e729c7d956124d1c7f5\"));\n        assert_eq!(before.len(), 24273);\n        assert_eq!(digest(before), \"fc7a3fe35db9670affa2d479fea6e0a9eadf50afed410e729c7d956124d1c7f5\");\n        assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(path)).unwrap(), physical);\n        return before.to_vec();\n    }\n", ""),
            (24680, "// Acquire only these five complete physical epochs before the unchanged\n", "// Acquire only these four complete physical epochs before the unchanged\n"),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => (18414, "074d4939f7c6a2c278f482219eb7e66419b3dfe66929602c8ab6cc942c826165", &[
            (8131, "        // Classification also lends b to work's private query scratch. End\n        // that owner before testing exclusive access to the fixture's Rc.\n        drop(work);\n", ""),
        ]),
        _ => return supplied.to_owned(),
    };
    assert_eq!(supplied.len(), length);
    assert_eq!(digest(supplied), sha);
    let mut prior = supplied.to_owned();
    for &(at, after, before) in edits {
        assert!(prior.is_char_boundary(at) && prior.is_char_boundary(at + after.len()));
        assert_eq!(&prior[at..at + after.len()], after);
        prior.replace_range(at..at + after.len(), before);
    }
    prior
}

fn current_part_before(path: &str, supplied: &str) -> String {
    let (length, sha, edits): (usize, &str, &[(usize, usize, &str, &str)]) = match path {
        "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs" => (29608, "793b4c291463fce1d4e102b2ebc029da527e8a13666344e9c522bb56d7da54fd", &[
            (24679, 29608, "\n// Acquire only these four complete physical epochs before the unchanged\n// fixed946/main42 source contracts. No inventory target or old edit is repinned.\nfn retained_fork_source_before(path: &str, current: &[u8]) -> Vec<u8> {\n    let (compiled, length, expected, before_length, before_digest, edits):\n        (&[u8], usize, &str, usize, &str, &[(usize, &str, &str)]) = match path {\n        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs\")).as_slice(), 22365, \"9b32edd9e75c3199c9b1b3c01a5bcdc9689872795e3222b6f684f4e967c08834\", 20575, \"d7db6c7fc49140f9dabec3e647bf64b50571ee71a8478aae1c03acd92bba6c5f\", &[\n        ]),\n        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs\")).as_slice(), 11753, \"7944d10f5515da0a4e9c4b6ed49882bb3e20ccf9cfc96fc4c20372370ebf955f\", 11246, \"1f55c14d5d7fe41614e045fc680d5df8a93d4e29e4698e47bdb7e39ce8203c68\", &[\n            (637, \"    failed: bool,\\n\", \"\"),\n            (1047, \"in crate::clean\", \"s\"),\n            (1064, \"_plan::\", \"e\"),\n            (1072, \"eplay\", \"\"),\n            (1562, \"size_of::<bool>() // initialized invocation-local sticky terminal state\\n    + \", \"\"),\n            (2004, \"\\n    + 3 * size_of::<bool>()\", \"\"),\n            (2033, \" // sticky read, guard, and first terminal publication\", \"\"),\n            (4590, \"\", \"!\"),\n            (4597, \"fai\", \"\"),\n            (4603, \"\", \"ger.current()\"),\n            (4691, \"if !\", \"\"),\n            (4701, \".ledger.current() {\", \"\"),\n            (4733, \"$sc\", \".w\"),\n            (4737, \"pe.failed = t\", \"\"),\n            (4751, \"ue;\", \"k\"),\n            (4767, \"r\", \".budg\"),\n            (4770, \"urn Err(Diagnostic::io(\\\"SPX-H006\\\", String::new()));\", \"\"),\n            (4830, \"}\\n\", \"\"),\n            (4836, \"    match $scope.work.budget\", \"\"),\n            (4923, \" {\\n            Ok(()) => {},\\n            Err(error) => {\\n                $scope.failed = true\", \"?\"),\n            (5017, \"\\n                return Err(error);\\n            }\\n        }\", \"\"),\n            (5092, \"in crate::clean\", \"s\"),\n            (5109, \"_plan::\", \"e\"),\n            (5117, \"eplay\", \"\"),\n            (5447, \"e,\\n        failed: fals\", \"\"),\n        ]),\n        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs\")).as_slice(), 18243, \"f2219248dd29ce8f0b9ce4c1a545b545f6a740a9d0e09c8dded8c903e9585090\", 18010, \"066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41\", &[\n            (778, \"e,\\n        failed: fals\", \"\"),\n            (6214, \";\\n        // Isolate the validation owner's Rc lifetime from the fixture's\\n        // destruction scratch, which otherwise retains its own strong/weak owners.\\n        work.release_observation_scratch_for_test()\", \"\"),\n        ]),\n        \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs\" => (include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs\")).as_slice(), 24967, \"703b8e31e2c34896382513d8e4cff5d900d8c5990ccbdb96aad3dc363aed3e05\", 24843, \"d4757425643d96a03c1c6a05f31d7018cb5653c71a921bb736335e9102edf6f6\", &[\n            (15815, \"\", \"cur\"),\n            (15817, \"\", \"n\"),\n            (15818, \"ained_epoch\", \"\"),\n            (15832, \"crate::cleanup_plan::replay::skeleton_work::query_proof::before_emission(\", \"\"),\n            (15937, \"));\\n    let current = retained_epoch.as_str(\", \"\"),\n        ]),\n        _ => return current.to_vec(),\n    };\n    let physical = std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(path)).unwrap();\n    assert_eq!(physical.as_slice(), compiled);\n    assert_eq!(current, compiled);\n    assert_eq!(current.len(), length);\n    assert_eq!(digest(current), expected);\n    let mut before = if path == \"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs\" {\n        crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::before_emission(std::str::from_utf8(current).unwrap()).into_bytes()\n    } else { current.to_vec() };\n    for &(at, after, prior) in edits.iter().rev() {\n        assert_eq!(before.get(at..at + after.len()), Some(after.as_bytes()));\n        before.splice(at..at + after.len(), prior.bytes());\n    }\n    assert_eq!(before.len(), before_length);\n    assert_eq!(digest(&before), before_digest);\n    assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(path)).unwrap(), physical);\n    before\n}\n", ""),
            (20412, 20482, "                actual = retained_fork_source_before(&path, &actual);\n", ""),
            (14096, 14162, "            actual = retained_fork_source_before(&path, &actual);\n", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs" => (11753, "7944d10f5515da0a4e9c4b6ed49882bb3e20ccf9cfc96fc4c20372370ebf955f", &[
            (5450, 5473, "        failed: false,\n", ""),
            (5088, 5135, "pub(in crate::cleanup_plan::replay) fn inputs(\n", "pub(super) fn inputs(\n"),
            (4683, 5077, "        if !$scope.ledger.current() {\n            $scope.failed = true;\n            return Err(Diagnostic::io(\"SPX-H006\", String::new()));\n        }\n        match $scope.work.budget.reserve_bytes($scope.work.function, $bytes + AUTH, $phase) {\n            Ok(()) => {},\n            Err(error) => {\n                $scope.failed = true;\n                return Err(error);\n            }\n        }\n", "        $scope\n            .work\n            .budget\n            .reserve_bytes($scope.work.function, $bytes + AUTH, $phase)?;\n"),
            (4579, 4606, "        if $scope.failed {\n", "        if !$scope.ledger.current() {\n"),
            (1957, 2088, "    + 2 * size_of::<Result<bool, Diagnostic>>()\n    + 3 * size_of::<bool>(); // sticky read, guard, and first terminal publication\n", "    + 2 * size_of::<Result<bool, Diagnostic>>();\n"),
            (1556, 1634, "    + size_of::<bool>() // initialized invocation-local sticky terminal state\n", ""),
            (1043, 1136, "pub(in crate::cleanup_plan::replay) const CALL: usize = 8 * size_of::<&[ExprSkeletonPath]>()\n", "pub(super) const CALL: usize = 8 * size_of::<&[ExprSkeletonPath]>()\n"),
            (637, 655, "    failed: bool,\n", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => (18243, "f2219248dd29ce8f0b9ce4c1a545b545f6a740a9d0e09c8dded8c903e9585090", &[
            (6216, 6426, "        // Isolate the validation owner's Rc lifetime from the fixture's\n        // destruction scratch, which otherwise retains its own strong/weak owners.\n        work.release_observation_scratch_for_test();\n", ""),
            (781, 804, "        failed: false,\n", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs" => (24967, "703b8e31e2c34896382513d8e4cff5d900d8c5990ccbdb96aad3dc363aed3e05", &[
            (15807, 15941, "    let retained_epoch = crate::cleanup_plan::replay::skeleton_work::query_proof::before_emission(include_str!(\"../../emission.rs\"));\n", "    let retained_epoch = crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::before_emission(include_str!(\"../../emission.rs\"));\n"),
        ]),
        "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs" => (9547, "652294002414f8f91534668666c28bec3954d1ce250cffc7fd0f3522a1356a8e", &[
            (2203, 2326, "    if page[\"schema\"] == super::super::SCHEMA_V14 {\n        super::super::verify_current_v14(&page);\n        return;\n    }\n", ""),
        ]),
        _ => return supplied.to_owned(),
    };
    assert_eq!(supplied.len(), length);
    assert_eq!(digest(supplied), sha);
    let mut previous = supplied.to_owned();
    for &(start, end, after, before) in edits {
        assert!(previous.is_char_boundary(start) && previous.is_char_boundary(end));
        assert_eq!(&previous[start..end], after);
        previous.replace_range(start..end, before);
    }
    previous
}

// Whole current audit join only. The original production purity check runs
// before this historical facade route, and every original phase inverse runs
// after it. No source component is omitted from the current input.
pub(super) fn audit_join(supplied: &str) -> Option<String> {
    if supplied.len() != 1843345
        || digest(supplied) != "36cbb7835efaf67056aa5b3d13769d59fa92bcbb1b678a61ea646149890575bf"
    {
        return None;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut complete = String::new();
    let mut previous_complete = String::new();
    // Literal generated from the actual owning macro, in original order.
    // Data pieces remain read-only source evidence, never compiled authority.
    const PARTS: &[(&str, &str, usize, &str)] = &[
        ("tests/wasm/scalar_browser_ci_contract/current_profiles573.rs", include_str!("../../../tests/wasm/scalar_browser_ci_contract/current_profiles573.rs"), 10276, "99b71451c348ba6de7f457d2c0b5c9d8c944135fe36c08e796c1840d1df49aff"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs", include_str!("../../cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs"), 10205, "900af30a08f96959e6f9c553d28e6f82f3104473cf23dbbb3090436310a22669"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs"), 15950, "e4ad639ff6378b8d37a89fdddd90798d5aa7bd420f5ea2755a76c904d1d46602"),
        ("src/cleanup_plan/replay/selected_owner/ordinary.rs", include_str!("../../cleanup_plan/replay/selected_owner/ordinary.rs"), 4718, "fb794e530da2d722c135a4e2a9ac5bccd755bbd12fc0114f8ed9b6f5fd0e5eaf"),
        ("src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", include_str!("../../cleanup_plan/replay/selected_owner/ordinary/tests.rs"), 6987, "5bdf7210f9343246d20abc64d29ef049d632c1c991d92aff03ad29a443061302"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", include_str!("../../cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs"), 2424, "0916bc3f12ad83a9600fb2009a4ca09fafea8c4a4cc25e6d88d712e9209c851d"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", include_str!("../../cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs"), 86308, "755306bc0932240e81f62d01f1ec556235756c816d013d5d7f81417c4ba40417"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs", include_str!("../../cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs"), 7677, "6b870fe2d27431eade32b09e8f25703efe2ac56fbef286b544278682105ec89d"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs"), 15369, "56a40d51a09c3bf21f7b4683aee142dbe29524c2c97cac97e685bf2773747549"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs"), 32385, "e2d7bb7fc687c471da74528e29568efcc6eb8f48b0661778d051ba2a8652b293"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs"), 24146, "a71bd98e617a681d232f9f5dc80e59db39cf0dea461a313c077f80754e1fceff"),
        ("src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/boxed_states.rs"), 7076, "59e698eea731be50e9d9ecd35a5d817ec1be5aaa482444b6df38aa2ee70f18b3"),
        ("src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/branch_join.rs"), 10802, "34be6ba09bb7bac7123a1039df67948f0ab5e49bdab69d7c2da55ba8cd07b90c"),
        ("src/cleanup_plan/replay/path_join/before_frontier/mod.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/mod.rs"), 50333, "749400ec833f546f5c2f88c913c6c2ff5ba7fa5bea179de50d660411f9919211"),
        ("src/cleanup_plan/replay/path_join/before_frontier/storage.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/storage.rs"), 15694, "06fe50c24fcbb7db7763761387f990681acfa2c7963361bf6c8ed25260dbca79"),
        ("src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs"), 2720, "ab46c653becc07f401aa8aba33fe13b39992f41491f58bc43ff608056a011b10"),
        ("src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs"), 5189, "7313a9413f4e954ccc583034119763928dcd9445d595373ccf297f065a5be8cf"),
        ("src/cleanup_plan/replay/path_join/before_frontier/tests.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/tests.rs"), 5325, "d7841a842bf900a756905b56623fca40766ae1c3344bfb1dc4e1f86efe396df0"),
        ("src/cleanup_plan/replay/path_join/before_frontier/transition.rs", include_str!("../../cleanup_plan/replay/path_join/before_frontier/transition.rs"), 35563, "628c1c134e179b8e8ca0db4646b6219e4c3a939968598ac2935ac500de996616"),
        ("src/cleanup_plan/replay/path_join/boxed_flow.rs", include_str!("../../cleanup_plan/replay/path_join/boxed_flow.rs"), 3885, "e60f54339ffc357e805c8466ea21272bfef62531405ba2f06c0026a3ad390154"),
        ("src/cleanup_plan/replay/path_join/frontier.rs", include_str!("../../cleanup_plan/replay/path_join/frontier.rs"), 22575, "54dc6f27a755399709247f182124d7cd3a8f25070f07c865412587c4c0d93587"),
        ("src/cleanup_plan/replay/path_join/frontier_tests.rs", include_str!("../../cleanup_plan/replay/path_join/frontier_tests.rs"), 22107, "7423729bfe25901a69b6bebbdb7f954b55a15e350975a9cd6adfe0b8a51d53bf"),
        ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", include_str!("../../cleanup_plan/replay/path_join/frontier_tests/custody.rs"), 24543, "03c5e4fe51f2925421a899860e90976665d45664c999c07b174c0233d138b747"),
        ("src/cleanup_plan/replay/path_join/frontier_tests/whole.rs", include_str!("../../cleanup_plan/replay/path_join/frontier_tests/whole.rs"), 10925, "da179f47f2f3f41eb14350b137d2468f5ac241445d6ae04d131c927da1c09908"),
        ("src/call_index/validation_before.rs", include_str!("../../call_index/validation_before.rs"), 3463, "50097cb04d8577afd15d0209fd11f14e1ff2d3d60c5bb692b8128c753b201472"),
        ("src/call_index/validation_receipt_tests.rs", include_str!("../../call_index/validation_receipt_tests.rs"), 4015, "0cb673d2f420230bc3b49fa90937d83302bfd29dec44aac4e95515c758297ce7"),
        ("src/codegen/native_owned_data_provider/emit_provider_before.rs", include_str!("../../codegen/native_owned_data_provider/emit_provider_before.rs"), 2513, "a4a17ce516c1c360d12f7ca0855ff5c049dfdd2248dc22c03bdc0cd25b5fe4ab"),
        ("src/codegen/native_owned_data_provider/validation_reuse_tests.rs", include_str!("../../codegen/native_owned_data_provider/validation_reuse_tests.rs"), 7661, "bbe03233559921346ef3456859eda8f60842efa28563d6d44c9e1946fccc3cbd"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs"), 2168, "5cab19b5b7c91dad5fe1240b586ac7a5f3c92896a8575b56a62bf0ed92c5b9f5"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs"), 6801, "2c6e246527ec365687dbc3bd8339184b8ff4367c9e7ae546bda2d1920777ff60"),
        ("src/workspace_graph/expected_projection/private_layout/names/interface_census.rs", include_str!("../../workspace_graph/expected_projection/private_layout/names/interface_census.rs"), 5132, "5122d3abd1d880fb1598104c2d7367a747c668b00fa971383d38e7ff55bdee75"),
        ("src/workspace_graph/expected_projection/private_layout/names/interface_before.rs", include_str!("../../workspace_graph/expected_projection/private_layout/names/interface_before.rs"), 24587, "58dd9a33aa98e22cb8d89fbf3d47512e68dac56c10952cba055da265f5078dd8"),
        ("src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs", include_str!("../../workspace_graph/expected_projection/private_layout/names/interface_tests.rs"), 14300, "141dc56360504411f085f21117ee757eee43d8e1dca7a21854a01f45f53afdeb"),
        ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs", include_str!("../../cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs"), 11246, "1f55c14d5d7fe41614e045fc680d5df8a93d4e29e4698e47bdb7e39ce8203c68"),
        ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs", include_str!("../../cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs"), 18010, "066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41"),
        ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs", include_str!("../../cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs"), 10932, "792be2624414510de96336103ccc964f39ff7c474581aa90521be539b8e21d2d"),
        ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs", include_str!("../../cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs"), 24998, "a017f356ed78def8d10cdcf98287cc7e30a797413fd955d9bbfb0824e712cb5a"),
        ("src/source_reflows_merge/main_e177_overlay_labels_v12.rs", include_str!("../../source_reflows_merge/main_e177_overlay_labels_v12.rs"), 7219, "260cfb1eb837f15415a05f769e0d6554f6c26a3eeecf01a58eff16e8a2bf2446"),
        ("src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs", include_str!("../../source_reflows_merge/main_e177_overlay_labels_v12/tests.rs"), 9424, "6b316023a1d5f99cec3300b78bd84cd93077e1a806c774ab349aaea6356920e6"),
        ("src/workspace_graph/expected_projection/call_carriers_directory/modular.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_directory/modular.rs"), 3536, "894b8a45279a1afb3b8a2e10e07888f1567ae6fb30a5f0c860ee39a5cd107d62"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_census.rs"), 9819, "b5032e96c21ae66a011e08938a5dda8b366a9b495b6d421f7eb259c9e4ae3098"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_pair.rs"), 10077, "8031e83e8bdbe63b08131016dbee7c9cf3b6614bff75090dc88cc8f706a51167"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs"), 9105, "c7a201a333ce54f18ee0982beeb3fcc13d14517804710989a832de87209e1e3b"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_projection.rs"), 14818, "8d2fe0047c5e0f8e4359e4f2916ed0cd52e2f6d7a66e816ba2e91fb9556e01cf"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_session.rs"), 12946, "342ca60956a62f93878ae04b67aab70a971012aa38839a0a307cbf37001746ba"),
        ("src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/modular_tests.rs"), 12708, "2c3de671500f78caa65b3341f65ea930b50a63066773037fa42ee31c3495b0bf"),
        ("src/workspace_graph/expected_projection/shared_local_callees/paired.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/paired.rs"), 8606, "fe826f4e36cccd454d198513f640998934f963d136359d3063883cbb797d2c3d"),
        ("src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/paired_projection.rs"), 14766, "a997eed41a794bb5d0e80db0703fd07c480cd40c06dc1f51b1124e023a82cbc2"),
        ("src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/paired_tests.rs"), 11212, "1c34ee37bc564a908bc38cc58cc1b55a560c0f566fcc8770e6464febdab25a19"),
        ("src/workspace_graph/expected_projection/shared_local_callees/record_before.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/record_before.rs"), 12593, "f472fe9bc7bea0097f6ae0323fcf5fa593790ec229c8499f75fb2a14deff050a"),
        ("src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs"), 12324, "7f28afbdc7fad1d150acac616a45c884495803bf6487d055ba6ff43d6c05380b"),
        ("src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/record_signatures.rs"), 11968, "d588429142af0102d9d3cde1f9c2ee50f76661efe22b19c2b44a7e6d1dc95f52"),
        ("src/workspace_graph/expected_projection.rs", include_str!("../../workspace_graph/expected_projection.rs"), 59903, "6c6e07a5bc02662dbc273eac0bb9fa2cb3f580134dd208f58212fc6f64528f5e"),
        ("src/workspace_graph/expected_projection/shared_local_callees.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees.rs"), 11552, "99ab34b44805b5129051c18a14c2dcc6aa107a2ff786a53c2c2fa161c667820a"),
        ("src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs"), 14427, "f95beb41205dc47d5707ed4f189bf04dacde802016039e6b335be04b3bf213ef"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs"), 27085, "f0d8a4d374f082e478bc7cdd3b3fe3575da70e3836349ebd47d87d8a04265785"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reused.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reused.rs"), 10317, "ed964ba39e51dfd7dbdd8c391b831260adba376c6f8dad5b035e8df36ab59fd7"),
        ("src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/reused_controls.rs"), 6564, "87ecd2f33b9af73f98c499aab75e7ccb879f9c5213fb23119fc9768b05254227"),
        ("src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/before_projection.rs"), 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
        ("src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/controls_tests.rs"), 13127, "1c768293781d9a11d9d1484d1b45107b722ad93664cdd46a775e269569f28577"),
        ("src/workspace_graph/expected_projection/shared_local_callees/invoice.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/invoice.rs"), 9314, "50bfe2c1d81e5bd637a542d4254198bcd6132293c5826231fa9d08865f94643c"),
        ("src/workspace_graph/expected_projection/shared_local_callees/tests.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/tests.rs"), 9590, "a3079d6e5f427d20801582413905630ac9106a189bce6d49af977cf8a2133099"),
        ("src/workspace_graph/expected_projection/shared_local_callees/walk.rs", include_str!("../../workspace_graph/expected_projection/shared_local_callees/walk.rs"), 9350, "fafdcb7e6743647fcd1ad787ea98731a159fc155e0a8d35b02541f60dd74ce17"),
        ("src/workspace_graph/expected_projection/vec_constructor.rs", include_str!("../../workspace_graph/expected_projection/vec_constructor.rs"), 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
        ("src/workspace_graph/expected_projection/held_projection.rs", include_str!("../../workspace_graph/expected_projection/held_projection.rs"), 23082, "e53a5ad24eb87ee600bd01d1916d243c137d24ae7ef7f59b005ddb7af31cbca5"),
        ("src/workspace_graph/expected_projection/call_carriers.rs", include_str!("../../workspace_graph/expected_projection/call_carriers.rs"), 12722, "09a4f25c50527acf2be98d6f7cfcf4b41544340dc3bad24af858f7c8c6a077c7"),
        ("src/workspace_graph/expected_projection/call_carriers_joint.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_joint.rs"), 12510, "b0bc1737feb13d8936a41b2767fa6ace0b674dbe3316b6ba4371c2ad29afe8a6"),
        ("src/workspace_graph/expected_projection/call_carriers_tests.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_tests.rs"), 25183, "d5f1f5027b904e45143b0a65d05a7027d987c69dce23392f06fb134aa66d75a9"),
        ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_inverse.rs"), 10291, "d2e01f03652a4389d0fb78628e14f4ac7f2080b1df41f628e0dae83036588da7"),
        ("src/workspace_graph/expected_projection/call_carriers_original.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_original.rs"), 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
        ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", include_str!("../../workspace_graph/expected_projection/call_carriers_historical_tests.rs"), 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
        ("src/workspace_graph/expected_projection/prelude_forecast.rs", include_str!("../../workspace_graph/expected_projection/prelude_forecast.rs"), 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
        ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", include_str!("../../workspace_graph/expected_projection/prelude_forecast_tests.rs"), 20153, "3b6115c316aea6bf362fe23d8100dc5f2ea3b2163cbc4ac98e2e63c5d6d07564"),
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
        ("src/workspace_graph/tests.rs", include_str!("../../workspace_graph/tests.rs"), 68375, "6907e6c7112f3dd34b959de7f0d33b08821cb4ac1723040eb7aa8e71332bd2b7"),
        ("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", include_str!("../../cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs"), 19806, "6745c50ef06c4bab501efe623044c0044977a946df0aff8c95873a0c92de0fd1"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs", include_str!("../../cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs"), 30467, "79db9e907b352fe8cc96eaa3a1e87c97795436a3d566f0809af85986c118d92a"),
        ("src/cleanup_plan/replay/workspace_oracle.rs", include_str!("../../cleanup_plan/replay/workspace_oracle.rs"), 3669, "0cbd9a3eadf60e9611493e161e4c8692165231eaaf8c60d757070273d62a5ed0"),
        ("src/hir/inspection/paid_nul/tests.rs", include_str!("../../hir/inspection/paid_nul/tests.rs"), 43550, "d7bc8c78fe1f8e1bed6730376ec571db9eb60c49f896304585e05ff34282ef08"),
        ("src/hir/inspection/paid_nul/reuse_before.rs", include_str!("../../hir/inspection/paid_nul/reuse_before.rs"), 38440, "f244990419557597356086d2b3e6005f84c1d471160eb4b2e1506b1727e1ca3f"),
        ("src/hir/inspection/paid_nul/reuse_before/pending.rs", include_str!("../../hir/inspection/paid_nul/reuse_before/pending.rs"), 2490, "27622dc1bce1918fa8366f1ec97267692a655758907e270ff032156b385b0ebd"),
        ("src/hir/inspection/paid_nul/type_scratch.rs", include_str!("../../hir/inspection/paid_nul/type_scratch.rs"), 8271, "43bebbb4746fbd9182793e37806790df77bf590983657b1653568aed31435e5e"),
        ("src/cleanup_plan/replay/continuation_storage/tests.rs", include_str!("../../cleanup_plan/replay/continuation_storage/tests.rs"), 27358, "17bb6b512db22de653583ba59d27f2f682f66dd2bca1583510e513b7688565a6"),
        ("src/cleanup_plan/replay/continuation_storage/reached.rs", include_str!("../../cleanup_plan/replay/continuation_storage/reached.rs"), 6399, "bf60981f88a31c20b7151d010bc1360461048573b9164e00dd8c9124305d9278"),
        ("src/cleanup_plan/replay/continuation_storage/reached_before.rs", include_str!("../../cleanup_plan/replay/continuation_storage/reached_before.rs"), 9194, "980ab986d43f41e0e3f4d7aae59190c0f6ece37ce354ad967536341911782ceb"),
        ("src/cleanup_plan/replay/continuation_storage/reached_tests.rs", include_str!("../../cleanup_plan/replay/continuation_storage/reached_tests.rs"), 19239, "ee9bebd2537779b373c67a2e16334664cb13d594ef97761d3f987257c1f4c101"),
        ("src/cleanup_plan/replay_tests/factored.rs", include_str!("../../cleanup_plan/replay_tests/factored.rs"), 15975, "e70cd4f9a85d0711d5ab9bb84c8be8730a46e1ab506eb5fa4a9c0ee16a8f71a7"),
    ];
    for &(path, compiled, length, sha) in PARTS {
        // Authenticate complete physical/compiled current before acquiring its
        // named historical input. Foreign current bytes cannot reach an inverse.
        if !path.is_empty() {
            let physical = reader::read(&root.join(path), compiled.len(), &digest(compiled), 2_097_152)
                .expect("fresh complete current projection audit part");
            assert_eq!(physical, compiled);
        }
        let prior_current = post83_part_before(path, compiled);
        let previous = current_part_before(path, &prior_current);
        assert_eq!((previous.len(), digest(&previous)), (length, sha.to_owned()));
        complete.push_str(compiled);
        previous_complete.push_str(&previous);
    }
    assert_eq!(previous_complete.len(), 1835716);
    assert_eq!(digest(&previous_complete), "2d23fd56d8bf76d9c6f5b995d4a18bb593e2cc58d00fd0fc4f009614d75b50ef");
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
