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
    if supplied.len() != 1522425
        || digest(supplied) != "5c7285f6c8e91c92c4982fd03d14200159d9e6829a543639f077ecef972d0f5f"
    {
        return None;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut complete = String::new();
    // Literal generated from the actual owning macro, in original order.
    // Data pieces remain read-only source evidence, never compiled authority.
    const PARTS: &[(&str, &str, usize, &str)] = &[
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs", include_str!("../../cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs"), 10205, "900af30a08f96959e6f9c553d28e6f82f3104473cf23dbbb3090436310a22669"),
        ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", include_str!("../../cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs"), 15950, "e4ad639ff6378b8d37a89fdddd90798d5aa7bd420f5ea2755a76c904d1d46602"),
        ("src/cleanup_plan/replay/selected_owner/ordinary.rs", include_str!("../../cleanup_plan/replay/selected_owner/ordinary.rs"), 4718, "710e2aef425952528c9036dc3155a66f00fab781c4afba822d8d30580a4f6f84"),
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
        ("src/cleanup_plan/replay/path_join/frontier.rs", include_str!("../../cleanup_plan/replay/path_join/frontier.rs"), 18749, "a0cb34d597eb284342addb9ee75ffc605a31d6ce3993672f4427703aada6652d"),
        ("src/cleanup_plan/replay/path_join/frontier_tests.rs", include_str!("../../cleanup_plan/replay/path_join/frontier_tests.rs"), 22051, "9c5fb6769cc7bc5f6e5518f781095e66c9c4cc0db11ca19b88462b80217e2d1a"),
        ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", include_str!("../../cleanup_plan/replay/path_join/frontier_tests/custody.rs"), 19222, "f7d423a58f479b0288f13a7a7abf9a9670c6167839d9bc978ea1e877a8030c82"),
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
        ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs", include_str!("../../cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs"), 24843, "d4757425643d96a03c1c6a05f31d7018cb5653c71a921bb736335e9102edf6f6"),
        ("src/source_reflows_merge/main_e177_overlay_labels_v12.rs", include_str!("../../source_reflows_merge/main_e177_overlay_labels_v12.rs"), 7219, "260cfb1eb837f15415a05f769e0d6554f6c26a3eeecf01a58eff16e8a2bf2446"),
        ("src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs", include_str!("../../source_reflows_merge/main_e177_overlay_labels_v12/tests.rs"), 9000, "4b7909d105a26db233ae03b3a3386c1bbd43b981500764dbea2d31bdebe6ee81"),
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
