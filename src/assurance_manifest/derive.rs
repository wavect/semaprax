//! Automatic obligation derivation from one already-verified `ast::Program`.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Obligation derivation" for exactly which facts justify which assurance
//! class, and why every other obligation kind is deliberately absent here.

use crate::ast::{Expr, ExprKind, MatchPattern, Program};

use super::lattice::AssuranceClass;
use super::obligation::{MethodRecord, Obligation, ObligationKind};

/// The tool name recorded on every automatically derived `precondition`/
/// `postcondition` method record: every admitted backend (native, Wasm, and
/// the tree-walking interpreter) lowers a `requires`/`ensures` clause into a
/// trapping runtime check at the same call boundary, so one tool name
/// covers all of them without overstating which specific backend produced
/// the artifact this manifest was generated for (this producer never
/// compiles or runs a backend; see "Exact nonclaims").
const CONTRACT_GUARD_TOOL: &str = "semaprax-runtime-contract-guard";

/// The tool name recorded on every automatically derived
/// `ownership_parameter` method record.
const OWNERSHIP_CHECKER_TOOL: &str = "semaprax-ownership-checker";

/// The tool name recorded on every automatically derived `exhaustiveness`
/// method record: `source_verify`'s iterative verifier rejects a variant
/// `match` that does not cover every declared case (or a trailing
/// wildcard) with `SPX-M101` "non-exhaustive match; missing case", on every
/// admitted backend alike (the check runs once, at the AST level, before
/// any backend sees the program).
const MATCH_EXHAUSTIVENESS_CHECKER_TOOL: &str = "semaprax-match-exhaustiveness-checker";

/// Derive the automatic obligations for `program`. Callers must have
/// already run `verify::verify(program)` and confirmed no error diagnostic
/// fired; this function does not re-check that itself; see
/// [`super::generate`].
pub(super) fn derive_obligations(program: &Program) -> Vec<Obligation> {
    let mut obligations = Vec::new();
    for function in &program.functions {
        for index in 0..function.requires.len() {
            obligations.push(contract_obligation(
                &function.stable_id,
                ObligationKind::Precondition,
                "require",
                index,
            ));
        }
        for index in 0..function.ensures.len() {
            obligations.push(contract_obligation(
                &function.stable_id,
                ObligationKind::Postcondition,
                "ensure",
                index,
            ));
        }
        for index in 0..function.params.len() {
            obligations.push(ownership_parameter_obligation(&function.stable_id, index));
        }
        obligations.extend(exhaustiveness_obligations(function));
    }
    obligations
}

/// One `exhaustiveness` obligation per variant `match` expression reachable
/// from `function`'s `requires`, body, and `ensures` (in that fixed order,
/// matching [`super::validate_function_profile`]'s own traversal breadth so
/// a match inside a contract clause — legal; see
/// `docs/STANDARD-LIBRARY-CATALOG.md`'s `requires match borrow ...`
/// examples — is not silently skipped). A record match, a scalar match, or
/// any other expression is not exhaustiveness-checked by `SPX-M101` the
/// same way and so derives nothing here; over-deriving would misstate what
/// was actually proved.
fn exhaustiveness_obligations(function: &crate::ast::Function) -> Vec<Obligation> {
    let mut obligations = Vec::new();
    let mut index = 0usize;
    let mut visit = |expression: &Expr| {
        if is_variant_match(expression) {
            obligations.push(exhaustiveness_obligation(&function.stable_id, index));
            index += 1;
        }
    };
    for require in &function.requires {
        walk_expr(require, &mut visit);
    }
    walk_expr(&function.body, &mut visit);
    for ensure in &function.ensures {
        walk_expr(ensure, &mut visit);
    }
    obligations
}

