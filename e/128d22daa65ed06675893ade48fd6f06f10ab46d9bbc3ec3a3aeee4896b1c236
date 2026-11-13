use super::*;

// The relocated validator receiver shares these private helpers. Every older
// source consumer enters through the exact six visibility-only reversals.
type SourceEdits<'a> = &'a [(usize, &'a str, &'a str)];

pub(in super::super) fn lifecycle_visibility_entry(source: &str, supplied: &str) -> Option<String> {
    let (compiled, length, expected, prior_length, prior_sha, edits):
        (&str, usize, &str, usize, &str, SourceEdits<'_>) = match source {
        "src/hir/validation/function_scope/lifecycle.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle.rs")), 3978, "8095a604e5320261b71d1681fd53fe4ba2a2e7dac7f2a7a13c0e505563e815df", 3938, "d043c680becb6e6385559f4d0d8e86e5eff79f5dbdca72cfbe97b058cc18d8bd", &[
            (1296, "pub(in crate::hir::validation) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\n", "pub(super) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\n"),
            (3932, "pub(in crate::hir::validation) mod completed;\n", "pub(super) mod completed;\n"),
        ]),
        "src/hir/validation/function_scope/lifecycle/completed.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle/completed.rs")), 3672, "6444aa05d9e16fffa13f1d53018cad6793f32d5560adc09fe28309e20ef5fca9", 3736, "5ae07ffcc78eb7ac8c15abd3d8ff9e1bbf1f72d8307760ade92cb7d443135bfe", &[
            (287, "pub(in crate::hir::validation) struct Collector<'a> {\n", "pub(in crate::hir::validation::function_scope) struct Collector<'a> {\n"),
            (561, "    pub(in crate::hir::validation) fn new(program: &'a ResolvedProgram) -> Self {\n", "    pub(in crate::hir::validation::function_scope) fn new(program: &'a ResolvedProgram) -> Self {\n"),
            (1861, "    pub(in crate::hir::validation) fn extend(\n", "    pub(in crate::hir::validation::function_scope) fn extend(\n"),
            (3476, "    pub(in crate::hir::validation) fn iter(&self) -> impl Iterator<Item = &String> {\n", "    pub(in crate::hir::validation::function_scope) fn iter(&self) -> impl Iterator<Item = &String> {\n"),
        ]),
        _ => return None,
    };
    assert_eq!(
        supplied, compiled,
        "complete current lifecycle visibility source"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let preceding;
    let compiled = if compiled.len() == length && digest(compiled) == expected {
        assert_eq!(
            reader::read(&root.join(source), length, expected, 2_097_152).unwrap(),
            compiled
        );
        compiled
    } else {
        preceding = crate::source_reflows_merge::main_e177_final_inputs::declared_history_inputs::expected_value(
            source, expected, compiled,
        )
        .expect("complete lifecycle visibility predecessor");
        &preceding
    };
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    let mut before = compiled.to_owned();
    for &(at, current, previous) in edits.iter().rev() {
        assert_eq!(&before[at..at + current.len()], current);
        before.replace_range(at..at + current.len(), previous);
    }
    assert_eq!(before.len(), prior_length);
    assert_eq!(digest(&before), prior_sha);
    Some(before)
}

