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
            // Mode six additionally acquires its separate local-ID proof.
            // Keep the complete pre-carrier formula, adding the exact new fee
            // and removing only the independently certified callee slots.
            let (local_fee, local_removed) = if mode == 6 {
                let programs = vec![parse(&source)];
                let authored = crate::workspace_graph::index_authored(&programs).unwrap();
                let local = super::super::shared_local_callees::costs(
                    super::super::shared_local_callees::Context {
                        program: &programs[0],
                        authored: &authored,
                        programs: &programs,
                    },
                )
                .unwrap();
                let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
                (
                    local.bytes,
                    local.slots * maximum * super::super::HIR_IDENTITY_COPY_FACTOR,
                )
            } else {
                (0, 0)
            };
            assert_eq!(
                new_hir + copies * p.roots * carriers() + local_removed,
                old_hir + p.bytes + local_fee
            );
            assert_eq!(
                new_total + copies * p.roots * carriers() + local_removed,
                old_total + p.bytes + local_fee
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
    // Frozen pre-filter cold tariff remains independently executable without
    // repricing its receiver model. New current scope is qualified separately.
    scratch_before(|| {
        before_names(|| {
            for n in [1, 2, 16, 64] {
                let source = dense(n);
                let programs = vec![parse(&source)];
                let authored = crate::workspace_graph::index_authored(&programs).unwrap();
                let (p, allocation) = crate::test_allocation_observer::measure(|| {
                    count(&programs[0], &authored, true)
                });
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
                let (actual, allocation) = crate::test_allocation_observer::measure(|| {
                    names.contains(query, &mut invoice)
                });
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
            let (old, new, old_total, new_total) =
                costs(source, 7, cached_exact::Retention::Semantic);
            assert_eq!(new, old + p.bytes);
            assert_eq!(new_total, old_total + p.bytes);
        })
    });
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

// Full immediate-old method bodies execute lexically at the SAME caller ABI.
// Only self/name/proof token spelling changes for macro hygiene; whole original
// parent bytes remain in call_carriers.before_name_filter.rs.txt.
macro_rules! names_before_insert {
    ($this:ident, $name:ident, $proof:ident) => {{
        $proof.charge(&(&*$this, $name));
        if $this.len == $this.entries.len() {
            return false;
        }
        $this.entries[$this.len] = Some($name);
        $this.len += 1;
        true
    }};
}
pub(super) use names_before_insert;
macro_rules! names_before_contains {
    ($this:ident, $name:ident, $proof:ident) => {{
        $proof.charge(&($this, $name));
        // One scalar cursor. No iterator, closure environment, Some($name)
        // construction or Option equality pipeline is formed by this search.
        $proof.bytes = $proof.bytes.saturating_add(std::mem::size_of::<usize>());
        let mut index = 0;
        loop {
            // Actual cursor/length comparison values and its boolean result,
            // including the terminal probe on an absent spelling.
            $proof.bytes = $proof
                .bytes
                .saturating_add(2 * std::mem::size_of::<usize>() + std::mem::size_of::<bool>());
            if index == $this.len {
                $proof.charge(&false);
                return false;
            }
            // Array Index actual/formal receivers and indices, returned option
            // loan. Matching a borrowed place forms no Option<&str> value.
            $proof.bytes = $proof.bytes.saturating_add(
                2 * std::mem::size_of::<&[Option<&str>; 128]>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<&Option<&str>>(),
            );
            if let Some(item) = &$this.entries[index] {
                // The &&str pattern binding, str::eq's actual/formal fat loans,
                // and its returned boolean all belong to this reached probe.
                $proof.bytes = $proof.bytes.saturating_add(
                    std::mem::size_of::<&&str>()
                        + 4 * std::mem::size_of::<&str>()
                        + std::mem::size_of::<bool>(),
                );
                if str::eq(*item, $name) {
                    $proof.charge(&true);
                    return true;
                }
            }
            $proof.bytes = $proof.bytes.saturating_add(std::mem::size_of::<usize>());
            index += 1;
        }
    }};
}
pub(super) use names_before_contains;

thread_local! {
    static BEFORE_NAMES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn before_names_selected() -> bool {
    BEFORE_NAMES.with(|mode| mode.get())
}
pub(super) fn before_names<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE_NAMES.with(|mode| mode.set(self.0));
        }
    }
    let _restore = Restore(BEFORE_NAMES.with(|mode| mode.replace(true)));
    body()
}