/// `true` for a `match` with at least one variant-case arm: the only match
/// shape `source_verify`'s case-coverage check (`SPX-M101`) applies to. A
/// match's arms are homogeneous by construction (the parser never mixes
/// variant, record, and scalar-literal patterns in one match), so one
/// variant-pattern arm is sufficient to identify the whole expression as a
/// variant match, including one that also carries a trailing wildcard arm.
fn is_variant_match(expression: &Expr) -> bool {
    matches!(&expression.kind, ExprKind::Match { arms, .. }
        if arms.iter().any(|arm| matches!(arm.pattern, MatchPattern::Variant { .. })))
}

/// Depth-first pre-order visit of every expression reachable from `root`,
/// `root` itself first. Uses an explicit heap-allocated stack rather than
/// plain recursion, deliberately: this crate has known stack-overflow
/// hazards on deeply nested expressions on a default-stack debug build (see
/// `CLAUDE.md` "Known conditions"), and `Expr::child`/`Statement::child`
/// already define the exact left-to-right child order this traversal needs.
fn walk_expr<'a>(root: &'a Expr, visit: &mut impl FnMut(&'a Expr)) {
    let mut stack = vec![(root, 0usize)];
    while let Some((expression, next_child)) = stack.pop() {
        if next_child == 0 {
            visit(expression);
        }
        if let Some(child) = expression.child(next_child) {
            stack.push((expression, next_child + 1));
            stack.push((child, 0));
        }
    }
}

fn exhaustiveness_obligation(declaration_id: &str, index: usize) -> Obligation {
    let locator = format!("match:{index}");
    let method = MethodRecord::new(
        AssuranceClass::CompilerProved,
        MATCH_EXHAUSTIVENESS_CHECKER_TOOL,
        env!("CARGO_PKG_VERSION"),
    );
    let method = MethodRecord {
        detail: Some(
            "variant match case coverage checked at compile time by source_verify (SPX-M101); \
             generate() only derives this after verify::verify returned no error diagnostic"
                .to_owned(),
        ),
        ..method
    };
    Obligation::new(ObligationKind::Exhaustiveness, declaration_id, &locator).with_method(method)
}

fn contract_obligation(
    declaration_id: &str,
    kind: ObligationKind,
    locator_prefix: &str,
    index: usize,
) -> Obligation {
    let locator = format!("{locator_prefix}:{index}");
    let method = MethodRecord::new(
        AssuranceClass::RuntimeGuarded,
        CONTRACT_GUARD_TOOL,
        env!("CARGO_PKG_VERSION"),
    );
    let method = MethodRecord {
        runtime_fallback: true,
        detail: Some(
            "requires/ensures clause compiled to a trapping runtime guard on every admitted backend"
                .to_owned(),
        ),
        target: Some("all_backends".to_owned()),
        ..method
    };
    Obligation::new(kind, declaration_id, &locator).with_method(method)
}

fn ownership_parameter_obligation(declaration_id: &str, index: usize) -> Obligation {
    let locator = format!("param:{index}");
    let method = MethodRecord::new(
        AssuranceClass::CompilerProved,
        OWNERSHIP_CHECKER_TOOL,
        env!("CARGO_PKG_VERSION"),
    );
    let method = MethodRecord {
        detail: Some(
            "parameter ownership mode checked at compile time by source_verify; \
             generate() only derives this after verify::verify returned no error diagnostic"
                .to_owned(),
        ),
        ..method
    };
    Obligation::new(ObligationKind::OwnershipParameter, declaration_id, &locator)
        .with_method(method)
}

#[cfg(test)]
mod tests {
    use super::super::obligation::obligation_id;
    use super::*;

    fn program(source: &str) -> Program {
        crate::parse(source, "derive-test.spx").expect("parse")
    }

