//! Exact source-declaration refusal alongside independently known call facts.
use crate::ast::program_source::ProgramSource;
use crate::diagnostic::Diagnostic;
use crate::source_verify::{
    declared_type::check_declared_type, type_table::TypeTable, IterativeVerifier,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn source(helper: &str, body: &str) -> String {
    format!("module t; @id(\"t.f\") {helper} @id(\"t.main\") fn main()->i64 {{ {body} }}")
}

fn receive(source: &str, before: bool) -> (Option<crate::ast::Type>, Vec<Diagnostic>) {
    receive_function(source, before, "main")
}
fn receive_function(
    source: &str,
    before: bool,
    name: &str,
) -> (Option<crate::ast::Type>, Vec<Diagnostic>) {
    let program = crate::parse(source, Path::new("callee-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let main = functions[name].payload();
    let mut diagnostics = Vec::new();
    // Real signature checker creates the original receipts before body checking.
    for function in program.functions() {
        let parameters = function
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        check_declared_type(
            &program,
            &function.return_type,
            function.span,
            &types,
            &parameters,
            &mut diagnostics,
        );
        for parameter in &function.params {
            check_declared_type(
                &program,
                &parameter.ty,
                parameter.span,
                &types,
                &parameters,
                &mut diagnostics,
            );
        }
    }
    let variables = main
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                crate::source_verify::binding::Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: crate::source_verify::binding::Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: std::collections::BTreeSet::new(),
                    borrow_origin: None,
                },
            )
        })
        .collect();
    let mut verifier = IterativeVerifier::new(
        &program,
        functions
            .get(main.name.as_str())
            .expect("frozen current declaration cell"),
        variables,
        &functions,
        &types,
        None,
        true,
        &mut diagnostics,
    );
    verifier.operators_before_799 = before;
    let value = verifier.run(&main.body).unwrap().map(|value| value.ty);
    drop(verifier);
    (value, diagnostics)
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}
fn full_errors(source: &str) -> Vec<Diagnostic> {
    let program = crate::parse(source, Path::new("callee-components.spx")).unwrap();
    crate::source_verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .collect()
}

#[test]
fn refused_callee_parameter_excuses_only_its_type_comparison() {
    let source = self::source("fn f(a: Missing, b: bool)->i64 { 0 }", "f(1, 2)");
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&old.1), ["SPX-T001", "SPX-T205", "SPX-T205"]);
    assert_eq!(codes(&current.1), ["SPX-T001", "SPX-T205"]);
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(current.1[1], old.1[2]);
    assert_eq!(current.0, old.0);
    assert_eq!(full_errors(&source), current.1);
}

#[test]
fn valid_callee_result_keeps_independent_condition_mismatch_after_parameter_refusal() {
    let source = self::source("fn f(a: Missing)->i64 { 0 }", "if f(1) { 1 } else { 0 }");
    let current = receive(&source, false);
    assert_eq!(codes(&current.1), ["SPX-T001", "SPX-T210"]);
    assert_eq!(full_errors(&source), current.1);
}

#[test]
fn refused_callee_return_carries_original_origin_through_alias_and_zero_argument_call() {
    for (helper, body) in [
        (
            "fn f()->Missing { 0 }",
            "let alias = f(); string_len(alias)",
        ),
        ("fn f(a:i64)->Missing { 0 }", "let alias = f(1); alias + 1"),
    ] {
        let source = self::source(helper, body);
        let old = receive(&source, true);
        let current = receive(&source, false);
        assert_eq!(codes(&current.1), ["SPX-T001"]);
        assert_eq!(current.1[0], old.1[0]);
        assert!(old.1.len() > current.1.len());
        assert_eq!(full_errors(&source), current.1);
    }
}

#[test]
fn refused_callee_return_keeps_independent_argument_and_sibling_errors() {
    let source = self::source(
        "fn f(a:i64)->Missing { 0 }",
        "let alias = f(true); alias + missing_sibling",
    );
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&current.1), ["SPX-T001", "SPX-T205", "SPX-T202"]);
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(current.1[1], old.1[1]);
    assert_eq!(
        current.1[2].message,
        "unknown value `missing_sibling` in `main`"
    );
    assert_eq!(full_errors(&source), current.1);
}

#[test]
fn valid_signature_argument_failures_do_not_erase_independently_known_results() {
    let source = self::source("fn f(a:i64)->i64 { a }", "if f(true) { 1 } else { 0 }");
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&current.1), ["SPX-T205", "SPX-T210"]);
    assert_eq!(current.1, old.1);
    assert_eq!(full_errors(&source), current.1);
    let source = self::source("fn f(a:i64)->i64 { a }", "f(1) + 2");
    assert_eq!(receive(&source, false), receive(&source, true));
    assert!(full_errors(&source).is_empty());
}

#[test]
fn complete_pre_component_entry_and_call_sources_remain_exact() {
    let entry = ci_str!("../iterative/enter/enter_before_components.rs")
        .replace("before_components_", "")
        .replace(
            "crate::source_verify::vec_field::signature(",
            "super::super::vec_field::signature(",
        );
    let calls =
        ci_str!("../iterative/calls/calls_before_components.rs").replace("before_components_", "");
    assert_eq!(
        entry,
        ci_str!("../../../docs/evidence/callee-components-799/iterative__enter.rs.before")
    );
    assert_eq!(
        calls,
        ci_str!("../../../docs/evidence/callee-components-799/iterative__calls.rs.before")
    );
}

