use crate::byte_data_capacity::*;
use std::collections::BTreeMap;
fn write(site: &str) -> CapacityFlow {
    CapacityFlow::StdoutWrite {
        site: site.into(),
        source: TranscriptSource::Fixed(1),
    }
}
fn call(site: &str, callee: &str) -> CapacityFlow {
    CapacityFlow::Call {
        site: site.into(),
        callee: callee.into(),
    }
}
fn input(function: &str, execution: CapacityFlow) -> FunctionCapacityInput {
    FunctionCapacityInput {
        function: function.into(),
        array_slots: Vec::new(),
        execution,
    }
}
fn old_flow(flow: &CapacityFlow) -> before::CapacityFlow {
    use before::CapacityFlow as Old;
    let boxed = |node: &CapacityFlow| Box::new(old_flow(node));
    match flow {
        CapacityFlow::Empty => Old::Empty,
        CapacityFlow::Sequence(children) => Old::Sequence(children.iter().map(old_flow).collect()),
        CapacityFlow::Alternative(children) => {
            Old::Alternative(children.iter().map(old_flow).collect())
        }
        CapacityFlow::Call { site, callee } => Old::Call {
            site: site.clone(),
            callee: callee.clone(),
        },
        CapacityFlow::BytesCopy {
            site,
            conservative_payload_bytes,
        } => Old::BytesCopy {
            site: site.clone(),
            conservative_payload_bytes: *conservative_payload_bytes,
        },
        CapacityFlow::StdinRead {
            site,
            conservative_payload_bytes,
        } => Old::StdinRead {
            site: site.clone(),
            conservative_payload_bytes: *conservative_payload_bytes,
        },
        CapacityFlow::StdoutWrite { site, source } => Old::StdoutWrite {
            site: site.clone(),
            source: old_source(*source),
        },
        CapacityFlow::StderrWrite { site, source } => Old::StderrWrite {
            site: site.clone(),
            source: old_source(*source),
        },
        CapacityFlow::StdoutAppend { site } => Old::StdoutAppend { site: site.clone() },
        CapacityFlow::StderrAppend { site } => Old::StderrAppend { site: site.clone() },
        CapacityFlow::Loop { condition, body } => Old::Loop {
            condition: boxed(condition),
            body: boxed(body),
        },
        CapacityFlow::FiniteOwnedLoop {
            iterations,
            condition,
            body,
        } => Old::FiniteOwnedLoop {
            iterations: *iterations,
            condition: boxed(condition),
            body: boxed(body),
        },
        CapacityFlow::FusedConstruction {
            producer,
            rows,
            width,
            condition,
            body,
        } => Old::FusedConstruction {
            producer: producer.clone(),
            rows: *rows,
            width: *width,
            condition: boxed(condition),
            body: boxed(body),
        },
    }
}
fn old_source(source: TranscriptSource) -> before::TranscriptSource {
    match source {
        TranscriptSource::Fixed(n) => before::TranscriptSource::Fixed(n),
        TranscriptSource::CommandArguments => before::TranscriptSource::CommandArguments,
        TranscriptSource::Stdin => before::TranscriptSource::Stdin,
        TranscriptSource::Unknown => before::TranscriptSource::Unknown,
    }
}
fn old_inputs(inputs: &[FunctionCapacityInput]) -> Vec<before::FunctionCapacityInput> {
    inputs
        .iter()
        .map(|function| before::FunctionCapacityInput {
            function: function.function.clone(),
            array_slots: function
                .array_slots
                .iter()
                .map(|slot| before::ArrayStorageSlot {
                    identity: slot.identity.clone(),
                    kind: match slot.kind {
                        ArrayStorageKind::Parameter => before::ArrayStorageKind::Parameter,
                        ArrayStorageKind::Binding => before::ArrayStorageKind::Binding,
                        ArrayStorageKind::Temporary => before::ArrayStorageKind::Temporary,
                        ArrayStorageKind::CallStaging => before::ArrayStorageKind::CallStaging,
                        ArrayStorageKind::ProvisionalResult => {
                            before::ArrayStorageKind::ProvisionalResult
                        }
                    },
                    length: slot.length,
                })
                .collect(),
            execution: old_flow(&function.execution),
        })
        .collect()
}
fn refusal(inputs: &[FunctionCapacityInput]) -> CapacityError {
    let old = before::analyze(&old_inputs(inputs)).unwrap_err();
    let current = analyze(inputs).unwrap_err();
    assert_eq!(current.diagnostic.code(), old.diagnostic.code());
    assert_eq!(current.function, old.function);
    assert_eq!(current.detail, old.detail);
    assert_eq!(current.to_string(), old.to_string());
    current
}
#[test]
fn evaluated_nested_call_witness_keeps_alternatives_distinct_and_full_old_primary() {
    let branches = [input(
        "main",
        CapacityFlow::Alternative(vec![write("left"), write("right")]),
    )];
    assert_eq!(
        format!("{:?}", analyze(&branches).unwrap()),
        format!("{:?}", before::analyze(&old_inputs(&branches)).unwrap())
    );
    let inputs = [
        input(
            "main",
            CapacityFlow::Sequence(vec![
                call("first.call", "写"),
                CapacityFlow::Alternative(vec![CapacityFlow::Empty, call("second.call", "写")]),
            ]),
        ),
        input("写", write("输出")),
    ];
    let selected = refusal(&inputs);
    let help = selected.witness_help.unwrap();
    assert!(help.contains("first.call") && help.contains("second.call"));
    assert_eq!(help.matches("stdout_write").count(), 2);
    assert!(help.contains("写") && help.contains("输出"));
    assert!(help.find("first.call").unwrap() < help.find("second.call").unwrap());
}
#[test]
fn actual_loop_component_names_indirect_writer_not_allowed_direct_append() {
    let inputs = [
        input(
            "main",
            CapacityFlow::Loop {
                condition: Box::new(CapacityFlow::Empty),
                body: Box::new(CapacityFlow::Sequence(vec![
                    CapacityFlow::StdoutAppend {
                        site: "allowed.direct".into(),
                    },
                    call("indirect.site", "writer"),
                ])),
            },
        ),
        input("writer", write("actual.write")),
    ];
    let help = refusal(&inputs).witness_help.unwrap();
    assert!(
        help.contains("enclosing loop body")
            && help.contains("indirect.site")
            && help.contains("actual.write")
    );
    assert!(!help.contains("allowed.direct"));
    let condition = [input(
        "condition",
        CapacityFlow::Loop {
            condition: Box::new(write("condition.write")),
            body: Box::new(CapacityFlow::Empty),
        },
    )];
    assert!(refusal(&condition)
        .witness_help
        .unwrap()
        .contains("loop condition"));
}
#[test]
fn genuine_closed_mutual_cycle_witness_excludes_noncyclic_residual_tail() {
    let inputs = [
        input(
            "a",
            CapacityFlow::Sequence(vec![call("a.b", "b"), call("a.tail", "tail")]),
        ),
        input(
            "b",
            CapacityFlow::Sequence(vec![call("b.a", "a"), write("cycle.write")]),
        ),
        input("tail", write("tail.write")),
    ];
    let help = refusal(&inputs).witness_help.unwrap();
    assert!(help.contains("\"a\" -> \"b\" -> \"a\"") || help.contains("\"b\" -> \"a\" -> \"b\""));
    assert!(!help.contains("tail"));
}
#[test]
fn optional_witness_exact_short_zero_and_sticky_receiving_drop_are_paid() {
    let inputs = [input(
        "main",
        CapacityFlow::Sequence(vec![write("α"), write("β")]),
    )];
    let by_id = inputs
        .iter()
        .map(|f| (f.function.as_str(), f))
        .collect::<BTreeMap<_, _>>();
    let run = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            crate::test_allocation_observer::measure(|| {
                let text = witnesses::repeated(
                    &inputs[0].execution,
                    "main",
                    &by_id,
                    witnesses::Query::Stdout,
                );
                let admitted = text.is_some();
                drop(text);
                admitted
            })
        })
    };
    let ((admitted, physical), refused, paid) = run(usize::MAX);
    assert!(admitted && !refused && !physical.invalid);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
    assert!(physical.allocated <= paid);
    let ((exact, exact_physical), refused, exact_paid) = run(paid);
    assert!(exact && !refused);
    assert_eq!(exact_paid, paid);
    assert_eq!(exact_physical, physical);
    for limit in [0, paid - 1] {
        let ((admitted, physical), refused, _) = run(limit);
        assert!(!admitted && !refused && !physical.invalid);
        assert_eq!(physical.live, 0);
        assert_eq!(physical.allocated, physical.freed);
    }
    let ((admitted, physical), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            crate::test_allocation_observer::measure(|| {
                let first = witnesses::repeated(
                    &inputs[0].execution,
                    "main",
                    &by_id,
                    witnesses::Query::Stdout,
                );
                let second = witnesses::repeated(
                    &inputs[0].execution,
                    "main",
                    &by_id,
                    witnesses::Query::Stdout,
                );
                let admitted = first.is_some() || second.is_some();
                drop(first);
                drop(second);
                admitted
            })
        });
    assert!(refused && !admitted);
    assert_eq!(paid, 0);
    assert_eq!(physical.allocated, 0);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, 1);
    assert_eq!(receipt.remaining, 0);
}

