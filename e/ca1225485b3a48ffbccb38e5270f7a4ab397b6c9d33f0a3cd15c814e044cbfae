//! Fixed complete incoming Main44 metadata and exact source views; test authority only.
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub(crate) enum Fixture {
    Compact7,
    Receivers12,
}
pub(super) struct Spec {
    pub(super) path: &'static str,
    pub(super) length: usize,
    pub(super) sha256: [u8; 32],
}
impl Fixture {
    // A finite complete source epoch before the unchanged CI/Main44 inverses.
    // Actual physical current and the entire immutable predecessor are both read.
    pub(crate) fn closure_epoch<'a>(
        source: &str,
        expected: &str,
        supplied: &'a str,
    ) -> (
        std::borrow::Cow<'a, str>,
        Vec<(usize, &'static str, &'static str)>,
    ) {
        type Edit = (usize, &'static str, &'static str);
        let (compiled, length, sha, prior, prior_length, edits):
            (&str, usize, &str, &str, usize, &[Edit]) = match (source, expected) {
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs", "9dffc2821b02990bea08f9434b911c9dca98e52af5be26013bc1702916118742") => (
                include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"), 29922, "6b8839aa7294a09ffc95f9cbc9d04fc42b6e51cd611e8bbfca1bc0bb4e88dab4",
                include_str!("../../e/9dffc2821b02990bea08f9434b911c9dca98e52af5be26013bc1702916118742"), 29371, &[
                    (14267, "    // reserve_vec deliberately leaves ordinary unobserved storage to its\n    // caller. Unlike the producer's push-grown vectors, prepare must find all\n    // links allocated before traversal: capacity is a hard validation guard.\n    // Funded owners already acquired this backing above; keep their exact\n    // reservations and refusal order unchanged.\n    if budget.payer == AllocationPayer::LegacyUnobserved {\n        owner.left_cursor.links.reserve_exact(actual.max_depth);\n        owner.right_cursor.links.reserve_exact(actual.max_depth);\n    }\n", ""),
                ][..]),
            ("src/cleanup_plan/replay/factored.rs", "809181e23ff2dd185ba0f28504fa05eea592aa40e94144714c884d997d00d8cc") => (
                include_str!("../../src/cleanup_plan/replay/factored.rs"), 52363, "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0",
                include_str!("../../e/809181e23ff2dd185ba0f28504fa05eea592aa40e94144714c884d997d00d8cc"), 51339, &[
                    (9297, "    #[cfg(test)]\n    let _canonical_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::comparison_complete\",\n    );\n", ""),
                    (16880, "    #[cfg(test)]\n    let _construction_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::build_comparison\",\n    );\n", ""),
                    (17738, "        #[cfg(test)]\n        let hir_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_paths\",\n        );\n", ""),
                    (18341, "        #[cfg(test)]\n        drop(hir_allocation_phase);\n", ""),
                    (18538, "        #[cfg(test)]\n        let automaton_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_automaton\",\n        );\n", ""),
                    (20745, "        #[cfg(test)]\n        drop(automaton_allocation_phase);\n        #[cfg(test)]\n        let _plan_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_plan_automaton\",\n        );\n", ""),
                ][..]),
            ("src/cleanup_plan/replay/factored.rs", "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8") => (
                include_str!("../../src/cleanup_plan/replay/factored.rs"), 52363, "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0",
                include_str!("../../e/ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8"), 49837, &[
                    (9297, "    #[cfg(test)]\n    let _canonical_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::comparison_complete\",\n    );\n", ""),
                    (13075, "        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n", ""),
                    (13933, "            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS\n                + canonical_shared::inline_singleton::Canonicalizer::ACTIVE_DISPATCH_CONTROLS,\n", "            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,\n"),
                    (14305, "    let canonical_id = canonical.canonical_dispatch(&budget.payer);\n", ""),
                    (14658, "    let Some(hir) = canonical_id(\n        &mut canonical,\n", "    let Some(hir) = canonical.id_with_compact_receiving(\n"),
                    (14763, "        &mut *budget,\n", "        budget,\n"),
                    (14859, "    budget.reserve_bytes(\n        function,\n        canonical_shared::inline_singleton::hir_last_use::CALL,\n        \"factored HIR-private release call receiving\",\n    )?;\n    canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n", ""),
                    (15393, "    let Some(plan) = canonical_id(\n        &mut canonical,\n", "    let Some(plan) = canonical.id_with_compact_receiving(\n"),
                    (15500, "        &mut *budget,\n", "        budget,\n"),
                    (16880, "    #[cfg(test)]\n    let _construction_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::build_comparison\",\n    );\n", ""),
                    (17738, "        #[cfg(test)]\n        let hir_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_paths\",\n        );\n", ""),
                    (18341, "        #[cfg(test)]\n        drop(hir_allocation_phase);\n", ""),
                    (18538, "        #[cfg(test)]\n        let automaton_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_automaton\",\n        );\n", ""),
                    (20745, "        #[cfg(test)]\n        drop(automaton_allocation_phase);\n        #[cfg(test)]\n        let _plan_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_plan_automaton\",\n        );\n", ""),
                    (44381, "        if ordinary_plan_history::completed_fork::emission::before_selected() {\n            hir.sort();\n            plan.sort();\n            Ok(hir == plan)\n        } else {\n            enumerating.reserve_bytes(\n                function,\n                2 * skeleton_work::enumerated_prefix::ENTRY_CALL,\n                \"factored cross-check complete enumerated content receivers\",\n            )?;\n            skeleton_work::enumerated_prefix::sort(&mut hir, function, &mut enumerating)?;\n            if matches!(enumerating.payer, AllocationPayer::LegacyUnobserved) {\n                skeleton_work::enumerated_prefix::sort(&mut plan, function, &mut enumerating)?;\n            } else {\n                plan.sort();\n            }\n            skeleton_work::enumerated_prefix::equal(&hir, &plan, function, &mut enumerating)\n        }\n", "        hir.sort();\n        plan.sort();\n        Ok(hir == plan)\n"),
                    (52321, "\n#[cfg(test)]\nmod hir_memo_release_tests;\n", ""),
                ][..]),
            ("src/hir/validation/lexical_exit.rs", "5610b10bf134638d5a30dbee84058895d40263d46ac07057f154d47ff8487c0e") => (
                include_str!("../../src/hir/validation/lexical_exit.rs"), 48442, "efd93bf8154b11c5399749f8592bae4ee73617448a12a62f488d0b158bd94302",
                include_str!("../../e/5610b10bf134638d5a30dbee84058895d40263d46ac07057f154d47ff8487c0e"), 47750, &[
                    (25359, "        // Reporting storage is harness setup, outside compiler observation.\n        // Every compiler-owned diagnostic is still dropped inside the window.\n        use std::fmt::Write as _;\n        let mut diagnostic_report = String::with_capacity(1 << 16);\n", ""),
                    (25881, "                let measured = crate::test_allocation_observer::measure_with_capacity(\n                    1 << 16,\n                    || {\n", "                let measured =\n                    crate::test_allocation_observer::measure_with_capacity(1 << 16, || {\n"),
                    (26303, "                                    writeln!(&mut diagnostic_report, \"OPT904_DIAGNOSTIC phase=hir::resolve code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help).unwrap();\n", "                                    eprintln!(\"OPT904_DIAGNOSTIC phase=hir::resolve code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n"),
                    (27030, "                            writeln!(&mut diagnostic_report, \"OPT904_DIAGNOSTIC phase=hir::validate code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help).unwrap();\n", "                            eprintln!(\"OPT904_DIAGNOSTIC phase=hir::validate code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n"),
                    (27676, "                    },\n                );\n", "                    });\n"),
                    (28371, "        // Darwin stderr's process-lifetime lock belongs to reporting, never\n        // to the compiler's allocation/Drop interval.\n        if !diagnostic_report.is_empty() {\n            eprint!(\"{diagnostic_report}\");\n        }\n", ""),
                    (41252, "                format!(\n                    \"{:x}\",\n                    crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes()))\n                )\n", "                format!(\"{:x}\", crate::digest_hex::LowerHex(sha2::Sha256::digest(source.as_bytes())))\n"),
                    (44367, "            \"src/hir/validation.rs\",\n            include_str!(\"../validation.rs\"),\n        );\n", "            \"src/hir/validation.rs\", include_str!(\"../validation.rs\"));\n"),
                    (44533, "            \"src/hir/validation/lexical_exit.rs\",\n            include_str!(\"lexical_exit.rs\"),\n        );\n", "            \"src/hir/validation/lexical_exit.rs\", include_str!(\"lexical_exit.rs\"));\n"),
                ][..]),
            ("src/cleanup_plan/replay/factored.rs", "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0") => (
                include_str!("../../src/cleanup_plan/replay/factored.rs"), 52726, "8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4",
                include_str!("../../e/82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"), 52363, &[
                    (2105, "mod paired;\n", ""),
                    (9481, "    let built = build_comparison(program, function, budget, helpers, continuations)?;\n    if budget.payer == AllocationPayer::ActiveBuilder {\n        budget.reserve_bytes(\n            function,\n            paired::entry_controls(),\n            \"paired weighted comparison entry\",\n        )?;\n        if paired::positive(function, &built, budget)? == Some(true) {\n            return Ok(Some(true));\n        }\n    }\n", ""),
                    (9991, "    } = built;\n", "    } = build_comparison(program, function, budget, helpers, continuations)?;\n"),
                ][..]),
            ("src/cleanup_plan/replay/factored.rs", "8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4") => (
                include_str!("../../src/cleanup_plan/replay/factored.rs"), 55786, "1b28726cdb885c1d07033f71d0e22ae38a8c29ec04c0e314e9f2ecd3a85fa9a2",
                include_str!("../../e/8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4"), 52726, &[
                    (2637, "// Both ordinary builder strategies live with the paid comparison owner.\npub(super) use paired::{fitting_comparison_selected_paid, FITTING_SELECTOR_CONTROLS};\nuse paired::{selected_builder_paid, BUILDER_SELECTOR_CONTROLS};\n\n", ""),
                    (3002, "// Preserve the selected/reference representation crossover.\n", ""),
                    (6560, "    let mut selected = if let ReplayBudget {\n        payer: AllocationPayer::ActiveBuilder,\n        ..\n    } = budget\n    {\n        budget.reserve_bytes(\n            function,\n            BUILDER_SELECTOR_CONTROLS,\n            \"ordinary builder representation selector receivers\",\n        )?;\n        selected_builder_paid(\n            function,\n            cfg_paths,\n            semantic_paths,\n            &mut ReplayBudget::new(),\n        )?\n    } else {\n        selected_paid(\n            function,\n            cfg_paths,\n            semantic_paths,\n            &mut ReplayBudget::new(),\n        )?\n    };\n    // Keep both existing selectors above unchanged. Only the ordinary builder\n    // evaluates this nonselection guard; its two bool roles were prepaid in\n    // BUILDER_SELECTOR_CONTROLS before entering the initial selector.\n    if let ReplayBudget {\n        payer: AllocationPayer::ActiveBuilder,\n        ..\n    } = budget\n    {\n        if !selected {\n            // Every fitting header read follows payment of its own bank.\n            budget.reserve_bytes(\n                function,\n                FITTING_SELECTOR_CONTROLS,\n                \"fitting factored comparison selector receivers\",\n            )?;\n            selected =\n                fitting_comparison_selected_paid(function, cfg_paths, semantic_paths, budget)?;\n        }\n    }\n", "    let selected = selected_paid(\n        function,\n        cfg_paths,\n        semantic_paths,\n        &mut ReplayBudget::new(),\n    )?;\n"),
                    (14807, "        #[cfg(test)]\n        let hir_phase =\n            crate::test_allocation_observer::function_phase_guard(\"factored::canonical_hir_root\");\n", ""),
                    (15090, "        #[cfg(test)]\n        drop(hir_phase);\n        if let ReplayBudget {\n            payer: AllocationPayer::LegacyUnobserved,\n            ..\n        } = budget\n        {\n            budget.reserve_bytes(\n                function,\n                canonical_shared::inline_singleton::hir_last_use::STORAGE_CALL,\n                \"factored HIR-private release call receiving\",\n            )?;\n            canonical.release_hir_storage(function, budget, hir_root, plan_root)?;\n        } else {\n            budget.reserve_bytes(\n                function,\n                canonical_shared::inline_singleton::hir_last_use::CALL,\n                \"factored HIR-private release call receiving\",\n            )?;\n            canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n        }\n", "        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n"),
                    (16195, "        #[cfg(test)]\n        let _plan_phase =\n            crate::test_allocation_observer::function_phase_guard(\"factored::canonical_plan_root\");\n", ""),
                    (17347, "    #[cfg(test)]\n    let hir_phase =\n        crate::test_allocation_observer::function_phase_guard(\"factored::canonical_hir_root\");\n", ""),
                    (17680, "    #[cfg(test)]\n    drop(hir_phase);\n    if let ReplayBudget {\n        payer: AllocationPayer::LegacyUnobserved,\n        ..\n    } = budget\n    {\n        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::STORAGE_CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_storage(function, budget, hir_root, plan_root)?;\n    } else {\n        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n    }\n", "    budget.reserve_bytes(\n        function,\n        canonical_shared::inline_singleton::hir_last_use::CALL,\n        \"factored HIR-private release call receiving\",\n    )?;\n    canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n"),
                    (18681, "    #[cfg(test)]\n    let _plan_phase =\n        crate::test_allocation_observer::function_phase_guard(\"factored::canonical_plan_root\");\n", ""),
                ][..]),
            ("src/cleanup_plan/replay/factored.rs", "1b28726cdb885c1d07033f71d0e22ae38a8c29ec04c0e314e9f2ecd3a85fa9a2") => (
                include_str!("../../src/cleanup_plan/replay/factored.rs"), 55919, "a707ba025d94fa1dfcd81f44b1984ee945c3479e4e0bae6f5f5cda2dde010269",
                include_str!("../../e/1b28726cdb885c1d07033f71d0e22ae38a8c29ec04c0e314e9f2ecd3a85fa9a2"), 55786, &[
                    (2115, ";\n#[cfg(test)]\npub(super) use paired::tests::{\n    authenticate_prefitting_dispatch, prefitting_selected_paid, prefitting_validate,\n}", ""),
                ][..]),
            _ => return (std::borrow::Cow::Borrowed(supplied), Vec::new()),
        };
        let physical_compiled = compiled;
        // Compose this complete new main epoch before the unchanged four
        // acquisition records; the recursively selected fifth record is finite.
        let has_paired_epoch = source == "src/cleanup_plan/replay/factored.rs"
            && matches!(
                expected,
                "809181e23ff2dd185ba0f28504fa05eea592aa40e94144714c884d997d00d8cc"
                    | "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8"
            );
        let has_current_epoch = source == "src/cleanup_plan/replay/factored.rs"
            && expected == "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0";
        let has_export_epoch = source == "src/cleanup_plan/replay/factored.rs"
            && expected == "8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4";
        let (paired_epoch, prefix_edits) = if has_paired_epoch {
            Self::closure_epoch(
                source,
                "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0",
                supplied,
            )
        } else if has_current_epoch {
            Self::closure_epoch(
                source,
                "8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4",
                supplied,
            )
        } else if has_export_epoch {
            Self::closure_epoch(source, "1b28726cdb885c1d07033f71d0e22ae38a8c29ec04c0e314e9f2ecd3a85fa9a2", supplied)
        } else {
            (std::borrow::Cow::Borrowed(compiled), Vec::new())
        };
        let compiled = paired_epoch.as_ref();
        assert_eq!(
            supplied, physical_compiled,
            "complete current closure epoch"
        );
        assert_eq!(compiled.len(), length);
        assert_eq!(super::source_sha(compiled), sha);
        assert_eq!(prior.len(), prior_length);
        assert_eq!(super::source_sha(prior), expected);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            current_file(
                &if has_paired_epoch {
                    root.join("e/82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0")
                } else if has_current_epoch {
                    root.join("e/8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4")
                } else if has_export_epoch {
                    root.join("e/1b28726cdb885c1d07033f71d0e22ae38a8c29ec04c0e314e9f2ecd3a85fa9a2")
                } else {
                    root.join(source)
                },
                length,
                sha,
            )
            .unwrap(),
            compiled
        );
        assert_eq!(
            current_file(&root.join("e").join(expected), prior_length, expected).unwrap(),
            prior
        );
        let mut restored = compiled.to_owned();
        for &(at, after, before) in edits.iter().rev() {
            assert_eq!(restored.get(at..at + after.len()), Some(after));
            restored.replace_range(at..at + after.len(), before);
        }
        assert_eq!(restored, prior, "entire fixed closure predecessor");
        let mut composed = edits.to_vec();
        composed.extend(prefix_edits);
        (std::borrow::Cow::Owned(restored), composed)
    }
    pub(super) fn spec(self) -> Spec {
        match self {
            Self::Compact7 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main44-independent-custody-20261016/compact-inverse7.json"
                ),
                length: 64689,
                sha256: [
                    153, 168, 166, 192, 186, 118, 148, 200, 38, 143, 84, 161, 14, 105, 173, 157,
                    112, 255, 173, 144, 219, 53, 74, 198, 11, 128, 210, 106, 69, 174, 111, 128,
                ],
            },
            Self::Receivers12 => Spec {
                path: concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/main44-independent-custody-20261016/receiver-inverse12.json"
                ),
                length: 91294,
                sha256: [
                    117, 72, 249, 231, 212, 41, 231, 239, 143, 178, 92, 66, 45, 155, 38, 20, 208,
                    136, 104, 26, 99, 66, 163, 225, 25, 87, 73, 56, 29, 75, 222, 18,
                ],
            },
        }
    }
}
pub(super) fn input_spec() -> Spec {
    // MAIN44_INPUT_SPEC
    Spec {
        path: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main44-independent-custody-20261016/current-to-main44.json"
        ),
        length: 2475967,
        sha256: [
            17, 166, 197, 8, 165, 188, 199, 145, 238, 70, 80, 56, 250, 161, 135, 87, 222, 41, 20,
            69, 106, 215, 193, 87, 226, 47, 233, 72, 68, 135, 177, 143,
        ],
    }
}
pub(super) fn input(source: &str, current: &str) -> String {
    const KEYS: &[&str] = &[
        "src/cleanup_plan/replay.rs",
        "src/cleanup_plan/replay/branch_clone/tests.rs",
        "src/cleanup_plan/replay/compact_stage/before.rs",
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs",
        "src/cleanup_plan/replay/factored.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs",
        "src/cleanup_plan/replay/factored/oracle_hook.rs",
        "src/cleanup_plan/replay/path_array.rs",
        "src/cleanup_plan/replay/path_array/before.rs",
        "src/cleanup_plan/replay/path_array/final_before.rs",
        "src/cleanup_plan/replay/path_array/sequence.rs",
        "src/cleanup_plan/replay/path_array/sequence/before.rs",
        "src/cleanup_plan/replay/path_array/sequence/tests.rs",
        "src/cleanup_plan/replay/path_array/tests.rs",
        "src/cleanup_plan/replay/sequence_prefix/before.rs",
        "src/cleanup_plan/replay/sequence_vector.rs",
        "src/cleanup_plan/replay/sequence_vector/before.rs",
        "src/cleanup_plan/replay/sequence_vector/tests.rs",
        "src/cleanup_plan/replay/skeleton_work.rs",
        "src/hir.rs",
        "src/hir/expr_nodes.rs",
        "src/hir/validation.rs",
        "src/source_verify/declaration.rs",
        "tests/hir_module_boundaries.rs",
        "tests/workspace/semantic_graph/source_inventory.rs",
    ];
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
    let metadata = super::runtime_json::main44_input();
    let acquired = current_entry(source, current);
    super::project(&metadata, source, &acquired)
}