#[test]
fn callee_component_refusal_requires_full_original_receipt_and_generic_owner_context() {
    let source = self::source("fn f(a:Missing)->i64{0}", "f(1)");
    let program = crate::parse(&source, Path::new("callee-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let f = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let mut diagnostics = Vec::new();
    check_declared_type(
        &program,
        &f.params[0].ty,
        f.params[0].span,
        &types,
        &HashSet::new(),
        &mut diagnostics,
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        main,
        &HashMap::new(),
        &types,
        &diagnostics,
    );
    assert_eq!(
        recovery.callee_component_refusal(f, Some(0), &program, &types, &diagnostics),
        Some(0)
    );
    for field in 0..6 {
        let mut forged = diagnostics.clone();
        match field {
            0 => forged[0].message = "another error".into(),
            1 => forged[0].help = Some("another help".into()),
            2 => forged[0].path = Some("another.spx".into()),
            3 => forged[0].severity = crate::diagnostic::Severity::Warning,
            4 => forged[0].code = "SPX-T002",
            _ => forged[0].span.as_mut().unwrap().start += 1,
        }
        assert_eq!(
            recovery.callee_component_refusal(f, Some(0), &program, &types, &forged),
            None
        );
    }
    let source = self::source("fn f<T>(a:T)->i64{0}", "f<i64>(1)");
    let program = crate::parse(&source, Path::new("callee-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let f = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let forged = vec![
        Diagnostic::error("SPX-T001", "unknown type `T`", f.params[0].span).at_path(program.path()),
    ];
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        main,
        &HashMap::new(),
        &types,
        &forged,
    );
    assert_eq!(
        recovery.callee_component_refusal(f, Some(0), &program, &types, &forged),
        None
    );
}

#[test]
fn refused_parameter_keeps_known_sibling_owned_transfer_and_later_move_refusal() {
    let source = "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.drop\") drop trivial; } @id(\"t.f\") fn f(a:Missing,b:own Buffer)->i64{0} @id(\"t.consume\") fn consume(b:own Buffer)->i64{0} @id(\"t.main\") fn main(buffer:own Buffer)->i64{let first=f(1,buffer);consume(buffer)}";
    let old = receive(source, true);
    let current = receive(source, false);
    assert_eq!(old.1[0].code, "SPX-T001");
    assert_eq!(old.1[1].code, "SPX-T205");
    let mut expected = old.1.clone();
    expected.remove(1);
    assert_eq!(current.1, expected);
    assert!(current
        .1
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O101"));
    let program = crate::parse(source, Path::new("callee-components.spx")).unwrap();
    let mut public = current.1.clone();
    public.push(Diagnostic::error("SPX-T104", "entry function must be monomorphic with signature `fn main() -> i64`", program.functions.last().unwrap().span)
        .at_path("callee-components.spx").with_help("`main` takes no parameters; read command-line arguments with `args_len()` and `arg_utf8(index)`"));
    assert_eq!(full_errors(source), public);
    let source = source.replace("fn main(buffer:own Buffer)", "fn probe(buffer:own Buffer)")
        + " @id(\"t.entry\") fn main()->i64{0}";
    let old = receive_function(&source, true, "probe");
    let current = receive_function(&source, false, "probe");
    let mut expected = old.1.clone();
    assert_eq!(expected.remove(1).code, "SPX-T205");
    assert_eq!(current.1, expected);
    assert_eq!(codes(&current.1), ["SPX-T001", "SPX-O101"]);
    assert_eq!(full_errors(&source), current.1);
}

#[test]
fn initializer_keeps_first_real_argument_failure_ahead_of_return_dependency_origin() {
    let mut recovery = crate::source_verify::recovery::Recovery::default();
    recovery.fork(None, 0);
    let mut diagnostics = vec![Diagnostic::io_static("SPX-T001", "original return refusal")];
    let initializer = recovery.checkpoint(&diagnostics);
    diagnostics.push(Diagnostic::io_static(
        "SPX-T205",
        "actual argument refusal inside initializer",
    ));
    recovery.read_callee_refusal(0);
    recovery.refused_let(0, "alias", initializer, &diagnostics);
    let read = recovery.checkpoint(&diagnostics);
    assert!(recovery.reads_refused(0, "alias"));
    assert_eq!(recovery.refusal_origin(read, &diagnostics), Some(1));
}

#[test]
fn refused_return_component_requires_exact_function_span_and_full_receipt() {
    let source = self::source("fn f()->Missing{0}", "f()");
    let program = crate::parse(&source, Path::new("callee-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let f = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let mut diagnostics = Vec::new();
    check_declared_type(
        &program,
        &f.return_type,
        f.span,
        &types,
        &HashSet::new(),
        &mut diagnostics,
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        main,
        &HashMap::new(),
        &types,
        &diagnostics,
    );
    assert_eq!(
        recovery.callee_component_refusal(f, None, &program, &types, &diagnostics),
        Some(0)
    );
    let mut forged = diagnostics.clone();
    forged[0].span = Some(f.name_span);
    assert_eq!(
        recovery.callee_component_refusal(f, None, &program, &types, &forged),
        None
    );
    let mut forged = diagnostics.clone();
    forged[0].help = Some("not the original help".into());
    assert_eq!(
        recovery.callee_component_refusal(f, None, &program, &types, &forged),
        None
    );
}