#[test]
fn length_residue_filter_keeps_all_names_capacity_collisions_and_byte_spellings() {
    let mut strings = (0..128)
        .map(|n| format!("name{n}{}", "x".repeat(n % 31)))
        .collect::<Vec<_>>();
    strings[0] = String::new();
    strings[1] = "é\0x".to_owned();
    strings[2] = "z".repeat(Names::FILTER_BITS * 3 + strings[1].len());
    let mut queries = strings.clone();
    queries.extend([
        "missing".to_owned(),
        "é\0y".to_owned(),
        "m".repeat(Names::FILTER_BITS * 4 + 4),
    ]);
    for old in [false, true] {
        let body = || {
            let mut names = Names {
                entries: [None; 128],
                len: 0,
            };
            let mut invoice = Proof::default();
            for item in &strings {
                assert!(names.insert(item, &mut invoice));
            }
            assert!(!names.insert("one beyond the original128 slots", &mut invoice));
            for query in &queries {
                let expected = strings.iter().any(|item| item == query);
                assert_eq!(
                    names.contains(query, &mut invoice),
                    expected,
                    "old={old} query={query:?}"
                );
            }
            assert!(invoice.bytes > 0);
        };
        if old {
            before_names(body)
        } else {
            body()
        }
    }
    // Same-residue but unequal spelling must execute full original comparison.
    for old in [false, true] {
        let body = || {
            let value = "a";
            let collision = "q".repeat(1 + Names::FILTER_BITS);
            let mut names = Names {
                entries: [None; 128],
                len: 0,
            };
            let mut invoice = Proof::default();
            assert!(names.insert(value, &mut invoice));
            assert!(!names.contains(&collision, &mut invoice));
            assert!(names.contains(value, &mut invoice));
        };
        if old {
            before_names(body)
        } else {
            body()
        }
    }
}

#[test]
fn length_residue_whole_mode6_forecast_retains_old_constructor_fees_and_real_net() {
    let mut source = String::from(
        "module length_filter; @id(\"callee\") fn callee()->i64{1} @id(\"main\") fn main()->i64{",
    );
    for n in 0..64 {
        source.push_str(&format!("let local_{n:03}={n};"));
    }
    source.push_str(
        &std::iter::repeat_n("callee()", 64)
            .collect::<Vec<_>>()
            .join("+"),
    );
    source.push_str("+local_063}");
    let mut imported = source.clone();
    imported = imported.replacen(
        "module length_filter;",
        "module length_filter; use function @id(\"remote\") from provider as remote;",
        1,
    );
    let provider = "module provider; @id(\"remote\") fn remote()->i64{1}";
    for text in [&source, &imported] {
        let programs = vec![
            crate::parse(text, "length-filter.spx").unwrap(),
            crate::parse(provider, "provider.spx").unwrap(),
        ];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
        let observe = |old| {
            let body = || {
                crate::test_allocation_observer::measure(|| {
                    crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                        let (full, retained) =
                            super::super::shared_local_callees::modular_pair::selected_estimates(
                                &programs[0],
                                &authored,
                                &programs,
                                Some(maximum),
                                6,
                                true,
                                None,
                            )
                            .unwrap();
                        let costs = [
                            full.retained_hir,
                            retained.retained_hir,
                            full.synthetic_ast,
                            retained.synthetic_ast,
                            full.runtime,
                            retained.runtime,
                        ];
                        let coverage = retained.retained_scratch;
                        drop((full, retained));
                        (costs, coverage)
                    })
                })
            };
            if old {
                before_names(body)
            } else {
                body()
            }
        };
        let old = observe(true);
        let current = observe(false);
        assert!(!old.0 .1 && !current.0 .1);
        assert!(old.0 .3.is_none() && current.0 .3.is_none());
        for heap in [&old.1, &current.1] {
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
        }
        assert_eq!(
            &old.0 .0 .0[2..],
            &current.0 .0 .0[2..],
            "all AST/runtime terms unchanged"
        );
        assert_eq!(
            old.0 .0 .1, current.0 .0 .1,
            "complete scratch coverage unchanged"
        );
        assert!(
            current.0 .0 .0[0] < old.0 .0 .0[0],
            "full caller must earn actual tariff reduction"
        );
        assert!(
            current.0 .0 .0[1] < old.0 .0 .0[1],
            "retained caller must earn actual tariff reduction"
        );
        // Source census tariff is not allocator RSS or an application claim.
        eprintln!(
            "NAME_LENGTH_FILTER module={} imported={} current={current:?} old={old:?}",
            programs[0].path,
            !programs[0].module_uses.is_empty()
        );
    }
}

