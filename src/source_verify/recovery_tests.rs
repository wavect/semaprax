//! Complete original iterative receiver against intentional recovery deltas,
//! with the unchanged recursive receiver retained independently.
use super::binding::{Availability, Binding};
use super::check_expr;
use super::recovery::Recovery;
use super::type_table::TypeTable;
#[path = "recovery_tests/boundaries.rs"]
mod boundaries;
#[path = "recovery_tests/callee_components.rs"]
mod callee_components;
#[path = "recovery_tests/extended_components.rs"]
mod extended_components;
#[path = "recovery_tests/fixture_controls.rs"]
mod fixture_controls;
#[path = "recovery_tests/flow_799.rs"]
mod flow_799;
#[path = "recovery_tests/funding_boundary.rs"]
mod funding_boundary;
#[path = "recovery_tests/intentional_wrong_field.rs"]
pub(super) mod intentional_wrong_field;
#[path = "recovery_tests/operators_799.rs"]
mod operators_799;
#[path = "recovery_tests/proposal_corpus.rs"]
mod proposal_corpus;
#[path = "recovery_tests/refusal_dependency.rs"]
mod refusal_dependency;
use crate::ast::{ParamMode, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::test_allocation_observer::measure;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

fn receive(source: &str, old: bool) -> (Option<(Type, ParamMode, bool)>, Vec<Diagnostic>) {
    receive_route(source, u8::from(old))
}

fn receive_recursive(source: &str) -> (Option<(Type, ParamMode, bool)>, Vec<Diagnostic>) {
    receive_route(source, 2)
}

fn receive_route(source: &str, receiver: u8) -> (Option<(Type, ParamMode, bool)>, Vec<Diagnostic>) {
    let program = crate::parse(source, Path::new("scoped-recovery.spx")).unwrap();
    let current = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let types = TypeTable::new(&program);
    let mut variables = HashMap::new();
    for parameter in &current.params {
        variables.insert(
            parameter.name.clone(),
            Binding {
                contract_local: false,
                traversal_row: None,
                ty: parameter.ty.clone(),
                mode: parameter.mode,
                availability: Availability::Available,
                moved_places: HashMap::new(),
                definitely_partial: HashSet::new(),
                native_unit_discard: false,
                mutable: false,
                active_loans: BTreeSet::new(),
                borrow_origin: None,
            },
        );
    }
    let mut diagnostics = Vec::new();
    let actual = if receiver == 2 {
        // The entire original recursive receiver and all its children remain
        // unchanged, including old inferred types and dependent diagnostics.
        check_expr(
            &program,
            current,
            &current.body,
            &mut variables,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
        )
    } else if receiver == 3 {
        let mut verifier = super::IterativeVerifier::new(
            &program,
            functions
                .get(current.name.as_str())
                .expect("frozen current declaration cell"),
            variables,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
        );
        verifier.operators_before_799 = true;
        verifier.run(&current.body).unwrap()
    } else if receiver == 1 {
        super::recovery_legacy::check_expr_iterative_legacy(
            &program,
            current,
            &current.body,
            &mut variables,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
        )
    } else {
        super::iterative::check_expr_iterative(
            &program,
            current,
            &current.body,
            &mut variables,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
        )
    };
    (
        actual.map(|value| (value.ty, value.mode, value.native_unit)),
        diagnostics,
    )
}

fn source(body: &str) -> String {
    format!("module test.scoped_recovery; @id(\"app.main\") fn main() -> i64 {{ {body} }}")
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn scoped_refused_let_retains_original_root_and_independent_reads() {
    for body in [
        "let bad = missing; bad",
        "let bad = missing; let alias = bad; alias + bad",
        "let bad = missing; if true { bad } else { bad }",
        "let bad = missing; while false { let alias = bad; 0 } bad",
    ] {
        let source = source(body);
        let (_, old) = receive(&source, true);
        let (_, current) = receive(&source, false);
        assert!(old.len() > current.len(), "{source}: {old:?} / {current:?}");
        assert_eq!(codes(&current), ["SPX-T202"], "{source}: {current:?}");
        assert_eq!(current[0], old[0]);
    }
    let source = source("let bad = missing; bad + unrelated");
    let (_, old) = receive(&source, true);
    let (_, current) = receive(&source, false);
    assert_eq!(codes(&old), ["SPX-T202", "SPX-T202", "SPX-T202"]);
    assert_eq!(codes(&current), ["SPX-T202", "SPX-T202"]);
    assert_eq!(current, [old[0].clone(), old[2].clone()]);
}

#[test]
fn read_before_self_initializer_siblings_and_noncapturing_closures_remain_independent() {
    for body in [
        "let before = later; let later = 1; before",
        "let self_value = self_value; self_value",
        "let ignored = { let ended = missing; 0 }; ended",
        "let ignored = if true { let ended = missing; 0 } else { ended }; 0",
        "let bad = missing; let callback = fn() -> i64 { bad }; 0",
        "let bad = missing; let callback = fn(bad: i64) -> i64 { bad }; 0",
    ] {
        let source = source(body);
        let (_, old) = receive(&source, true);
        let (_, current) = receive(&source, false);
        assert_eq!(current.first(), old.first(), "{source}");
        // Every original error outside the precise refused binding's reads
        // remains byte-for-byte present, including the noncapturing closure.
        for diagnostic in &old {
            if diagnostic.message.contains("unknown value `ended`")
                || diagnostic.message.contains("unknown value `later`")
                || diagnostic.message.contains("unknown value `self_value`")
                    && diagnostic.span == old.first().and_then(|first| first.span)
                || diagnostic.message.contains("unknown value `bad`") && body.contains("fn()")
            {
                assert!(
                    current.contains(diagnostic),
                    "{source}: {diagnostic:?} / {current:?}"
                );
            }
        }
    }
}

#[test]
fn existing_bindings_function_values_moves_and_match_coverage_keep_complete_old_verdicts() {
    for body in [
        "let held = 1; let held = missing; held",
        "let bad = missing; let bad = 1; bad",
        "let ignored = { let bad = missing; 0 }; let bad = 1; bad",
        "if true { let local = missing; 0 } else { local }",
        "match 1 { 0 => missing, n => n, }",
    ] {
        let source = source(body);
        let old = receive(&source, true);
        let current = receive(&source, false);
        if body == "let bad = missing; let bad = 1; bad" {
            assert_eq!(
                old.0,
                Some((Type::I64, crate::ast::ParamMode::Value, false))
            );
            assert_eq!(codes(&old.1), ["SPX-T202"]);
            assert_eq!(current.0, None);
            let program = crate::parse(&source, Path::new("scoped-recovery.spx")).unwrap();
            let crate::ast::ExprKind::Block { statements, .. } = &program.functions[0].body.kind
            else {
                panic!("block");
            };
            let crate::ast::Statement::Let { name_span, .. } = &statements[1] else {
                panic!("let");
            };
            assert_eq!(
                current.1,
                vec![
                    old.1[0].clone(),
                    Diagnostic::error(
                        "SPX-T209",
                        "local binding `bad` shadows an existing value",
                        *name_span
                    )
                    .at_path("scoped-recovery.spx")
                    .with_help(crate::source_verify::hints::SHADOW_HELP)
                ]
            );
        } else {
            assert_eq!(current, old, "{source}");
        }
    }
    let source = "module test.coverage; @id(\"choice\") variant Choice { @id(\"choice.one\") One { @id(\"choice.one.n\") n: i64, }, @id(\"choice.two\") Two, } @id(\"app.main\") fn main(value: Choice) -> i64 { match value { Other::One { n } => n, } }";
    let old = receive(source, true);
    let current = receive(source, false);
    assert_eq!(codes(&old.1), ["SPX-M103", "SPX-T202", "SPX-M101"]);
    assert_eq!(codes(&current.1), ["SPX-M103", "SPX-M101"]);
    assert_eq!(current.1, [old.1[0].clone(), old.1[2].clone()]);
}

#[test]
fn refused_operators_do_not_publish_synthetic_checked_types_and_keep_first_diagnostic() {
    for (body, code) in [
        ("let bad = \"a\" + \"b\"; bad", "SPX-T250"),
        ("let bad = -true; bad", "SPX-T206"),
        ("let bad = !1; bad", "SPX-T206"),
    ] {
        let source = source(body);
        let old = receive(&source, true);
        let current = receive(&source, false);
        assert!(old.0.is_some(), "saved original inferred value: {old:?}");
        assert!(current.0.is_none());
        assert_eq!(codes(&current.1), [code]);
        assert_eq!(current.1.first(), old.1.first());
    }
}

#[test]
fn lazy_directory_reuses_ordinals_and_never_scans_authored_function_nodes() {
    let diagnostic = Diagnostic::error("SPX-T202", "missing root", Span::default());
    let diagnostics = vec![diagnostic];
    let mut recovery = Recovery::default();
    recovery.fork(Some(0), 1);
    assert_eq!(recovery.checkpoint(&[]).diagnostics, 0);
    // No successful scope fork acquires backing before the first refusal.
    let (_, empty) = measure(|| {
        let mut directory = Recovery::default();
        for index in 1..1024 {
            directory.fork(Some(index - 1), index);
        }
        drop(directory);
    });
    assert_eq!(empty.allocated, 0);
    assert_eq!(empty.live, 0);
    let checkpoint = recovery.checkpoint(&[]);
    recovery.refused_let(1, "bad", checkpoint, &diagnostics);
    for index in 2..65 {
        recovery.fork(Some(index - 1), index);
    }
    for _ in 0..128 {
        assert!(recovery.reads_refused(64, "bad"));
    }
    assert_eq!(recovery.visited, 128 * 64);
    recovery.fork(Some(0), 1);
    assert!(!recovery.reads_refused(1, "bad"));
    recovery.fork(None, 2);
    assert!(!recovery.reads_refused(2, "bad"));
}

#[test]
fn directory_and_full_receivers_observe_real_acquisition_and_receiving_drop() {
    let diagnostics = vec![Diagnostic::error(
        "SPX-T202",
        "missing root",
        Span::default(),
    )];
    let (_, storage) = measure(|| {
        let mut recovery = Recovery::default();
        let checkpoint = recovery.checkpoint(&[]);
        recovery.refused_let(0, "bad", checkpoint, &diagnostics);
        for index in 1..65 {
            recovery.fork(Some(index - 1), index);
        }
        for _ in 0..128 {
            assert!(recovery.reads_refused(64, "bad"));
        }
        drop(recovery);
    });
    assert!(!storage.invalid);
    assert!(storage.allocated > 0);
    assert_eq!(storage.live, 0);
    assert_eq!(
        storage.allocated, storage.freed,
        "directory storage allocated/freed mismatch: {storage:?}"
    );
    // `prelude::declarations()` is a process-lived OnceLock that the first
    // receiver would otherwise initialize inside the measured scope, leaving its
    // owner live after Drop. Hold exactly that immutable table before measuring;
    // the verifier, type table and receivers are never warmed.
    let held = crate::prelude::declarations();
    let identity = (held.as_ptr(), held.len());
    for body in [
        "let bad = missing; let alias = bad; alias + bad",
        "let bad = missing; let callback = fn() -> i64 { bad }; 0",
    ] {
        let source = source(body);
        for old in [true, false] {
            let (_, receiver) = measure(|| {
                drop(receive(&source, old));
            });
            assert!(
                !receiver.invalid,
                "old={old}: invalid observation with prelude held: {receiver:?}"
            );
            assert_eq!(
                receiver.live, 0,
                "old={old}: live bytes after receiving Drop with prelude held: {receiver:?}"
            );
            assert_eq!(
                receiver.allocated, receiver.freed,
                "old={old}: allocated/freed mismatch with prelude held: {receiver:?}"
            );
            let after = crate::prelude::declarations();
            assert_eq!(identity, (after.as_ptr(), after.len()), "old={old}");
            assert!(std::ptr::eq(held, after), "old={old}");
        }
    }
}