// Exact #913 and cfg(test) repair ingress; every immutable predecessor stays whole.
pub(in super::super) fn batch_913_entry(source: &str, supplied: &str) -> Option<String> {
    let (compiled, length, expected, prior_length, prior_sha, edits):
        (&str, usize, &str, usize, &str, &[(&str, &str)]) = match source {
        "src/hir/validation/scope_checkpoint/marks.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/scope_checkpoint/marks.rs")), 14549, "2965bb2d4a43a3ca060b21db1a7ad11ca0af3a7e5061e48832366a833bca3c0e", 10578, "08b3d1a43078fcc73d6ba8514a093d5bdd4b7a5b00378bfd20a8a9a67b75e464", &[
            ("        // The original walker does nothing for these exact root shapes. This\n        // is only a source-move pass: full expression validation already has\n        // its own owner, and every propagating root retains the old driver.\n        // Fund the additional required-reservation argument/formal and bool\n        // return/receiving, plus the borrowed match scrutinee, before formation.\n        // The existing outbound Result and static refusal constructor retain\n        // their roles; no forwarding Result/helper or owned selector is added.\n        {\n            #[cfg(test)]\n            let _operation = crate::bounded_output::stage_debits::operation_guard(\n                crate::bounded_output::stage_debits::Operation::ScopeCheckpoint,\n            );\n            #[cfg(feature = \"unstable-workflow-profiling\")]\n            let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n                crate::bounded_output::exclusive_debits::Operation::ScopeCheckpoint,\n            );\n            if !crate::bounded_output::reserve_active_required(INERT_ROOT_CONTROLS) {\n                // Same allocation-free diagnostic and complete outbound Result\n                // as the original first Storage payer. It is selected before\n                // touching scope, including a pre-existing sticky refusal.\n                return Err(Diagnostic::io_static(\n                    \"SPX-H006\",\n                    \"ownership scope checkpoint exceeds the active validation budget\",\n                ));\n            }\n        }\n        match &expression.kind {\n            ResolvedExprKind::Closure { .. }\n            | ResolvedExprKind::FunctionReference { .. }\n            | ResolvedExprKind::Int(_)\n            | ResolvedExprKind::Int32(_)\n            | ResolvedExprKind::Char(_)\n            | ResolvedExprKind::Uint8(_)\n            | ResolvedExprKind::Usize(_)\n            | ResolvedExprKind::Float32(_)\n            | ResolvedExprKind::Float64(_)\n            | ResolvedExprKind::Bool(_)\n            | ResolvedExprKind::String(_)\n            | ResolvedExprKind::BorrowedUtf8Literal(_)\n            | ResolvedExprKind::ArrayU8(_)\n            | ResolvedExprKind::RepeatArrayU8 { .. }\n            | ResolvedExprKind::BorrowPlace { .. }\n            | ResolvedExprKind::ByteRange { .. }\n            | ResolvedExprKind::Call { .. }\n            | ResolvedExprKind::LiteralFormat { .. }\n            | ResolvedExprKind::VecFieldRead { .. }\n            | ResolvedExprKind::RowLoan { .. }\n            | ResolvedExprKind::NativeRustImportCall(_)\n            | ResolvedExprKind::HostCommandCall(_)\n            | ResolvedExprKind::Unary { .. }\n            | ResolvedExprKind::Binary { .. }\n            | ResolvedExprKind::ConstructRecord { .. }\n            | ResolvedExprKind::ConstructVariant { .. }\n            | ResolvedExprKind::Try { .. }\n            | ResolvedExprKind::TryOption { .. }\n            | ResolvedExprKind::Yield { .. }\n            | ResolvedExprKind::UpdateRecord { .. } => return Ok(()),\n            ResolvedExprKind::LexicalExit { .. }\n            | ResolvedExprKind::Invoke { .. }\n            | ResolvedExprKind::Place(_)\n            | ResolvedExprKind::Block { .. }\n            | ResolvedExprKind::VecConstructor { .. }\n            | ResolvedExprKind::If { .. }\n            | ResolvedExprKind::Match { .. }\n            | ResolvedExprKind::Project { .. }\n            | ResolvedExprKind::Upcast { .. } => {}\n        }\n", ""),
            ("        #[cfg(test)]\n        if tests::before_selected() {\n            return self.mark_value_sources_moved_before_inert_root(expression, scope);\n        }\n", ""),
            ("\n// One real primitive site, no new fallible wrapper or helper call. The bool\n// branch reads its original primitive result; match creates no bool predicate.\nconst INERT_ROOT_CONTROLS: usize = 2 * std::mem::size_of::<usize>()\n    + 3 * std::mem::size_of::<bool>()\n    + std::mem::size_of::<&ResolvedExprKind>();\n\n#[cfg(test)]\nmod before;\n#[cfg(test)]\nmod tests;\n", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs")), 10608, "c82fb33f537bed7561a0267298de31c1577b00e3d2e7b6db1ec70b52b9deec7d", 10327, "4d9b686be048b16ae777c5990227c1afb97a166f583757dbddc11989b5ac1f01", &[
            ("        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))\n            ),\n            \"a176917b2f70a4ddff56230d92de0abd6575bfc3e6869b08612d5fff767499a1\"\n        );\n", "        assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), \"a176917b2f70a4ddff56230d92de0abd6575bfc3e6869b08612d5fff767499a1\");\n"),
            ("        assert_eq!(\n            restored\n                .matches(\"use super::*;\\nuse crate::bounded_output::with_limit_usage_refusal;\\n\")\n                .count(),\n            1\n        );\n", "        assert_eq!(restored.matches(\"use super::*;\\nuse crate::bounded_output::with_limit_usage_refusal;\\n\").count(), 1);\n"),
            ("    use crate::test_allocation_observer::{measure, Metrics};\n", "    use crate::test_allocation_observer::{Metrics, measure};\n"),
            ("    assert_eq!(\n        TRACE.with(|trace| trace\n            .borrow()\n            .as_ref()\n            .expect(\"active trace lease\")\n            .events\n            .len()),\n        4096\n    );\n", "    assert_eq!(TRACE.with(|trace| trace.borrow().events.len()), 4096);\n"),
            ("    let (empty, lost) =\n        super::comparison_owner::pair_scope_census::finish_empty_for_frontier_test();\n", "    let (empty, lost) = super::comparison_owner::pair_scope_census::finish_empty_for_frontier_test();\n"),
        ]),
        "src/hir/validation/frame_reuse/tests.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/tests.rs")), 13930, "11c3145c9b06cbde60f592bcf4def6403ea5b30e7f442381becd1c4324999de8", 13594, "2d874d35d5139662abf9a10e0fe45abbd666136349eae390220f134da2c8ebc9", &[
            ("            root.id = ExpressionId::from_owned(\"forged.trailing.identity\".to_owned());\n", "            root.id = ExpressionId::new(\"forged.trailing.identity\".to_owned());\n"),
            ("                    tail.id = ExpressionId::from_owned(\"forged.tail\".to_owned());\n", "                    tail.id = ExpressionId::new(\"forged.tail\".to_owned());\n"),
        ]),
        "src/hir/validation/lexical_exit.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/lexical_exit.rs")), 47750, "5610b10bf134638d5a30dbee84058895d40263d46ac07057f154d47ff8487c0e", 47202, "ed95841d0c0bb977e8120046a0182ff2dcef8731f443d0d1c17af2233ce15bc4", &[
            ("                                for error in &errors {\n                                    eprintln!(\"OPT904_DIAGNOSTIC phase=hir::resolve code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n                                }\n", ""),
            ("                            eprintln!(\"OPT904_DIAGNOSTIC phase=hir::validate code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n", ""),
        ]),
        "src/hir/validation/frame_reuse/source_inverse.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/source_inverse.rs")), 10055, "1b9b6134ab855b3396f602949c7a681efe6bf840189712f326b22917a803ec82", 9708, "4432050d6538ee4469137dc7f147256591d256c569f1e49a0727e6a223fe7f38", &[
            ("    let observed;\n    let current = if source == \"src/hir/validation/lexical_exit.rs\" {\n        observed = crate::source_reflows::batch_913_entry(source, current)\n            .expect(\"complete diagnostic observer ingress\");\n        observed.as_str()\n    } else {\n        current\n    };\n", ""),
            ("        let predecessor = if matches!(path, \"src/hir/validation/frame_reuse/tests.rs\" | \"src/hir/validation/frame_reuse/source_inverse.rs\") {", "        let predecessor = if path == \"src/hir/validation/frame_reuse/tests.rs\" {"),
        ]),
        _ => return None,
    };
    let physical_compiled = compiled;
    let supplied_stage = supplied;
    let current_join =
        current_source_epoch::physical_batch_input(source, supplied, physical_compiled);
    let supplied = current_join.as_ref();
    let (closure_epoch, _) =
        crate::source_reflows_merge::Main44Fixture::closure_epoch(source, expected, compiled);
    let batch_input = current_source_epoch::batch_compiled_input(source, closure_epoch.as_ref());
    let compiled = batch_input.as_ref();
    assert_eq!(supplied, physical_compiled, "complete current batch source");
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if source == "src/hir/validation/frame_reuse/source_inverse.rs" {
        assert_eq!(
            reader::read(
                &root.join(source),
                physical_compiled.len(),
                &digest(physical_compiled),
                2_097_152
            )
            .unwrap(),
            physical_compiled
        );
    }
    assert_eq!(
        reader::read(
            &if matches!(
                source,
                "src/hir/validation/scope_checkpoint/marks.rs"
                    | "src/hir/validation/lexical_exit.rs"
                    | "src/hir/validation/frame_reuse/source_inverse.rs"
                    | "src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs"
            ) {
                root.join("e").join(expected)
            } else {
                root.join(source)
            },
            length,
            expected,
            2_097_152,
        )
        .unwrap(),
        compiled
    );
    if source == "src/hir/validation/scope_checkpoint/marks.rs" {
        for (path, child, child_length, child_sha) in [
            (
                "src/hir/validation/scope_checkpoint/marks/before.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/hir/validation/scope_checkpoint/marks/before.rs"
                )),
                10603,
                "429e331bf13d326e4ea5e26185f0d4d6945c2ad13dbadc5c457da65f4867140c",
            ),
            (
                "src/hir/validation/scope_checkpoint/marks/tests.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/hir/validation/scope_checkpoint/marks/tests.rs"
                )),
                20658,
                "7f07e20bdc0b2797f0c2f8abc2af9c5427fef9472bdf91fa87b5cfbb30f51994",
            ),
        ] {
            let acquired = current_source_predecessor(path, child);
            let archive = match &acquired {
                Cow::Borrowed(_) => root.join(path),
                Cow::Owned(_) => root.join(format!("e/{child_sha}")),
            };
            assert_eq!(acquired.len(), child_length);
            assert_eq!(digest(&acquired), child_sha);
            assert_eq!(
                reader::read(&archive, child_length, child_sha, 2_097_152).unwrap(),
                acquired
            );
        }
    }
    let mut before = compiled.to_owned();
    if source == "src/hir/validation/frame_reuse/tests.rs" {
        // Complete current executable-entry fixture transport, then the exact
        // unchanged #913 identifier inverse below. Neither old endpoint moves.
        assert_eq!(before.matches("    source.push_str(\"@id(\\\"reuse.main\\\") fn main()->i64 {f31(1)}\\n\");\n    program(&source)\n}\nfn decimal_program() -> ResolvedProgram {\n    // Preserve the entire library; executable resolution also needs an entry.\n    let source = format!(\n        \"{}\\n@id(\\\"reuse.decimal.main\\\") fn main()->i64 {{let value=add(\\\"1\\\",\\\"2\\\");string_len(value)}}\\n\",\n        include_str!(\"../../../../std/int-decimal/src/decimal.spx\")\n    );\n").count(), 1);
        before = before.replacen("    source.push_str(\"@id(\\\"reuse.main\\\") fn main()->i64 {f31(1)}\\n\");\n    program(&source)\n}\nfn decimal_program() -> ResolvedProgram {\n    // Preserve the entire library; executable resolution also needs an entry.\n    let source = format!(\n        \"{}\\n@id(\\\"reuse.decimal.main\\\") fn main()->i64 {{let value=add(\\\"1\\\",\\\"2\\\");string_len(value)}}\\n\",\n        include_str!(\"../../../../std/int-decimal/src/decimal.spx\")\n    );\n", "", 1);
        assert_eq!(before.matches("        decimal_program(),\n").count(), 1);
        before = before.replacen(
            "        decimal_program(),\n",
            "        program(include_str!(\"../../../../std/int-decimal/src/decimal.spx\")),\n",
            1,
        );
        assert_eq!(
            before
                .matches("        (\"decimal\", decimal_program(), true),\n")
                .count(),
            1
        );
        before = before.replacen("        (\"decimal\", decimal_program(), true),\n", "        (\"decimal\", program(include_str!(\"../../../../std/int-decimal/src/decimal.spx\")), true),\n", 1);
        const MIDDLE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"
        ));
        assert_eq!(MIDDLE.len(), 13608);
        assert_eq!(
            digest(MIDDLE),
            "a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"
        );
        assert_eq!(
            reader::read(
                &root.join("e/a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"),
                13608,
                "a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb",
                2_097_152
            )
            .unwrap(),
            MIDDLE
        );
        assert_eq!(
            before, MIDDLE,
            "ENTIRE unchanged pre-fixture current source"
        );
    }
    if source == "src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs" {
        assert_eq!(
            reader::read(
                &root.join(source),
                physical_compiled.len(),
                &digest(physical_compiled),
                2_097_152
            )
            .unwrap(),
            physical_compiled,
            "complete late physical history observer source"
        );
    }
    for &(after, prior) in edits {
        assert_eq!(before.matches(after).count(), 1);
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), prior_length);
    assert_eq!(digest(&before), prior_sha);
    let _ = current_source_epoch::batch_compiled_input(source, closure_epoch.as_ref());
    let _ = current_source_epoch::physical_batch_input(source, supplied_stage, physical_compiled);
    Some(before)
}
// Enter the immutable selected-path predecessor only through complete live images.
pub(in super::super) fn compact_frontier_entry(source: &str, supplied: &str) -> Option<String> {
    let compiled = match source {
        "src/cleanup_plan/replay/path_join/selected.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/walk.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/walk.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/queries.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/queries.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/receiving.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/receiving.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/tests/mod.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/tests/mod.rs"
        )),
        _ => return None,
    };
    assert_eq!(
        supplied, compiled,
        "complete current compact frontier source"
    );
    let encoded = include_str!("../compact_frontier_inverse.json");
    assert_eq!(encoded.len(), 30712);
    assert_eq!(
        digest(encoded),
        "f3c2c90ae071a1e777dbb007ef1ac80adccdfc140892177cabd4b022e0778645"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/compact_frontier_inverse.json"),
            encoded.len(),
            &digest(encoded),
            2_097_152
        )
        .unwrap(),
        encoded
    );
    let binding: serde_json::Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(
        binding["schema"],
        "semaprax.compact-frontier-whole-source-ingress.v1"
    );
    for (path, child) in [
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/boxed_flow.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/boxed_flow.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs"))),
        ("src/cleanup_plan/replay/cfg_bounds/tests/old.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/cfg_bounds/tests/old.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/whole.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/whole.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/renewal.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/renewal.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/session.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/session.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/strings.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/strings.rs"))),
    ] {
        let pin = &binding["children"][path];
        let length = usize::try_from(pin["length"].as_u64().unwrap()).unwrap();
        let expected = pin["sha256"].as_str().unwrap();
        let acquired = current_source_predecessor(path, child);
        let archive = match &acquired {
            Cow::Borrowed(_) => root.join(path),
            Cow::Owned(_) => root.join(format!("e/{expected}")),
        };
        assert_eq!(acquired.len(), length);
        assert_eq!(digest(&acquired), expected);
        assert_eq!(reader::read(&archive, length, expected, 2_097_152).unwrap(), acquired);
    }
    let row = &binding["rows"][source];
    let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
    let expected = row["current_sha256"].as_str().unwrap();
    let acquired = current_source_predecessor(source, compiled);
    let archive = match &acquired {
        Cow::Borrowed(_) => root.join(source),
        Cow::Owned(_) => root.join(format!("e/{expected}")),
    };
    let compiled = acquired.as_ref();
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    assert_eq!(
        reader::read(&archive, length, expected, 2_097_152).unwrap(),
        compiled
    );
    let mut before = compiled.as_bytes().to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(
            before.get(start..end).unwrap(),
            edit["current"].as_str().unwrap().as_bytes()
        );
        before.splice(
            start..end,
            edit["before"].as_str().unwrap().as_bytes().iter().copied(),
        );
    }
    let before = String::from_utf8(before).unwrap();
    assert_eq!(before.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(digest(&before), row["before_sha256"].as_str().unwrap());
    Some(before)
}