#[test]
fn source_and_independent_hostile_hir_projection_keep_primary_and_real_write_sites() {
    const SOURCE: &str = r#"module witness.projection;
permit { process.stdout.write }
@id("emit") fn emit(view: borrow Slice<u8>) -> usize uses { process.stdout.write } {
    if true { stdout_write(view) } else { stdout_write(view) }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(SOURCE, std::path::Path::new("witness.spx")).unwrap();
    let mut program = crate::hir::resolve(&ast).unwrap();
    let admitted = crate::hir::byte_data_capacity_inputs(&program).unwrap();
    assert_eq!(
        format!("{:?}", analyze(&admitted).unwrap()),
        format!("{:?}", before::analyze(&old_inputs(&admitted)).unwrap())
    );
    let emit = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "emit")
        .unwrap();
    let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut emit.body.kind else {
        panic!("checked function block")
    };
    let crate::hir::ResolvedExprKind::If {
        then_branch,
        else_branch,
        ..
    } = tail.kind.clone()
    else {
        panic!("checked alternative")
    };
    let span = then_branch.span;
    tail.kind = crate::hir::ResolvedExprKind::Block {
        statements: vec![crate::hir::ResolvedStatement::CopyDiscard {
            value: *then_branch,
            span,
        }],
        tail: else_branch,
    };
    // Hostile independent HIR projection only: no canonical-plan/validator
    // authority is granted by this changed tree.
    let hostile = crate::hir::byte_data_capacity_inputs(&program).unwrap();
    let error = refusal(&hostile);
    let independent = crate::hir::analyze_byte_data_capacity(&program).unwrap_err();
    assert_eq!(independent.code, error.diagnostic.code());
    assert_eq!(independent.message.as_ref(), error.to_string());
    assert_eq!(independent.help, error.witness_help);
    let bad = SOURCE.replace(
        "if true { stdout_write(view) } else { stdout_write(view) }",
        "let first = stdout_write(view); stdout_write(view)",
    );
    let source_errors = crate::check(&bad, "witness.spx").unwrap_err();
    let source = source_errors.iter().find(|d| d.code == "SPX-T269").unwrap();
    assert_eq!(source.message.as_ref(), error.detail);
    use crate::ast::program_source::ProgramSource;
    let parsed_bad = crate::parse(&bad, std::path::Path::new("witness.spx")).unwrap();
    let expected_span = parsed_bad
        .functions()
        .iter()
        .find(|f| f.stable_id == "emit")
        .unwrap()
        .span;
    assert_eq!(source.span, Some(expected_span));
    assert_eq!(source.path.as_deref(), Some("witness.spx"));
    assert_eq!(source.severity, crate::diagnostic::Severity::Error);
    assert!(source.help.as_ref().unwrap().contains("first two writes"));
}