    #[test]
    fn derives_one_obligation_per_clause_and_parameter() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.check")
fn check(a: i64, b: i64) -> i64
    requires a >= 0
    requires b >= 0
    ensures result >= 0
{ a + b }
"#,
        );
        let obligations = derive_obligations(&program);
        // Two `requires` + one `ensures` + two ownership_parameter (a, b).
        assert_eq!(obligations.len(), 5);
        let kinds: Vec<ObligationKind> = obligations.iter().map(|o| o.kind).collect();
        assert_eq!(
            kinds
                .iter()
                .filter(|&&k| k == ObligationKind::Precondition)
                .count(),
            2
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|&&k| k == ObligationKind::Postcondition)
                .count(),
            1
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|&&k| k == ObligationKind::OwnershipParameter)
                .count(),
            2
        );
        for obligation in &obligations {
            assert_eq!(obligation.declaration_id, "app.derive.check");
            assert_eq!(obligation.methods.len(), 1);
        }
    }

    #[test]
    fn contract_obligations_are_runtime_guarded_not_compiler_proved() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.guarded")
fn guarded(a: i64) -> i64
    ensures result == a
{ a }
"#,
        );
        let obligations = derive_obligations(&program);
        let postcondition = obligations
            .iter()
            .find(|o| o.kind == ObligationKind::Postcondition)
            .expect("one postcondition obligation");
        assert_eq!(
            postcondition.methods[0].class,
            AssuranceClass::RuntimeGuarded
        );
    }

    #[test]
    fn ownership_parameter_obligations_are_compiler_proved() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.owned")
fn owned(value: i64) -> i64 { value }
"#,
        );
        let obligations = derive_obligations(&program);
        let ownership = obligations
            .iter()
            .find(|o| o.kind == ObligationKind::OwnershipParameter)
            .expect("one ownership obligation");
        assert_eq!(ownership.methods[0].class, AssuranceClass::CompilerProved);
    }

    #[test]
    fn a_function_with_no_clauses_and_no_parameters_derives_nothing() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.nullary")
fn nullary() -> i64 { 0 }
"#,
        );
        assert!(derive_obligations(&program).is_empty());
    }

    #[test]
    fn derivation_is_deterministic_across_repeated_calls() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.check")
fn check(a: i64) -> i64
    requires a >= 0
    ensures result >= 0
{ a }
"#,
        );
        let first = derive_obligations(&program);
        let second = derive_obligations(&program);
        assert_eq!(first, second);
    }

    #[test]
    fn exhaustive_variant_match_derives_one_compiler_proved_obligation() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.choice")
variant Choice {
    @id("app.derive.choice.empty")
    Empty,
    @id("app.derive.choice.number")
    Number {
        @id("app.derive.choice.number.value")
        value: i64,
    },
}

@id("app.derive.classify")
fn classify() -> i64
{
    let value = Choice::Empty {};
    match value {
        Choice::Empty {} => 0,
        Choice::Number { value } => value,
    }
}
"#,
        );
        let obligations = derive_obligations(&program);
        let matches: Vec<&Obligation> = obligations
            .iter()
            .filter(|o| o.kind == ObligationKind::Exhaustiveness)
            .collect();
        assert_eq!(matches.len(), 1, "{obligations:?}");
        assert_eq!(matches[0].declaration_id, "app.derive.classify");
        assert_eq!(
            matches[0].id,
            obligation_id(
                ObligationKind::Exhaustiveness,
                "app.derive.classify",
                "match:0"
            ),
            "obligation id must be exactly obligation_id(Exhaustiveness, decl, \"match:0\")"
        );
        assert!(
            matches[0].id.contains("match:0"),
            "locator must be structural, not a byte offset: {}",
            matches[0].id
        );
        assert_eq!(matches[0].methods.len(), 1);
        assert_eq!(matches[0].methods[0].class, AssuranceClass::CompilerProved);
        assert_eq!(
            matches[0].methods[0].tool,
            MATCH_EXHAUSTIVENESS_CHECKER_TOOL
        );
    }

    #[test]
    fn a_wildcard_covered_variant_match_still_derives_an_obligation() {
        // SPX-M101 accepts either every case explicitly covered, or a
        // trailing wildcard; this must derive the obligation either way,
        // since both are the same checked fact (case coverage), not two
        // different ones.
        let program = program(
            r#"
module app.derive;

@id("app.derive.choice2")
variant Choice2 {
    @id("app.derive.choice2.empty")
    Empty,
    @id("app.derive.choice2.number")
    Number {
        @id("app.derive.choice2.number.value")
        value: i64,
    },
}

@id("app.derive.classify_wildcard")
fn classify_wildcard() -> i64
{
    let value = Choice2::Empty {};
    match value {
        Choice2::Empty {} => 0,
        _ => 1,
    }
}
"#,
        );
        let obligations = derive_obligations(&program);
        let matches = obligations
            .iter()
            .filter(|o| o.kind == ObligationKind::Exhaustiveness)
            .count();
        assert_eq!(matches, 1);
    }

    #[test]
    fn two_variant_matches_in_one_function_derive_two_obligations_in_traversal_order() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.choice3")