#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
pub(in super::super) fn next_custody_before(owner: &str, source: &str, supplied: &str) -> String {
    if (owner, source)
        == (
            "modular916",
            "src/workspace_graph/expected_projection/held_projection.rs",
        )
        && supplied.len() == 23069
        && digest(supplied) == "19dd7ed0772d873ba5d7854b3c8c16c298009c2c0454d12b538524cbc7b56e29"
    {
        let complete = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/workspace_graph/expected_projection/held_projection.rs"
        ));
        let prior = next_custody_before("production7834", source, complete);
        assert_eq!(
            supplied, prior,
            "exact named production7834 output at modular916 ingress"
        );
        return prior;
    }
    if (owner, source)
        == (
            "modular916",
            "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs",
        )
        && supplied.len() == 14772
        && digest(supplied) == "fdbdd20cb63d8bf8f32e8244e96be2526df769614f1ed325c9b515d7fd67e722"
    {
        let complete = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs"
        ));
        let prior = next_custody_before("production7834", source, complete);
        assert_eq!(
            supplied, prior,
            "exact named production7834 output at modular916 ingress"
        );
        return prior;
    }
    if owner == "constructor903" {
        return constructor903_before(source, supplied).unwrap_or_else(|| supplied.to_owned());
    }
    const PACKET: &str = include_str!("../next_custody_inverse.json");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_inverse.json"),
            PACKET.len(),
            &digest(PACKET),
            2_097_152
        )
        .unwrap(),
        PACKET
    );
    let packet: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let owners = packet["owners"].as_array().unwrap();
    for (index, row) in owners.iter().enumerate() {
        assert!(
            !owners[..index]
                .iter()
                .any(|prior| prior["owner"] == row["owner"] && prior["source"] == row["source"]),
            "duplicate finite source pair"
        );
    }
    let row = packet["owners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["owner"] == owner && row["source"] == source);
    let Some(row) = row else {
        return constructor903_before(source, supplied).unwrap_or_else(|| supplied.to_owned());
    };
    let compiled = match source {
        "docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md")),
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py")),
        "src/bounded_output/classifier_string.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/classifier_string.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs")),
        "src/prelude.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/prelude.rs")),
        "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_fragments_v3.rs")),
        "src/source_reflows_merge/main_e177_overlay_pins_v10.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_pins_v10.rs")),
        "src/source_reflows_merge/main_e177_overlay_v2.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_v2.rs")),
        "src/source_verify/constructor_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/constructor_tests.rs")),
        "src/workspace_graph/expected_projection/call_carriers_inverse.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_inverse.rs")),
        "src/workspace_graph/expected_projection/held_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection.rs")),
        "src/workspace_graph/expected_projection/private_layout.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout.rs")),
        "src/workspace_graph/expected_projection/private_layout/names.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names.rs")),
        "src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs")),
        "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),
        _ => unreachable!("closed finite source entry"),
    };
    // Authenticate the actual compiled/physical channel before any finite epoch projection.
    let physical = reader::read(
        &root.join(source),
        compiled.len(),
        &digest(compiled),
        2_097_152,
    )
    .unwrap();
    assert_eq!(
        physical, compiled,
        "complete actual compiled source channel"
    );
    // Some inherited consumers converge on this interface more than once.
    // A repeated input must equal the complete predecessor reconstructed below.
    let registry_scratch = owner == "interface754"
        && source
            == "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs";
    let registry_archive =
        registry_scratch && supplied.len() == 17719 && digest(supplied) == REGISTRY_SCRATCH_BEFORE;
    let interface_predecessor =
        owner == "interface754" && supplied == row["before_text"].as_str().unwrap();
    let constructor = constructor903_before(source, compiled);
    let acquired = constructor
        .filter(|before| supplied == compiled || supplied == before.as_str())
        .or_else(|| {
            (supplied.len() as u64 == row["current_length"].as_u64().unwrap()
                && digest(supplied) == row["current_sha256"].as_str().unwrap())
            .then(|| supplied.to_owned())
        })
        .or_else(|| {
            (registry_scratch
                && (supplied == compiled || interface_predecessor || registry_archive))
                .then(|| interface754_scratch_before_format(compiled))
        })
        .or_else(|| next_emission_before(owner, source, supplied, compiled))
        .or_else(|| next_custody_front(owner, source, supplied, compiled, row));
    if acquired.is_none() && !interface_predecessor && !registry_archive {
        assert_eq!(supplied, compiled, "complete compiled source channel");
    }
    let compiled = acquired.as_deref().unwrap_or(compiled);
    assert_eq!(compiled.len() as u64, row["current_length"]);
    assert_eq!(digest(compiled), row["current_sha256"]);
    let mut prior_end = 0;
    for edit in row["edits"].as_array().unwrap() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(prior_end <= start && start <= end && end <= compiled.len());
        assert!(compiled.is_char_boundary(start) && compiled.is_char_boundary(end));
        prior_end = end;
    }
    let mut restored = compiled.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(restored, row["before_text"].as_str().unwrap());
    assert_eq!(restored.len() as u64, row["before_length"]);
    assert_eq!(digest(&restored), row["before_sha256"]);
    if registry_scratch {
        let before = registry_scratch_before_transport(root, restored);
        if registry_archive {
            assert_eq!(
                before, supplied,
                "complete inherited registry scratch input"
            );
        }
        return before;
    }
    restored
}