#[test]
fn wide_late_branch_spill_and_long_identity_omission_keep_full_old_refusal() {
    let mut branches = (0..80).map(|_| CapacityFlow::Empty).collect::<Vec<_>>();
    branches.push(CapacityFlow::Sequence(vec![
        write("late.first"),
        write("late.second"),
    ]));
    let inputs = [input("main", CapacityFlow::Alternative(branches))];
    let old_current = refusal(&inputs);
    assert!(old_current.witness_help.unwrap().contains("late.first"));
    let by_id = inputs
        .iter()
        .map(|f| (f.function.as_str(), f))
        .collect::<BTreeMap<_, _>>();
    let (((), physical), refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            crate::test_allocation_observer::measure(|| {
                let help = witnesses::repeated(
                    &inputs[0].execution,
                    "main",
                    &by_id,
                    witnesses::Query::Stdout,
                )
                .unwrap();
                assert!(help.contains("late.second"));
                drop(help);
            })
        });
    assert!(!refused && !physical.invalid);
    assert!(physical.reallocations > 0);
    assert!(physical.allocated <= paid);
    assert_eq!(physical.allocated, physical.freed);
    assert_eq!(physical.live, 0);
    let long = [input(
        "main",
        CapacityFlow::Sequence(vec![write(&"界".repeat(4096)), write("other")]),
    )];
    let selected = refusal(&long);
    assert!(selected.witness_help.is_none());
    let mut deep = (0..18)
        .map(|n| {
            input(
                &format!("f{n}"),
                call(&format!("c{n}"), &format!("f{}", n + 1)),
            )
        })
        .collect::<Vec<_>>();
    deep.push(input("f18", write("leaf")));
    deep.push(input(
        "main",
        CapacityFlow::Sequence(vec![call("first", "f0"), call("second", "f0")]),
    ));
    assert!(refusal(&deep).witness_help.is_none());
    let forged = [input(
        "main",
        CapacityFlow::Sequence(vec![write("same"), write("same")]),
    )];
    let forged = refusal(&forged);
    assert_eq!(forged.diagnostic, CapacityDiagnostic::Invariant);
    assert!(forged.witness_help.is_none());
}

