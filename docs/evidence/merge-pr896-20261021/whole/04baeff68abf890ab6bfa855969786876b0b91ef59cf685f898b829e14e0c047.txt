//! Independent AST/HIR absence and complete old/current forecast controls.
use super::super::{cached_exact, call_carriers_original, dependency_identity_max};
use super::*;
use crate::hir::{ResolvedExpr, ResolvedExprKind};
use std::mem::size_of;

fn carriers() -> usize {
    size_of::<crate::hir::ResolvedBinding>()
        + size_of::<crate::hir::ResolvedStatement>()
        + size_of::<crate::hir::ResolvedFieldInitializer>()
        + size_of::<crate::hir::ResolvedMatchArm>()
        + size_of::<crate::hir::ResolvedMatchPatternField>()
        + size_of::<crate::hir::ResolvedRecordMatchPatternField>()
}
fn parse(source: &str) -> Program {
    crate::parse(source, "ordinary.spx").unwrap()
}
fn proof(source: &str) -> Proof {
    let programs = vec![parse(source)];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    count(&programs[0], &authored, true)
}
const HEADER: &str = "module ordinary; @id(\"ordinary.f\") fn f()->i64 { 1 } ";
fn dense(n: usize) -> String {
    let calls = std::iter::repeat_n("f()", n)
        .collect::<Vec<_>>()
        .join(" + ");
    format!("{HEADER}@id(\"ordinary.main\") fn main()->i64 {{ {calls} }}")
}

/// Independent full AST position oracle for the closed f/g fixture, including
/// every delegated child. Target/binding hostility is tested separately.
fn ast(expression: &Expr, free: bool, outside: bool) -> (usize, usize) {
    let mut result = (
        1,
        usize::from(
            free && outside
                && matches!(&expression.kind,
        ExprKind::Call { name, type_arguments, .. } if (name == "f" || name == "g") && type_arguments.is_empty()),
        ),
    );
    let outside = outside && !matches!(expression.kind, ExprKind::Closure { .. });
    let mut i = 0;
    while let Some(child) = expression.child(i) {
        let child_free = match &expression.kind {
            ExprKind::Unary { value, .. } => std::ptr::eq(child, value.as_ref()),
            ExprKind::Binary { left, right, .. } => {
                std::ptr::eq(child, left.as_ref()) || std::ptr::eq(child, right.as_ref())
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => [condition, then_branch, else_branch]
                .iter()
                .any(|v| std::ptr::eq(child, v.as_ref())),
            ExprKind::Call { args, .. } => args.iter().any(|v| std::ptr::eq(child, v)),
            ExprKind::Block { tail, .. } => std::ptr::eq(child, tail.as_ref()),
            _ => false,
        };
        let next = ast(child, child_free, outside);
        result.0 += next.0;
        result.1 += next.1;
        i += 1;
    }
    result
}

/// Walk actual resolved containers, independently of AST tags. Each qualified
/// root is a bare Call in an owning Box/vector; no auxiliary carrier is formed
/// by its own variant. Statement-owned/delegated roots are excluded.
fn hir(expression: &ResolvedExpr, free: bool) -> (usize, usize) {
    let mut result = (1, 0);
    if let ResolvedExprKind::Call {
        type_arguments,
        instance,
        ..
    } = &expression.kind
    {
        assert!(type_arguments.is_empty());
        assert!(instance.is_none());
        result.1 += usize::from(free);
    }
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => {
            for arg in args {
                let next = hir(arg, true);
                result.0 += next.0;
                result.1 += next.1;
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            let next = hir(value, true);
            result.0 += next.0;
            result.1 += next.1;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            for child in [left, right] {
                let next = hir(child, true);
                result.0 += next.0;
                result.1 += next.1;
            }
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for child in [condition, then_branch, else_branch] {
                let next = hir(child, true);
                result.0 += next.0;
                result.1 += next.1;
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for i in 0..statement.child_count() {
                    let next = hir(statement.child(i).unwrap(), false);
                    result.0 += next.0;
                    result.1 += next.1;
                }
            }
            let next = hir(tail, true);
            result.0 += next.0;
            result.1 += next.1;
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Place(_) | ResolvedExprKind::Bool(_) => {}
        other => panic!("oracle fixture must explicitly own every HIR variant: {other:?}"),
    }
    result
}

fn costs(
    source: &str,
    mode: u8,
    retention: cached_exact::Retention,
) -> (usize, usize, usize, usize) {
    let programs = vec![parse(source)];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let program = &programs[0];
    let maximum = Some(dependency_identity_max(program, &authored, &programs).unwrap());
    let old = call_carriers_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
        program,
        &authored,
        &programs,
        maximum,
        true,
        (mode, retention),
        None,
    )
    .unwrap();
    let current =
        super::super::held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
            program,
            &authored,
            &programs,
            maximum,
            true,
            (mode, retention),
            None,
        )
        .unwrap();
    (
        old.retained_hir,
        current.retained_hir,
        old.retained_clone_and_hir,
        current.retained_clone_and_hir,
    )
}

