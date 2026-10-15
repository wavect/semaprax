//! Annotation refusal is an expected-type explanation, never a replacement
//! checked type, owner, place state, or lexical loan.
use super::{codes, receive, source};
use crate::ast::{ParamMode, Type};
use crate::diagnostic::Diagnostic;
use std::path::Path;

pub(super) const CALLS: &str = r#"
@id("bool.id") fn bool_id(value: bool) -> bool { value }
@id("bool.consume") fn f_bool(value: bool) -> i64 { 0 }
@id("string.consume") fn f_string(value: string) -> i64 { 0 }
@id("bytes.consume") fn consume_bytes(value: own Bytes) -> i64 { 0 }
"#;

fn module(body: &str) -> String {
    format!(
        "module test.refusal_dependency; {CALLS} @id(\"app.main\") fn main() -> i64 {{ {body} }}"
    )
}

fn full(source: &str, old: bool) -> Vec<Diagnostic> {
    let program = crate::parse(source, Path::new("refusal-dependency.spx")).unwrap();
    if old {
        crate::source_verify::declaration::old::verify(&program)
    } else {
        crate::source_verify::verify(&program)
    }
}

fn selected(source: &str, old_codes: &[&str], current_codes: &[&str]) {
    let old = full(source, true);
    let current = full(source, false);
    assert_eq!(codes(&old), old_codes, "old: {source}\n{old:?}");
    assert_eq!(
        codes(&current),
        current_codes,
        "current: {source}\n{current:?}"
    );
    assert_eq!(current.first(), old.first(), "original root: {source}");
    let mut remaining = old.iter();
    for diagnostic in &current {
        assert!(
            remaining.any(|original| original == diagnostic),
            "lost exact original diagnostic provenance: {source}: {diagnostic:?}"
        );
    }
}

#[test]
fn annotation_view_excuses_only_its_expected_type_and_fixed_call_results_reset_it() {
    let mutable = module("let mut bad: bool = 1; bad = 2; let independent = f_bool(bad); 0");
    assert_eq!(full(&mutable, false), full(&mutable, true));
    assert_eq!(codes(&full(&mutable, true)), ["SPX-T232", "SPX-T205"]);
    for (body, old, current) in [
        (
            "let bad: bool = 1; let alias = bad; let dependent = f_bool(alias); let independent = f_string(alias); 0",
            &["SPX-T232", "SPX-T205", "SPX-T205"][..],
            &["SPX-T232", "SPX-T205"][..],
        ),
        (
            "let bad: bool = 1; let outcome = bool_id(bad); let independent = f_string(outcome); 0",
            &["SPX-T232", "SPX-T205", "SPX-T205"][..],
            &["SPX-T232", "SPX-T205"][..],
        ),
        (
            "let bad: bool = 1; let alias: bool = bad; let independent = f_string(alias); 0",
            &["SPX-T232", "SPX-T232", "SPX-T205"][..],
            &["SPX-T232", "SPX-T205"][..],
        ),
        (
            "let bad: bool = 1; let alias: string = bad; let independent = f_bool(alias); 0",
            &["SPX-T232", "SPX-T232", "SPX-T205"][..],
            &["SPX-T232", "SPX-T232", "SPX-T205"][..],
        ),
        (
            "let bad: bool = 1; let alias: i64 = bad; let independent = f_bool(alias); 0",
            &["SPX-T232", "SPX-T205"][..],
            &["SPX-T232", "SPX-T205"][..],
        ),
    ] {
        selected(&module(body), old, current);
    }
}

