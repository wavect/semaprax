//! Full receiver controls for lexical, known-binding and ownership boundaries.
use super::{codes, receive, source};
use crate::diagnostic::Diagnostic;
use std::path::Path;

fn full(source: &str, old: bool) -> Vec<Diagnostic> {
    let program = crate::parse(source, Path::new("recovery-boundary.spx")).unwrap();
    if old {
        crate::source_verify::declaration::old::verify(&program)
    } else {
        crate::source_verify::verify(&program)
    }
}

#[test]
fn noncapturing_and_nested_closure_refusals_remain_complete_original_errors() {
    for body in [
        "let bad = missing; let callback = fn() -> i64 { bad }; 0",
        "let bad = missing; let callback = fn() -> i64 { let nested = fn() -> i64 { bad }; 0 }; 0",
        "let bad = missing; let callback = fn(bad: i64) -> i64 { bad + unrelated }; 0",
        "let bad = missing; let callback = fn(bad: i64) -> i64 { bad }; 0",
    ] {
        let source = source(body);
        assert_eq!(receive(&source, false), receive(&source, true), "{source}");
        assert_eq!(full(&source, false), full(&source, true), "{source}");
    }
}

#[test]
fn existing_function_value_wins_over_a_missing_local_recovery_marker() {
    let source = "module test.recovery_fn; @id(\"chosen\") fn chosen() -> i64 { 3 } @id(\"app.main\") fn main() -> i64 { let chosen = missing; let callback = chosen; callback() }";
    let old = full(source, true);
    let current = full(source, false);
    assert_eq!(codes(&old), ["SPX-T202"]);
    assert_eq!(current, old);
    assert_eq!(receive(source, false), receive(source, true));
}

#[test]
fn owned_and_borrowed_annotation_mismatches_preserve_real_transfer_verdicts() {
    for source in [
        "module test.owned_annotation; @id(\"app.main\") fn main() -> i64 { let held: bool = \"x\"; let moved = string_concat(held, \"y\"); string_len(held) }",
        "module test.borrowed_annotation; @id(\"consume\") fn consume(value: own Bytes) -> i64 { 0 } @id(\"app.main\") fn main(held: borrow Bytes) -> i64 { let alias: bool = held; let moved = consume(alias); 0 }",
        "module test.scalar_annotation; @id(\"app.main\") fn main() -> i64 { let value: Bytes = 1; let next = value + unrelated; 0 }",
    ] {
        let old = full(source, true);
        let current = full(source, false);
        assert!(codes(&old).contains(&"SPX-T232"));
        for diagnostic in &old {
            if diagnostic.code.starts_with("SPX-O") {
                assert!(
                    current.contains(diagnostic),
                    "lost ownership boundary: {source}: {diagnostic:?}"
                );
            }
        }
        if source.contains("owned_annotation") || source.contains("borrowed_annotation") {
            assert_eq!(current, old, "{source}");
            assert!(codes(&current).iter().any(|code| code.starts_with("SPX-O")));
        } else {
            assert!(codes(&current).contains(&"SPX-T202"));
            assert!(!codes(&current).contains(&"SPX-O104"));
        }
    }
}

#[test]
fn valid_binding_and_sibling_scope_are_not_replaced_by_annotation_recovery() {
    for body in [
        "let valid = true; let valid: bool = 1; if valid { 1 } else { 0 }",
        "let ignored = { let bad: bool = 1; if bad { 1 } else { 0 } }; let bad = true; if bad { 1 } else { 0 }",
        "let bad: bool = 1; let callback = fn(bad: i64) -> i64 { if bad { 1 } else { 0 } }; 0",
    ] {
        let source = source(body);
        let old = full(&source, true);
        let current = full(&source, false);
        assert_eq!(current.first(), old.first());
        if body.contains("fn(bad:") {
            assert_eq!(current, old);
        }
        for diagnostic in &current {
            assert!(old.contains(diagnostic));
        }
    }
}

#[test]
fn refused_pattern_names_stay_in_their_own_arm_and_do_not_hide_coverage_or_closures() {
    let prefix = "module test.pattern_recovery; @id(\"choice\") variant Choice { @id(\"choice.one\") One { @id(\"choice.one.n\") n: i64, }, @id(\"choice.two\") Two, } @id(\"app.main\") fn main(value: Choice) -> i64 ";
    for body in [
        "{ match value { Other::One { n } => n + missing, } }",
        "{ match value { Other::One { n } => 0, Choice::Two {} => n, } }",
        "{ match value { Other::One { n } => { let callback = fn() -> i64 { n }; 0 }, } }",
        "{ match value { Choice::One { n } => n, Choice::Two {} => 0, } }",
    ] {
        let source = format!("{prefix}{body}");
        let old = full(&source, true);
        let current = full(&source, false);
        assert_eq!(current.first(), old.first());
        for diagnostic in &old {
            if diagnostic.code == "SPX-M101"
                || diagnostic.code == "SPX-T288"
                || diagnostic.message.contains("unknown value `missing`")
                || body.contains("=> n, }") && diagnostic.message.contains("unknown value `n`")
            {
                assert!(current.contains(diagnostic), "{source}: {diagnostic:?}");
            }
        }
        for diagnostic in &current {
            assert!(old.contains(diagnostic), "{source}");
        }
    }
}