#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
fn interface754_scratch_before_format(compiled: &str) -> String {
    const FORMATTED: &str = concat!(
        "    assert!(\n",
        "        super::super::names::before_interface_source(include_str!(\"../names.rs\"))\n",
        "            .contains(&format!(\"pub(super) fn first({old_first}\"))\n",
        "    );\n",
    );
    const BEFORE: &str = concat!(
        "    assert!(super::super::names::before_interface_source(include_str!(\"../names.rs\"))\n",
        "        .contains(&format!(\"pub(super) fn first({old_first}\")));\n",
    );
    assert_eq!(
        digest(compiled),
        "9949e7c0061ee45937655ff0b4a549ea0fb1d07a16749bad8e3724f380c343ba"
    );
    assert_eq!(compiled.matches(FORMATTED).count(), 1);
    compiled.replacen(FORMATTED, BEFORE, 1)
}

#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
const REGISTRY_SCRATCH_BEFORE: &str =
    "0bf3a868125c5d381ead3b67127f622085d8fba09b23862db68bdad13c271ec6";

// The two retained registry fixtures also predate the test data transport.
#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
fn registry_scratch_before_transport(root: &std::path::Path, mut prior: String) -> String {
    const TRANSPORT_EDITS: [(&str, &str); 2] = [
        (
            concat!(
                "    let packet: serde_json::Value =\n",
                "        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(\n",
                "            crate::source_reflows_merge::ReceiverFixture::RegistryScratch,\n",
                "        ))\n",
                "        .unwrap();\n",
            ),
            concat!(
                "    let packet: serde_json::Value = serde_json::from_str(include_str!(\n",
                "        \"../../../../../docs/evidence/registry-census-scratch-833/source-inverses.json\"\n",
                "    ))\n",
                "    .unwrap();\n",
            ),
        ),
        (
            concat!(
                "        crate::source_reflows_merge::receiver_child_entry(\n",
                "            \"src/workspace_graph/expected_projection/private_layout/before_registry.rs\",\n",
                "            include_str!(\"../before_registry.rs\"),\n",
                "        ),\n",
            ),
            "        include_str!(\"../before_registry.rs\"),\n",
        ),
    ];
    for (after, before) in TRANSPORT_EDITS {
        assert_eq!(prior.matches(after).count(), 1);
        prior = prior.replacen(after, before, 1);
    }
    let archive = root.join("e").join(REGISTRY_SCRATCH_BEFORE);
    assert_eq!(
        reader::read(&archive, 17719, REGISTRY_SCRATCH_BEFORE, 2_097_152).unwrap(),
        prior,
        "complete independently saved registry scratch source"
    );
    prior
}