#[test]
fn complete_original_receivers_and_current_named_hook_projection_are_source_exact() {
    use sha2::{Digest, Sha256};
    let projection = include_str!(
        "../../docs/evidence/transcript-flow-witness-821/full-current-hook-projection.json"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(projection.as_bytes()))
        ),
        "2290f7fdd985b3cc33e382db9eedb9f65bb7b25bf95c2ed475aec07b639bd884"
    );
    let rows: serde_json::Value = serde_json::from_str(projection).unwrap();
    let originals = [
        ("src/byte_data_capacity.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__byte_data_capacity.rs.before.txt"), include_str!("../../src/byte_data_capacity.rs")),
        ("src/hir/byte_capacity.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__hir__byte_capacity.rs.before.txt"), include_str!("../../src/hir/byte_capacity.rs")),
        ("src/source_verify/declaration.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__source_verify__declaration.rs.before.txt"), include_str!("../../src/source_verify/declaration.rs")),
        ("src/source_verify/capacity/command_io.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__source_verify__capacity__command_io.rs.before.txt"), include_str!("../../src/source_verify/capacity/command_io.rs")),
        ("src/source_verify/capacity/fused_selection.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__source_verify__capacity__fused_selection.rs.before.txt"), include_str!("../../src/source_verify/capacity/fused_selection.rs")),
        ("src/hir/byte_capacity/fused_selection.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__hir__byte_capacity__fused_selection.rs.before.txt"), include_str!("../../src/hir/byte_capacity/fused_selection.rs")),
        ("src/byte_data_capacity/fused.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__byte_data_capacity__fused.rs.before.txt"), include_str!("../../src/byte_data_capacity/fused.rs")),
        ("src/byte_data_capacity/tests.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__byte_data_capacity__tests.rs.before.txt"), include_str!("../../src/byte_data_capacity/tests.rs")),
        ("src/byte_data_capacity/fused_tests.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__byte_data_capacity__fused_tests.rs.before.txt"), include_str!("../../src/byte_data_capacity/fused_tests.rs")),
        ("src/lib.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/src__lib.rs.before.txt"), include_str!("../../src/lib.rs")),
        ("tests/cleanup_backends.rs", include_str!("../../docs/evidence/transcript-flow-witness-821/tests__cleanup_backends.rs.before.txt"), include_str!("../../tests/cleanup_backends.rs")),
    ];
    for row in rows.as_array().unwrap() {
        let (_, original, current) = originals
            .iter()
            .find(|(path, _, _)| *path == row["source"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["original_sha256"].as_str().unwrap()
        );
        let lines = original.split_inclusive('\n').collect::<Vec<_>>();
        let mut expected = String::new();
        let mut at = 0;
        for patch in row["patch"].as_array().unwrap() {
            let start = patch["old_start"].as_u64().unwrap() as usize;
            let end = patch["old_end"].as_u64().unwrap() as usize;
            for line in &lines[at..start] {
                expected.push_str(line);
            }
            expected.push_str(patch["replacement"].as_str().unwrap());
            at = end;
        }
        for line in &lines[at..] {
            expected.push_str(line);
        }
        let acquired = if expected.as_str() == *current {
            current.to_string()
        } else {
            crate::source_reflows::original_fixture_source(
                row["source"].as_str().unwrap(),
                &format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(expected.as_bytes()))
                ),
                current,
            )
        };
        assert_eq!(expected, acquired);
    }
    let original = include_str!(
        "../../docs/evidence/transcript-flow-witness-821/byte_data_capacity.rs.before.txt"
    );
    let compiled = crate::source_reflows::original_fixture_source(
        "src/byte_data_capacity/before.rs",
        "b8c2763e47884f92c44adacba73fa7a509f1998c1cb2faff3ab197652451780c",
        include_str!("before.rs"),
    );
    assert_eq!(
        compiled,
        original
            .replace("\"byte_data_capacity/fused.rs\"", "\"fused.rs\"")
            .replace("\"byte_data_capacity/tests.rs\"", "\"tests.rs\"")
            .replace(
                "\"byte_data_capacity/fused_tests.rs\"",
                "\"fused_tests.rs\""
            )
    );
}

