//! Automatic obligation derivation from one already-verified `ast::Program`.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Obligation derivation" for exactly which facts justify which assurance
//! class, and why every other obligation kind is deliberately absent here.

use crate::ast::Program;

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
    }
    obligations
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
}
