//! `session_protocol` obligations (issue #297): one per declared session
//! protocol, keyed to the declaration's persistent `@id` with the fixed
//! locator `protocol:static-validation`.
//!
//! The single method is `compiler_proved` for static validation only: the
//! verifier's `SPX-K101`..`SPX-K105` checks (well-formed declared graph, the
//! kernel's `ProtocolSpec::validate`, capability attribution to each `via`
//! function's declared effects) plus the HIR `via` binding this module
//! re-runs. The kernel's bounded reachability check also runs in the
//! verifier, but this manifest's fixed `no_model_checker_invoked` nonclaim
//! stands: nothing here is recorded as `model_checked`, and no legal order is
//! recorded as authority. A program without a declaration derives nothing,
//! so its envelope bytes are unchanged.
//!
//! [`follows_obligations`] is this same module's issue #297 follow-on (R21):
//! one further `compiler_proved` obligation per function that opts into
//! endpoint typestate checking with `follows`, keyed to the *function's*
//! `@id` with the distinct locator `protocol:call-order-typestate`. It is a
//! **static call-order** claim over one function's own body
//! (`session_protocol::typestate::check`, `SPX-K107`..`SPX-K109`), clearly
//! not runtime authority and clearly distinct from [`obligations`]'s own
//! static-validation claim over the declared graph. A program with no
//! `follows` clause derives nothing, so its envelope bytes are unchanged.

use std::collections::BTreeMap;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::lattice::AssuranceClass;
use super::obligation::{MethodRecord, Obligation, ObligationKind};

const TOOL: &str = "semaprax-session-protocol-checker";
const LOCATOR: &str = "protocol:static-validation";
/// Issue #297 follow-on (R21): the locator for one function's endpoint
/// typestate `follows` obligation, distinct from [`LOCATOR`] (a
/// declaration's own static-validation obligation), and keyed to the
/// *function's* `stable_id` rather than the protocol's.
const FOLLOWS_LOCATOR: &str = "protocol:call-order-typestate";

pub(super) fn obligations(
    program: &Program,
    resolved: &ResolvedProgram,
) -> Result<Vec<Obligation>, Diagnostic> {
    crate::session_protocol::source::bind_to_hir(program, resolved)?;
    Ok(program
        .session_protocols
        .iter()
        .map(|declaration| {
            let mut method = MethodRecord::new(
                AssuranceClass::CompilerProved,
                TOOL,
                env!("CARGO_PKG_VERSION"),
            );
            method.inputs = vec![declaration.name.clone()];
            method.inputs.extend(
                declaration
                    .transitions
                    .iter()
                    .filter_map(|transition| transition.via.as_ref())
                    .map(|via| via.name.clone()),
            );
            method.detail = Some(
                "declared session protocol statically validated at compile time (SPX-K101..SPX-K105: \
                 declared graph well-formed under ProtocolSpec::validate, every via bound to a checked \
                 HIR function, every required capability already declared by its via function); \
                 static validation only, not model checking, and legal order grants no authority"
                    .to_owned(),
            );
            Obligation::new(
                ObligationKind::SessionProtocol,
                &declaration.stable_id,
                LOCATOR,
            )
            .with_method(method)
        })
        .collect())
}