// Only these four exact current fronts may precede the immutable26 input epochs.
#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
fn next_custody_front(
    owner: &str,
    source: &str,
    supplied: &str,
    actual: &str,
    old: &serde_json::Value,
) -> Option<String> {
    const FRONT: &str = include_str!("../next_custody_fronts.json");
    const PAIRS: [(&str, &str); 4] = [
        (
            "prelude286",
            "tests/workspace/semantic_graph/source_inventory.rs",
        ),
        (
            "v12-root",
            "src/source_reflows_merge/main_e177_overlay_v2.rs",
        ),
        (
            "v12-python",
            "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
        ),
        (
            "main295-v12-fragments",
            "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs",
        ),
    ];
    let index = PAIRS.iter().position(|pair| *pair == (owner, source))?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_fronts.json"),
            FRONT.len(),
            &digest(FRONT),
            2_097_152
        )
        .unwrap(),
        FRONT
    );
    let packet: serde_json::Value = front_pool::decode(FRONT).unwrap();
    assert_eq!(packet["schema"], "semaprax.next-custody-four-fronts.v1");
    assert_eq!(
        packet["original_packet"]["length"],
        include_str!("../next_custody_inverse.json").len()
    );
    assert_eq!(
        packet["original_packet"]["sha256"],
        digest(include_str!("../next_custody_inverse.json"))
    );
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), PAIRS.len());
    for (row, pair) in rows.iter().zip(PAIRS) {
        assert_eq!(row["owner"], pair.0);
        assert_eq!(row["source"], pair.1);
    }
    let row = &rows[index];
    assert_eq!(actual.len() as u64, row["length"]);
    assert_eq!(digest(actual), row["sha256"]);
    fn project(actual: &str, stage: &serde_json::Value) -> String {
        let mut previous = 0;
        for edit in stage["edits"].as_array().unwrap() {
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            assert!(previous <= start && start <= end && end <= actual.len());
            assert!(actual.is_char_boundary(start) && actual.is_char_boundary(end));
            assert_eq!(&actual[start..end], edit["after"].as_str().unwrap());
            previous = end;
        }
        let mut result = actual.to_owned();
        for edit in stage["edits"].as_array().unwrap().iter().rev() {
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            result.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        assert_eq!(result.len() as u64, stage["length"]);
        assert_eq!(digest(&result), stage["sha256"]);
        result
    }
    let middle = (!row["middle"].is_null()).then(|| project(actual, &row["middle"]));
    assert!(
        supplied == actual || middle.as_deref() == Some(supplied),
        "only the same actual source or its exact bound V13 intermediate"
    );
    assert_eq!(row["target"]["length"], old["current_length"]);
    assert_eq!(row["target"]["sha256"], old["current_sha256"]);
    let restored = project(actual, &row["target"]);
    assert_eq!(
        reader::read(&root.join(source), actual.len(), &digest(actual), 2_097_152).unwrap(),
        actual
    );
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_fronts.json"),
            FRONT.len(),
            &digest(FRONT),
            2_097_152
        )
        .unwrap(),
        FRONT
    );
    let original = include_str!("../next_custody_inverse.json");
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_inverse.json"),
            original.len(),
            &digest(original),
            2_097_152
        )
        .unwrap(),
        original
    );
    Some(restored)
}