#[test]
fn optional_owned_help_clone_is_paid_and_omits_after_sticky_refusal() {
    let inputs = [input(
        "main",
        CapacityFlow::Sequence(vec![write("first"), write("second")]),
    )];
    let selected = refusal(&inputs);
    let original = selected.witness_help.as_deref().unwrap();
    let (((), physical), refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            crate::test_allocation_observer::measure(|| {
                let copy = witnesses::clone_help(original).unwrap();
                assert_eq!(copy, original);
                drop(copy);
            })
        });
    assert!(!refused && !physical.invalid);
    assert!(physical.allocated <= paid);
    assert_eq!(physical.allocated, physical.freed);
    assert_eq!(physical.live, 0);
    let (copy, refused, _) = crate::bounded_output::with_limit_usage(0, || selected.clone());
    assert!(!refused && copy.witness_help.is_none());
    assert_eq!(copy.diagnostic, selected.diagnostic);
    assert_eq!(copy.function, selected.function);
    assert_eq!(copy.detail, selected.detail);
    let (copy, refused, _, receipt) = crate::bounded_output::with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        selected.clone()
    });
    assert!(refused && copy.witness_help.is_none());
    assert_eq!(receipt.unwrap().requested, 1);
    assert_eq!(copy.to_string(), selected.to_string());
}

#[test]
fn reservation_only_facade_uses_existing_payer_zero_and_sticky_refusal() {
    let ((), refused, paid) = crate::bounded_output::with_limit_usage(9, || {
        assert!(!crate::reserve_transcript_witness_preserving(10, 0));
        assert!(crate::reserve_transcript_witness_preserving(4, 0));
        assert!(crate::bounded_output::reserve_active_required(5));
        assert!(crate::reserve_transcript_witness_preserving(0, 0));
    });
    assert!(!refused);
    assert_eq!(paid, 9);
    let ((), refused, paid, receipt) = crate::bounded_output::with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert!(!crate::reserve_transcript_witness_preserving(0, 0));
    });
    assert!(refused);
    assert_eq!(paid, 0);
    assert_eq!(receipt.unwrap().requested, 1);
}
