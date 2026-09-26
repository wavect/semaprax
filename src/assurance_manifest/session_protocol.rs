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

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::lattice::AssuranceClass;
use super::obligation::{MethodRecord, Obligation, ObligationKind};

const TOOL: &str = "semaprax-session-protocol-checker";
const LOCATOR: &str = "protocol:static-validation";

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
}