// Only the original 26-key owner reaches this ingress. The fixed metadata
// authenticates first; every original full inverse and final hash runs after it.
fn current_entry(source: &str, supplied: &str) -> String {
    let compiled = crate::cleanup_plan::main44_current_source(source)
        .expect("original compiled Main44 source domain");
    let (archive, length, expected) = match source {
        "src/cleanup_plan/replay.rs" => ("docs/evidence/main44-current-entry-20261022/entries/807735eb8713ff6e02abd9ea6d3d4aea4fdc53f95bc6b6d1a2a1e51dba605afa.txt", 323567, "807735eb8713ff6e02abd9ea6d3d4aea4fdc53f95bc6b6d1a2a1e51dba605afa"),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f.txt", 23912, "46bf0451ef390225f540d031f326100c7f4288cf7e9d50e30ce7ddb68a448e0f"),
        "src/cleanup_plan/replay/compact_stage/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/47fc6ffa22a2a47aab804971a0c483310cf506591c993fde481c712882d4c699.txt", 21938, "47fc6ffa22a2a47aab804971a0c483310cf506591c993fde481c712882d4c699"),
        "src/cleanup_plan/replay/compact_stage/lexical_before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/2523a5f847df2d945fd324ea64908670b4c121f684f3274ca0b2de174ed7d523.txt", 960, "2523a5f847df2d945fd324ea64908670b4c121f684f3274ca0b2de174ed7d523"),
        "src/cleanup_plan/replay/factored.rs" => ("docs/evidence/main44-current-entry-20261022/entries/343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6.txt", 44197, "343eda3712d0c3b224742d72c34838a327e0c69449c8a94fd7eb81dc5e0134a6"),
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => ("docs/evidence/main44-current-entry-20261022/entries/01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1.txt", 8266, "01b3e408874930f2aad015d20cf8317fc1507183f92aae740d11985b99d223e1"),
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/a5f456bd1d6f7fc43c08baf8f192d4c81f6efb036b6d35d8a15f9d785eb06b46.txt", 15541, "a5f456bd1d6f7fc43c08baf8f192d4c81f6efb036b6d35d8a15f9d785eb06b46"),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => ("docs/evidence/main44-current-entry-20261022/entries/0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae.txt", 11716, "0126d25150ca4b96f11995293fc9043d23fc8fff2008485e9560abe5c85cb7ae"),
        "src/cleanup_plan/replay/path_array.rs" => ("docs/evidence/main44-current-entry-20261022/entries/adaca15c3ae85005ba22ad48351f41595ef30e6489d6a7e8d1b369c7e0f8bac3.txt", 7342, "adaca15c3ae85005ba22ad48351f41595ef30e6489d6a7e8d1b369c7e0f8bac3"),
        "src/cleanup_plan/replay/path_array/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/1e6f9b2f9d6252d0c24a70584d7ba67d203ff01d611ff381810c3fb8415dc7fb.txt", 5041, "1e6f9b2f9d6252d0c24a70584d7ba67d203ff01d611ff381810c3fb8415dc7fb"),
        "src/cleanup_plan/replay/path_array/final_before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e866a597b35940a2b37c8449c98ac85ef89472f48c1c21cfbd50be7be7c9709e.txt", 3894, "e866a597b35940a2b37c8449c98ac85ef89472f48c1c21cfbd50be7be7c9709e"),
        "src/cleanup_plan/replay/path_array/sequence.rs" => ("docs/evidence/main44-current-entry-20261022/entries/8f0da852b94f3e8ecd001a528131ffcab6ea2be287892dece6c6711765a30984.txt", 2991, "8f0da852b94f3e8ecd001a528131ffcab6ea2be287892dece6c6711765a30984"),
        "src/cleanup_plan/replay/path_array/sequence/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/82f9459f88d10e960903c8001c366c6b057e8818aa613c441d7f9e13e7678314.txt", 3490, "82f9459f88d10e960903c8001c366c6b057e8818aa613c441d7f9e13e7678314"),
        "src/cleanup_plan/replay/path_array/sequence/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/53df021d2239f2754fd0c0c15fbb710d2402d8cf4c2ec4f8c193a63b5b2b5883.txt", 16543, "53df021d2239f2754fd0c0c15fbb710d2402d8cf4c2ec4f8c193a63b5b2b5883"),
        "src/cleanup_plan/replay/path_array/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/b641a21d3e5ba8f517f79b3184f4bc3496fa32c74e3b36951877fd145b528619.txt", 14913, "b641a21d3e5ba8f517f79b3184f4bc3496fa32c74e3b36951877fd145b528619"),
        "src/cleanup_plan/replay/sequence_prefix/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/268d548459b7ef72cc260ece1570f1dffc3fbd320146f1941af103b18a87b7a8.txt", 2070, "268d548459b7ef72cc260ece1570f1dffc3fbd320146f1941af103b18a87b7a8"),
        "src/cleanup_plan/replay/sequence_vector.rs" => ("docs/evidence/main44-current-entry-20261022/entries/6129acfd3338cbd3178cf4440c1056762dbc7a666774aab7896a55e6aada7d6b.txt", 10668, "6129acfd3338cbd3178cf4440c1056762dbc7a666774aab7896a55e6aada7d6b"),
        "src/cleanup_plan/replay/sequence_vector/before.rs" => ("docs/evidence/main44-current-entry-20261022/entries/9585d39576779e49265c8a75536e8b772ba0e41749f1ed9cc61639c4890c2bd7.txt", 3019, "9585d39576779e49265c8a75536e8b772ba0e41749f1ed9cc61639c4890c2bd7"),
        "src/cleanup_plan/replay/sequence_vector/tests.rs" => ("docs/evidence/main44-current-entry-20261022/entries/9199f60a9fa34805f8de4ca6013dfa1931c668ac0fa2fdd3ff8f98a644694ffe.txt", 18626, "9199f60a9fa34805f8de4ca6013dfa1931c668ac0fa2fdd3ff8f98a644694ffe"),
        "src/cleanup_plan/replay/skeleton_work.rs" => ("docs/evidence/main44-current-entry-20261022/entries/2532173c152df743989e4afd5fc84ae4b3dd32bf45b0a72fe09039a832887c4b.txt", 47594, "2532173c152df743989e4afd5fc84ae4b3dd32bf45b0a72fe09039a832887c4b"),
        "src/hir.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04.txt", 58967, "e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04"),
        "src/hir/expr_nodes.rs" => ("docs/evidence/main44-current-entry-20261022/entries/e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f.txt", 38066, "e2291267786bdec4371b969031209b4e77aa6876f4564048ad1c7561b004b73f"),
        "src/hir/validation.rs" => ("docs/evidence/main44-current-entry-20261022/entries/f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba.txt", 399117, "f89e882a3d8ed796b8b8b864247e4e7eb19631d6299cd0dd94ddcc6b5ca17eba"),
        "src/source_verify/declaration.rs" => ("docs/evidence/main44-current-entry-20261022/entries/54bca5bc3be89d2b788655f72cb8671c76b540edd99c126a0862a4f1f22a0a88.txt", 15793, "54bca5bc3be89d2b788655f72cb8671c76b540edd99c126a0862a4f1f22a0a88"),
        "tests/hir_module_boundaries.rs" => ("docs/evidence/main44-current-entry-20261022/entries/5090d08b52cb9650af5fc2c2a28739a9783c3d43c7a0f25037ce032b04f51137.txt", 98174, "5090d08b52cb9650af5fc2c2a28739a9783c3d43c7a0f25037ce032b04f51137"),
        "tests/workspace/semantic_graph/source_inventory.rs" => ("docs/evidence/main44-current-entry-20261022/entries/aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989.txt", 91318, "aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989"),
        _ => unreachable!("original Main44 key checked before acquisition"),
    };
    assert!(
        supplied == compiled
            || (supplied.len() == length && super::source_sha(supplied) == expected),
        "compiled current or exact sealed Main44 entry: {source}"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let current_hash = super::source_sha(compiled);
    let physical_source = match source {
        "src/cleanup_plan/replay/sequence_vector/before.rs" => {
            "src/cleanup_plan/replay/path_array/main44/vector_before.rs"
        }
        "src/cleanup_plan/replay/sequence_vector/tests.rs" => {
            "src/cleanup_plan/replay/path_array/main44/vector_tests.rs"
        }
        _ => source,
    };
    let physical = current_file(&root.join(physical_source), compiled.len(), &current_hash)
        .expect("complete no-follow current Main44 owner");
    assert_eq!(
        physical, compiled,
        "physical/compiled Main44 owner: {source}"
    );
    current_file(&root.join(archive), length, expected).expect("complete sealed Main44 entry")
}

fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.mode() != b.mode()
            || a.mtime() != b.mtime()
            || a.mtime_nsec() != b.mtime_nsec()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}
fn current_file(
    path: &std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    use std::io::Read;
    if length > 1_048_576 {
        return Err("bound");
    }
    let before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err("nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let opened = file.metadata().map_err(|_| "metadata")?;
    if !same_file(&before, &opened) || opened.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-identity");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    let mut limited = (&mut file).take(length as u64 + 1);
    limited.read_to_end(&mut bytes).map_err(|_| "read")?;
    if bytes.len() != length
        || format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes))) != expected
    {
        return Err("length-or-digest");
    }
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !same_file(&opened, &after)
        || !same_file(&after, &path_after)
    {
        return Err("identity-drift");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}

// Closed diagnostic-only scopes: authenticate whole current source before
// restoring the exact immediate production image for every older source audit.
pub(crate) fn before_phase_probes<'a>(
    source: &str,
    supplied: &'a str,
) -> std::borrow::Cow<'a, str> {
    let (compiled, length, expected, prior_length, prior_sha, edits) = match source {
        "src/cleanup_plan/validate.rs" => (include_str!("../../src/cleanup_plan/validate.rs"), 4475, "75ac0859815d3a47a94b81493c04a8ae89516f4939fe79a8da7f94d141e41c1b", 4269, "794785605a074c9c2b80f48e665e6e0a03723a9ace4802c88561ca084bb7632e", &[
            ("        crate::workflow_profile::plan_comparison::PaidPhase::CleanupValidation,\n    );\n\n    #[cfg(test)]\n    let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::validate::canonical_rebuild\",\n    );\n    let mut frontier = crate::loop_calls::byte_writer::validation_frontier::Frontier::new(program);\n    for function in &program.functions {\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n", "        crate::workflow_profile::plan_comparison::PaidPhase::CleanupValidation,\n    );\n\n    let mut frontier = crate::loop_calls::byte_writer::validation_frontier::Frontier::new(program);\n    for function in &program.functions {\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n"),
            ("        frontier.advance()?;\n    }\n    drop(frontier);\n    #[cfg(test)]\n    drop(_allocation_phase);\n    // Complete fresh independent replay never borrows frontier proofs.\n    super::replay::validate_program(program)\n}\n", "        frontier.advance()?;\n    }\n    drop(frontier);\n    // Complete fresh independent replay never borrows frontier proofs.\n    super::replay::validate_program(program)\n}\n"),
        ][..]),
        "src/cleanup_plan/replay.rs" => (include_str!("../../src/cleanup_plan/replay.rs"), 326632, "ef98c1c3ee202b9802fb9040036e36ddb7a7e412cdbd8f6a627be1286400ac97", 326351, "c4eb6d914f71ea5ea5ec3cee2710b4c18fee4c45b2c9019e8b48b5fb46653bbb", &[
            ("use expression_children::replay_expression_child;\nuse expression_facts::expression_facts;\nuse std::collections::{BTreeMap, BTreeSet, VecDeque};\n#[cfg(test)]\nuse crate::hir::ResolvedTypeDeclarationKind;\n#[cfg(test)]\nuse std::cell::Cell;\nuse super::{\n    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,\n    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StagedCopyResultSource,\n", "use expression_children::replay_expression_child;\nuse expression_facts::expression_facts;\nuse std::collections::{BTreeMap, BTreeSet, VecDeque};\n\n#[cfg(test)]\nuse crate::hir::ResolvedTypeDeclarationKind;\n#[cfg(test)]\nuse std::cell::Cell;\n\nuse super::{\n    BlockId, CleanupPlace, CleanupRegionId, CleanupResultSource, CleanupTerminator,\n    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StagedCopyResultSource,\n"),
            ("#[cfg(test)]\nuse path_summary::{cleanup_plan_requires_path_replay, STATUS_ONLY_PATH_SUMMARY_THRESHOLD};\nuse skeleton_work::{empty_expr_path, Observations, SkeletonWork};\n#[cfg(test)]\nmod context_tests;\n#[cfg(test)]\n", "#[cfg(test)]\nuse path_summary::{cleanup_plan_requires_path_replay, STATUS_ONLY_PATH_SUMMARY_THRESHOLD};\nuse skeleton_work::{empty_expr_path, Observations, SkeletonWork};\n\n#[cfg(test)]\nmod context_tests;\n#[cfg(test)]\n"),
            ("    resolved_call_params, seal_changed_success_try_residual as seal_try_residual,\n};\npub(crate) use schema::selected_schema;\n/// Provisional normal-scope end of a region whose independently proven flow has\n/// no normal completion. Only the lexical certificate replay may accept it.\nconst UNRESOLVED_NORMAL_END: super::ExitTargetId = super::ExitTargetId(u32::MAX);\nconst MAX_REPLAY_PATHS: usize = 65_536;\nconst MAX_REPLAY_WORK_UNITS: usize = 32_000_000;\nmod allocation;\nuse allocation::{AdmissionMode, AllocationPayer};\n// Setup identity for the direct selected-budget compatibility receiver only.\n// General Session spending remains multi-function; this is not admission data.\nstruct SelectedDirectAnchor {\n", "    resolved_call_params, seal_changed_success_try_residual as seal_try_residual,\n};\npub(crate) use schema::selected_schema;\n\n/// Provisional normal-scope end of a region whose independently proven flow has\n/// no normal completion. Only the lexical certificate replay may accept it.\nconst UNRESOLVED_NORMAL_END: super::ExitTargetId = super::ExitTargetId(u32::MAX);\nconst MAX_REPLAY_PATHS: usize = 65_536;\nconst MAX_REPLAY_WORK_UNITS: usize = 32_000_000;\n\nmod allocation;\nuse allocation::{AdmissionMode, AllocationPayer};\n\n// Setup identity for the direct selected-budget compatibility receiver only.\n// General Session spending remains multi-function; this is not admission data.\nstruct SelectedDirectAnchor {\n"),
            ("    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,\n    continuations: Option<&mut continuation_storage::Storage<'hir>>,\n) -> Result<Vec<SkeletonPath>, Diagnostic> {\n    #[cfg(test)]\n    let _allocation_phase =\n        crate::test_allocation_observer::function_phase_guard(\"cleanup_plan::replay::hir_paths\");\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _paid_phase = budget.paid_phase(PaidPhase::HirPaths);\n\n", "    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,\n    continuations: Option<&mut continuation_storage::Storage<'hir>>,\n) -> Result<Vec<SkeletonPath>, Diagnostic> {\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _paid_phase = budget.paid_phase(PaidPhase::HirPaths);\n\n"),
            ("    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<Vec<SkeletonPath>, Diagnostic> {\n    #[cfg(test)]\n    let _allocation_phase =\n        crate::test_allocation_observer::function_phase_guard(\"cleanup_plan::replay::plan_paths\");\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _paid_phase = budget.paid_phase(PaidPhase::PlanPaths);\n\n", "    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<Vec<SkeletonPath>, Diagnostic> {\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _paid_phase = budget.paid_phase(PaidPhase::PlanPaths);\n\n"),
        ][..]),
        "src/cleanup_plan/replay/factored.rs" => (include_str!("../../src/cleanup_plan/replay/factored.rs"), 48944, "e70c71d81ddf3c10d9edc1251e2bb58e107e4a0b35d0b3dcca3915388816175b", 48612, "d3808da9519009c1a7b223a91a635bc94cdeb80a2c8bc40b22bcfc8b1f2f918c", &[
            ("    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let paid_enumeration = enumerating\n        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);\n    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::crosscheck::enumerated_hir_plan\",\n    );\n    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {\n        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);\n        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;\n", "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let paid_enumeration = enumerating\n        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);\n    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {\n        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);\n        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;\n"),
            ("        plan.sort();\n        Ok(hir == plan)\n    });\n    drop(enumeration_phase);\n    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    drop(paid_enumeration);\n    if let Ok(true) = enumerated {\n        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::crosscheck::factored_compare\",\n        );\n        let mut factored = ReplayBudget::with_skeleton_limit(0);\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_crosscheck = factored\n", "        plan.sort();\n        Ok(hir == plan)\n    });\n    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    drop(paid_enumeration);\n    if let Ok(true) = enumerated {\n        let mut factored = ReplayBudget::with_skeleton_limit(0);\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_crosscheck = factored\n"),
        ][..]),
        "src/cleanup_plan/replay/factored/continuation_receiver.rs" => (include_str!("../../src/cleanup_plan/replay/factored/continuation_receiver.rs"), 2919, "f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a", 2587, "4d18c9761da6c9f3e8226706b5c5720ced45f845eda3d8d19d017f455af2a41d", &[
            ("    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let paid_enumeration = enumerating\n        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);\n    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::crosscheck::enumerated_hir_plan\",\n    );\n    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {\n        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);\n        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;\n", "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let paid_enumeration = enumerating\n        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);\n    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {\n        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);\n        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;\n"),
            ("        plan.sort();\n        Ok(hir == plan)\n    });\n    drop(enumeration_phase);\n    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    drop(paid_enumeration);\n    if let Ok(true) = enumerated {\n        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::crosscheck::factored_compare\",\n        );\n        let mut factored = ReplayBudget::with_skeleton_limit(0);\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_crosscheck = factored\n", "        plan.sort();\n        Ok(hir == plan)\n    });\n    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    drop(paid_enumeration);\n    if let Ok(true) = enumerated {\n        let mut factored = ReplayBudget::with_skeleton_limit(0);\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_crosscheck = factored\n"),
        ][..]),
        _ => return std::borrow::Cow::Borrowed(supplied),
    };
    if supplied != compiled {
        return std::borrow::Cow::Borrowed(supplied);
    }
    // Authenticate the exact early-release epoch before the unchanged incoming
    // main and diagnostic-probe inverses; no predecessor pins move.
    let physical_compiled = compiled;
    let (closure_epoch, _) = Fixture::closure_epoch(
        source,
        "809181e23ff2dd185ba0f28504fa05eea592aa40e94144714c884d997d00d8cc",
        compiled,
    );
    let compiled = closure_epoch.as_ref();
    let workspace_epoch = if source == "src/cleanup_plan/replay.rs" {
        assert_eq!(compiled.len(), 327551);
        assert_eq!(
            super::source_sha(compiled),
            "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"
        );
        let addition = "#[cfg(test)]\nmod workspace_oracle;\n#[cfg(test)]\npub(crate) use factored::{EpsilonAppCensus, SuccessorAppCensus};\n#[cfg(test)]\npub(crate) use workspace_oracle::with_financed_workspace_test_oracles;\nmod selected_type_drop;\n";
        assert_eq!(compiled.matches(addition).count(), 1);
        let prior = compiled.replacen(addition, "\n#[cfg(test)]\npub(crate) use factored::{EpsilonAppCensus, SuccessorAppCensus};\n\nmod selected_type_drop;\n\n", 1);
        assert_eq!(prior.len(), 327435);
        assert_eq!(
            super::source_sha(&prior),
            "0aeeb9967f3272bfc1371131cba43dc39e5bb7b155eae28500512de0c34af387"
        );
        std::borrow::Cow::Owned(prior)
    } else {
        std::borrow::Cow::Borrowed(compiled)
    };
    let compiled = workspace_epoch.as_ref();
    let receiving_epoch = if source == "src/cleanup_plan/replay/factored/continuation_receiver.rs" {
        assert_eq!(compiled.len(), 3211);
        assert_eq!(super::source_sha(compiled), "57fb8fa2a03008f6088a2891972a8c3ff17d4eea9eec508809e898eb85a5e60b");
        let mut prior = compiled.to_owned();
        for &(at, after, before) in &[
            (1631, "\n        }", ""),
            (1612, "    ", ""),
            (1579, "    ", ""),
            (1293, "if matches!(enumerating.payer, AllocationPayer::LegacyUnobserved)\n            && !ordinary_plan_history::completed_fork::emission::before_selected()\n        {\n            plan_prefix::equal_paths(function, &mut enumerating, &mut hir, &mut plan)\n        } else {\n            ", ""),
        ] {
            assert_eq!(prior.get(at..at + after.len()), Some(after));
            prior.replace_range(at..at + after.len(), before);
        }
        let fixture = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a"));
        assert_eq!(prior, fixture); assert_eq!(prior.len(), 2919);
        assert_eq!(super::source_sha(&prior), "f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a");
        std::borrow::Cow::Owned(prior)
    } else { std::borrow::Cow::Borrowed(compiled) };
    let compiled = receiving_epoch.as_ref();
    // Each later implementation is acquired in full before the unchanged
    // incoming-main and diagnostic-probe epochs below. Fixed endpoints remain
    // the complete independently retained Git source images.
    let implementation_epoch = match source {
        "src/cleanup_plan/replay.rs" => {
            assert_eq!(compiled.len(), 327435);
            assert_eq!(
                super::source_sha(compiled),
                "0aeeb9967f3272bfc1371131cba43dc39e5bb7b155eae28500512de0c34af387"
            );
            let mut prior = compiled.to_owned();
            for &(at, after, before) in [
                (1097, "mod plan_prefix;\n", ""),
                (108404, "#[rustfmt::skip]\n", ""),
                (109364, "    ordinary_plan_history::completed_fork::shipping::finish!(function, budget, expected);\n", ""),
                (112083, "        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\n", ""),
                (116424, "        budget.reserve_bytes(function, skeleton_work::query_proof::HEADER, \"skeleton query neutral slot receivers\")?;\n        SkeletonWork { queries: skeleton_work::query_proof::Slot::new(function, budget),\n", "        SkeletonWork {\n"),
                (122436, "    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(\n        prefixes, suffixes, work\n    );\n", ""),
                (122660, "    let share_prefix = work.prefix_tail_sharing(&suffixes)?;\n", "    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n"),
                (123169, "        sequence_vector::reuse!(prefixes, suffixes, work, token share_prefix, minimum, precharged);\n", "        sequence_vector::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged);\n"),
                (123805, "        if !precharged {\n            skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);\n", "        if !precharged && share_prefix {\n            work.share_prefix_tail(&mut prefix.observations)?;\n"),
                (269073, "    let share_prefix = work.prefix_tail_sharing(&suffixes)?;\n", "    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n"),
                (269543, "        skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);\n\n", "        if share_prefix {\n            work.share_prefix_tail(&mut prefix.observations)?;\n        }\n"),
                (269792, "            let observations = ordinary_plan_history::completed_fork::emission::extend_call!(\n                observations, suffix, work\n", "            let observations = work.extend_owned_observations(\n                observations,\n                &suffix.observations,\n                \"call suffix clone\",\n"),
                (279166, "#[rustfmt::skip]\n", ""),
                (279316, "    budget.reserve_bytes(function, plan_prefix::QUERY_CONTROLS, \"PLAN prefix eligibility receivers\")?;\n    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\n", ""),
                (289554, "                compact_path::plan_selection!(budget, function);\n", "                budget.reserve_bytes(\n                    function,\n                    branch_clone::direct::PLAN_SMALL_SELECTION_CONTROLS,\n                    \"plan final-output selection controls\",\n                )?;\n"),
                (289948, "                        physical_budget::compact_try!(budget.reserve_compact_bytes(\n", "                        budget.reserve_bytes(\n"),
                (290070, "                            branch_clone::direct::plan_compact_roles::<\n", "                            branch_clone::direct::compact_roles::<\n"),
                (290357, "                        ));\n", "                        )?;\n"),
                (291020, "                        physical_budget::compact_try!(budget.reserve_compact_bytes(\n", "                        budget.reserve_bytes(\n"),
                (291142, "                            branch_clone::plan_compact_roles::<std::rc::Rc<SkeletonObservation>>(),\n", "                            branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),\n"),
                (291305, "                        ));\n", "                        )?;\n"),
                (291645, "                        physical_budget::compact_try!(budget.reserve_compact_bytes(\n", "                        budget.reserve_bytes(\n"),
                (291767, "                            branch_clone::direct::plan_compact_roles::<\n", "                            branch_clone::direct::compact_roles::<\n"),
                (292054, "                        ));\n", "                        )?;\n"),
                (292911, "                    compact_path::plan_move!(budget, function);\n                    compact_path::plan_work!(\n                        budget,\n", "                    budget.reserve_bytes(\n"),
                (293087, "                        \"cleanup-plan skeleton branch clone\"\n                    );\n", "                        final_output::PLAN_MOVE_CONTROLS,\n                        \"plan final-output move controls\",\n                    )?;\n                    budget.charge_skeleton(function, 1, \"cleanup-plan skeleton branch clone\")?;\n"),
                (293480, "                    compact_path::plan_work!(budget, function, \"plan branch observation push\");\n", "                    budget.charge_skeleton(function, 1, \"plan branch observation push\")?;\n"),
                (293629, "                    compact_path::plan_auth!(budget, function);\n                    compact_path::plan_conversion!(budget, function, observation);\n", "                    budget.reserve_bytes(function, 0, \"plan branch observation push\")?;\n                    budget.reserve_bytes(\n                        function,\n                        physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(\n                            &observation,\n                        ),\n                        \"plan branch observation push\",\n                    )?;\n"),
                (297849, "                        branch_clone::short_plan::produce!(\n                            budget,\n                            function,\n                            observations,\n                            observation,\n                            edge,\n                            queue\n                        );\n", ""),
            ].iter().rev() {
                assert_eq!(prior.get(at..at + after.len()), Some(after));
                prior.replace_range(at..at + after.len(), before);
            }
            assert_eq!(prior.len(), 326426);
            assert_eq!(
                super::source_sha(&prior),
                "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
            );
            std::borrow::Cow::Owned(prior)
        }
        "src/cleanup_plan/replay/factored.rs" => {
            assert_eq!(compiled.len(), 51339);
            assert_eq!(
                super::source_sha(compiled),
                "809181e23ff2dd185ba0f28504fa05eea592aa40e94144714c884d997d00d8cc"
            );
            let mut prior = compiled.to_owned();
            for &(at, after, before) in [
                (12903, "        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n", ""),
                (13761, "            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS\n                + canonical_shared::inline_singleton::Canonicalizer::ACTIVE_DISPATCH_CONTROLS,\n", "            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,\n"),
                (14133, "    let canonical_id = canonical.canonical_dispatch(&budget.payer);\n", ""),
                (14486, "    let Some(hir) = canonical_id(\n        &mut canonical,\n", "    let Some(hir) = canonical.id_with_compact_receiving(\n"),
                (14591, "        &mut *budget,\n", "        budget,\n"),
                (14687, "    budget.reserve_bytes(\n        function,\n        canonical_shared::inline_singleton::hir_last_use::CALL,\n        \"factored HIR-private release call receiving\",\n    )?;\n    canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n", ""),
                (15221, "    let Some(plan) = canonical_id(\n        &mut canonical,\n", "    let Some(plan) = canonical.id_with_compact_receiving(\n"),
                (15328, "        &mut *budget,\n", "        budget,\n"),
                (43357, "        if ordinary_plan_history::completed_fork::emission::before_selected() {\n            hir.sort();\n            plan.sort();\n            Ok(hir == plan)\n        } else {\n            enumerating.reserve_bytes(\n                function,\n                2 * skeleton_work::enumerated_prefix::ENTRY_CALL,\n                \"factored cross-check complete enumerated content receivers\",\n            )?;\n            skeleton_work::enumerated_prefix::sort(&mut hir, function, &mut enumerating)?;\n            if matches!(enumerating.payer, AllocationPayer::LegacyUnobserved) {\n                skeleton_work::enumerated_prefix::sort(&mut plan, function, &mut enumerating)?;\n            } else {\n                plan.sort();\n            }\n            skeleton_work::enumerated_prefix::equal(&hir, &plan, function, &mut enumerating)\n        }\n", "        hir.sort();\n        plan.sort();\n        Ok(hir == plan)\n"),
                (51297, "\n#[cfg(test)]\nmod hir_memo_release_tests;\n", ""),
            ].iter().rev() {
                assert_eq!(prior.get(at..at + after.len()), Some(after));
                prior.replace_range(at..at + after.len(), before);
            }
            assert_eq!(prior.len(), 49837);
            assert_eq!(
                super::source_sha(&prior),
                "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8"
            );
            std::borrow::Cow::Owned(prior)
        }
        _ => std::borrow::Cow::Borrowed(compiled),
    };
    let compiled = implementation_epoch.as_ref();
    let early_release_epoch = if source == "src/cleanup_plan/replay/factored.rs" {
        assert_eq!(compiled.len(), 49837);
        assert_eq!(
            super::source_sha(compiled),
            "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8"
        );
        let mut prior = compiled.to_owned();
        for (current, previous) in [
            ("#[cfg(test)]\nmod early_release;\n", ""),
            ("        // Automaton labels now own every retained observation. Each explicit\n        // release pays its argument/formal and reservation-result transport.\n        #[cfg(test)]\n        early_release::frontier(0, budget);\n        budget.reserve_array::<(dag_memo::Memo, dag_memo::Memo, Result<(), Diagnostic>)>(\n            function,\n            1,\n            \"factored DAG memo release transport\",\n        )?;\n        drop(memo);\n        #[cfg(test)]\n        early_release::frontier(1, budget);\n        budget.reserve_array::<(Vec<SkeletonPath>, Vec<SkeletonPath>, Result<(), Diagnostic>)>(\n            function,\n            1,\n            \"factored HIR paths release transport\",\n        )?;\n        drop(hir_paths);\n        #[cfg(test)]\n        early_release::frontier(2, budget);\n", ""),
        ] {
            assert_eq!(prior.matches(current).count(), 1);
            prior = prior.replacen(current, previous, 1);
        }
        assert_eq!(prior.len(), 49022);
        assert_eq!(
            super::source_sha(&prior),
            "3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02"
        );
        std::borrow::Cow::Owned(prior)
    } else {
        std::borrow::Cow::Borrowed(compiled)
    };
    let compiled = early_release_epoch.as_ref();
    // The incoming production changes are independently authenticated, then
    // reversed only for this older probe/source epoch. Its original pins and
    // every diagnostic-scope inverse below remain unchanged.
    let (live_length, live_sha, merge_edits): (usize, &str, &[(usize, &str, &str)]) = match source {
        "src/cleanup_plan/replay.rs" => (326426, "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d", &[
            (576, "use final_output::compact as compact_path;\n", ""),
            (122062, "    let minimum = compact_path::count!(\n", "    let minimum = path_array::count!(\n"),
            (122331, "    final_output::compact::hir_selection!(work);\n", "    work.budget.reserve_bytes(\n        work.function,\n        final_output::HIR_SELECTION_CONTROLS,\n        \"HIR single-suffix selection controls\",\n    )?;\n"),
            (122852, "            compact_path::push!(\n", "            path_array::push!(\n"),
            (123324, "                final_output::compact::hir_move!(work);\n                final_output::compact::hir_work!(work);\n", "                work.budget.reserve_bytes(\n                    work.function,\n                    final_output::HIR_MOVE_CONTROLS,\n                    \"HIR single-suffix move controls\",\n                )?;\n                work.charge(1, \"skeleton prefix clone\")?;\n"),
            (123938, "            compact_path::push!(\n", "            path_array::push!(\n"),
            (127075, "            compact_path::push!(\n", "            path_array::push!(\n"),
            (129471, "        compact_path::push!(\n", "        path_array::push!(\n"),
        ]),
        "src/cleanup_plan/replay/factored.rs" => (49022, "3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02", &[
            (14022, "    let Some(hir) = canonical.id_with_compact_receiving(\n        function,\n        vec![(hir_root, 1)],\n        budget,\n        &mut receiving,\n    )?\n", "    let Some(hir) =\n        canonical.id_with_receiving(function, vec![(hir_root, 1)], budget, &mut receiving)?\n"),
            (14504, "    let Some(plan) = canonical.id_with_compact_receiving(\n        function,\n        vec![(plan_root, 1)],\n        budget,\n        &mut receiving,\n    )?\n", "    let Some(plan) =\n        canonical.id_with_receiving(function, vec![(plan_root, 1)], budget, &mut receiving)?\n"),
        ]),
        _ => (length, expected, &[]),
    };
    assert_eq!(
        compiled.len(),
        live_length,
        "complete current merged probe source"
    );
    assert_eq!(super::source_sha(compiled), live_sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        current_file(
            &root.join(source),
            physical_compiled.len(),
            &super::source_sha(physical_compiled)
        )
        .expect("complete no-follow diagnostic-probe owner"),
        physical_compiled
    );
    let mut restored = compiled.to_owned();
    for &(at, after, before) in merge_edits.iter().rev() {
        assert_eq!(
            &restored[at..at + after.len()],
            after,
            "exact incoming-main source inverse"
        );
        restored.replace_range(at..at + after.len(), before);
    }
    assert_eq!(
        restored.len(),
        length,
        "complete original diagnostic-probe epoch"
    );
    assert_eq!(super::source_sha(&restored), expected);
    for &(after, before) in edits {
        assert_eq!(
            restored.matches(after).count(),
            1,
            "unique diagnostic-probe scope"
        );
        restored = restored.replacen(after, before, 1);
    }
    assert_eq!(restored.len(), prior_length);
    assert_eq!(
        super::source_sha(&restored),
        prior_sha,
        "entire pre-probe source"
    );
    std::borrow::Cow::Owned(restored)
}

// Select an explicitly authenticated historical parent view before the exact
// probe inverse; incoming-main views retain their own full source bodies.
pub(crate) fn phase_probe_stage<'a>(
    source: &str,
    expected: &str,
    supplied: &'a str,
) -> std::borrow::Cow<'a, str> {
    if !matches!(
        source,
        "src/cleanup_plan/validate.rs"
            | "src/cleanup_plan/replay.rs"
            | "src/cleanup_plan/replay/factored.rs"
            | "src/cleanup_plan/replay/factored/continuation_receiver.rs"
    ) {
        return std::borrow::Cow::Borrowed(supplied);
    }
    let acquired = super::main_e177_final_inputs::value_direct(source, expected, supplied);
    std::borrow::Cow::Owned(before_phase_probes(source, &acquired).into_owned())
}