// Only the two complete OPT925 test successors may acquire their fixed b23
// source epochs before the entire unchanged original26 inverse is applied.
#[allow(
    dead_code,
    reason = "Used by historical workspace consumers that are absent from the standalone Wasm audit"
)]
fn next_emission_before(owner: &str, source: &str, supplied: &str, actual: &str) -> Option<String> {
    if owner != "cfgtest-repair" {
        return None;
    }
    // Authenticate the exact runtime-repaired raw source before its unchanged925 input view.
    let raw_actual = actual;
    let raw_supplied = supplied;
    type SourcePrefix<'a> = (usize, &'a str, SourceEdits<'a>);
    type SourceInverse<'a> = (usize, &'a str, &'a str, &'a str, SourceEdits<'a>);
    let prefix: Option<SourcePrefix<'_>> = match source {
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => Some((17977,"f483721064e9e81524fdf22c6b41e07d045edfab0d9f4d0627e35e8c470bc33e",&[(6817,"","                let mut work = SkeletonWork::new(&program, function, &mut budget);\n"),(6852,"                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;\n",""),(8179,"                        budget\n","                        work.budget\n"),(8377,"","                drop(work);\n")])),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => Some((35081,"5d34472a0f59197c69d691fd9cc9b2dc90424ad09c74287b31ea42b2df263eb6",&[(1549,"    let raw_source = include_str!(\"../../../../replay.rs\");\n    assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src/cleanup_plan/replay.rs\")).unwrap(), raw_source.as_bytes());\n    let digest = |bytes: &[u8]| { use sha2::{Digest, Sha256}; format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(bytes))) };\n    assert_eq!(raw_source.len(), 327551);\n    assert_eq!(digest(raw_source.as_bytes()), \"751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5\");\n    let mut source = raw_source.to_owned();\n    let prefix_edits: &[(usize, &str, &str)] = &[\n        (1097, \"mod plan_prefix;\\n\", \"\"),\n        (108404, \"#[rustfmt::skip]\\n\", \"\"),\n        (112083, \"        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\\n\", \"\"),\n        (116424, \"        budget.reserve_bytes(function, skeleton_work::query_proof::HEADER, \\\"skeleton query neutral slot receivers\\\")?;\\n        SkeletonWork { queries: skeleton_work::query_proof::Slot::new(function, budget),\\n\", \"        SkeletonWork {\\n\"),\n        (279166, \"#[rustfmt::skip]\\n\", \"\"),\n        (279316, \"    budget.reserve_bytes(function, plan_prefix::QUERY_CONTROLS, \\\"PLAN prefix eligibility receivers\\\")?;\\n    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\\n\", \"\"),\n        (289554, \"                compact_path::plan_selection!(budget, function);\\n\", \"                budget.reserve_bytes(\\n                    function,\\n                    branch_clone::direct::PLAN_SMALL_SELECTION_CONTROLS,\\n                    \\\"plan final-output selection controls\\\",\\n                )?;\\n\"),\n        (289948, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (290070, \"                            branch_clone::direct::plan_compact_roles::<\\n\", \"                            branch_clone::direct::compact_roles::<\\n\"),\n        (290357, \"                        ));\\n\", \"                        )?;\\n\"),\n        (291020, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (291142, \"                            branch_clone::plan_compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\", \"                            branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\"),\n        (291305, \"                        ));\\n\", \"                        )?;\\n\"),\n        (291645, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (291767, \"                            branch_clone::direct::plan_compact_roles::<\\n\", \"                            branch_clone::direct::compact_roles::<\\n\"),\n        (292054, \"                        ));\\n\", \"                        )?;\\n\"),\n        (292911, \"                    compact_path::plan_move!(budget, function);\\n                    compact_path::plan_work!(\\n                        budget,\\n\", \"                    budget.reserve_bytes(\\n\"),\n        (293087, \"                        \\\"cleanup-plan skeleton branch clone\\\"\\n                    );\\n\", \"                        final_output::PLAN_MOVE_CONTROLS,\\n                        \\\"plan final-output move controls\\\",\\n                    )?;\\n                    budget.charge_skeleton(function, 1, \\\"cleanup-plan skeleton branch clone\\\")?;\\n\"),\n        (293480, \"                    compact_path::plan_work!(budget, function, \\\"plan branch observation push\\\");\\n\", \"                    budget.charge_skeleton(function, 1, \\\"plan branch observation push\\\")?;\\n\"),\n        (293629, \"                    compact_path::plan_auth!(budget, function);\\n                    compact_path::plan_conversion!(budget, function, observation);\\n\", \"                    budget.reserve_bytes(function, 0, \\\"plan branch observation push\\\")?;\\n                    budget.reserve_bytes(\\n                        function,\\n                        physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(\\n                            &observation,\\n                        ),\\n                        \\\"plan branch observation push\\\",\\n                    )?;\\n\"),\n        (297849, \"                        branch_clone::short_plan::produce!(\\n                            budget,\\n                            function,\\n                            observations,\\n                            observation,\\n                            edge,\\n                            queue\\n                        );\\n\", \"\"),\n        (324946, \"#[cfg(test)]\\nmod workspace_oracle;\\n\", \"\\n\"),\n        (325059, \"#[cfg(test)]\\npub(crate) use workspace_oracle::with_financed_workspace_test_oracles;\\n\", \"\\n\"),\n        (325167, \"\", \"\\n\"),\n    ];\n    for &(at, current, previous) in prefix_edits.iter().rev() {\n        assert_eq!(source.get(at..at + current.len()), Some(current));\n        source.replace_range(at..at + current.len(), previous);\n    }\n    assert_eq!(source.len(), 326623);\n    assert_eq!(digest(source.as_bytes()), \"027903ce7f76fd0a9883e2736adfa1214bdb1e687303d939c0e40751c6130320\");\n    assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src/cleanup_plan/replay.rs\")).unwrap(), raw_source.as_bytes());\n","    let mut source = include_str!(\"../../../../replay.rs\").to_owned();\n"),(30433,"\n// Exact pre-sticky validation participates in this full caller boundary.\nfn joint_frozen_immediate(\n    program: &ResolvedProgram,\n    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<(), Diagnostic> {\n    let mut expected = with_frozen_immediate(|| hir_skeleton_paths(program, function, budget))?;\n    budget.reserve_bytes(\n        function,\n        sequence::ENTRY_CALL,\n        \"whole HIR complete content sort\",\n    )?;\n    sequence::sort(&mut expected, function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::PRODUCE_CALL,\n        \"whole independent PLAN producer\",\n    )?;\n    let produced = view::produce(function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::COMPARE_CALL,\n        \"whole independent history comparison\",\n    )?;\n    view::compare(produced, &expected, budget)\n}\n\n#[test]\nfn retained_fork_true_frozen_immediate_whole_net_and_cold_costs_are_disclosed() {\n    let mut improvement = false;\n    for program in [fixture(), nested_call_program()] {\n        for function in &program.functions {\n            let run = |immediate, limit, floor| {\n                with_limit_usage_refusal(limit, || {\n                    measure(|| {\n                        assert!(set_active_floor(floor));\n                        let mut budget = ReplayBudget::new();\n                        let result = reserve_program_skeleton_work(\n                            &program,\n                            std::iter::once(function),\n                            &mut budget,\n                        )\n                        .and_then(|_reserved| {\n                            if immediate {\n                                joint_frozen_immediate(&program, function, &mut budget)\n                            } else {\n                                joint(&program, function, &mut budget)\n                            }\n                        });\n                        let work = (\n                            budget.skeleton_spent,\n                            budget.skeleton_remaining,\n                            budget.remaining,\n                        );\n                        if result.is_err() {\n                            assert!(budget\n                                .reserve_bytes(function, 0, \"retained whole fork sticky zero\")\n                                .is_err());\n                        }\n                        drop(budget);\n                        result.map(|()| work).map_err(|error| {\n                            let code = error.code;\n                            drop(error);\n                            code\n                        })\n                    })\n                })\n            };\n            let ((before, old_heap), old_refused, old_paid, _) = run(true, usize::MAX, 0);\n            let ((after, new_heap), new_refused, new_paid, _) = run(false, usize::MAX, 0);\n            let old_work = before.unwrap();\n            let new_work = after.unwrap();\n            assert_eq!(old_work, new_work);\n            assert!(!old_refused && !new_refused);\n            for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n                assert!(!heap.invalid && heap.live == 0);\n                assert_eq!(heap.allocated, heap.freed);\n                assert!(heap.allocated <= paid);\n            }\n            improvement |= new_paid < old_paid;\n            eprintln!(\"925 true_immediate_whole function={} old_paid={} new_paid={} old_heap={:?} new_heap={:?} work={:?}\",\n                function.id.as_str(), old_paid, new_paid, old_heap, new_heap, new_work);\n            // Exact/short/floor/zero are independently determined for each full\n            // producer. A changed net invoice never becomes a cap increase.\n            for (immediate, paid) in [(true, old_paid), (false, new_paid)] {\n                for (limit, floor, success) in [\n                    (paid, 0, true),\n                    (paid - 1, 0, false),\n                    (0, 0, false),\n                    (paid, paid, false),\n                ] {\n                    let ((result, heap), refused, _, first) = run(immediate, limit, floor);\n                    assert_eq!(result.is_ok(), success);\n                    assert!(!heap.invalid && heap.live == 0);\n                    assert_eq!(heap.allocated, heap.freed);\n                    if !success {\n                        assert!(refused && first.is_some());\n                        assert_eq!(result.unwrap_err(), \"SPX-H006\");\n                    }\n                }\n            }\n        }\n    }\n    assert!(\n        improvement,\n        \"complete composed receiver must earn a positive net invoice\"\n    );\n}\n","")])),
        _ => None,
    };
    let acquired = prefix.map(|(length, sha, edits)| {
        assert_eq!(raw_supplied, raw_actual);
        assert_eq!(raw_actual.len(), length);
        assert_eq!(digest(raw_actual), sha);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            reader::read(&root.join(source), length, sha, 2_097_152).unwrap(),
            raw_actual
        );
        let mut prior = raw_actual.to_owned();
        for &(at, post, pre) in edits.iter().rev() {
            assert_eq!(prior.get(at..at + post.len()), Some(post));
            prior.replace_range(at..at + post.len(), pre);
        }
        prior
    });
    let actual = acquired.as_deref().unwrap_or(actual);
    let supplied = if acquired.is_some() { actual } else { supplied };
    let (length, sha, target, target_sha, edits): SourceInverse<'_> = match source {
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => (17992, "86eb3c62bd63b7b04a30adc265bde874e9faf1910cdd538d9d9e6c0265180fa0", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745")), "5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745", &[(10327, "\n#[test]\nfn retained_fork_receivers_preserve_immediate_emitter_and_singleton_allocation() {\n    let program = fixture();\n    let function = &program.functions[0];\n    with_limit_usage_refusal(usize::MAX, || {\n        for len in [0, 1, 7, 127] {\n            for count in [0, 1, 2, 5] {\n                let mut old_budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);\n                let (prefixes, suffixes) = inputs(&mut old_work, len, count);\n                let old = with_immediate(|| call(prefixes, &suffixes, &mut old_work)).unwrap();\n                let old_spent = old_work.budget.skeleton_spent;\n                let mut new_budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                let mut new_work = SkeletonWork::new(&program, function, &mut new_budget);\n                let (prefixes, suffixes) = inputs(&mut new_work, len, count);\n                let pointer = prefixes.as_ptr();\n                let capacity = prefixes.capacity();\n                let actual = call(prefixes, &suffixes, &mut new_work).unwrap();\n                assert_eq!(old_spent, new_work.budget.skeleton_spent);\n                if count == 1 {\n                    assert_eq!(actual.as_ptr(), pointer);\n                    assert_eq!(actual.capacity(), capacity);\n                }\n                assert!(equal_paths(&actual, &old, &mut new_work));\n            }\n        }\n        // A real shared singleton, both stopped states and a flat empty suffix\n        // all use the same cardinality proof; no empty input is manufactured.\n        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n        let mut work = SkeletonWork::new(&program, function, &mut budget);\n        let (prefixes, suffixes) = inputs(&mut work, 7, 2);\n        let shared = with_immediate(|| call(prefixes, &suffixes, &mut work)).unwrap();\n        let (prefixes, _) = inputs(&mut work, 3, 0);\n        let mut prefixes = prefixes;\n        prefixes[1].failed = false;\n        prefixes[1].residual = true;\n        let pointer = prefixes.as_ptr();\n        let actual = call(prefixes, &shared[..1], &mut work).unwrap();\n        assert_eq!(actual.as_ptr(), pointer);\n        let (mut prefixes, _) = inputs(&mut work, 3, 0);\n        prefixes[1].failed = false;\n        prefixes[1].residual = true;\n        let old = with_immediate(|| call(prefixes, &shared[..1], &mut work)).unwrap();\n        assert!(equal_paths(&actual, &old, &mut work));\n    });\n}\n\n#[test]\nfn retained_fork_immediate_work_cuts_and_all_diagnostic_fields_match() {\n    let program = fixture();\n    let function = &program.functions[0];\n    // Cuts apply after the same complete paid setup. Both independent emitters\n    // retain the held skeleton loan, including stopped and final-output events.\n    for count in [0, 1, 2, 5] {\n        let attempt = |cut, immediate| {\n            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n            let mut work = SkeletonWork::new(&program, function, &mut budget);\n            let (prefixes, suffixes) = inputs(&mut work, 7, count);\n            work.budget.skeleton_remaining = cut;\n            let before = work.budget.skeleton_spent;\n            let result = if immediate {\n                with_immediate(|| call(prefixes, &suffixes, &mut work))\n            } else {\n                call(prefixes, &suffixes, &mut work)\n            }\n            .map(drop);\n            (result, work.budget.skeleton_spent - before)\n        };\n        for cut in 0..32 {\n            let (old, old_work) = attempt(cut, true);\n            let (new, new_work) = attempt(cut, false);\n            assert_eq!(old_work, new_work);\n            match (old, new) {\n                (Ok(()), Ok(())) => {}\n                (Err(a), Err(b)) => assert_eq!(\n                    (a.code, a.severity, a.message, a.path, a.span, a.help),\n                    (b.code, b.severity, b.message, b.path, b.span, b.help)\n                ),\n                _ => panic!(\"immediate held-work refusal frontier changed\"),\n            }\n        }\n    }\n}\n\n#[test]\nfn retained_fork_keeps_the_entire_immediate_parent_and_old_producer() {\n    let current = include_str!(\"../emission.rs\");\n    let before = retained::before_emission(current);\n    let body = |s: &str| {\n        let begin = s\n            .find(\"pub(in crate::cleanup_plan::replay) fn emit_prepaid(\")\n            .unwrap();\n        let end = s[begin..]\n            .find(\"\\n// The original caller's actual-payer pattern\")\n            .unwrap()\n            + begin;\n        s[begin..end].to_owned()\n    };\n    assert_eq!(body(current), body(&before));\n    // Both the original public receiver invoice and the complete STEP inventory\n    // stay compiled with the old body; no candidate discounts reach that arm.\n    let banks = |s: &str| {\n        let begin = s\n            .find(\"pub(in crate::cleanup_plan::replay) const ENTRY:\")\n            .unwrap();\n        let end = s.find(\"const FREEZE:\").unwrap();\n        s[begin..end].to_owned()\n    };\n    assert_eq!(banks(current), banks(&before));\n}\n\n#[test]\nfn retained_fork_cold_empty_tiny_and_repeated_packets_disclose_actual_costs() {\n    let program = fixture();\n    let function = &program.functions[0];\n    // Direct emitter boundary: deliberately includes cold/unsupported-empty\n    // shapes that the shipping dispatcher can avoid. No app inference follows.\n    for (empty, len, count, repeats) in [\n        (true, 0, 0, 1),\n        (false, 0, 1, 1),\n        (false, 1, 2, 1),\n        (false, 127, 1, 3),\n        (false, 127, 5, 3),\n    ] {\n        let run = |immediate| {\n            with_limit_usage_refusal(usize::MAX, || {\n                measure(|| {\n                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                    let mut work = SkeletonWork::new(&program, function, &mut budget);\n                    let result = (|| {\n                        let (mut paths, suffixes) = inputs(&mut work, len, count);\n                        if empty {\n                            paths.clear();\n                        }\n                        for _ in 0..repeats {\n                            paths = if immediate {\n                                with_immediate(|| call(paths, &suffixes, &mut work))\n                            } else {\n                                call(paths, &suffixes, &mut work)\n                            }?;\n                        }\n                        drop(paths);\n                        drop(suffixes);\n                        Ok::<_, Diagnostic>(())\n                    })();\n                    let work_used = work.budget.skeleton_spent;\n                    drop(work);\n                    drop(budget);\n                    result.map(|()| work_used).map_err(|error| {\n                        let code = error.code;\n                        drop(error);\n                        code\n                    })\n                })\n            })\n        };\n        let ((old, old_heap), old_refused, old_paid, _) = run(true);\n        let ((new, new_heap), new_refused, new_paid, _) = run(false);\n        assert_eq!(old.unwrap(), new.unwrap());\n        assert!(!old_refused && !new_refused);\n        for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n            assert!(!heap.invalid && heap.live == 0);\n            assert_eq!(heap.allocated, heap.freed);\n            assert!(heap.allocated <= paid);\n        }\n        eprintln!(\"925 direct empty={} len={} suffixes={} repeats={} old_paid={} new_paid={} old_heap={:?} new_heap={:?}\",\n            empty,len,count,repeats,old_paid,new_paid,old_heap,new_heap);\n    }\n}\n", "")]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => (24809, "40926dadafd6738359bd52c3a7ba3fb1f9dbc27beebbf19b4a4120aaf9cb405b", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d")), "67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d", &[(20122, "\n// Complete immediate-b23 producer; the sort, independent PLAN producer,\n// comparison and owner destruction are identical to joint's whole boundary.\nfn joint_immediate(\n    program: &ResolvedProgram,\n    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<(), Diagnostic> {\n    let mut expected = with_immediate(|| hir_skeleton_paths(program, function, budget))?;\n    budget.reserve_bytes(\n        function,\n        sequence::ENTRY_CALL,\n        \"whole HIR complete content sort\",\n    )?;\n    sequence::sort(&mut expected, function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::PRODUCE_CALL,\n        \"whole independent PLAN producer\",\n    )?;\n    let produced = view::produce(function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::COMPARE_CALL,\n        \"whole independent history comparison\",\n    )?;\n    view::compare(produced, &expected, budget)\n}\n\n#[test]\nfn retained_fork_whole_hir_sort_plan_net_invoice_and_cold_tiny_are_disclosed() {\n    let mut improvement = false;\n    for program in [fixture(), nested_call_program()] {\n        for function in &program.functions {\n            let run = |immediate, limit, floor| {\n                with_limit_usage_refusal(limit, || {\n                    measure(|| {\n                        assert!(set_active_floor(floor));\n                        let mut budget = ReplayBudget::new();\n                        let result = reserve_program_skeleton_work(\n                            &program,\n                            std::iter::once(function),\n                            &mut budget,\n                        )\n                        .and_then(|_reserved| {\n                            if immediate {\n                                joint_immediate(&program, function, &mut budget)\n                            } else {\n                                joint(&program, function, &mut budget)\n                            }\n                        });\n                        let work = (\n                            budget.skeleton_spent,\n                            budget.skeleton_remaining,\n                            budget.remaining,\n                        );\n                        if result.is_err() {\n                            assert!(budget\n                                .reserve_bytes(function, 0, \"retained whole fork sticky zero\")\n                                .is_err());\n                        }\n                        drop(budget);\n                        result.map(|()| work).map_err(|error| {\n                            let code = error.code;\n                            drop(error);\n                            code\n                        })\n                    })\n                })\n            };\n            let ((before, old_heap), old_refused, old_paid, _) = run(true, usize::MAX, 0);\n            let ((after, new_heap), new_refused, new_paid, _) = run(false, usize::MAX, 0);\n            let old_work = before.unwrap();\n            let new_work = after.unwrap();\n            assert_eq!(old_work, new_work);\n            assert!(!old_refused && !new_refused);\n            for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n                assert!(!heap.invalid && heap.live == 0);\n                assert_eq!(heap.allocated, heap.freed);\n                assert!(heap.allocated <= paid);\n            }\n            improvement |= new_paid < old_paid;\n            eprintln!(\"925 whole function={} old_paid={} new_paid={} old_heap={:?} new_heap={:?} work={:?}\",\n                function.id.as_str(), old_paid, new_paid, old_heap, new_heap, new_work);\n            // Exact/short/floor/zero are independently determined for each full\n            // producer. A changed net invoice never becomes a cap increase.\n            for (immediate, paid) in [(true, old_paid), (false, new_paid)] {\n                for (limit, floor, success) in [\n                    (paid, 0, true),\n                    (paid - 1, 0, false),\n                    (0, 0, false),\n                    (paid, paid, false),\n                ] {\n                    let ((result, heap), refused, _, first) = run(immediate, limit, floor);\n                    assert_eq!(result.is_ok(), success);\n                    assert!(!heap.invalid && heap.live == 0);\n                    assert_eq!(heap.allocated, heap.freed);\n                    if !success {\n                        assert!(refused && first.is_some());\n                        assert_eq!(result.unwrap_err(), \"SPX-H006\");\n                    }\n                }\n            }\n        }\n    }\n    assert!(\n        improvement,\n        \"complete composed receiver must earn a positive net invoice\"\n    );\n}\n", "")]),
        _ => return None,
    };
    assert_eq!(supplied, actual, "complete actual OPT925 source channel");
    assert_eq!(actual.len(), length);
    assert_eq!(digest(actual), sha);
    assert_eq!(digest(target), target_sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join(format!("e/{target_sha}")),
            target.len(),
            target_sha,
            2_097_152
        )
        .unwrap(),
        target
    );
    let mut restored = actual.to_owned();
    for (at, post, prior) in edits.iter().rev() {
        let end = at.checked_add(post.len()).unwrap();
        assert!(*at <= end && end <= restored.len());
        assert!(restored.is_char_boundary(*at) && restored.is_char_boundary(end));
        assert_eq!(&restored[*at..end], *post);
        restored.replace_range(*at..end, prior);
    }
    assert_eq!(restored, target, "entire fixed b23 test source");
    if acquired.is_some() {
        assert_eq!(
            reader::read(
                &root.join(source),
                raw_actual.len(),
                &digest(raw_actual),
                2_097_152
            )
            .unwrap(),
            raw_actual
        );
    }
    Some(restored)
}