#[test]
fn repeated_source_uses_visit_only_active_scope_rows_and_release_actual_storage() {
    use crate::test_allocation_observer::measure;
    for count in [1usize, 8, 64, 256] {
        let body = format!("let bad = missing; {} 0", "let alias = bad; ".repeat(count));
        // Different authored aliases avoid denied shadow declarations.
        let body = (0..count).fold(body, |text, index| {
            text.replacen("let alias =", &format!("let alias_{index} ="), 1)
        });
        let source = source(&body);
        let program = crate::parse(&source, Path::new("repeated-recovery.spx")).unwrap();
        let current_function = &program.functions[0];
        let functions = program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect();
        let types = super::TypeTable::new(&program);
        let (visited, storage) = measure(|| {
            let mut diagnostics = Vec::new();
            let mut verifier = crate::source_verify::IterativeVerifier::new(
                &program,
                current_function,
                std::collections::HashMap::new(),
                &functions,
                &types,
                None,
                true,
                &mut diagnostics,
            );
            drop(verifier.run(&current_function.body).unwrap());
            let visited = verifier.recovery.visited;
            drop(verifier);
            assert_eq!(codes(&diagnostics), ["SPX-T202"]);
            drop(diagnostics);
            visited
        });
        assert_eq!(visited, count, "one active directory row per dependent use");
        assert!(!storage.invalid);
        assert_eq!(storage.live, 0);
        assert_eq!(storage.allocated, storage.freed);
        let current = full(&source, false);
        let old = full(&source, true);
        assert_eq!(codes(&current), ["SPX-T202"]);
        assert_eq!(old.len(), count + 1);
        assert_eq!(current.first(), old.first());
    }
}

#[test]
fn accepted_source_roundtrip_graph_revision_and_original_receiver_stay_equal() {
    for body in [
        "let value = 1; if true { value + 1 } else { value }",
        "let callback = fn(n: i64) -> i64 { n + 1 }; callback(2)",
        "let mut n = 0; while n < 2 { n = n + 1; 0 } n",
        "match 1 { 0 => 0, n => n, }",
    ] {
        let source = source(body);
        assert_eq!(full(&source, false), full(&source, true));
        assert!(full(&source, false).is_empty(), "{source}");
        assert_eq!(receive(&source, false), receive(&source, true));
        let program = crate::parse(&source, Path::new("accepted-recovery.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let reparsed = crate::parse(&canonical, Path::new("accepted-recovery.spx")).unwrap();
        assert_eq!(canonical, crate::format::canonical(&reparsed));
        assert_eq!(
            crate::graph::revision(&program),
            crate::graph::revision(&reparsed)
        );
        assert!(crate::source_verify::verify(&reparsed).is_empty());
    }
}

#[test]
fn initializer_completion_and_loop_ended_locals_keep_exact_root_or_independent_errors() {
    for body in [
        "let outer = { let inner = missing; inner }; outer",
        "let outer = { let inner = outer; inner }; outer",
    ] {
        let source = source(body);
        let old = receive(&source, true);
        let current = receive(&source, false);
        assert_eq!(codes(&current.1), ["SPX-T202"]);
        assert_eq!(current.1.first(), old.1.first());
        assert_eq!(old.1.len(), 3);
    }
    let source = source("while false { let ended = missing; 0 } ended");
    assert_eq!(receive(&source, false), receive(&source, true));
    assert_eq!(codes(&receive(&source, false).1), ["SPX-T202", "SPX-T202"]);
}

#[test]
fn unknown_record_keeps_duplicate_fields_and_independent_initializer_errors() {
    let source = source("let ignored = Missing { x: missing, x: 1 }; 0");
    let old = full(&source, true);
    let current = full(&source, false);
    assert_eq!(codes(&current), ["SPX-T001", "SPX-T202", "SPX-T212"]);
    assert_eq!(current.first(), old.first());
    let mut remaining = old.iter();
    for diagnostic in &current {
        assert!(remaining.any(|original| original == diagnostic));
    }
}

#[test]
fn unchanged_recursive_receiver_keeps_its_original_scalar_verdicts() {
    for body in [
        "let bad = missing; bad",
        "let bad = missing; let alias = bad; alias + bad",
        "let bad = missing; if true { bad } else { bad }",
        "let bad = missing; while false { let alias = bad; 0 } bad",
        "let before = later; let later = 1; before",
        "let self_value = self_value; self_value",
        "let ignored = { let ended = missing; 0 }; ended",
        "let bad = -true; bad",
        "let bad = !1; bad",
        "let bad = \"a\" + \"b\"; bad",
        "let held = 1; let held = missing; held",
    ] {
        let source = source(body);
        assert_eq!(
            super::receive_recursive(&source),
            receive(&source, true),
            "{source}"
        );
    }
}

#[test]
fn overlapping_signature_spans_cannot_poison_a_different_known_binding() {
    use crate::source_verify::binding::{Availability, Binding};
    use crate::source_verify::recovery::Recovery;
    use std::collections::{BTreeSet, HashMap, HashSet};
    let mut program = crate::parse("module test.signature_spans; @id(\"probe\") fn probe(bad: int, good: bool) -> i64 { 0 } @id(\"app.main\") fn main() -> i64 { 0 }", Path::new("overlapping-recovery.spx")).unwrap();
    let current = &mut program.functions[0];
    current.params[1].span = current.params[0].span;
    let current = &program.functions[0];
    let types = super::TypeTable::new(&program);
    let mut diagnostics = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &current.params[0].ty,
        current.params[0].span,
        &types,
        &HashSet::new(),
        &mut diagnostics,
    );
    let variables = current
        .params
        .iter()
        .map(|parameter| {
            (
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
            )
        })
        .collect();
    let mut recovery = Recovery::signature(&program, current, &variables, &types, &diagnostics);
    assert!(recovery.reads_known_refused(0, "bad"));
    assert!(!recovery.reads_known_refused(0, "good"));
}