/// One `compiler_proved` obligation per function that opts into endpoint
/// typestate checking with `follows session protocol "<id>"` (issue #297
/// follow-on, R21): the verifier's `SPX-K107`..`SPX-K109` static call-order
/// check (`session_protocol::typestate::check`). This is a **static
/// call-order** claim only, distinct from [`obligations`]'s own
/// static-validation claim over the declared graph itself, and it is never
/// runtime authority: passing this check proves only that the function's own
/// call sequence traces one legal path through the named protocol's declared
/// graph (`docs/SESSION-PROTOCOL-TYPES-V1.md`, "Legal order is still not
/// authority"). A program with no `follows` clause derives nothing.
pub(super) fn follows_obligations(program: &Program) -> Result<Vec<Obligation>, Diagnostic> {
    crate::session_protocol::source::bind_follows(program)?;
    let declared: BTreeMap<&str, &str> = program
        .session_protocols
        .iter()
        .map(|declaration| (declaration.stable_id.as_str(), declaration.name.as_str()))
        .collect();
    Ok(program
        .functions
        .iter()
        .filter_map(|function| function.follows.as_ref().map(|follows| (function, follows)))
        .map(|(function, follows)| {
            // `bind_follows` above already refused anything else, so this is
            // always present.
            let protocol_name = declared
                .get(follows.protocol_id.as_str())
                .copied()
                .unwrap_or(follows.protocol_id.as_str());
            let mut method = MethodRecord::new(
                AssuranceClass::CompilerProved,
                TOOL,
                env!("CARGO_PKG_VERSION"),
            );
            method.inputs = vec![protocol_name.to_owned(), function.name.clone()];
            method.detail = Some(
                "function opts into endpoint typestate checking with `follows session protocol` \
                 (SPX-K107..SPX-K109: starting from the protocol's initial state, every call to a \
                 via-bound function is legal from the function's current state at that point, and \
                 every path through the function's body ends in a declared terminal state); static \
                 call-order check of one function's own body only, never model checking or \
                 interprocedural analysis, and legal order grants no effect, capability, or \
                 resource authority"
                    .to_owned(),
            );
            Obligation::new(
                ObligationKind::SessionProtocol,
                &function.stable_id,
                FOLLOWS_LOCATOR,
            )
            .with_method(method)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    const DECLARED: &str = include_str!("../session_protocol/tests/fixtures/declared.spx");

    fn obligations_of(source: &str) -> Vec<super::Obligation> {
        let program = crate::check(source, "session.spx").unwrap();
        let resolved = crate::hir::resolve(&program).unwrap();
        super::obligations(&program, &resolved).unwrap()
    }

    #[test]
    fn one_compiler_proved_static_validation_obligation_per_declaration() {
        let obligations = obligations_of(DECLARED);
        assert_eq!(obligations.len(), 1);
        let obligation = &obligations[0];
        assert_eq!(obligation.declaration_id, "fixture.session.transaction");
        assert_eq!(obligation.kind, super::ObligationKind::SessionProtocol);
        assert_eq!(obligation.methods.len(), 1);
        assert_eq!(
            obligation.methods[0].class,
            super::AssuranceClass::CompilerProved
        );
        assert_eq!(
            obligation.methods[0].inputs,
            vec![
                "fixture-transaction-v1".to_owned(),
                "fixture.session.begin".to_owned(),
                "fixture.session.commit".to_owned()
            ]
        );
        assert!(obligation.methods[0]
            .detail
            .as_deref()
            .unwrap()
            .contains("not model checking"));
    }

    #[test]
    fn a_program_without_a_declaration_derives_nothing() {
        let start = DECLARED
            .find("@id(\"fixture.session.transaction\")")
            .unwrap();
        assert!(obligations_of(&DECLARED[..start]).is_empty());
    }

    #[test]
    fn the_hir_binding_is_rechecked_before_any_obligation_is_derived() {
        let program = crate::check(DECLARED, "session.spx").unwrap();
        let mut resolved = crate::hir::resolve(&program).unwrap();
        resolved
            .functions
            .retain(|function| function.id.as_str() != "fixture.session.commit");
        assert_eq!(
            super::obligations(&program, &resolved).unwrap_err().code,
            "SPX-K104"
        );
    }

    #[test]
    fn the_generated_envelope_records_the_obligation_replays_and_claims_no_model_check() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-assurance-session-protocol-{}-{}.spx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, DECLARED).unwrap();
        let options = crate::assurance_manifest::AssuranceManifestOptions::default();
        let outcome = crate::assurance_manifest::generate(&path, &options).map(|envelope| {
            let structural = crate::assurance_manifest::verify_envelope(&envelope);
            let source_bound =
                crate::assurance_manifest::verify_envelope_against_source(&envelope, &path);
            let second = crate::assurance_manifest::generate(&path, &options);
            (envelope, structural, source_bound, second)
        });
        std::fs::remove_file(&path).unwrap();
        let (envelope, structural, source_bound, second) = outcome.unwrap();
        structural.unwrap();
        source_bound.unwrap();
        assert_eq!(envelope, second.unwrap());
        let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        let obligations = value["payload"]["obligations"].as_array().unwrap();
        let protocol = obligations
            .iter()
            .filter(|obligation| obligation["kind"] == "session_protocol")
            .collect::<Vec<_>>();
        assert_eq!(protocol.len(), 1);
        assert_eq!(protocol[0]["declaration_id"], "fixture.session.transaction");
        assert_eq!(protocol[0]["classification"], "compiler_proved");
        assert!(obligations.iter().all(|obligation| obligation["methods"]
            .as_array()
            .unwrap()
            .iter()
            .all(|method| method["class"] != "model_checked")));
        assert!(value["payload"]["nonclaims"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("no_model_checker_invoked")));
    }

    // Issue #297 follow-on (R21): endpoint typestate `follows` obligations.

    const FOLLOWS: &str = include_str!("../session_protocol/tests/fixtures/follows.spx");

    fn follows_obligations_of(source: &str) -> Vec<super::Obligation> {
        let program = crate::check(source, "follows.spx").unwrap();
        super::follows_obligations(&program).unwrap()
    }

    #[test]
    fn one_compiler_proved_call_order_obligation_per_follows_using_function() {
        let obligations = follows_obligations_of(FOLLOWS);
        assert_eq!(obligations.len(), 1);
        let obligation = &obligations[0];
        assert_eq!(obligation.declaration_id, "fixture.follows.main");
        assert_eq!(obligation.kind, super::ObligationKind::SessionProtocol);
        assert_eq!(obligation.methods.len(), 1);
        assert_eq!(
            obligation.methods[0].class,
            super::AssuranceClass::CompilerProved
        );
        assert_eq!(
            obligation.methods[0].inputs,
            vec!["fixture-follows-v1".to_owned(), "main".to_owned()]
        );
        assert!(obligation.methods[0]
            .detail
            .as_deref()
            .unwrap()
            .contains("never model checking"));
        // Distinct locator/identity from the declaration's own obligation, so
        // both can coexist without colliding.
        assert!(obligation.id.contains("call-order-typestate"));
    }

    #[test]
    fn a_program_without_a_follows_clause_derives_nothing() {
        let without_follows = FOLLOWS.replacen(
            "\n    follows session protocol \"fixture.follows.protocol\"\n",
            "\n",
            1,
        );
        assert!(follows_obligations_of(&without_follows).is_empty());
    }

    #[test]
    fn a_follows_naming_no_declared_protocol_fails_closed() {
        let source = "module x;\n\n\
@id(\"x.f\")\nfn f() -> i64\n    follows session protocol \"x.missing\"\n{ 0 }\n";
        let program = crate::parse(source, "x.spx").unwrap();
        assert_eq!(
            super::follows_obligations(&program).unwrap_err().code,
            "SPX-K107"
        );
    }

    #[test]
    fn the_generated_envelope_records_both_the_declaration_and_follows_obligations() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-assurance-session-protocol-follows-{}-{}.spx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, FOLLOWS).unwrap();
        let options = crate::assurance_manifest::AssuranceManifestOptions::default();
        let outcome = crate::assurance_manifest::generate(&path, &options).map(|envelope| {
            let structural = crate::assurance_manifest::verify_envelope(&envelope);
            let source_bound =
                crate::assurance_manifest::verify_envelope_against_source(&envelope, &path);
            (envelope, structural, source_bound)
        });
        std::fs::remove_file(&path).unwrap();
        let (envelope, structural, source_bound) = outcome.unwrap();
        structural.unwrap();
        source_bound.unwrap();
        let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        let obligations = value["payload"]["obligations"].as_array().unwrap();
        let protocol = obligations
            .iter()
            .filter(|obligation| obligation["kind"] == "session_protocol")
            .collect::<Vec<_>>();
        assert_eq!(protocol.len(), 2);
        assert!(protocol
            .iter()
            .any(|obligation| obligation["declaration_id"] == "fixture.follows.protocol"));
        assert!(protocol
            .iter()
            .any(|obligation| obligation["declaration_id"] == "fixture.follows.main"));
        assert!(protocol
            .iter()
            .all(|obligation| obligation["classification"] == "compiler_proved"));
    }
}