#[test]
fn complete_old_current_algorithm_delta_keeps_storage_children_and_cold_proof_fees() {
    for source in [
        dense(1),
        dense(64),
        format!("{HEADER}@id(\"ordinary.main\") fn main()->i64 {{ let x = f(); f() + x }}"),
    ] {
        let program = parse(&source);
        let checked = crate::check(&source, "ordinary.spx").unwrap();
        let resolved = crate::hir::resolve(&checked).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let ast_census = program
            .functions
            .iter()
            .map(|f| ast(&f.body, false, true))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let hir_census = resolved
            .functions
            .iter()
            .map(|f| hir(&f.body, false))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        assert_eq!(ast_census, hir_census);
        let p = proof(&source);
        assert_eq!(p.roots, ast_census.1);
        assert!(p.bytes > 0);
        for (mode, retention, copies) in [
            (5, cached_exact::Retention::Parsed, 1),
            (6, cached_exact::Retention::Parsed, 1),
            (7, cached_exact::Retention::Parsed, 1),
            (7, cached_exact::Retention::Semantic, 2),
        ] {
            let (old_hir, new_hir, old_total, new_total) = costs(&source, mode, retention);
            assert_eq!(new_hir + copies * p.roots * carriers(), old_hir + p.bytes);
            assert_eq!(
                new_total + copies * p.roots * carriers(),
                old_total + p.bytes
            );
            // Cold repeated estimation must keep the same proof invoice.
            assert_eq!(
                costs(&source, mode, retention),
                (old_hir, new_hir, old_total, new_total)
            );
        }
        for mode in 0..5 {
            let (a, b, c, d) = costs(&source, mode, cached_exact::Retention::Parsed);
            assert_eq!((a, c), (b, d));
        }
    }
}

#[test]
fn hostile_target_binding_and_delegated_routes_fail_closed() {
    for body in [
        "fn main(f:fn()->i64)->i64 { f() }",
        "fn main()->i64 { let f = fn()->i64 { 2 }; f() }",
        "fn main()->i64 { let x = f(); x }",
        "fn main()->i64 { let callback = fn()->i64 { f() }; callback() }",
        "fn main()->i64 { missing() }",
        "fn main()->i64 { f<i64>() }",
        "fn main()->i64 { let f = 2; f() }",
        "fn main()->i64 { match true { f => f(), } }",
        "fn main()->i64 { match true { f => 1, _ => f(), } }",
    ] {
        let source = format!("{HEADER}@id(\"ordinary.main\") {body}");
        assert_eq!(proof(&source).roots, 0, "{body}");
        assert!(proof(&source).bytes > 0, "failed proof still pays");
    }
    assert_eq!(proof("module ordinary; @id(\"ordinary.f\") fn f<T>()->i64{1} @id(\"ordinary.main\") fn main()->i64{f()}").roots, 0);
    for name in [
        "byte_len",
        "string_len",
        "vec_len",
        "box_new",
        "list_nil",
        "map_new",
    ] {
        assert_eq!(proof(&format!("module ordinary; @id(\"ordinary.f\") fn {name}()->i64{{1}} @id(\"ordinary.main\") fn main()->i64{{{name}()}}")).roots, 0, "{name}");
    }
    let source = "module ordinary; @id(\"ordinary.f\") fn f()->i64{1} @id(\"ordinary.main\") fn main<T>()->i64{f()}";
    assert_eq!(proof(source).roots, 0);
}

#[test]
fn imported_target_requires_actual_unique_authenticated_monomorphic_header() {
    let provider = parse("module provider; @id(\"provider.f\") fn f()->i64{1}");
    let consumer = parse(
        "module ordinary; use function @id(\"provider.f\") from provider as f; @id(\"ordinary.main\") fn main()->i64{f()}",
    );
    let programs = vec![provider, consumer];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    assert_eq!(count(&programs[1], &authored, true).roots, 1);
    assert_eq!(count(&programs[1], &authored, false).roots, 0);
    for edit in 0..3 {
        let mut forged = programs[1].clone();
        match edit {
            0 => forged.module_uses[0].target_module = "wrong".into(),
            1 => forged.module_uses[0].persistent_id = "missing".into(),
            _ => forged.module_uses.push(forged.module_uses[0].clone()),
        }
        assert_eq!(count(&forged, &authored, true).roots, 0);
    }
}