#[test]
fn owned_and_borrowed_annotation_views_keep_complete_original_ownership_errors() {
    selected(
        &format!(
            "module test.owned_view; {CALLS} @id(\"app.probe\") fn probe(owner: string) -> i64 {{ let alias: Bytes = owner; let consumed = consume_bytes(alias); let independent = string_len(owner); 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
        ),
        &["SPX-T232", "SPX-T205", "SPX-O101"],
        &["SPX-T232", "SPX-O101"],
    );
    selected(
        &format!(
            "module test.borrowed_view; {CALLS} @id(\"app.probe\") fn probe(held: borrow Bytes) -> i64 {{ let alias: string = held; let consumed = f_string(alias); 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
        ),
        &["SPX-T232", "SPX-T205", "SPX-O102"],
        &["SPX-T232", "SPX-O102"],
    );
    selected(
        "module test.borrowed_return; @id(\"app.probe\") fn probe(held: borrow Bytes) -> Bytes { let alias: string = held; alias } @id(\"app.main\") fn main() -> i64 { 0 }",
        &["SPX-T232", "SPX-O104"],
        &["SPX-T232", "SPX-O104"],
    );
    selected(
        "module test.lexical_borrowed_return; @id(\"app.probe\") fn probe(held: borrow Bytes) -> string { let alias: string = held; return alias; } @id(\"app.main\") fn main() -> i64 { 0 }",
        &["SPX-T232", "SPX-T103", "SPX-O104"],
        &["SPX-T232"],
    );
    let loan = format!(
        "module test.loan_view; {CALLS} @id(\"app.probe\") fn probe(owner: own Bytes) -> i64 {{ let view: bool = bytes_as_slice(owner); let consumed = consume_bytes(owner); let still_live = byte_len(view); 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    let old = full(&loan, true);
    let current = full(&loan, false);
    assert_eq!(current, old, "full loan obligations: {loan}");
    assert_eq!(codes(&old), ["SPX-T232", "SPX-T265"]);
}

#[test]
fn actual_scalar_borrow_and_owner_results_are_retained_without_fabricated_resource() {
    for (source, expected) in [
        (source("let value: Bytes = 1; value"), (Type::I64, ParamMode::Value, false)),
        ("module test.actual_owner; @id(\"app.main\") fn main() -> string { let value: Bytes = \"x\"; value }".to_owned(), (Type::String, ParamMode::Own, false)),
        ("module test.actual_borrow; @id(\"app.main\") fn main(held: borrow Bytes) -> i64 { let value: string = held; value }".to_owned(), (Type::Bytes, ParamMode::Borrow, false)),
    ] {
        assert_eq!(receive(&source, true).0, Some(expected.clone()));
        assert_eq!(receive(&source, false).0, Some(expected));
    }
    selected(
        &module(
            "let phantom: Bytes = 1; let alias = phantom; let consumed = consume_bytes(alias); 0",
        ),
        &["SPX-T232", "SPX-T205"],
        &["SPX-T232"],
    );
}

#[test]
fn transparent_blocks_aliases_and_lazy_obligations_keep_independent_names() {
    for (body, old, current) in [
        (
            "let bad: bool = 1; let alias = { bad }; if alias { 1 } else { 0 }",
            &["SPX-T232", "SPX-T210"][..],
            &["SPX-T232"][..],
        ),
        (
            "let bad: bool = 1; if bad && false { 1 } else { 0 }",
            &["SPX-T232", "SPX-T208"][..],
            &["SPX-T232"][..],
        ),
        (
            "let bad: bool = 1; if bad && missing { 1 } else { 0 }",
            &["SPX-T232", "SPX-T202", "SPX-T208"][..],
            &["SPX-T232", "SPX-T202"][..],
        ),
        (
            "let bad: bool = 1; let local = { let alias = bad; if alias { 1 } else { 0 } }; alias",
            &["SPX-T232", "SPX-T210", "SPX-T202"][..],
            &["SPX-T232", "SPX-T202"][..],
        ),
    ] {
        selected(&module(body), old, current);
    }
}

#[test]
fn unary_while_and_direct_return_obligations_use_only_the_refused_annotation() {
    selected(
        &module("let bad: bool = 1; if !bad { 1 } else { 0 }"),
        &["SPX-T232", "SPX-T206"],
        &["SPX-T232"],
    );
    selected(
        &module("let bad: bool = 1; while bad { 0 } 0"),
        &["SPX-T232", "SPX-T251"],
        &["SPX-T232"],
    );
    selected(
        "module test.return_view; @id(\"app.probe\") fn probe() -> bool { let bad: bool = 1; bad } @id(\"app.main\") fn main() -> i64 { 0 }",
        &["SPX-T232", "SPX-T103"],
        &["SPX-T232"],
    );
    selected(
        "module test.independent_return; @id(\"app.probe\") fn probe() -> string { let bad: bool = 1; bad } @id(\"app.main\") fn main() -> i64 { 0 }",
        &["SPX-T232", "SPX-T103", "SPX-O104"],
        &["SPX-T232", "SPX-T103"],
    );
}

#[test]
fn detached_closures_keep_independent_parameter_and_result_errors() {
    selected(
        &module("let bad: bool = 1; let callback = fn() -> i64 { if bad { 1 } else { 0 } }; 0"),
        &["SPX-T232", "SPX-T210"],
        &["SPX-T232", "SPX-T210"],
    );
    selected(
        &module("let bad: bool = \"x\"; let callback = fn() -> bool { bad }; 0"),
        &["SPX-T232", "SPX-T288"],
        &["SPX-T232", "SPX-T288"],
    );
    selected(
        &module(
            "let bad: bool = 1; let callback = fn(bad: i64) -> i64 { if bad { 1 } else { 0 } }; 0",
        ),
        &["SPX-T232", "SPX-T210"],
        &["SPX-T232", "SPX-T210"],
    );
    selected(
        &module("let callback = fn() -> bool { let bad: bool = 1; bad }; 0"),
        &["SPX-T232", "SPX-T288"],
        &["SPX-T232"],
    );
    selected(
        &module("let callback = fn() -> usize { let bad: bool = 1; bad }; 0"),
        &["SPX-T232", "SPX-T288"],
        &["SPX-T232", "SPX-T288"],
    );
    selected(
        &module("let callback = fn() -> string { let bad: bool = 1; bad }; 0"),
        &["SPX-T288"],
        &["SPX-T288"],
    );
}

#[test]
fn match_guards_retain_independent_coverage_and_ownership_mode_obligations() {
    const VARIANT: &str =
        r#"@id("choice") variant Choice { @id("choice.one") One, @id("choice.two") Two, }"#;
    let complete = |initializer: &str, arms: &str| {
        format!(
            "module test.guard_view; {VARIANT} @id(\"app.probe\") fn probe(selected: Choice) -> i64 {{ let bad: bool = {initializer}; match selected {{ {arms} }} }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
        )
    };
    selected(
        &complete(
            "1",
            "Choice::One {} if bad => 1, Choice::One {} => 0, Choice::Two {} => 0,",
        ),
        &["SPX-T232", "SPX-T256"],
        &["SPX-T232"],
    );
    selected(
        &complete("1", "Choice::One {} if bad => 1, Choice::One {} => 0,"),
        &["SPX-T232", "SPX-T256", "SPX-M101"],
        &["SPX-T232", "SPX-M101"],
    );
    let owned = complete(
        "\"x\"",
        "Choice::One {} if bad => 1, Choice::One {} => 0, Choice::Two {} => 0,",
    );
    assert_eq!(full(&owned, false), full(&owned, true));
    assert_eq!(codes(&full(&owned, true)), ["SPX-T232", "SPX-T256"]);
}

#[test]
fn fixed_record_results_reset_dependency_but_field_obligations_remain_selective() {
    const RECORD: &str = r#"@id("record") record Flags { @id("record.flag") flag: bool, }"#;
    let source = format!(
        "module test.field_view; {RECORD} @id(\"app.main\") fn main() -> i64 {{ let bad: bool = 1; let record = Flags {{ flag: bad }}; if record.flag {{ 1 }} else {{ 0 }} }}"
    );
    selected(&source, &["SPX-T232", "SPX-T215"], &["SPX-T232"]);
    let independent = format!(
        "module test.field_independent; {RECORD} @id(\"app.main\") fn main() -> i64 {{ let bad: string = 1; let record = Flags {{ flag: bad }}; 0 }}"
    );
    selected(
        &independent,
        &["SPX-T232", "SPX-T215"],
        &["SPX-T232", "SPX-T215"],
    );
}

#[test]
fn accepted_annotations_keep_full_original_receiver_canonical_and_graph_results() {
    for body in [
        "let flag: bool = true; let alias = flag; if alias && true { 1 } else { 0 }",
        "let text: string = \"x\"; let size = string_len(text); size",
        "let mut flag: bool = false; while flag { flag = false; 0 } 0",
        "let callback = fn(value: i64) -> i64 { let alias: i64 = value; alias }; 0",
        "match 1 { 0 => 1, n => n, }",
    ] {
        let source = module(body);
        assert!(full(&source, true).is_empty(), "old acceptance: {source}");
        assert_eq!(full(&source, false), full(&source, true));
        assert_eq!(receive(&source, false), receive(&source, true));
        let original = crate::parse(&source, Path::new("success.spx")).unwrap();
        let canonical = crate::format::canonical(&original);
        let roundtrip = crate::parse(&canonical, Path::new("success.spx")).unwrap();
        assert_eq!(crate::format::canonical(&roundtrip), canonical);
        assert_eq!(
            crate::graph::revision(&original),
            crate::graph::revision(&roundtrip)
        );
    }
}

#[derive(Debug, Eq, PartialEq)]
struct BindingFacts {
    contract_local: bool,
    ty: Type,
    mode: ParamMode,
    availability: crate::source_verify::binding::Availability,
    moved_places:
        std::collections::HashMap<Vec<String>, crate::source_verify::binding::Availability>,
    definitely_partial: std::collections::HashSet<Vec<String>>,
    native_unit_discard: bool,
    mutable: bool,
    active_loans: std::collections::BTreeSet<crate::source_verify::binding::SourceLoan>,
    borrow_origin: Option<crate::source_verify::binding::BorrowOrigin>,
}

fn state(source: &str, old: bool) -> std::collections::BTreeMap<String, BindingFacts> {
    use crate::source_verify::binding::{Availability, Binding};
    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
    let program = crate::parse(source, Path::new("binding-facts.spx")).unwrap();
    let current = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let types = crate::source_verify::type_table::TypeTable::new(&program);
    let mut variables = current
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
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();
    let check = if old {
        crate::source_verify::recovery_legacy::check_expr_iterative_legacy
    } else {
        crate::source_verify::iterative::check_expr_iterative
    };
    check(
        &program,
        current,
        &current.body,
        &mut variables,
        &functions,
        &types,
        None,
        true,
        &mut diagnostics,
    );
    variables
        .into_iter()
        .map(|(name, binding)| {
            assert!(binding.traversal_row.is_none());
            (
                name,
                BindingFacts {
                    contract_local: binding.contract_local,
                    ty: binding.ty,
                    mode: binding.mode,
                    availability: binding.availability,
                    moved_places: binding.moved_places,
                    definitely_partial: binding.definitely_partial,
                    native_unit_discard: binding.native_unit_discard,
                    mutable: binding.mutable,
                    active_loans: binding.active_loans,
                    borrow_origin: binding.borrow_origin,
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
}

#[test]
fn full_receiver_binding_ownership_place_and_loan_facts_equal_complete_original() {
    for source in [
        format!(
            "module test.owner_facts; {CALLS} @id(\"app.main\") fn main(owner: own string) -> i64 {{ let alias: Bytes = owner; let consumed = consume_bytes(alias); 0 }}"
        ),
        format!(
            "module test.borrow_facts; {CALLS} @id(\"app.main\") fn main(held: borrow Bytes) -> i64 {{ let alias: string = held; let consumed = f_string(alias); 0 }}"
        ),
        format!(
            "module test.loan_facts; {CALLS} @id(\"app.main\") fn main(owner: own Bytes) -> i64 {{ let view: bool = bytes_as_slice(owner); let consumed = consume_bytes(owner); let still_live = byte_len(view); 0 }}"
        ),
    ] {
        assert_eq!(state(&source, false), state(&source, true), "{source}");
    }
}

#[test]
fn actual_dependency_directory_and_repeated_receiver_acquisitions_release_through_drop() {
    use crate::source_verify::recovery::Recovery;
    use crate::test_allocation_observer::measure;
    use std::collections::HashMap;
    let accepted = crate::parse(
        &source("let valid: bool = true; valid"),
        Path::new("accepted-directory.spx"),
    )
    .unwrap();
    let expression = &accepted.functions[0].body;
    let (_, empty) = measure(|| {
        let mut recovery = Recovery::default();
        for ordinal in 1..=64 {
            recovery.fork(Some(0), ordinal);
            recovery.begin(expression);
            assert!(!recovery.reads_value(ordinal, "valid", expression));
            assert!(recovery.view(expression).is_none());
        }
        drop(recovery);
    });
    assert_eq!(empty.allocated, 0);
    assert_eq!(empty.live, 0);
    for count in [1, 8, 64, 256] {
        let mut body = String::from("let bad: bool = 1; ");
        for ordinal in 0..count {
            body.push_str(&format!(
                "let alias{ordinal} = bad; let used{ordinal} = f_bool(alias{ordinal}); "
            ));
        }
        body.push('0');
        let text = module(&body);
        let old = full(&text, true);
        assert_eq!(old.len(), count + 1);
        assert_eq!(old[0].code, "SPX-T232");
        assert!(old[1..]
            .iter()
            .all(|diagnostic| diagnostic.code == "SPX-T205"));
        let program = crate::parse(&text, Path::new("dependent-directory.spx")).unwrap();
        let current = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        let functions = program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect::<HashMap<_, _>>();
        let types = crate::source_verify::type_table::TypeTable::new(&program);
        let (_, observed) = measure(|| {
            let mut diagnostics = Vec::new();
            let mut verifier = crate::source_verify::IterativeVerifier::new(
                &program,
                current,
                HashMap::new(),
                &functions,
                &types,
                None,
                true,
                &mut diagnostics,
            );
            assert!(verifier.run(&current.body).unwrap().is_some());
            drop(verifier);
            assert_eq!(codes(&diagnostics), ["SPX-T232"]);
            drop(diagnostics);
        });
        assert!(observed.allocated > 0);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn native_unit_refusal_keeps_full_original_metadata_and_capability_diagnostics() {
    let source = r#"module test.native_unit_view;
@id("host") interface Host permits { } {
    @id("host.ping") import rust fn ping(value: i64) -> unit effects { } failure infallible;
}
@id("app.main") fn main() -> i64 { let bad: bool = ping(1); let alias = bad; 0 }
"#;
    let old = full(source, true);
    let current = full(source, false);
    assert_eq!(current, old);
    assert!(codes(&old).contains(&"SPX-B107"));
    assert_eq!(receive(source, false), receive(source, true));
}

#[test]
fn expression_identity_and_reentry_cannot_reuse_another_refusal_view() {
    use crate::ast::ExprKind;
    use crate::source_verify::recovery::Recovery;
    let program =
        crate::parse(&source("let bad: bool = 1; bad"), Path::new("identity.spx")).unwrap();
    let ExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("block");
    };
    let copied = tail.as_ref().clone();
    assert_eq!(copied.span, tail.span);
    let declared = Type::Bool;
    let mut recovery = Recovery::default();
    recovery.refused_annotation(0, "bad", &declared, 0);
    assert!(!recovery.reads_value(0, "bad", tail));
    assert!(recovery.explains(tail, &Type::Bool));
    assert!(!recovery.explains(&copied, &Type::Bool));
    assert!(!recovery.explains(tail, &Type::String));
    recovery.begin(tail);
    assert!(!recovery.explains(tail, &Type::Bool));
    recovery.fork(None, 1);
    assert!(!recovery.reads_value(1, "bad", &copied));
    assert!(recovery.view(&copied).is_none());
    recovery.fork(Some(0), 1);
    assert!(!recovery.reads_value(1, "bad", &copied));
    assert!(recovery.explains(&copied, &Type::Bool));
    recovery.fork(None, 1);
    recovery.begin(&copied);
    assert!(!recovery.reads_value(1, "bad", &copied));
    assert!(recovery.view(&copied).is_none());
}

#[test]
fn refused_annotation_does_not_fabricate_projected_fields_or_nominal_carrier() {
    let source = r#"module test.nominal_view;
@id("flags") record Flags { @id("flags.flag") flag: bool, }
@id("app.main") fn main() -> i64 { let bad: Flags = 1; let field = bad.flag; 0 }
"#;
    selected(source, &["SPX-T232", "SPX-T214"], &["SPX-T232", "SPX-T214"]);
}

#[test]
fn contract_expected_type_view_preserves_independent_result_and_transfer_obligations() {
    selected(
        "module test.contract_view; @id(\"app.main\") fn main() -> i64 requires { let bad: bool = 1; bad } { 0 }",
        &["SPX-T232", "SPX-C101"],
        &["SPX-T232"],
    );
    selected(
        "module test.contract_independent; @id(\"app.main\") fn main() -> i64 requires { let bad: string = 1; bad } { 0 }",
        &["SPX-T232", "SPX-C101"],
        &["SPX-T232", "SPX-C101"],
    );
    let borrowed = format!(
        "module test.contract_borrow; {CALLS} @id(\"app.probe\") fn probe(held: borrow Bytes) -> i64 requires {{ let alias: string = held; let used = f_string(alias); true }} {{ 0 }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    selected(
        &borrowed,
        &["SPX-T232", "SPX-T205", "SPX-O102"],
        &["SPX-T232", "SPX-O102"],
    );
}
