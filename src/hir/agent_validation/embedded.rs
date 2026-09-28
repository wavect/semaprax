//! Independent replay of checked embedded operation/helper associations.
use super::*;
use crate::hir::{ResolvedAgentDeclaration, ResolvedAgentOperationRoleKind};

pub(super) fn validate(
    program: &ResolvedProgram,
    agent: &ResolvedAgentDeclaration,
) -> Result<(), Diagnostic> {
    if agent.model_wait.is_none() && !agent.operations.iter().any(|op| op.embedded) {
        return Ok(());
    }
    for operation in &agent.operations {
        if operation.kind != ResolvedAgentOperationKind::Deterministic {
            if operation.embedded {
                return Err(invalid("model/effect Agent body metadata is invalid"));
            }
            continue;
        }
        let mut matches = program
            .functions
            .iter()
            .filter(|f| f.id == operation.stable_id);
        let function = matches
            .next()
            .ok_or_else(|| invalid("opted-in Agent role has no checked function"))?;
        let name = match operation.role {
            ResolvedAgentOperationRoleKind::Initialize => "initialize",
            ResolvedAgentOperationRoleKind::Observe => "observe",
            ResolvedAgentOperationRoleKind::Authorize => "authorize",
            ResolvedAgentOperationRoleKind::Reduce => "reduce",
            _ => return Err(invalid("embedded Agent operation role is invalid")),
        };
        if matches.next().is_some()
            || function.name != name
            || !function.effects.is_empty()
            || function.yields.is_some()
        {
            return Err(invalid(
                "opted-in Agent role is outside the checked pure function profile",
            ));
        }
    }
    if let Some(binding) = &agent.model_wait {
        if !crate::agent_definition::canonical_identifier(binding.helper_id.as_str())
            || binding.helper_id == agent.stable_id
            || agent.types.iter().any(|r| r.stable_id == binding.helper_id)
            || agent
                .operations
                .iter()
                .any(|r| r.stable_id == binding.helper_id)
        {
            return Err(invalid(
                "checked model wait identity is invalid or aliases an Agent role",
            ));
        }
        let mut matches = program
            .functions
            .iter()
            .filter(|f| f.id == binding.helper_id);
        let function = matches
            .next()
            .ok_or_else(|| invalid("checked model wait helper is missing"))?;
        if matches.next().is_some()
            || program.agents.iter().any(|owner| {
                owner
                    .operations
                    .iter()
                    .any(|op| op.embedded && op.stable_id == binding.helper_id)
            })
            || !program
                .declarations
                .declaration(&function.id)
                .is_some_and(|d| d.identity_origin == crate::hir::IdentityOrigin::Explicit)
        {
            return Err(invalid(
                "checked model wait helper must have a unique explicit identity",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{cache_codec, hir, parser::agent_embedded_tests};

    fn source() -> String {
        format!(
            "{}\n@id(\"helper\") fn wait(value:i64)->i64 yields i64 -> i64 {{ yield value }}",
            agent_embedded_tests::source().replace(
                "runtime_v1",
                "model_wait_v1 { propose = \"helper\"; } runtime_v1"
            )
        )
    }

    #[test]
    fn embedded_origin_and_wait_binding_survive_both_private_cache_codecs() {
        let ast = crate::check(&source(), "agent-cache.spx").unwrap();
        let bytes = cache_codec::encode(&ast).unwrap();
        let decoded: crate::ast::Program = cache_codec::decode(&bytes).unwrap();
        assert_eq!(cache_codec::encode(&decoded).unwrap(), bytes);
        assert_eq!(
            crate::format::canonical(&decoded),
            crate::format::canonical(&ast)
        );
        decoded.agents[0]
            .validate_execution_metadata(&decoded)
            .unwrap();
        let resolved = hir::resolve(&decoded).unwrap();
        let bytes = cache_codec::encode(&resolved).unwrap();
        let decoded: hir::ResolvedProgram = cache_codec::decode(&bytes).unwrap();
        hir::validate(&decoded).unwrap();
        assert_eq!(cache_codec::encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.agents[0]
                .model_wait
                .as_ref()
                .unwrap()
                .helper_id
                .as_str(),
            "helper"
        );
        assert_eq!(
            decoded.agents[0]
                .operations
                .iter()
                .filter(|o| o.embedded)
                .count(),
            4
        );
        assert_eq!(decoded.functions.len(), 6);
    }

    #[test]
    fn inert_cache_decode_does_not_admit_forged_checked_agent_metadata() {
        let ast = crate::check(&source(), "agent-cache.spx").unwrap();
        let original = hir::resolve(&ast).unwrap();
        for selector in 0..3 {
            let mut forged = original.clone();
            match selector {
                0 => forged.agents[0].operations[2].embedded = true,
                1 => {
                    forged.agents[0].model_wait.as_mut().unwrap().helper_id =
                        hir::DeclarationId::new("op.propose")
                }
                _ => forged.agents[0].operations[0].stable_id = hir::DeclarationId::new("missing"),
            }
            let bytes = cache_codec::encode(&forged).unwrap();
            let decoded: hir::ResolvedProgram = cache_codec::decode(&bytes).unwrap();
            assert_eq!(hir::validate(&decoded).unwrap_err().code, "SPX-H006");
        }
    }
}