struct Limit(usize);
impl Drop for Limit {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|l| l.set(self.0));
    }
}
#[test]
fn exact_one_short_zero_and_whole_allocation_drop_controls() {
    let source = dense(64);
    let programs = vec![parse(&source)];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    for retention in [
        cached_exact::Retention::Parsed,
        cached_exact::Retention::Semantic,
    ] {
        let expected = cached_exact::summed(&programs, &authored, retention).unwrap();
        let _restore =
            Limit(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|l| l.replace(expected.1)));
        assert_eq!(
            cached_exact::summed(&programs, &authored, retention).unwrap(),
            expected
        );
        for limit in [expected.1 - 1, 0] {
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|l| l.set(limit));
            let errors = cached_exact::summed(&programs, &authored, retention).unwrap_err();
            assert!(errors.iter().all(|e| e.code == "SPX-G171"));
        }
    }
    let checked = crate::check(&source, "ordinary.spx").unwrap();
    // The complete resolver keeps more than the default 16,384 pointers live.
    // Freed slots are already reclaimed; this is simultaneous HIR backing,
    // not a tombstone leak. Use the existing explicit whole-HIR registry size;
    // its backing is prepared outside observation, with exhaustion still fatal.
    let (((after_resolve, after_validate, resolved_functions), allocation), phases) =
        crate::test_allocation_observer::with_phase_trace(|| {
            crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                let resolved = crate::hir::resolve(&checked).unwrap();
                let after_resolve = crate::test_allocation_observer::current_metrics();
                crate::hir::validate(&resolved).unwrap();
                let after_validate = crate::test_allocation_observer::current_metrics();
                let resolved_functions = resolved.functions.len();
                drop(resolved);
                (after_resolve, after_validate, resolved_functions)
            })
        });
    assert!(
        !phases.overflow,
        "fixed allocation-phase instrumentation overflow"
    );
    assert!(phases.intervals.iter().any(Option::is_some));
    eprintln!("ORDINARY_CALL_INCLUSIVE_PHASE_INTERVALS {phases:?}; cumulative highwaters, nested intervals, requested layouts only");
    assert!(
        !allocation.invalid,
        "complete HIR allocation observation: reason={:?}, metrics={allocation:?}",
        crate::test_allocation_observer::invalid_reason(),
    );
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocations > 0);
    let (old_hir, forecast, old_clone_and_hir, current_clone_and_hir) =
        costs(&source, 5, cached_exact::Retention::Parsed);
    // These are snapshots of ONE observation, not independently measured phase
    // costs. Parse/check/authored inputs and the fixed registry are excluded;
    // public resolve (including its source verification), explicit validation,
    // and complete resolved-owner Drop remain inside the original window.
    eprintln!(
        "ORDINARY_CALL_WHOLE_HIR dense_calls=64 source_bytes={} modules={} authored_declarations={} checked_functions={} resolved_functions={resolved_functions} mode=5 retention=Parsed old_retained_hir={old_hir} current_retained_hir={forecast} old_retained_clone_and_hir={old_clone_and_hir} current_retained_clone_and_hir={current_clone_and_hir} after_resolve={after_resolve:?} after_validate={after_validate:?} after_drop={allocation:?}; phase snapshots are cumulative requested-layout metrics, not RSS or reservation debits",
        source.len(),
        programs.len(),
        authored.len(),
        checked.functions.len(),
    );
    assert!(
        allocation.highwater <= forecast,
        "whole public resolve + validate + Drop exceeds mode-5 Parsed retained-HIR forecast: forecast={forecast}, old_retained_hir={old_hir}, current_retained_clone_and_hir={current_clone_and_hir}, old_retained_clone_and_hir={old_clone_and_hir}, after_resolve={after_resolve:?}, after_validate={after_validate:?}, after_drop={allocation:?}, dense_calls=64, source_bytes={}, checked_functions={}, resolved_functions={resolved_functions}; parse/check inputs and observer registry excluded",
        source.len(),
        checked.functions.len(),
    );
}