// Exact full physical source and named pre-903 view; every old route stays strict.
#[rustfmt::skip]
pub(super) fn constructor903_before(source: &str, supplied: &str) -> Option<String> {
    const FRONT: &str = include_str!("../next_custody_fronts.json");
    let packet = front_pool::decode_physical(FRONT);
    let row = packet["constructor_retention_903"]["records"].get(source)?;
    assert_eq!(packet["constructor_retention_903"]["schema"], "semaprax.constructor-retention-903-source-fronts.v1");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let acquire = |path: &str, length: usize, sha: &str| reader::read(&root.join(path), length, sha, 2_097_152).unwrap();
    let actual = acquire(source, row["current"]["length"].as_u64().unwrap().try_into().unwrap(), row["current"]["sha256"].as_str().unwrap());
    let mut previous = 0; for e in row["edits"].as_array().unwrap() { let a: usize = e["start"].as_u64().unwrap().try_into().unwrap(); let b: usize = e["end"].as_u64().unwrap().try_into().unwrap(); assert!(previous <= a && a <= b && b <= actual.len() && actual.is_char_boundary(a) && actual.is_char_boundary(b)); assert_eq!(&actual[a..b], e["after"].as_str().unwrap()); previous = b; }
    let mut restored = actual.clone(); for e in row["edits"].as_array().unwrap().iter().rev() { restored.replace_range::<std::ops::Range<usize>>(e["start"].as_u64().unwrap().try_into().unwrap()..e["end"].as_u64().unwrap().try_into().unwrap(), e["before"].as_str().unwrap()); }
    assert_eq!(restored, row["before_text"].as_str().unwrap()); assert_eq!(restored.len() as u64, row["target"]["length"]); assert_eq!(digest(&restored), row["target"]["sha256"]);
    assert!(supplied == actual || supplied == restored, "only complete physical or exact named pre-903 input view");
    assert_eq!(acquire(source, actual.len(), &digest(&actual)), actual); assert_eq!(acquire("src/source_reflows/next_custody_fronts.json", FRONT.len(), &digest(FRONT)), FRONT); Some(restored)
}