variant Choice3 {
    @id("app.derive.choice3.empty")
    Empty,
    @id("app.derive.choice3.number")
    Number {
        @id("app.derive.choice3.number.value")
        value: i64,
    },
}

@id("app.derive.classify_twice")
fn classify_twice() -> i64
{
    let first = Choice3::Empty {};
    let second = Choice3::Number { value: 5 };
    let a = match first {
        Choice3::Empty {} => 0,
        Choice3::Number { value } => value,
    };
    let b = match second {
        Choice3::Empty {} => 0,
        Choice3::Number { value } => value,
    };
    a + b
}
"#,
        );
        let obligations = derive_obligations(&program);
        let matches: Vec<&Obligation> = obligations
            .iter()
            .filter(|o| o.kind == ObligationKind::Exhaustiveness)
            .collect();
        assert_eq!(matches.len(), 2, "{obligations:?}");
        assert!(matches[0].id.contains("match:0"));
        assert!(matches[1].id.contains("match:1"));
        assert_ne!(matches[0].id, matches[1].id);
    }

    /// Negative control for [`exhaustive_variant_match_derives_one_compiler_proved_obligation`]:
    /// a scalar match (literal patterns plus a wildcard, no variant case)
    /// is not exhaustiveness-checked by `SPX-M101` the same way, so it must
    /// derive nothing here. Without this, a bug that fired for *every*
    /// match rather than only variant matches would pass the positive test
    /// above undetected.
    #[test]
    fn a_scalar_match_derives_no_exhaustiveness_obligation() {
        let program = program(
            r#"
module app.derive;

@id("app.derive.classify_scalar")
fn classify_scalar(value: i64) -> i64
{
    match value {
        0 => 100,
        _ => 200,
    }
}
"#,
        );
        let obligations = derive_obligations(&program);
        assert!(obligations
            .iter()
            .all(|o| o.kind != ObligationKind::Exhaustiveness));
    }

    #[test]
    fn exhaustiveness_locator_covers_a_match_inside_a_requires_clause() {
        // `docs/STANDARD-LIBRARY-CATALOG.md` shows `requires match borrow
        // input { ... }` is legal source; this must not be silently
        // skipped merely because it sits in a contract clause rather than
        // the body.
        let program = program(
            r#"
module app.derive;

@id("app.derive.choice4")
variant Choice4 {
    @id("app.derive.choice4.empty")
    Empty,
    @id("app.derive.choice4.number")
    Number {
        @id("app.derive.choice4.number.value")
        value: i64,
    },
}

@id("app.derive.guarded")
fn guarded(input: borrow Choice4) -> i64
    requires match borrow input { Choice4::Empty {} => true, Choice4::Number { value } => value >= 0, }
{
    0
}
"#,
        );
        let obligations = derive_obligations(&program);
        let matches: Vec<&Obligation> = obligations
            .iter()
            .filter(|o| o.kind == ObligationKind::Exhaustiveness)
            .collect();
        assert_eq!(matches.len(), 1, "{obligations:?}");
        assert!(matches[0].id.contains("match:0"));
    }
}
