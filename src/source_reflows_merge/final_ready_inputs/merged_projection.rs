//! Exact merged current join before either independently retained historical chain.
use super::digest;
use crate::source_reflows_merge::main_e177_final_inputs::ProjectionInvocation;
const CURRENT_LENGTH: usize = 2283501;
const CURRENT_SHA: &str = "d93d71610738c60a5a728f8ffd5a5168f05e272ec048fc3a0b9b579c01c82d29";
// Literal expansion of the owning semantic_graph projection macro, in full order.
#[rustfmt::skip]
const CURRENT_PARTS: &[(&str, &str, usize, &str)] = &[
    ("tests/wasm/scalar_browser_ci_contract/current_profiles573.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wasm/scalar_browser_ci_contract/current_profiles573.rs")), 10600, "06638e2675d38575c33c43743a3a50ab6d6d6f6b9edf6a68dde51198a1078e69"),
    ("tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs")), 19935, "b259958397633eb1c8be6d84644295633adaa6990cfd0b880dcd3c04a6b13f29"),
    ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs")), 10205, "900af30a08f96959e6f9c553d28e6f82f3104473cf23dbbb3090436310a22669"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs")), 15950, "e4ad639ff6378b8d37a89fdddd90798d5aa7bd420f5ea2755a76c904d1d46602"),
    ("src/cleanup_plan/replay/selected_owner/ordinary.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/ordinary.rs")), 4718, "fb794e530da2d722c135a4e2a9ac5bccd755bbd12fc0114f8ed9b6f5fd0e5eaf"),
    ("src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/selected_owner/ordinary/tests.rs")), 6987, "5bdf7210f9343246d20abc64d29ef049d632c1c991d92aff03ad29a443061302"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs")), 2845, "6b070dcc9543929b7671d9e07ecb8c18d08d71f8cf869c9fa3c1bb669108d0af"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs")), 86308, "755306bc0932240e81f62d01f1ec556235756c816d013d5d7f81417c4ba40417"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs")), 7677, "6b870fe2d27431eade32b09e8f25703efe2ac56fbef286b544278682105ec89d"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs")), 17353, "e6ae7f63df349b2a4f8e260b8ec25312bf9362eed35882af10a3da3b2520c534"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs")), 32385, "e2d7bb7fc687c471da74528e29568efcc6eb8f48b0661778d051ba2a8652b293"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs")), 24353, "d6ce5ade786b2027c78ce173f548a0d9dacb09faba44e11435e55d467a2632f0"),
    ("src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs")), 7076, "59e698eea731be50e9d9ecd35a5d817ec1be5aaa482444b6df38aa2ee70f18b3"),
    ("src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs")), 10802, "34be6ba09bb7bac7123a1039df67948f0ab5e49bdab69d7c2da55ba8cd07b90c"),
    ("src/cleanup_plan/replay/path_join/before_frontier/mod.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/mod.rs")), 50333, "749400ec833f546f5c2f88c913c6c2ff5ba7fa5bea179de50d660411f9919211"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/storage.rs")), 15694, "06fe50c24fcbb7db7763761387f990681acfa2c7963361bf6c8ed25260dbca79"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs")), 2720, "ab46c653becc07f401aa8aba33fe13b39992f41491f58bc43ff608056a011b10"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs")), 5189, "7313a9413f4e954ccc583034119763928dcd9445d595373ccf297f065a5be8cf"),
    ("src/cleanup_plan/replay/path_join/before_frontier/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/tests.rs")), 5325, "d7841a842bf900a756905b56623fca40766ae1c3344bfb1dc4e1f86efe396df0"),
    ("src/cleanup_plan/replay/path_join/before_frontier/transition.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/transition.rs")), 35563, "628c1c134e179b8e8ca0db4646b6219e4c3a939968598ac2935ac500de996616"),
    ("src/cleanup_plan/replay/path_join/boxed_flow.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/boxed_flow.rs")), 3885, "e60f54339ffc357e805c8466ea21272bfef62531405ba2f06c0026a3ad390154"),
    ("src/cleanup_plan/replay/path_join/frontier.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier.rs")), 22575, "54dc6f27a755399709247f182124d7cd3a8f25070f07c865412587c4c0d93587"),
    ("src/cleanup_plan/replay/path_join/frontier_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests.rs")), 22107, "7423729bfe25901a69b6bebbdb7f954b55a15e350975a9cd6adfe0b8a51d53bf"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests/custody.rs")), 45676, "9741a25ba95c97dddbd9b507d4b190b926cd1db44f5adb812e48d59b2940658d"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/whole.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests/whole.rs")), 10925, "da179f47f2f3f41eb14350b137d2468f5ac241445d6ae04d131c927da1c09908"),
    ("src/call_index/validation_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/call_index/validation_before.rs")), 3463, "50097cb04d8577afd15d0209fd11f14e1ff2d3d60c5bb692b8128c753b201472"),
    ("src/call_index/validation_receipt_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/call_index/validation_receipt_tests.rs")), 4015, "0cb673d2f420230bc3b49fa90937d83302bfd29dec44aac4e95515c758297ce7"),
    ("src/codegen/native_owned_data_provider/emit_provider_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/codegen/native_owned_data_provider/emit_provider_before.rs")), 2513, "a4a17ce516c1c360d12f7ca0855ff5c049dfdd2248dc22c03bdc0cd25b5fe4ab"),
    ("src/codegen/native_owned_data_provider/validation_reuse_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/codegen/native_owned_data_provider/validation_reuse_tests.rs")), 7661, "bbe03233559921346ef3456859eda8f60842efa28563d6d44c9e1946fccc3cbd"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs")), 2168, "5cab19b5b7c91dad5fe1240b586ac7a5f3c92896a8575b56a62bf0ed92c5b9f5"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs")), 6801, "2c6e246527ec365687dbc3bd8339184b8ff4367c9e7ae546bda2d1920777ff60"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_census.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names/interface_census.rs")), 5132, "5122d3abd1d880fb1598104c2d7367a747c668b00fa971383d38e7ff55bdee75"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names/interface_before.rs")), 24587, "58dd9a33aa98e22cb8d89fbf3d47512e68dac56c10952cba055da265f5078dd8"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs")), 14300, "141dc56360504411f085f21117ee757eee43d8e1dca7a21854a01f45f53afdeb"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs")), 11753, "7944d10f5515da0a4e9c4b6ed49882bb3e20ccf9cfc96fc4c20372370ebf955f"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs")), 18414, "074d4939f7c6a2c278f482219eb7e66419b3dfe66929602c8ab6cc942c826165"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs")), 10932, "792be2624414510de96336103ccc964f39ff7c474581aa90521be539b8e21d2d"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs")), 24967, "703b8e31e2c34896382513d8e4cff5d900d8c5990ccbdb96aad3dc363aed3e05"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_labels_v12.rs")), 7219, "260cfb1eb837f15415a05f769e0d6554f6c26a3eeecf01a58eff16e8a2bf2446"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs")), 10368, "000028e4120e98e590ea4c03d182f5a22ae58d402f3f16ed9e5aa87f42e97d89"),
    ("src/workspace_graph/expected_projection/call_carriers_directory/modular.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_directory/modular.rs")), 3594, "b00352308d4dc48079e3807aca42493c4d42fed5a7412ce51c4ec8288c5d4952"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs")), 34238, "e2245dcba9a549068d00edc637e17296dd90e6c71476f84b0ddf49fb9728774b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs")), 10164, "12331004bc5ff08252bb790c870d298dca32160d07059d368c56c2a823cf57a3"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs")), 9105, "c7a201a333ce54f18ee0982beeb3fcc13d14517804710989a832de87209e1e3b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs")), 15243, "cee965c826e9598b05cec771ffb684e0f3720d1ecfa8ba2719b84c8bcb359c59"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs")), 12946, "342ca60956a62f93878ae04b67aab70a971012aa38839a0a307cbf37001746ba"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs")), 12853, "fd37d96b94154a3609d5eaedc6cd8ad7a14f8d3310d77eaeeaff4a251d7536ba"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired.rs")), 8693, "5303b15254477d9b5d3598326ebbb076dce7a76c609804bb1131534bc3641bea"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs")), 15191, "f794e09256426bd659799818a609b5b00dd5f4e97ea0e5c74b27ab987ad6635c"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs")), 12373, "f3e048b188faffc52497a842aec8fa2cdcdfc64ea0a3ead918e13b03166ff2f1"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/record_before.rs")), 12593, "f472fe9bc7bea0097f6ae0323fcf5fa593790ec229c8499f75fb2a14deff050a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs")), 12467, "b8b2ec4ffd658e7637c6fed7baeba1c46c33b790aa29cb14b5deca1dc8d11bd4"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs")), 11968, "d588429142af0102d9d3cde1f9c2ee50f76661efe22b19c2b44a7e6d1dc95f52"),
    ("src/workspace_graph/expected_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection.rs")), 60158, "f068236583110ceca6d18a6ec8321afc6da4559c1455b917ebd9e28095dfb6c0"),
    ("src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs")), 30573, "422caa6bb19da7b623d3cc03077f2aa07853b91ddc8de928bf75a22b6a6609f2"),
    ("src/workspace_graph/expected_projection/retained_scratch.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/retained_scratch.rs")), 1746, "26f398991a4d64fe85416c823e1080b6395170625c2822e2e67793357d200555"),
    ("src/workspace_graph/expected_projection/uncached_peak_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/uncached_peak_before.rs")), 14449, "0766f215f87fd212385e6829d531932e16765e1911a76311bef648dab215592b"),
    ("src/loan_plan/owned_capacity.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/owned_capacity.rs")), 7745, "4c92a94a5837bb794e0e5023b058da68c6bf9bf5b7b17c95b2a16f39f5ca8399"),
    ("src/workspace_graph/type_proof_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/type_proof_tests.rs")), 52672, "b8594959cb38cd0c7f684ad4872210af742d8e95cc5f8b53cb02eefdfa80fc8f"),
    ("src/workspace_graph/tests/builder_limits.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/tests/builder_limits.rs")), 39529, "0415977761dffbe2b5bda7fd517953af84b8d2855d27a66b3a5a02b4335bc49c"),
    ("src/bounded_output/stage_debits.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/stage_debits.rs")), 11427, "10d58a16ec8943f65d54b56f365c93c9edda40293408681ba7e580c13c4f331e"),
    ("src/cleanup_plan/replay/factored/oracle_order/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_order/tests.rs")), 21616, "bd69a7c25d0b254fe038507e0cdd8f238e6142571a547bad24f3c4aa50f7bbdc"),
    ("src/workspace_graph/expected_projection/shared_local_callees.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees.rs")), 11768, "b0a025e9365860a3c754d4a80b0dbafabdc57cffb4b289865e62b4be62787c2f"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903.rs")), 4193, "8a260a6eecfb85291defaca6e3f137e9c6f51f0546f1be28397bb691ddf3211d"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs")), 12727, "970c11a1df79cb10ed52ed0f0b5e41aa7d94b0f1582ebc52c0b573ef953dcf1b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs")), 22139, "cc9c89b34def67c380a69ccc4d1f5b8ddd1dc01f10abed418b24517baa390be7"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs")), 3537, "8a9e354004f9708bf02c02f15fee8028824c35d243fa49b34039d9a23b85271a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs")), 7032, "4b65f035fc1bf5748a4c6430b7d64edf3fc93eae7b4254e94df1db3419858fe3"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs")), 12517, "7881b169ea6cc3e4351c6d1b788bfbdbf48975605bc80c8356f1e5acbbce77ff"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs")), 9812, "e1f2fa9eab0ccc2c222713c24801b9463d74464e5e7c378a1fc2df53a0deccb0"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs")), 14836, "8df842787ee297c2eb78aad64e29c8d463a4dc185d7f44ec191ed1fbea4ca2b0"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs")), 14784, "4c1d4b3e2a6de391c39da456edc8d49da47433084c0fefcf7b661df2a5b3db8e"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs")), 10087, "9dee10813247b88639004ed0eff2849779505cba902a7ab62d998cb1e897627d"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs")), 8998, "737df7a2bb44db63bca40dea4165b59208bf74dfc80701694bdb0c3d211a185f"),
    ("src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs")), 14473, "9b4ade3c7f2353a48959535dc9c5bcb76f8fff2f5db63cfcae1299477d07b275"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs")), 27085, "f0d8a4d374f082e478bc7cdd3b3fe3575da70e3836349ebd47d87d8a04265785"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reused.rs")), 10317, "ed964ba39e51dfd7dbdd8c391b831260adba376c6f8dad5b035e8df36ab59fd7"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs")), 6564, "87ecd2f33b9af73f98c499aab75e7ccb879f9c5213fb23119fc9768b05254227"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs")), 21962, "0bddcfcb9aeb7b233ab3b842965cb97cfa11e471151a5491a9ec874cdc7adbd6"),
    ("src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs")), 13127, "1c768293781d9a11d9d1484d1b45107b722ad93664cdd46a775e269569f28577"),
    ("src/workspace_graph/expected_projection/shared_local_callees/invoice.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/invoice.rs")), 9314, "50bfe2c1d81e5bd637a542d4254198bcd6132293c5826231fa9d08865f94643c"),
    ("src/workspace_graph/expected_projection/shared_local_callees/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/tests.rs")), 9823, "d547905c8783ef393b7ddc972dd65ae4dc886da8add88138cc8dc0b711552c0c"),
    ("src/workspace_graph/expected_projection/shared_local_callees/walk.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/walk.rs")), 9350, "fafdcb7e6743647fcd1ad787ea98731a159fc155e0a8d35b02541f60dd74ce17"),
    ("src/workspace_graph/expected_projection/vec_constructor.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/vec_constructor.rs")), 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
    ("src/workspace_graph/expected_projection/held_projection.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection.rs")), 23128, "4c1346bbacc8be3ee7d9184b4e99b994a1f367eed37df9d41a156ecc62ad4096"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers.rs")), 24825, "2bdd41001d03315ef11904dbe5fa7c05dc03ec4e4550c4278eda8416ba9e2f8c"),
    ("src/workspace_graph/expected_projection/call_carriers_joint.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_joint.rs")), 36925, "93aef5478f35ff42f07bfcce851d01d72339be93fce7ef8e0ac9bfb57573ac7d"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_tests.rs")), 52632, "f220d11c99ec22e0027f0dca24ff0554a0fc52443decfcdcc4fe4366d2267fd3"),
    ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_inverse.rs")), 10466, "108703b1f80f79b6a62d482fc65a0a434053cd2f60713a9c568681c43ab0136f"),
    ("src/workspace_graph/expected_projection/call_carriers_original.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_original.rs")), 23610, "b1ce4d28f8364aed9967eba52699cd95968799b29779cb68eee71a4b7ef8fde3"),
    ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_historical_tests.rs")), 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
    ("src/workspace_graph/expected_projection/prelude_forecast.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/prelude_forecast.rs")), 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/prelude_forecast_tests.rs")), 23845, "44e6c853fde53e3f2594550c6679b7780de98ba16fb1732713719d53b0067c15"),
    ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/prelude_forecast_original.rs")), 21078, "8aaa2384f1c6d31d6e9c1ac2a777da0364a84d86d8bb768109c27ac616c24c6e"),
    ("src/workspace_graph/expected_projection/held_projection_original.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection_original.rs")), 18283, "9a0e2b526b5fe4546e27f3ac870f35f46572e9ccbb5609b2586d584f0362b1fb"),
    ("src/workspace_graph/expected_projection/borrowed_leaf.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_leaf.rs")), 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_leaf/tests.rs")), 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs")), 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs")), 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports.rs")), 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed.rs")), 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs")), 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs")), 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs")), 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/inline.rs")), 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs")), 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports_original.rs")), 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs")), 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests.rs")), 22861, "cff55f0e3f3aa8ac3240c483bd2b90fe99a036ee184bdb472e41686e3703084f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs")), 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs")), 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs")), 29594, "04214dc37fb912d136d52daeb5b56cd63a88035e97b5c80be4bd31e2300b3d23"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs")), 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/receiving.rs")), 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs")), 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs")), 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs")), 9661, "ccda5de031a1b7f493181e996e1590caeaf01a44af7459fa2f99b0fe6a8e1a6f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs")), 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
    ("src/bounded_output/key_directory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/key_directory.rs")), 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
    ("src/bounded_output/key_directory/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/key_directory/tests.rs")), 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
    ("src/ast/program_source/receiving_roots.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/receiving_roots.rs")), 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
    ("src/ast/program_source/generated_roots.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/generated_roots.rs")), 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
    ("src/ast/program_source.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source.rs")), 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
    ("src/ast/program_source/checked_key.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/checked_key.rs")), 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
    ("src/ast/program_source/checked_key/encode.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/checked_key/encode.rs")), 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
    ("src/ast/program_source/decl_slices.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/decl_slices.rs")), 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
    ("src/ast/program_source/decl_slices_original.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast/program_source/decl_slices_original.rs")), 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
    ("", "\n", 1, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"),
    ("src/workspace_graph/expected_projection/record_traversal.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/record_traversal.rs")), 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
    ("src/workspace_graph/expected_projection/sort_keys.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/sort_keys.rs")), 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
    ("src/workspace_graph/expected_projection/field_unique.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/field_unique.rs")), 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
    ("src/workspace_graph/expected_projection/field_update.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/field_update.rs")), 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
    ("src/workspace_graph/expected_projection/string_slots.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/string_slots.rs")), 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
    ("src/workspace_graph/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/tests.rs")), 68668, "e2c0662c308f33c4936acf9c5e935d7522dd5efa12eb4a31ac1267901eddbb76"),
    ("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs")), 19806, "6745c50ef06c4bab501efe623044c0044977a946df0aff8c95873a0c92de0fd1"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs")), 30467, "79db9e907b352fe8cc96eaa3a1e87c97795436a3d566f0809af85986c118d92a"),
    ("src/cleanup_plan/replay/workspace_oracle.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/workspace_oracle.rs")), 5654, "824db909deb40d4e42d846481f9e8b5949aa3cd361b63c4576b642eabae5d52f"),
    ("src/hir/inspection/paid_nul/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection/paid_nul/tests.rs")), 43550, "d7bc8c78fe1f8e1bed6730376ec571db9eb60c49f896304585e05ff34282ef08"),
    ("src/hir/inspection/paid_nul/reuse_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection/paid_nul/reuse_before.rs")), 38440, "f244990419557597356086d2b3e6005f84c1d471160eb4b2e1506b1727e1ca3f"),
    ("src/hir/inspection/paid_nul/reuse_before/pending.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection/paid_nul/reuse_before/pending.rs")), 2490, "27622dc1bce1918fa8366f1ec97267692a655758907e270ff032156b385b0ebd"),
    ("src/hir/inspection/paid_nul/type_scratch.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/inspection/paid_nul/type_scratch.rs")), 8271, "43bebbb4746fbd9182793e37806790df77bf590983657b1653568aed31435e5e"),
    ("src/cleanup_plan/replay/continuation_storage/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/tests.rs")), 27358, "17bb6b512db22de653583ba59d27f2f682f66dd2bca1583510e513b7688565a6"),
    ("src/cleanup_plan/replay/continuation_storage/reached.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/reached.rs")), 6548, "d8c98f2a9a41aaa5a03ff2c9d6bf6b7476f7f77ecdc548958b45386fa7c4238a"),
    ("src/cleanup_plan/replay/continuation_storage/reached_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/reached_before.rs")), 9194, "980ab986d43f41e0e3f4d7aae59190c0f6ece37ce354ad967536341911782ceb"),
    ("src/cleanup_plan/replay/continuation_storage/reached_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/continuation_storage/reached_tests.rs")), 20538, "5636a96a1d98a0e7ea08d0a963fbdb7a2671a638137f6dbd88c6d76d6c9c30e2"),
    ("src/cleanup_plan/replay_tests/factored.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay_tests/factored.rs")), 15975, "e70cd4f9a85d0711d5ab9bb84c8be8730a46e1ab506eb5fa4a9c0ee16a8f71a7"),
];
#[rustfmt::skip]
const V10_CURRENT_PARTS: &[(&str, usize, &str)] = &[
    ("tests/wasm/scalar_browser_ci_contract/current_profiles573.rs", 10600, "06638e2675d38575c33c43743a3a50ab6d6d6f6b9edf6a68dde51198a1078e69"),
    ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs", 10205, "900af30a08f96959e6f9c553d28e6f82f3104473cf23dbbb3090436310a22669"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", 15950, "e4ad639ff6378b8d37a89fdddd90798d5aa7bd420f5ea2755a76c904d1d46602"),
    ("src/cleanup_plan/replay/selected_owner/ordinary.rs", 4718, "fb794e530da2d722c135a4e2a9ac5bccd755bbd12fc0114f8ed9b6f5fd0e5eaf"),
    ("src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", 6987, "5bdf7210f9343246d20abc64d29ef049d632c1c991d92aff03ad29a443061302"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", 2424, "0916bc3f12ad83a9600fb2009a4ca09fafea8c4a4cc25e6d88d712e9209c851d"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", 86308, "755306bc0932240e81f62d01f1ec556235756c816d013d5d7f81417c4ba40417"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs", 7677, "6b870fe2d27431eade32b09e8f25703efe2ac56fbef286b544278682105ec89d"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs", 15369, "56a40d51a09c3bf21f7b4683aee142dbe29524c2c97cac97e685bf2773747549"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs", 32385, "e2d7bb7fc687c471da74528e29568efcc6eb8f48b0661778d051ba2a8652b293"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs", 24146, "a71bd98e617a681d232f9f5dc80e59db39cf0dea461a313c077f80754e1fceff"),
    ("src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs", 7076, "59e698eea731be50e9d9ecd35a5d817ec1be5aaa482444b6df38aa2ee70f18b3"),
    ("src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs", 10802, "34be6ba09bb7bac7123a1039df67948f0ab5e49bdab69d7c2da55ba8cd07b90c"),
    ("src/cleanup_plan/replay/path_join/before_frontier/mod.rs", 50333, "749400ec833f546f5c2f88c913c6c2ff5ba7fa5bea179de50d660411f9919211"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage.rs", 15694, "06fe50c24fcbb7db7763761387f990681acfa2c7963361bf6c8ed25260dbca79"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs", 2720, "ab46c653becc07f401aa8aba33fe13b39992f41491f58bc43ff608056a011b10"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs", 5189, "7313a9413f4e954ccc583034119763928dcd9445d595373ccf297f065a5be8cf"),
    ("src/cleanup_plan/replay/path_join/before_frontier/tests.rs", 5325, "d7841a842bf900a756905b56623fca40766ae1c3344bfb1dc4e1f86efe396df0"),
    ("src/cleanup_plan/replay/path_join/before_frontier/transition.rs", 35563, "628c1c134e179b8e8ca0db4646b6219e4c3a939968598ac2935ac500de996616"),
    ("src/cleanup_plan/replay/path_join/boxed_flow.rs", 3885, "e60f54339ffc357e805c8466ea21272bfef62531405ba2f06c0026a3ad390154"),
    ("src/cleanup_plan/replay/path_join/frontier.rs", 22575, "54dc6f27a755399709247f182124d7cd3a8f25070f07c865412587c4c0d93587"),
    ("src/cleanup_plan/replay/path_join/frontier_tests.rs", 22107, "7423729bfe25901a69b6bebbdb7f954b55a15e350975a9cd6adfe0b8a51d53bf"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", 42710, "3d68e87d1e68adabab772808f670ca189f7b3f776dfc2b54cf612b8c6d2f029b"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/whole.rs", 10925, "da179f47f2f3f41eb14350b137d2468f5ac241445d6ae04d131c927da1c09908"),
    ("src/call_index/validation_before.rs", 3463, "50097cb04d8577afd15d0209fd11f14e1ff2d3d60c5bb692b8128c753b201472"),
    ("src/call_index/validation_receipt_tests.rs", 4015, "0cb673d2f420230bc3b49fa90937d83302bfd29dec44aac4e95515c758297ce7"),
    ("src/codegen/native_owned_data_provider/emit_provider_before.rs", 2513, "a4a17ce516c1c360d12f7ca0855ff5c049dfdd2248dc22c03bdc0cd25b5fe4ab"),
    ("src/codegen/native_owned_data_provider/validation_reuse_tests.rs", 7661, "bbe03233559921346ef3456859eda8f60842efa28563d6d44c9e1946fccc3cbd"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs", 2168, "5cab19b5b7c91dad5fe1240b586ac7a5f3c92896a8575b56a62bf0ed92c5b9f5"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs", 6801, "2c6e246527ec365687dbc3bd8339184b8ff4367c9e7ae546bda2d1920777ff60"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_census.rs", 5132, "5122d3abd1d880fb1598104c2d7367a747c668b00fa971383d38e7ff55bdee75"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_before.rs", 24587, "58dd9a33aa98e22cb8d89fbf3d47512e68dac56c10952cba055da265f5078dd8"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs", 14300, "141dc56360504411f085f21117ee757eee43d8e1dca7a21854a01f45f53afdeb"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs", 11753, "7944d10f5515da0a4e9c4b6ed49882bb3e20ccf9cfc96fc4c20372370ebf955f"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs", 18414, "074d4939f7c6a2c278f482219eb7e66419b3dfe66929602c8ab6cc942c826165"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs", 10932, "792be2624414510de96336103ccc964f39ff7c474581aa90521be539b8e21d2d"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs", 24967, "703b8e31e2c34896382513d8e4cff5d900d8c5990ccbdb96aad3dc363aed3e05"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12.rs", 7219, "260cfb1eb837f15415a05f769e0d6554f6c26a3eeecf01a58eff16e8a2bf2446"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs", 9547, "652294002414f8f91534668666c28bec3954d1ce250cffc7fd0f3522a1356a8e"),
    ("src/workspace_graph/expected_projection/call_carriers_directory/modular.rs", 3536, "894b8a45279a1afb3b8a2e10e07888f1567ae6fb30a5f0c860ee39a5cd107d62"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs", 9819, "b5032e96c21ae66a011e08938a5dda8b366a9b495b6d421f7eb259c9e4ae3098"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs", 10077, "8031e83e8bdbe63b08131016dbee7c9cf3b6614bff75090dc88cc8f706a51167"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs", 9105, "c7a201a333ce54f18ee0982beeb3fcc13d14517804710989a832de87209e1e3b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs", 14818, "8d2fe0047c5e0f8e4359e4f2916ed0cd52e2f6d7a66e816ba2e91fb9556e01cf"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs", 12946, "342ca60956a62f93878ae04b67aab70a971012aa38839a0a307cbf37001746ba"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs", 12708, "2c3de671500f78caa65b3341f65ea930b50a63066773037fa42ee31c3495b0bf"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired.rs", 8606, "fe826f4e36cccd454d198513f640998934f963d136359d3063883cbb797d2c3d"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs", 14766, "a997eed41a794bb5d0e80db0703fd07c480cd40c06dc1f51b1124e023a82cbc2"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs", 11212, "1c34ee37bc564a908bc38cc58cc1b55a560c0f566fcc8770e6464febdab25a19"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_before.rs", 12593, "f472fe9bc7bea0097f6ae0323fcf5fa593790ec229c8499f75fb2a14deff050a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs", 12324, "7f28afbdc7fad1d150acac616a45c884495803bf6487d055ba6ff43d6c05380b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs", 11968, "d588429142af0102d9d3cde1f9c2ee50f76661efe22b19c2b44a7e6d1dc95f52"),
    ("src/workspace_graph/expected_projection.rs", 59903, "6c6e07a5bc02662dbc273eac0bb9fa2cb3f580134dd208f58212fc6f64528f5e"),
    ("src/workspace_graph/expected_projection/shared_local_callees.rs", 11552, "99ab34b44805b5129051c18a14c2dcc6aa107a2ff786a53c2c2fa161c667820a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs", 14427, "f95beb41205dc47d5707ed4f189bf04dacde802016039e6b335be04b3bf213ef"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs", 27085, "f0d8a4d374f082e478bc7cdd3b3fe3575da70e3836349ebd47d87d8a04265785"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused.rs", 10317, "ed964ba39e51dfd7dbdd8c391b831260adba376c6f8dad5b035e8df36ab59fd7"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs", 6564, "87ecd2f33b9af73f98c499aab75e7ccb879f9c5213fb23119fc9768b05254227"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
    ("src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs", 13127, "1c768293781d9a11d9d1484d1b45107b722ad93664cdd46a775e269569f28577"),
    ("src/workspace_graph/expected_projection/shared_local_callees/invoice.rs", 9314, "50bfe2c1d81e5bd637a542d4254198bcd6132293c5826231fa9d08865f94643c"),
    ("src/workspace_graph/expected_projection/shared_local_callees/tests.rs", 9590, "a3079d6e5f427d20801582413905630ac9106a189bce6d49af977cf8a2133099"),
    ("src/workspace_graph/expected_projection/shared_local_callees/walk.rs", 9350, "fafdcb7e6743647fcd1ad787ea98731a159fc155e0a8d35b02541f60dd74ce17"),
    ("src/workspace_graph/expected_projection/vec_constructor.rs", 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
    ("src/workspace_graph/expected_projection/held_projection.rs", 23082, "e53a5ad24eb87ee600bd01d1916d243c137d24ae7ef7f59b005ddb7af31cbca5"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", 12722, "09a4f25c50527acf2be98d6f7cfcf4b41544340dc3bad24af858f7c8c6a077c7"),
    ("src/workspace_graph/expected_projection/call_carriers_joint.rs", 12510, "b0bc1737feb13d8936a41b2767fa6ace0b674dbe3316b6ba4371c2ad29afe8a6"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 25183, "d5f1f5027b904e45143b0a65d05a7027d987c69dce23392f06fb134aa66d75a9"),
    ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", 10291, "d2e01f03652a4389d0fb78628e14f4ac7f2080b1df41f628e0dae83036588da7"),
    ("src/workspace_graph/expected_projection/call_carriers_original.rs", 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
    ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
    ("src/workspace_graph/expected_projection/prelude_forecast.rs", 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", 20153, "3b6115c316aea6bf362fe23d8100dc5f2ea3b2163cbc4ac98e2e63c5d6d07564"),
    ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", 21032, "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187"),
    ("src/workspace_graph/expected_projection/held_projection_original.rs", 18237, "b976530c05c01bb64e685f1ed12b36d1305032cda8560778ce0f832bcc88e6f6"),
    ("src/workspace_graph/expected_projection/borrowed_leaf.rs", 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", 23207, "94aa760de11d6317cebdc638946b4d2a76d822b7b9249ea8a8789d6c681777e9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 27568, "f7ada2db950526d26e0c8e91ef90548c3a5154216b42d61e8c30a89beb978ef2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 9293, "f30d8ceea4d53421be07e2eec5610b9c5f0ac05ff06005f67c74028128157149"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
    ("src/bounded_output/key_directory.rs", 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
    ("src/bounded_output/key_directory/tests.rs", 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
    ("src/ast/program_source/receiving_roots.rs", 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
    ("src/ast/program_source/generated_roots.rs", 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
    ("src/ast/program_source.rs", 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
    ("src/ast/program_source/checked_key.rs", 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
    ("src/ast/program_source/checked_key/encode.rs", 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
    ("src/ast/program_source/decl_slices.rs", 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
    ("src/ast/program_source/decl_slices_original.rs", 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
    ("", 1, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"),
    ("src/workspace_graph/expected_projection/record_traversal.rs", 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
    ("src/workspace_graph/expected_projection/sort_keys.rs", 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
    ("src/workspace_graph/expected_projection/field_unique.rs", 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
    ("src/workspace_graph/expected_projection/field_update.rs", 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
    ("src/workspace_graph/expected_projection/string_slots.rs", 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
    ("src/workspace_graph/tests.rs", 68375, "6907e6c7112f3dd34b959de7f0d33b08821cb4ac1723040eb7aa8e71332bd2b7"),
    ("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", 19806, "6745c50ef06c4bab501efe623044c0044977a946df0aff8c95873a0c92de0fd1"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs", 30467, "79db9e907b352fe8cc96eaa3a1e87c97795436a3d566f0809af85986c118d92a"),
    ("src/cleanup_plan/replay/workspace_oracle.rs", 3669, "0cbd9a3eadf60e9611493e161e4c8692165231eaaf8c60d757070273d62a5ed0"),
    ("src/hir/inspection/paid_nul/tests.rs", 43550, "d7bc8c78fe1f8e1bed6730376ec571db9eb60c49f896304585e05ff34282ef08"),
    ("src/hir/inspection/paid_nul/reuse_before.rs", 38440, "f244990419557597356086d2b3e6005f84c1d471160eb4b2e1506b1727e1ca3f"),
    ("src/hir/inspection/paid_nul/reuse_before/pending.rs", 2490, "27622dc1bce1918fa8366f1ec97267692a655758907e270ff032156b385b0ebd"),
    ("src/hir/inspection/paid_nul/type_scratch.rs", 8271, "43bebbb4746fbd9182793e37806790df77bf590983657b1653568aed31435e5e"),
    ("src/cleanup_plan/replay/continuation_storage/tests.rs", 27358, "17bb6b512db22de653583ba59d27f2f682f66dd2bca1583510e513b7688565a6"),
    ("src/cleanup_plan/replay/continuation_storage/reached.rs", 6399, "bf60981f88a31c20b7151d010bc1360461048573b9164e00dd8c9124305d9278"),
    ("src/cleanup_plan/replay/continuation_storage/reached_before.rs", 9194, "980ab986d43f41e0e3f4d7aae59190c0f6ece37ce354ad967536341911782ceb"),
    ("src/cleanup_plan/replay/continuation_storage/reached_tests.rs", 19239, "ee9bebd2537779b373c67a2e16334664cb13d594ef97761d3f987257c1f4c101"),
    ("src/cleanup_plan/replay_tests/factored.rs", 15975, "e70cd4f9a85d0711d5ab9bb84c8be8730a46e1ab506eb5fa4a9c0ee16a8f71a7"),
];
#[rustfmt::skip]
const V10_PREVIOUS_PARTS: &[(&str, usize, &str)] = &[
    ("tests/wasm/scalar_browser_ci_contract/current_profiles573.rs", 10276, "99b71451c348ba6de7f457d2c0b5c9d8c944135fe36c08e796c1840d1df49aff"),
    ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs", 10205, "900af30a08f96959e6f9c553d28e6f82f3104473cf23dbbb3090436310a22669"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs", 15950, "e4ad639ff6378b8d37a89fdddd90798d5aa7bd420f5ea2755a76c904d1d46602"),
    ("src/cleanup_plan/replay/selected_owner/ordinary.rs", 4718, "fb794e530da2d722c135a4e2a9ac5bccd755bbd12fc0114f8ed9b6f5fd0e5eaf"),
    ("src/cleanup_plan/replay/selected_owner/ordinary/tests.rs", 6987, "5bdf7210f9343246d20abc64d29ef049d632c1c991d92aff03ad29a443061302"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs", 2424, "0916bc3f12ad83a9600fb2009a4ca09fafea8c4a4cc25e6d88d712e9209c851d"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs", 86308, "755306bc0932240e81f62d01f1ec556235756c816d013d5d7f81417c4ba40417"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs", 7677, "6b870fe2d27431eade32b09e8f25703efe2ac56fbef286b544278682105ec89d"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs", 15369, "56a40d51a09c3bf21f7b4683aee142dbe29524c2c97cac97e685bf2773747549"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs", 32385, "e2d7bb7fc687c471da74528e29568efcc6eb8f48b0661778d051ba2a8652b293"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs", 24146, "a71bd98e617a681d232f9f5dc80e59db39cf0dea461a313c077f80754e1fceff"),
    ("src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs", 7076, "59e698eea731be50e9d9ecd35a5d817ec1be5aaa482444b6df38aa2ee70f18b3"),
    ("src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs", 10802, "34be6ba09bb7bac7123a1039df67948f0ab5e49bdab69d7c2da55ba8cd07b90c"),
    ("src/cleanup_plan/replay/path_join/before_frontier/mod.rs", 50333, "749400ec833f546f5c2f88c913c6c2ff5ba7fa5bea179de50d660411f9919211"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage.rs", 15694, "06fe50c24fcbb7db7763761387f990681acfa2c7963361bf6c8ed25260dbca79"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs", 2720, "ab46c653becc07f401aa8aba33fe13b39992f41491f58bc43ff608056a011b10"),
    ("src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs", 5189, "7313a9413f4e954ccc583034119763928dcd9445d595373ccf297f065a5be8cf"),
    ("src/cleanup_plan/replay/path_join/before_frontier/tests.rs", 5325, "d7841a842bf900a756905b56623fca40766ae1c3344bfb1dc4e1f86efe396df0"),
    ("src/cleanup_plan/replay/path_join/before_frontier/transition.rs", 35563, "628c1c134e179b8e8ca0db4646b6219e4c3a939968598ac2935ac500de996616"),
    ("src/cleanup_plan/replay/path_join/boxed_flow.rs", 3885, "e60f54339ffc357e805c8466ea21272bfef62531405ba2f06c0026a3ad390154"),
    ("src/cleanup_plan/replay/path_join/frontier.rs", 22575, "54dc6f27a755399709247f182124d7cd3a8f25070f07c865412587c4c0d93587"),
    ("src/cleanup_plan/replay/path_join/frontier_tests.rs", 22107, "7423729bfe25901a69b6bebbdb7f954b55a15e350975a9cd6adfe0b8a51d53bf"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", 24543, "03c5e4fe51f2925421a899860e90976665d45664c999c07b174c0233d138b747"),
    ("src/cleanup_plan/replay/path_join/frontier_tests/whole.rs", 10925, "da179f47f2f3f41eb14350b137d2468f5ac241445d6ae04d131c927da1c09908"),
    ("src/call_index/validation_before.rs", 3463, "50097cb04d8577afd15d0209fd11f14e1ff2d3d60c5bb692b8128c753b201472"),
    ("src/call_index/validation_receipt_tests.rs", 4015, "0cb673d2f420230bc3b49fa90937d83302bfd29dec44aac4e95515c758297ce7"),
    ("src/codegen/native_owned_data_provider/emit_provider_before.rs", 2513, "a4a17ce516c1c360d12f7ca0855ff5c049dfdd2248dc22c03bdc0cd25b5fe4ab"),
    ("src/codegen/native_owned_data_provider/validation_reuse_tests.rs", 7661, "bbe03233559921346ef3456859eda8f60842efa28563d6d44c9e1946fccc3cbd"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs", 2168, "5cab19b5b7c91dad5fe1240b586ac7a5f3c92896a8575b56a62bf0ed92c5b9f5"),
    ("src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs", 6801, "2c6e246527ec365687dbc3bd8339184b8ff4367c9e7ae546bda2d1920777ff60"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_census.rs", 5132, "5122d3abd1d880fb1598104c2d7367a747c668b00fa971383d38e7ff55bdee75"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_before.rs", 24587, "58dd9a33aa98e22cb8d89fbf3d47512e68dac56c10952cba055da265f5078dd8"),
    ("src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs", 14300, "141dc56360504411f085f21117ee757eee43d8e1dca7a21854a01f45f53afdeb"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs", 11246, "1f55c14d5d7fe41614e045fc680d5df8a93d4e29e4698e47bdb7e39ce8203c68"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs", 18010, "066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs", 10932, "792be2624414510de96336103ccc964f39ff7c474581aa90521be539b8e21d2d"),
    ("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs", 24998, "a017f356ed78def8d10cdcf98287cc7e30a797413fd955d9bbfb0824e712cb5a"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12.rs", 7219, "260cfb1eb837f15415a05f769e0d6554f6c26a3eeecf01a58eff16e8a2bf2446"),
    ("src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs", 9424, "6b316023a1d5f99cec3300b78bd84cd93077e1a806c774ab349aaea6356920e6"),
    ("src/workspace_graph/expected_projection/call_carriers_directory/modular.rs", 3536, "894b8a45279a1afb3b8a2e10e07888f1567ae6fb30a5f0c860ee39a5cd107d62"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs", 9819, "b5032e96c21ae66a011e08938a5dda8b366a9b495b6d421f7eb259c9e4ae3098"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs", 10077, "8031e83e8bdbe63b08131016dbee7c9cf3b6614bff75090dc88cc8f706a51167"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs", 9105, "c7a201a333ce54f18ee0982beeb3fcc13d14517804710989a832de87209e1e3b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs", 14818, "8d2fe0047c5e0f8e4359e4f2916ed0cd52e2f6d7a66e816ba2e91fb9556e01cf"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs", 12946, "342ca60956a62f93878ae04b67aab70a971012aa38839a0a307cbf37001746ba"),
    ("src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs", 12708, "2c3de671500f78caa65b3341f65ea930b50a63066773037fa42ee31c3495b0bf"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired.rs", 8606, "fe826f4e36cccd454d198513f640998934f963d136359d3063883cbb797d2c3d"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs", 14766, "a997eed41a794bb5d0e80db0703fd07c480cd40c06dc1f51b1124e023a82cbc2"),
    ("src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs", 11212, "1c34ee37bc564a908bc38cc58cc1b55a560c0f566fcc8770e6464febdab25a19"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_before.rs", 12593, "f472fe9bc7bea0097f6ae0323fcf5fa593790ec229c8499f75fb2a14deff050a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs", 12324, "7f28afbdc7fad1d150acac616a45c884495803bf6487d055ba6ff43d6c05380b"),
    ("src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs", 11968, "d588429142af0102d9d3cde1f9c2ee50f76661efe22b19c2b44a7e6d1dc95f52"),
    ("src/workspace_graph/expected_projection.rs", 59903, "6c6e07a5bc02662dbc273eac0bb9fa2cb3f580134dd208f58212fc6f64528f5e"),
    ("src/workspace_graph/expected_projection/shared_local_callees.rs", 11552, "99ab34b44805b5129051c18a14c2dcc6aa107a2ff786a53c2c2fa161c667820a"),
    ("src/workspace_graph/expected_projection/shared_local_callees/coupled_projection.rs", 14427, "f95beb41205dc47d5707ed4f189bf04dacde802016039e6b335be04b3bf213ef"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs", 27085, "f0d8a4d374f082e478bc7cdd3b3fe3575da70e3836349ebd47d87d8a04265785"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused.rs", 10317, "ed964ba39e51dfd7dbdd8c391b831260adba376c6f8dad5b035e8df36ab59fd7"),
    ("src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs", 6564, "87ecd2f33b9af73f98c499aab75e7ccb879f9c5213fb23119fc9768b05254227"),
    ("src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
    ("src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs", 13127, "1c768293781d9a11d9d1484d1b45107b722ad93664cdd46a775e269569f28577"),
    ("src/workspace_graph/expected_projection/shared_local_callees/invoice.rs", 9314, "50bfe2c1d81e5bd637a542d4254198bcd6132293c5826231fa9d08865f94643c"),
    ("src/workspace_graph/expected_projection/shared_local_callees/tests.rs", 9590, "a3079d6e5f427d20801582413905630ac9106a189bce6d49af977cf8a2133099"),
    ("src/workspace_graph/expected_projection/shared_local_callees/walk.rs", 9350, "fafdcb7e6743647fcd1ad787ea98731a159fc155e0a8d35b02541f60dd74ce17"),
    ("src/workspace_graph/expected_projection/vec_constructor.rs", 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
    ("src/workspace_graph/expected_projection/held_projection.rs", 23082, "e53a5ad24eb87ee600bd01d1916d243c137d24ae7ef7f59b005ddb7af31cbca5"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", 12722, "09a4f25c50527acf2be98d6f7cfcf4b41544340dc3bad24af858f7c8c6a077c7"),
    ("src/workspace_graph/expected_projection/call_carriers_joint.rs", 12510, "b0bc1737feb13d8936a41b2767fa6ace0b674dbe3316b6ba4371c2ad29afe8a6"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 25183, "d5f1f5027b904e45143b0a65d05a7027d987c69dce23392f06fb134aa66d75a9"),
    ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", 10291, "d2e01f03652a4389d0fb78628e14f4ac7f2080b1df41f628e0dae83036588da7"),
    ("src/workspace_graph/expected_projection/call_carriers_original.rs", 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
    ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
    ("src/workspace_graph/expected_projection/prelude_forecast.rs", 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", 20153, "3b6115c316aea6bf362fe23d8100dc5f2ea3b2163cbc4ac98e2e63c5d6d07564"),
    ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", 21032, "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187"),
    ("src/workspace_graph/expected_projection/held_projection_original.rs", 18237, "b976530c05c01bb64e685f1ed12b36d1305032cda8560778ce0f832bcc88e6f6"),
    ("src/workspace_graph/expected_projection/borrowed_leaf.rs", 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", 23207, "94aa760de11d6317cebdc638946b4d2a76d822b7b9249ea8a8789d6c681777e9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 27568, "f7ada2db950526d26e0c8e91ef90548c3a5154216b42d61e8c30a89beb978ef2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 9293, "f30d8ceea4d53421be07e2eec5610b9c5f0ac05ff06005f67c74028128157149"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
    ("src/bounded_output/key_directory.rs", 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
    ("src/bounded_output/key_directory/tests.rs", 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
    ("src/ast/program_source/receiving_roots.rs", 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
    ("src/ast/program_source/generated_roots.rs", 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
    ("src/ast/program_source.rs", 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
    ("src/ast/program_source/checked_key.rs", 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
    ("src/ast/program_source/checked_key/encode.rs", 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
    ("src/ast/program_source/decl_slices.rs", 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
    ("src/ast/program_source/decl_slices_original.rs", 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
    ("", 1, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"),
    ("src/workspace_graph/expected_projection/record_traversal.rs", 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
    ("src/workspace_graph/expected_projection/sort_keys.rs", 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
    ("src/workspace_graph/expected_projection/field_unique.rs", 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
    ("src/workspace_graph/expected_projection/field_update.rs", 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
    ("src/workspace_graph/expected_projection/string_slots.rs", 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
    ("src/workspace_graph/tests.rs", 68375, "6907e6c7112f3dd34b959de7f0d33b08821cb4ac1723040eb7aa8e71332bd2b7"),
    ("src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs", 19806, "6745c50ef06c4bab501efe623044c0044977a946df0aff8c95873a0c92de0fd1"),
    ("src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs", 30467, "79db9e907b352fe8cc96eaa3a1e87c97795436a3d566f0809af85986c118d92a"),
    ("src/cleanup_plan/replay/workspace_oracle.rs", 3669, "0cbd9a3eadf60e9611493e161e4c8692165231eaaf8c60d757070273d62a5ed0"),
    ("src/hir/inspection/paid_nul/tests.rs", 43550, "d7bc8c78fe1f8e1bed6730376ec571db9eb60c49f896304585e05ff34282ef08"),
    ("src/hir/inspection/paid_nul/reuse_before.rs", 38440, "f244990419557597356086d2b3e6005f84c1d471160eb4b2e1506b1727e1ca3f"),
    ("src/hir/inspection/paid_nul/reuse_before/pending.rs", 2490, "27622dc1bce1918fa8366f1ec97267692a655758907e270ff032156b385b0ebd"),
    ("src/hir/inspection/paid_nul/type_scratch.rs", 8271, "43bebbb4746fbd9182793e37806790df77bf590983657b1653568aed31435e5e"),
    ("src/cleanup_plan/replay/continuation_storage/tests.rs", 27358, "17bb6b512db22de653583ba59d27f2f682f66dd2bca1583510e513b7688565a6"),
    ("src/cleanup_plan/replay/continuation_storage/reached.rs", 6399, "bf60981f88a31c20b7151d010bc1360461048573b9164e00dd8c9124305d9278"),
    ("src/cleanup_plan/replay/continuation_storage/reached_before.rs", 9194, "980ab986d43f41e0e3f4d7aae59190c0f6ece37ce354ad967536341911782ceb"),
    ("src/cleanup_plan/replay/continuation_storage/reached_tests.rs", 19239, "ee9bebd2537779b373c67a2e16334664cb13d594ef97761d3f987257c1f4c101"),
    ("src/cleanup_plan/replay_tests/factored.rs", 15975, "e70cd4f9a85d0711d5ab9bb84c8be8730a46e1ab506eb5fa4a9c0ee16a8f71a7"),
];
#[rustfmt::skip]
const MAIN_PARTS: &[(&str, usize, &str)] = &[
    ("src/workspace_graph/expected_projection.rs", 59698, "cdd35c1e31d513032b5f4943ee86efdb7cce9dd4a8b77e152b4640d2ba094ab4"),
    ("src/workspace_graph/expected_projection/vec_constructor.rs", 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
    ("src/workspace_graph/expected_projection/held_projection.rs", 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", 12395, "d0f832f517a6037e1c3f8c7f5b1cca79af990ed2837b8c29b38903cb4baa6033"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 23994, "04baeff68abf890ab6bfa855969786876b0b91ef59cf685f898b829e14e0c047"),
    ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", 4106, "5a0c22d81a09e58528af9e394df7313cb79198961b6de14a9159af360666f62f"),
    ("src/workspace_graph/expected_projection/call_carriers_original.rs", 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
    ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
    ("src/workspace_graph/expected_projection/prelude_forecast.rs", 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", 12973, "1118299b35a0009ce7476507741ea9f1495b0d12191af673a852a6da7cb771af"),
    ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", 21032, "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187"),
    ("src/workspace_graph/expected_projection/held_projection_original.rs", 18237, "b976530c05c01bb64e685f1ed12b36d1305032cda8560778ce0f832bcc88e6f6"),
    ("src/workspace_graph/expected_projection/borrowed_leaf.rs", 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16948, "da6714b4bd67b30038a39e4d958849e850a75208a7b932d5268024c641711e8f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", 14941, "a6b2c250cda981f4a7130884582d3e8a4184e51819ce572ebf32677e16823ed3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", 22861, "cff55f0e3f3aa8ac3240c483bd2b90fe99a036ee184bdb472e41686e3703084f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 29466, "45400d59682efc6576eb7e45aa8f37bc9f33b1015776b7e4d9bd572f66d3836c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 9661, "ccda5de031a1b7f493181e996e1590caeaf01a44af7459fa2f99b0fe6a8e1a6f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
    ("src/bounded_output/key_directory.rs", 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
    ("src/bounded_output/key_directory/tests.rs", 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
    ("src/ast/program_source/receiving_roots.rs", 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
    ("src/ast/program_source/generated_roots.rs", 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
    ("src/ast/program_source.rs", 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
    ("src/ast/program_source/checked_key.rs", 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
    ("src/ast/program_source/checked_key/encode.rs", 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
    ("src/ast/program_source/decl_slices.rs", 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
    ("src/ast/program_source/decl_slices_original.rs", 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
    ("", 1, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"),
    ("src/workspace_graph/expected_projection/record_traversal.rs", 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
    ("src/workspace_graph/expected_projection/sort_keys.rs", 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
    ("src/workspace_graph/expected_projection/field_unique.rs", 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
    ("src/workspace_graph/expected_projection/field_update.rs", 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
    ("src/workspace_graph/expected_projection/string_slots.rs", 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
];
// Incoming constructor epoch; the entire previous MAIN_PARTS chain remains fixed.
#[rustfmt::skip]
const CONSTRUCTOR_PARTS: &[(&str, usize, &str)] = &[
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 29594, "04214dc37fb912d136d52daeb5b56cd63a88035e97b5c80be4bd31e2300b3d23"),
];
#[rustfmt::skip]
const EARLIER_PARTS: &[(&str, usize, &str)] = &[
    ("src/workspace_graph/expected_projection.rs", 59698, "cdd35c1e31d513032b5f4943ee86efdb7cce9dd4a8b77e152b4640d2ba094ab4"),
    ("src/workspace_graph/expected_projection/vec_constructor.rs", 13779, "4831e29cce4d39b5c263ffd3743d3cb2227ffc9c57f94e28763994cd78721e10"),
    ("src/workspace_graph/expected_projection/held_projection.rs", 21916, "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", 12395, "d0f832f517a6037e1c3f8c7f5b1cca79af990ed2837b8c29b38903cb4baa6033"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 23994, "04baeff68abf890ab6bfa855969786876b0b91ef59cf685f898b829e14e0c047"),
    ("src/workspace_graph/expected_projection/call_carriers_inverse.rs", 4106, "5a0c22d81a09e58528af9e394df7313cb79198961b6de14a9159af360666f62f"),
    ("src/workspace_graph/expected_projection/call_carriers_original.rs", 23564, "90f8c704f7a8eeb72cfc5318123b27febe80ce5bbba39b7b6a1952ec6e058f58"),
    ("src/workspace_graph/expected_projection/call_carriers_historical_tests.rs", 30577, "30ee9b7d27da7985809d73823ad6b2297548873c76731723659b4464c9c49170"),
    ("src/workspace_graph/expected_projection/prelude_forecast.rs", 3225, "9c1b5e17bb19fe44d33a043c157b853168d2680e02eb4dac79c3e3ba2fdd09f2"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", 12973, "1118299b35a0009ce7476507741ea9f1495b0d12191af673a852a6da7cb771af"),
    ("src/workspace_graph/expected_projection/prelude_forecast_original.rs", 21032, "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187"),
    ("src/workspace_graph/expected_projection/held_projection_original.rs", 18237, "b976530c05c01bb64e685f1ed12b36d1305032cda8560778ce0f832bcc88e6f6"),
    ("src/workspace_graph/expected_projection/borrowed_leaf.rs", 13625, "ec70684eeead756b71ac2643a36d56af6548645621349458f217b3b7397dc1dc"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/tests.rs", 9718, "5558256fbfa1461e510b4387a586bfa631c65578488db91f473913ad97983b82"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key.rs", 6184, "b4beba2c92fa9107b10e7083561f83243827da7e8a5edc4df035b571b608bc62"),
    ("src/workspace_graph/expected_projection/borrowed_leaf/checked_key/tests.rs", 6383, "a57ca67affeb834a92f1fe46faee17f14107ad79961b9ef65939da03574464d3"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16722, "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 20105, "ad172362be3655aacfef822c2834f76d08a2119d33f5774f611058ca7998792b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", 19913, "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", 24267, "97d414632c2eb68e7c1e6d27785e78a003d4ff9d439306629f23fa5a1cf256cb"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/before_alias.rs", 9900, "e3765f7769af43df16f0ddec00d1df1af56414da65a35633dc3435d3c6145f8c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/inline.rs", 9225, "30586033a207b11b600cdb0d08361a37dd78c7a159660aa645115c82b7475422"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", 41916, "f2f3e2251e1f07a737eeeca4e0f12ffea68b1612f67c59b83c2ae5f46a68da01"),
    ("src/workspace_graph/expected_projection/borrowed_imports_original.rs", 14714, "f2c4873ebc9e093e5d837490cc5b5573a282c9b7efa9fd5bb258af81910917e2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs", 2171, "b0459d75aafc04ae2d83b59e3683874e0949b9988742bcbc34cc8abeb799c4e3"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", 22861, "cff55f0e3f3aa8ac3240c483bd2b90fe99a036ee184bdb472e41686e3703084f"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests.rs", 23207, "94aa760de11d6317cebdc638946b4d2a76d822b7b9249ea8a8789d6c681777e9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/alias_helpers.rs", 13022, "15d9906bec6769fd96adc9c17d81fcfec5d05b2c6b4bd497fa6e9da3d7ba4067"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", 5964, "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 28982, "024618aff23bc6e1135b9d3c453fc660f42b59ef4dbfc2d2a01148e3ea6f3bb1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs", 27568, "f7ada2db950526d26e0c8e91ef90548c3a5154216b42d61e8c30a89beb978ef2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/initialization.rs", 6162, "f2609c5d3880aa06992de4b92657912fbb579eaa00e7b44e19bfbad96a2eafa9"),
    ("src/workspace_graph/expected_projection/borrowed_imports/receiving.rs", 9411, "175a32cb294f72f01fd465d21bd18b43ad93a06cbe209b51283f4ff6ca33c09b"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/receiving.rs", 10032, "b17f58e7c4a6e45b67deb0e685bcea5655cfac22f34d6667daf458df989975f0"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/transient.rs", 7303, "440e894c8ddcbac79d436f78bde44382308adf3e8ca885b4e2b3592c44bc6131"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 9662, "c12827317afbd1d6217092d47e6881513ada701c5aa51512842553b4d5de5145"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 9293, "f30d8ceea4d53421be07e2eec5610b9c5f0ac05ff06005f67c74028128157149"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", 8462, "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
    ("src/bounded_output/key_directory.rs", 3939, "524be62afd97f136779521a24a9fa9cbd67c2f70be6863e5ecba9f49a28fbe64"),
    ("src/bounded_output/key_directory/tests.rs", 5295, "7e89ba70b1a79f9b4fce7629caca44dc0c3c289345ede02d713adf7b1ff3bdbc"),
    ("src/ast/program_source/receiving_roots.rs", 2713, "5fcc2c44173ae171f7b9746db52c376d8045aeccf195f492e09a6a8a60f4f2ff"),
    ("src/ast/program_source/generated_roots.rs", 1654, "3a0e1d16057f700dc0babe5a8e9751b8e58cecd2c2247020082b8579d21356ea"),
    ("src/ast/program_source.rs", 8207, "2b3d504ef2cbcea751116cfb4314877034251ba6cda77dd6826a3fd37a88f855"),
    ("src/ast/program_source/checked_key.rs", 8558, "8d4bc38177d9af2ea67378d904cf4e0b53a5b9bc304c187a787970d3a1eba3a4"),
    ("src/ast/program_source/checked_key/encode.rs", 1562, "04284868282b2dc99a25490bd6f1106216498d3406b12579b753d627149235ee"),
    ("src/ast/program_source/decl_slices.rs", 9428, "3e6cbf1036e73125905bdf4ac705e876929d24cd626eb365db2ae08434f33e12"),
    ("src/ast/program_source/decl_slices_original.rs", 5965, "2eef00bc4197488d860b8176d4c223d0f747cd6a6f8a670c1ad12758980b9ced"),
    ("src/workspace_graph/expected_projection/record_traversal.rs", 3654, "6056465c93261dd5c9c99940e1f011da0816395bf0b2dfe7100afed683a2b61e"),
    ("src/workspace_graph/expected_projection/sort_keys.rs", 11331, "9cea0d3c3425766255f7e0bc0b158e42d0613f480ab4a53b72654396de266649"),
    ("src/workspace_graph/expected_projection/field_unique.rs", 7839, "8283a892cb8ce7b4ad460be23c4c80a70c4de92eed4606aec008b3ee8650f6c9"),
    ("src/workspace_graph/expected_projection/field_update.rs", 7630, "b3e1eea8dd6bf25da2e9a2e928177973070450ffe34f095ed266b21d792fcebc"),
    ("src/workspace_graph/expected_projection/string_slots.rs", 11811, "570d9033ca799f4564b0b2f9e90abd82f063747f45cb8f20a3f23a082db09a5f"),
    ("src/workspace_graph/expected_projection/call_carriers.rs", 12395, "84802a57f88f708bbc10da608f169ebedba8307677b2e6858cd8e0850c505aaa"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 23577, "bf17711b9ee9801531b16d3a505163c32df52e2ef13a46d1426eeee2cc8c3934"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 19919, "78e0166bad3ffd1a7a4594f52a9f4be0287dae4bf3d6d56f2964820df0a797f1"),
    ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", 7068, "569dca7d1273c3c4cfbd1eda7b6ed9e3909db7cd607556eddb7c515838078a5f"),
    ("src/workspace_graph/expected_projection.rs", 59451, "974468e7035d1a7baa48b13078ce7ec7b0711c2c7610d64db7feea561b192f0c"),
    ("src/workspace_graph/expected_projection/held_projection.rs", 21629, "1530f578236abe624f92941698def06f8de44d9deba0003936fb60206afe74bd"),
    ("src/workspace_graph/expected_projection/prelude_forecast_tests.rs", 10768, "ffb60b4728460a4972be5820c709d687977287aeb82ea229a8a1736114b2813d"),
    ("src/workspace_graph/expected_projection/borrowed_imports.rs", 16761, "326d155ffc918d43dd00ccbb2f1aa562e45fbfd9d0fe5b4809ac4401408c7d1c"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed.rs", 19540, "2532586630034d6c7f2953a1f487c34c25953a5b13dc86a2170bb7561a8fac36"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs", 15496, "434640ce68332a1d96840ae10b8e4a500799a567f6d75cc903a6e272196a24b2"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs", 23969, "fdd92e3f7dd1a2064a727f478bdc15f217c5c93edc9d4bb32d0d901d510ec1da"),
    ("src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs", 41674, "fb3a784a0aaad104bf580d3677d7e22078c59539a2a7c9f55d3129e39183aa79"),
    ("src/ast/program_source/decl_slices.rs", 6910, "7b86d102694e1358c2a5ce00738a882df320d3e4860ba062c855cea56c198397"),
    ("src/workspace_graph/expected_projection/call_carriers_tests.rs", 25183, "d5f1f5027b904e45143b0a65d05a7027d987c69dce23392f06fb134aa66d75a9"),
];
fn physical(source: &str) -> Option<&'static str> {
    let &(path, compiled, length, sha) = CURRENT_PARTS.iter().find(|p| p.0 == source)?;
    assert!(!path.is_empty());
    assert_eq!((compiled.len(), digest(compiled)), (length, sha.to_owned()));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = super::raw_reader::read(&root.join(path), length, sha, 2_097_152)
        .expect("whole physical merged projection source");
    assert_eq!(
        actual, compiled,
        "whole compiled and physical merged source"
    );
    Some(compiled)
}
// Only the two entire named predecessor tables may request historical bytes.
// A full current compiled/physical acquisition always precedes ordinary lookup.
pub(super) fn named_part(
    source: &str,
    length: usize,
    sha: &str,
    invocation: &mut ProjectionInvocation,
) -> Option<String> {
    if source.is_empty()
        || !V10_CURRENT_PARTS
            .iter()
            .chain(V10_PREVIOUS_PARTS)
            .chain(MAIN_PARTS)
            .chain(CONSTRUCTOR_PARTS)
            .chain(EARLIER_PARTS)
            .any(|&(p, n, h)| (p, n, h) == (source, length, sha))
    {
        return None;
    }
    let compiled = physical(source).expect("named predecessor must have a complete current owner");
    let previous = if compiled.len() == length && digest(compiled) == sha {
        compiled.to_owned()
    } else {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        invocation
            .file(&root.join(source), length, sha)
            .expect("ordinary full named predecessor and independent current source channels")
    };
    assert_eq!(
        (previous.len(), digest(&previous)),
        (length, sha.to_owned())
    );
    assert_eq!(
        physical(source),
        Some(compiled),
        "late whole physical source check"
    );
    Some(previous)
}
// The V10 loop's PARTS tuple describes its final previous image, while this
// lookup selects the distinct whole V10 current component consumed by old edits.
pub(super) fn v10_current_part(
    source: &str,
    invocation: &mut ProjectionInvocation,
) -> Option<String> {
    let &(path, length, sha) = V10_CURRENT_PARTS.iter().find(|p| p.0 == source)?;
    named_part(path, length, sha, invocation)
}
// Transport only an explicitly named historical supplied image back to its
// independently authenticated compiled current channel for an old direct call.
pub(super) fn current_for_named(
    source: &str,
    length: usize,
    sha: &str,
    supplied: &str,
) -> Option<&'static str> {
    let previous = named_part(source, length, sha, &mut ProjectionInvocation::new())?;
    let current = physical(source).unwrap();
    assert!(
        supplied == current || supplied == previous,
        "foreign named source channel"
    );
    Some(current)
}
pub(super) fn named_file(path: &std::path::Path, length: usize, sha: &str) -> Option<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?;
    if !relative
        .components()
        .all(|p| matches!(p, std::path::Component::Normal(_)))
    {
        return None;
    }
    named_part(
        relative.to_str()?,
        length,
        sha,
        &mut ProjectionInvocation::new(),
    )
}
fn named_join(parts: &[(&str, usize, &str)], invocation: &mut ProjectionInvocation) -> String {
    let mut joined = String::new();
    for &(source, length, sha) in parts {
        if source.is_empty() {
            assert_eq!(
                (length, sha),
                (
                    1,
                    "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"
                )
            );
            joined.push('\n');
        } else {
            joined.push_str(&named_part(source, length, sha, invocation).unwrap());
        }
    }
    joined
}
fn compiled_join() -> String {
    CURRENT_PARTS.iter().map(|p| p.1).collect()
}
pub(super) fn stages(
    supplied: &str,
    invocation: &mut ProjectionInvocation,
) -> Option<(String, String)> {
    if supplied.len() != CURRENT_LENGTH || digest(supplied) != CURRENT_SHA {
        return None;
    }
    let mut actual = String::new();
    for &(source, compiled, length, sha) in CURRENT_PARTS {
        assert_eq!((compiled.len(), digest(compiled)), (length, sha.to_owned()));
        if !source.is_empty() {
            assert_eq!(physical(source), Some(compiled));
        }
        actual.push_str(compiled);
    }
    assert_eq!(
        actual, supplied,
        "entire actual owning join before either old normalization"
    );
    // Authenticate the complete immediate owning join before either old chain.
    // Only this registered child changed. Its ordinary current/compiled/archive
    // acquisition restores its whole predecessor; every other joined byte and
    // its original position remain present in the independently pinned image.
    let mut immediate = String::new();
    for &(source, compiled, _, _) in CURRENT_PARTS {
        if source == "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs" {
            let prior = invocation.value_direct(source, "b156a04e239ea0dba48e30d95d8040d45f84d0100f3fb51ab6edbe67248f890a", compiled);
            assert_eq!(prior.len(), 10073);
            assert_eq!(digest(&prior), "b156a04e239ea0dba48e30d95d8040d45f84d0100f3fb51ab6edbe67248f890a");
            immediate.push_str(&prior);
        } else { immediate.push_str(compiled); }
    }
    assert_eq!((immediate.len(), digest(&immediate)), (2283206, "a298bf70cc8c5c8f6c7011d730e35c81ff8e1a2e83145ad9b60ff42eba7cf906".to_owned()));
    for &(source, compiled, _, _) in CURRENT_PARTS {
        if !source.is_empty() { assert_eq!(physical(source), Some(compiled)); }
    }
    let v10 = named_join(V10_CURRENT_PARTS, invocation);
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/projection-current-entry-20261022/whole-current.txt"
    ));
    assert_eq!(
        v10, old,
        "entire independent V10 audit predecessor, not a partial source projection"
    );
    assert_eq!(
        (v10.len(), digest(&v10)),
        (
            1855210,
            "5a90a2504295a354667e3bd7d4509a0fb58e4e672b6ac3f83ec1b951c4e2eea5".to_owned()
        )
    );
    let main = named_join(MAIN_PARTS, invocation);
    assert_eq!(
        (main.len(), digest(&main)),
        (
            652723,
            "9fe94926e99b1460c8ed81a16e503b9fbac44c9a6b833a7eed5f7697c74c3ece".to_owned()
        )
    );
    // Rebuild the complete incoming constructor subject in the same original order.
    // Each named lookup authenticates whole compiled and physical current bytes
    // before ordinary historical acquisition; the old whole-stage guard above runs first.
    let mut constructor_main = String::new();
    for &(source, length, sha) in MAIN_PARTS {
        let &(source, length, sha) = CONSTRUCTOR_PARTS
            .iter()
            .find(|part| part.0 == source)
            .unwrap_or(&(source, length, sha));
        if source.is_empty() {
            constructor_main.push('\n');
        } else {
            constructor_main.push_str(&named_part(source, length, sha, invocation).unwrap());
        }
    }
    assert_eq!(
        (constructor_main.len(), digest(&constructor_main)),
        (
            652398,
            "a51889fd387c580b3642d62510efae43d4fa5cbf4869b401cb7a2f0cf224b0d2".to_owned()
        )
    );
    assert_eq!(
        actual,
        compiled_join(),
        "all current members and order retained after historical reads"
    );
    for &(source, compiled, _, _) in CURRENT_PARTS {
        if !source.is_empty() {
            assert_eq!(physical(source), Some(compiled));
        }
    }
    assert_ne!(
        v10.len(),
        CURRENT_LENGTH,
        "historical recursion cannot select the merged stage"
    );
    assert_ne!(
        main.len(),
        CURRENT_LENGTH,
        "historical recursion cannot select the merged stage"
    );
    Some((v10, constructor_main))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_merged_join_executes_both_entire_named_predecessor_chains() {
        let current = compiled_join();
        assert_eq!(
            (current.len(), digest(&current)),
            (CURRENT_LENGTH, CURRENT_SHA.to_owned())
        );
        let (v10, main) = stages(&current, &mut ProjectionInvocation::new()).unwrap();
        assert_ne!(current, v10);
        assert_ne!(current, main);
        let actual = super::super::projection344_stage(&current);
        assert_eq!(actual, super::super::projection344_stage(&v10));
        assert_eq!(actual, super::super::projection344_stage(&main));
        let prior = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/current339-source-custody-repair-20261017/projection344-prior.whole.txt"
        ));
        assert_eq!(actual, prior, "whole original facade image retained");
    }
    #[test]
    fn merged_join_refuses_foreign_same_length_truncation_extension_and_duplicates() {
        let current = compiled_join();
        let mut changed = current.as_bytes().to_vec();
        changed[0] ^= 1;
        for wrong in [
            String::new(),
            String::from_utf8(changed).unwrap(),
            current[1..].to_owned(),
            format!("{current} "),
            format!("{current}{current}"),
        ] {
            assert!(stages(&wrong, &mut ProjectionInvocation::new()).is_none());
            assert!(
                std::panic::catch_unwind(|| super::super::projection344_stage(&wrong)).is_err()
            );
        }
    }
    #[test]
    fn named_parts_keep_complete_inverse_images_and_reject_foreign_requested_tuples() {
        let current = compiled_join();
        let (v10, main) = stages(&current, &mut ProjectionInvocation::new()).unwrap();
        assert_eq!(
            v10,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/projection-current-entry-20261022/whole-current.txt"
            ))
        );
        assert_eq!(
            super::super::projection344_constructor_subject_join_with(
                &main,
                &mut ProjectionInvocation::new(),
            )
            .expect("entire incoming constructor stage must execute its own guard"),
            named_join(MAIN_PARTS, &mut ProjectionInvocation::new())
        );
        for &(source, length, sha) in MAIN_PARTS {
            if source.is_empty() {
                continue;
            }
            let previous =
                named_part(source, length, sha, &mut ProjectionInvocation::new()).unwrap();
            assert_eq!(
                (previous.len(), digest(&previous)),
                (length, sha.to_owned())
            );
            assert!(
                named_part(source, length + 1, sha, &mut ProjectionInvocation::new()).is_none()
            );
            assert!(
                named_part(
                    source,
                    length,
                    &"0".repeat(64),
                    &mut ProjectionInvocation::new()
                )
                .is_none()
            );
        }
        assert!(named_part("unlisted", 1, CURRENT_SHA, &mut ProjectionInvocation::new()).is_none());
        let mut v10_current = String::new();
        let mut v10_previous = String::new();
        assert_eq!(V10_CURRENT_PARTS.len(), V10_PREVIOUS_PARTS.len());
        let mut different = 0;
        for (&(source, current_length, current_sha), &(prior_source, prior_length, prior_sha)) in
            V10_CURRENT_PARTS.iter().zip(V10_PREVIOUS_PARTS)
        {
            assert_eq!(source, prior_source);
            if source.is_empty() {
                v10_current.push('\n');
                v10_previous.push('\n');
                continue;
            }
            let whole_current = v10_current_part(source, &mut ProjectionInvocation::new()).unwrap();
            assert_eq!(
                (whole_current.len(), digest(&whole_current)),
                (current_length, current_sha.to_owned())
            );
            let whole_previous = super::super::projection_publication::previous_part_for_control(
                source,
                prior_length,
                prior_sha,
                &whole_current,
            );
            different += usize::from(whole_current != whole_previous);
            v10_current.push_str(&whole_current);
            v10_previous.push_str(&whole_previous);
        }
        assert_eq!(
            different, 6,
            "distinct V10 current and previous part domains"
        );
        assert_eq!(v10_current, v10);
        assert_eq!(
            (v10_previous.len(), digest(&v10_previous)),
            (
                1835716,
                "2d23fd56d8bf76d9c6f5b995d4a18bb593e2cc58d00fd0fc4f009614d75b50ef".to_owned()
            )
        );
        assert_eq!(current, compiled_join());
    }
}