#[test]
fn length_filter_full_old_owner_and_lexical_method_bodies_remain_source_bound() {
    use sha2::{Digest, Sha256};
    let physical_current = include_str!("call_carriers.rs");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/workspace_graph/expected_projection/call_carriers.rs");
    assert_eq!(std::fs::read(&path).unwrap(), physical_current.as_bytes());
    let acquired = scratch_source_before(
        "src/workspace_graph/expected_projection/call_carriers.rs",
        physical_current,
    );
    let current = acquired.as_str();
    let saved = include_str!("call_carriers.before_name_filter.rs.txt");
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("call_carriers.name_filter_inverse.json")).unwrap();
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    assert_eq!(packet["current"]["length"], current.len());
    assert_eq!(packet["current"]["sha256"], digest(current.as_bytes()));
    assert_eq!(packet["before"]["length"], saved.len());
    assert_eq!(packet["before"]["sha256"], digest(saved.as_bytes()));
    let mut restored = current.to_owned();
    for edit in packet["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(
        restored, saved,
        "WHOLE immediate-old source and all untouched controls"
    );
    // The compiled old expansions contain each complete body with only
    // explicit macro argument qualification; whitespace is formatter-owned.
    let test_source = include_str!("call_carriers_tests.rs");
    for (method, next) in [("insert", "contains"), ("contains", "pattern")] {
        let begin = saved.find(&format!("    fn {method}(")).unwrap();
        let finish = saved[begin..].find(&format!("    fn {next}(")).unwrap() + begin;
        let definition = &saved[begin..finish];
        let body = &definition[definition.find('{').unwrap() + 1..definition.rfind('}').unwrap()];
        let begin = test_source
            .find(&format!("macro_rules! names_before_{method} {{"))
            .unwrap();
        let fragment = &test_source[begin..];
        let begin = fragment.find("=> {{").unwrap() + 5;
        let end = fragment[begin..].find("    }};").unwrap() + begin;
        let body_after = fragment[begin..end]
            .replace("$this", "self")
            .replace("$name", "name")
            .replace("$proof", "proof");
        let compact = |text: &str| {
            text.chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        };
        assert_eq!(
            compact(&body_after),
            compact(body),
            "entire old {method} body"
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), physical_current.as_bytes());
}

#[test]
fn length_filter_direct_constructor_invoice_reports_miss_hit_and_empty_costs() {
    let cases: &[(&str, &[&str], [&str; 4])] = &[
        ("empty", &[], ["one", "é", "", "nul\0"]),
        ("hits", &["one", "é", "nul\0"], ["one", "é", "nul\0", "one"]),
        (
            "negative",
            &["aa", "bbb", "cccc", "dddd"],
            ["xxxxx", "yyyyy", "zzzzz", "qqqqq"],
        ),
    ];
    for (label, input, queries) in cases {
        let observe = |old| {
            let body = || {
                crate::test_allocation_observer::measure(|| {
                    let mut proof = Proof::default();
                    let mut names = Names {
                        entries: [None; 128],
                        len: 0,
                    };
                    proof.charge(&names);
                    for name in *input {
                        assert!(names.insert(name, &mut proof));
                    }
                    let mut answers = [false; 4];
                    // Both complete owners execute the same bounded query
                    // workload; constructor overhead is included, never reset.
                    for _ in 0..8 {
                        for (i, query) in queries.iter().enumerate() {
                            answers[i] = names.contains(query, &mut proof);
                        }
                    }
                    (answers, proof.bytes)
                })
            };
            if old {
                before_names(body)
            } else {
                body()
            }
        };
        let old = observe(true);
        let current = observe(false);
        assert_eq!(old.0 .0, current.0 .0);
        for heap in [&old.1, &current.1] {
            assert!(!heap.invalid);
            assert_eq!(
                (
                    heap.allocations,
                    heap.reallocations,
                    heap.allocated,
                    heap.freed,
                    heap.live
                ),
                (0, 0, 0, 0, 0)
            );
        }
        if *label == "negative" {
            assert!(current.0 .1 < old.0 .1, "whole constructor and all queries");
        } else {
            assert!(
                current.0 .1 >= old.0 .1,
                "empty/hit regressions stay visible"
            );
        }
        eprintln!("NAME_FILTER_DIRECT family={label} current={current:?} old={old:?}");
    }
    assert_eq!(
        size_of::<Names<'_>>(),
        size_of::<([Option<&str>; 128], usize)>(),
        "no new storage/layout baseline padding"
    );
}

thread_local! {
    static SCRATCH_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn scratch_before_selected() -> bool {
    SCRATCH_BEFORE.with(|mode| mode.get())
}
fn scratch_before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCRATCH_BEFORE.with(|mode| mode.set(self.0));
        }
    }
    let _restore = Restore(SCRATCH_BEFORE.with(|mode| mode.replace(true)));
    body()
}