// Independently reconstruct every typed fee of the closed no-binding dense
// family. No production fee/census helper supplies this invoice.
fn charged<T>() -> usize {
    size_of::<T>() + size_of::<&mut Proof>() + size_of::<&T>()
}
fn dense_invoice(n: usize) -> usize {
    type Index<'a> = BTreeMap<&'a str, AuthoredDeclaration<'a>>;
    let expressions = 2 * n + 2;
    let functions = 2;
    let entry = charged::<(&Program, &Index<'_>, bool)>() + size_of::<Proof>();
    let owners = functions
        * (charged::<Names<'_>>()
            + charged::<&crate::ast::Function>()
            + size_of::<[Option<(&Expr, usize)>; 129]>()
            + charged::<bool>()
            + charged::<usize>());
    let census = expressions
        * (charged::<&Expr>()
            + charged::<(
                &Expr,
                bool,
                bool,
                usize,
                &Names<'_>,
                &Program,
                &Index<'_>,
                bool,
            )>()
            + charged::<bool>())
        + (expressions - functions) * charged::<(usize, &Expr, bool, bool)>();
    let targets = n
        * (charged::<(&Names<'_>, &str)>()
            + size_of::<usize>() // empty-name cursor
            + 2 * size_of::<usize>() + size_of::<bool>() // terminal length probe
            + charged::<bool>()
            + charged::<(&Program, &Index<'_>, bool, &str, usize)>()
            + charged::<(&str, bool)>()
            + functions * charged::<&crate::ast::Function>()
            + charged::<(Option<&crate::ast::Function>, bool)>()
            + charged::<bool>());
    entry + owners + census + targets
}
#[test]
fn independent_cold_invoice_prices_zero_and_every_closed_dense_proof_role() {
    for n in [1, 2, 16, 64] {
        let source = dense(n);
        let programs = vec![parse(&source)];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let (p, allocation) =
            crate::test_allocation_observer::measure(|| count(&programs[0], &authored, true));
        assert_eq!(p.roots, n);
        let before = super::before_directory::count(&programs[0], &authored, true);
        assert_eq!(
            before.bytes,
            dense_invoice(n),
            "complete pre-directory tariff"
        );
        assert_eq!(p.bytes, dense_invoice(n) + super::directory::EMPTY_SELECTOR);
        assert_eq!(p.roots, before.roots);
        assert!(!allocation.invalid);
        assert_eq!(allocation.allocations, 0);
        assert_eq!(allocation.reallocations, 0);
        assert_eq!(allocation.live, 0);
    }
    // Existing shadowing controls cover semantic refusal. This independent
    // source-role equation additionally reaches nonempty names and early hits.
    for (query, probes, terminal, increments, found) in [
        ("first", 1usize, 0usize, 0usize, true),
        ("second", 2, 0, 1, true),
        ("absent", 2, 1, 2, false),
    ] {
        let mut names = Names {
            entries: [None; 128],
            len: 2,
        };
        names.entries[0] = Some("first");
        names.entries[1] = Some("second");
        let mut invoice = Proof::default();
        let (actual, allocation) =
            crate::test_allocation_observer::measure(|| names.contains(query, &mut invoice));
        assert_eq!(actual, found);
        let entry = charged::<(&Names<'_>, &str)>() + size_of::<usize>();
        let predicate = 2 * size_of::<usize>() + size_of::<bool>();
        let index = 2 * size_of::<&[Option<&str>; 128]>()
            + 2 * size_of::<usize>()
            + size_of::<&Option<&str>>();
        let comparison = size_of::<&&str>() + 4 * size_of::<&str>() + size_of::<bool>();
        assert_eq!(
            invoice.bytes,
            entry
                + (probes + terminal) * predicate
                + probes * (index + comparison)
                + increments * size_of::<usize>()
                + charged::<bool>()
        );
        assert!(!allocation.invalid);
        assert_eq!(allocation.allocations, 0);
        assert_eq!(allocation.reallocations, 0);
        assert_eq!(allocation.live, 0);
    }
    let source = "module ordinary; @id(\"ordinary.main\") fn main()->i64{1}";
    let p = proof(source);
    assert_eq!(p.roots, 0);
    assert!(p.bytes > 0);
    let (old, new, old_total, new_total) = costs(source, 7, cached_exact::Retention::Semantic);
    assert_eq!(new, old + p.bytes);
    assert_eq!(new_total, old_total + p.bytes);
}

#[test]
fn owned_result_keeps_every_cleanup_storage_and_scratch_failure_keeps_carriers() {
    let source = "module ordinary; @id(\"ordinary.f\") fn f()->string{\"s\"} @id(\"ordinary.main\") fn main()->i64{string_len(f())}";
    assert_eq!(proof(source).roots, 1);
    let p = proof(source);
    let (old, new, old_total, new_total) = costs(source, 7, cached_exact::Retention::Semantic);
    assert_eq!(new + 2 * carriers(), old + p.bytes);
    assert_eq!(new_total + 2 * carriers(), old_total + p.bytes);
    let checked = crate::check(source, "ordinary.spx").unwrap();
    let resolved = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let mut source = String::from(HEADER);
    source.push_str("@id(\"ordinary.main\") fn main()->i64 {");
    for i in 0..129 {
        source.push_str(&format!("let v{i} = 1;"));
    }
    source.push_str("f() }");
    let p = proof(&source);
    assert_eq!(p.roots, 0);
    assert!(p.bytes > 0);
    let source = format!(
        "{HEADER}@id(\"ordinary.main\") fn main()->i64 {{ {}f() }}",
        "-".repeat(130)
    );
    // The original depth-hostile source is not an admitted proof fixture.
    // Its parser refusal remains explicit; the 129 flat sibling bindings above
    // independently exercise the real bounded Names exhaustion.
    assert_eq!(
        crate::parse(&source, "ordinary.spx").unwrap_err(),
        crate::diagnostic::Diagnostic {
            code: "SPX-P207",
            severity: crate::diagnostic::Severity::Error,
            message: "source nesting depth exceeds the admitted maximum (128)".into(),
            path: Some("ordinary.spx".into()),
            span: Some(crate::ast::Span {
                start: 219,
                end: 220,
                line: 1,
                column: 220
            }),
            help: Some("split the expression or block into named helper functions".into()),
        }
    );
}

#[test]
fn identical_admitted_hir_and_six_field_diagnostics_at_matched_successful_budgets() {
    let source = dense(32);
    let (old, new, _, _) = costs(&source, 7, cached_exact::Retention::Semantic);
    let checked = crate::check(&source, "ordinary.spx").unwrap();
    let mut observations = Vec::new();
    for budget in [old, new] {
        let _restore =
            Limit(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|l| l.replace(budget)));
        let resolved = crate::hir::resolve(&checked).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let diagnostics = crate::check(
            "module ordinary; @id(\"ordinary.main\") fn main()->i64{missing()}",
            "ordinary.spx",
        )
        .unwrap_err();
        assert!(!diagnostics.is_empty());
        observations.push((resolved, diagnostics));
    }
    assert_eq!(
        observations[0], observations[1],
        "Diagnostic Eq covers code, severity, message, path, span and help"
    );
}

#[test]
fn native_import_priority_and_malformed_or_runtime_censuses_keep_old_charges() {
    let source = format!(
        "{HEADER}@id(\"host.f\") import rust fn f()->i64 failure infallible; @id(\"ordinary.main\") fn main()->i64{{f()}}"
    );
    assert_eq!(
        crate::parse(&source, "ordinary.spx").unwrap_err(),
        crate::diagnostic::Diagnostic {
            code: "SPX-P104",
            severity: crate::diagnostic::Severity::Error,
            message: "expected `fn`".into(),
            path: Some("ordinary.spx".into()),
            span: Some(crate::ast::Span {
                start: 67,
                end: 73,
                line: 1,
                column: 68
            }),
            help: None,
        }
    );
    // Imports are admitted only inside an interface, including real Rust
    // imports. Its name takes precedence over the competing local function.
    let admitted = format!(
        "{HEADER}@id(\"ordinary.host\") interface Host permits {{}} {{ @id(\"host.f\") import rust fn f()->i64 effects {{}} failure infallible; }} @id(\"ordinary.main\") fn main()->i64{{f()}}"
    );
    assert_eq!(proof(&admitted).roots, 0);
    let mut raw = super::super::StructuralCost::new();
    raw.exact_expressions = 1;
    raw.exact_carrier_free_calls = 2;
    assert!(super::super::cost::exact_absent_storage(&raw).is_err());
    raw.exact_carrier_free_calls = usize::MAX;
    assert!(super::super::cost::exact_absent_storage(&raw).is_err());
    raw.exact_carrier_free_calls = 0;
    let mut runtime = super::super::StructuralCost::new();
    runtime.exact_carrier_free_calls = usize::MAX;
    assert_eq!(
        super::super::cost::fixed_hir_discount(5, &raw, &runtime).unwrap(),
        0
    );
}