fn scratch_source_before(path: &str, current: &str) -> String {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value =
        serde_json::from_str(include_str!("call_carriers.scratch_reuse_inverse.json")).unwrap();
    let row = packet["owners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path)
        .unwrap();
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    assert_eq!(row["current"]["length"], current.len());
    assert_eq!(row["current"]["sha256"], digest(current.as_bytes()));
    let mut before = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&before[start..end], edit["current"].as_str().unwrap());
        before.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(before, row["edits"][0]["before"].as_str().unwrap());
    assert_eq!(row["before"]["length"], before.len());
    assert_eq!(row["before"]["sha256"], digest(before.as_bytes()));
    before
}

#[test]
fn census_scratch_all_four_complete_old_bodies_and_sources_remain_independent() {
    let sources = [
        (
            "src/workspace_graph/expected_projection/call_carriers.rs",
            include_str!("call_carriers.rs"),
            "fn count_linear(",
            &["program", "authored", "imports"][..],
        ),
        (
            "src/workspace_graph/expected_projection/call_carriers_directory.rs",
            include_str!("call_carriers_directory.rs"),
            "fn indexed<const N: usize>(",
            &["N", "program", "authored", "imports", "selector"][..],
        ),
        (
            "src/workspace_graph/expected_projection/call_carriers_joint.rs",
            include_str!("call_carriers_joint.rs"),
            "pub(super) fn count_with_lookup<",
            &["context", "imports", "proof", "lookup"][..],
        ),
        (
            "src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs",
            include_str!("shared_local_callees/modular_census.rs"),
            "pub(in crate::workspace_graph::expected_projection) fn count_with_lookup<",
            &["context", "imports", "proof", "lookup"][..],
        ),
    ];
    for (path, current, signature, args) in sources {
        let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        assert_eq!(std::fs::read(&physical).unwrap(), current.as_bytes());
        let before = scratch_source_before(path, current);
        let function = &before[before.find(signature).unwrap()..];
        let start = function.find('{').unwrap() + 1;
        let mut depth = 1;
        let mut end = start;
        // The full old function body contains no brace-bearing string literal.
        for (offset, ch) in function[start..].char_indices() {
            if ch == '{' {
                depth += 1;
            }
            if ch == '}' {
                depth -= 1;
            }
            if depth == 0 {
                end = start + offset;
                break;
            }
        }
        assert_eq!(depth, 0);
        let fragment = &current[current.find("macro_rules! before_scratch_body {").unwrap()..];
        let from = fragment.find("=> {{").unwrap() + 5;
        let to = fragment[from..].find("    }};").unwrap() + from;
        let mut actual = fragment[from..to].to_owned();
        for arg in args {
            actual = actual.replace(&format!("${arg}"), arg);
        }
        let compact = |text: &str| {
            // rustfmt adds only this optional trailing comma to the existing
            // three-element cost type. All three paid types remain identical.
            text.chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
                .replace(
                    "([usize;12],[Option<usize>;4],[bool;4],)",
                    "([usize;12],[Option<usize>;4],[bool;4])",
                )
        };
        assert_eq!(
            compact(&actual),
            compact(&function[start..end]),
            "whole old ABI body: {path}"
        );
        assert_eq!(std::fs::read(physical).unwrap(), current.as_bytes());
    }
}

#[test]
fn census_scratch_reset_all_four_owners_preserves_capacity_shadowing_imports_and_incomplete() {
    let mut text = String::from("module scratch; @id(\"callee\") fn callee()->i64{1} @id(\"shadow\") fn shadow(callee:i64)->i64{callee} ");
    for (id, count) in [("edge", 128), ("overflow", 129)] {
        text.push_str(&format!("@id(\"{id}\") fn {id}()->i64{{"));
        for n in 0..count {
            text.push_str(&format!("let local_{n}={n};"));
        }
        text.push_str("callee()} ");
    }
    text.push_str("@id(\"after\") fn after()->i64{callee()} ");
    for n in 0..64 {
        text.push_str(&format!("@id(\"pad{n}\") fn pad{n}()->i64{{callee()}} "));
    }
    for imported in [false, true] {
        let source = if imported {
            text.replacen(
                "module scratch;",
                "module scratch; use function @id(\"remote\") from provider as remote;",
                1,
            )
        } else {
            text.clone()
        };
        let programs = vec![
            crate::parse(&source, "scratch.spx").unwrap(),
            crate::parse(
                "module provider; @id(\"remote\") fn remote()->i64{1}",
                "provider.spx",
            )
            .unwrap(),
        ];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let context = super::super::shared_local_callees::Context {
            program: &programs[0],
            authored: &authored,
            programs: &programs,
        };
        let observe = |old| {
            let body = || {
                crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                    crate::test_allocation_observer::measure(|| {
                        let data = [
                            count_linear(&programs[0], &authored, true),
                            count(&programs[0], &authored, true),
                            count_joint(context, true).unwrap().0,
                            count_modular(context, true).unwrap().0,
                        ];
                        let roots = data.each_ref().map(|proof| proof.roots);
                        let fees = data.each_ref().map(|proof| proof.bytes);
                        let coverage = data.each_ref().map(|proof| proof.retained);
                        drop(data);
                        (roots, fees, coverage)
                    })
                })
            };
            if old {
                scratch_before(body)
            } else {
                body()
            }
        };
        let old = observe(true);
        let current = observe(false);
        assert_eq!(
            current.0 .0 .0, old.0 .0 .0,
            "complete root census across all four owners"
        );
        assert_eq!(
            current.0 .0 .2, old.0 .0 .2,
            "complete constructor coverage unchanged"
        );
        assert!(old.0 .0 .0.iter().all(|roots| *roots >= 64));
        for (new, old) in current.0 .0 .1.iter().zip(old.0 .0 .1.iter()) {
            assert!(new < old, "constructor hoist must earn whole-census net");
        }
        for row in [&old, &current] {
            assert!(!row.1 && row.3.is_none());
            assert!(!row.0 .1.invalid && row.0 .1.live == 0);
            assert_eq!(row.0 .1.allocated, row.0 .1.freed);
            assert!(row.0 .1.allocated <= row.2);
        }
        eprintln!("CENSUS_NAMES_REUSE imported={imported} current={current:?} old={old:?}");
    }
}

#[test]
fn census_scratch_whole_mode6_forecast_reports_net_and_honest_cold_loss() {
    for count in [0, 1, 64] {
        for imported in [false, true] {
            let mut source = String::from("module reuse;");
            if imported {
                source.push_str("use function @id(\"remote\") from provider as remote;");
            }
            for n in 0..count {
                source.push_str(&format!("@id(\"f{n}\") fn f{n}()->i64{{{n}}} "));
            }
            let programs = vec![
                crate::parse(&source, "reuse.spx").unwrap(),
                crate::parse(
                    "module provider; @id(\"remote\") fn remote()->i64{1}",
                    "provider.spx",
                )
                .unwrap(),
            ];
            let authored = crate::workspace_graph::index_authored(&programs).unwrap();
            let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
            let observe = |old| {
                let body = || {
                    crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                        crate::test_allocation_observer::measure(|| {
                            let (full, retained) =
                            super::super::shared_local_callees::modular_pair::selected_estimates(
                                &programs[0],
                                &authored,
                                &programs,
                                Some(maximum),
                                6,
                                true,
                                None,
                            )
                            .unwrap();
                            let result = [
                                full.retained_hir,
                                retained.retained_hir,
                                full.synthetic_ast,
                                retained.synthetic_ast,
                                full.runtime,
                                retained.runtime,
                            ];
                            let coverage = retained.retained_scratch;
                            drop((full, retained));
                            (result, coverage)
                        })
                    })
                };
                if old {
                    scratch_before(body)
                } else {
                    body()
                }
            };
            let old = observe(true);
            let current = observe(false);
            assert_eq!(&old.0 .0 .0[2..], &current.0 .0 .0[2..]);
            assert_eq!(old.0 .0 .1, current.0 .0 .1);
            if count == 64 {
                assert!(current.0 .0 .0[0] < old.0 .0 .0[0]);
                assert!(current.0 .0 .0[1] < old.0 .0 .0[1]);
            } else {
                // Once-per-invocation backing and new reset controls are real.
                // Empty/tiny callers do not receive a fabricated amortized gain.
                assert!(current.0 .0 .0[0] >= old.0 .0 .0[0]);
                assert!(current.0 .0 .0[1] >= old.0 .0 .0[1]);
            }
            for row in [&old, &current] {
                assert!(!row.1 && row.3.is_none());
                assert!(!row.0 .1.invalid && row.0 .1.live == 0);
                assert_eq!(row.0 .1.allocated, row.0 .1.freed);
                assert!(row.0 .1.allocated <= row.2);
            }
            eprintln!("CENSUS_NAMES_REUSE_WHOLE count={count} imported={imported} current={current:?} old={old:?}");
        }
    }
}
