//! Independent replay of checked embedded operation/helper associations.
use super::*;
use crate::hir::{ResolvedAgentDeclaration, ResolvedAgentOperationRoleKind};

pub(super) fn validate(
    program: &ResolvedProgram,
    agent: &ResolvedAgentDeclaration,
) -> Result<(), Diagnostic> {
    if !agent.has_execution_metadata() && agent.source_association.is_none() {
        return Ok(());
    }
    let source = agent
        .source_association
        .as_ref()
        .ok_or_else(|| invalid("checked Agent source association is missing"))?;
    if source.module.is_empty()
        || source.operations != agent.operations
        || source.model_wait != agent.model_wait
        || source.helper_top_level != agent.model_wait.is_some()
    {
        return Err(invalid(
            "checked Agent metadata differs from its source association",
        ));
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

/// Re-derive association provenance from retained original source, not decoded
/// strings or a synthetic module's imported stubs. This walk allocates nothing.
pub(crate) fn replay_agent_source_associations(
    original: &crate::ast::Program,
    resolved: &[ResolvedAgentDeclaration],
) -> Result<(), Diagnostic> {
    if original.agents.len() != resolved.len() {
        return Err(invalid(
            "source Agent inventory differs from retained checked metadata",
        ));
    }
    for declaration in &original.agents {
        declaration
            .validate_execution_metadata(original)
            .map_err(invalid)?;
        let mut matches = resolved
            .iter()
            .filter(|agent| agent.stable_id.as_str() == declaration.stable_id);
        let agent = matches
            .next()
            .ok_or_else(|| invalid("retained source Agent is missing"))?;
        if matches.next().is_some()
            || agent.name != declaration.name
            || agent.runtime_v1_json != declaration.runtime_v1_json
            || agent.types.len() != declaration.types.len()
            || agent
                .types
                .iter()
                .zip(&declaration.types)
                .any(|(actual, expected)| {
                    actual.role as u8 != expected.role as u8
                        || actual.stable_id.as_str() != expected.stable_id
                })
            || agent.operations.len() != declaration.operations.len()
            || agent
                .operations
                .iter()
                .zip(&declaration.operations)
                .any(|(actual, expected)| {
                    actual.role as u8 != expected.role as u8
                        || actual.kind as u8 != expected.kind as u8
                        || actual.stable_id.as_str() != expected.stable_id
                        || actual.embedded != expected.embedded_function_index.is_some()
                })
            || agent
                .model_wait
                .as_ref()
                .map(|binding| binding.helper_id.as_str())
                != declaration
                    .model_wait
                    .as_ref()
                    .map(|binding| binding.helper_id.as_str())
        {
            return Err(invalid(
                "checked Agent metadata does not replay against original source",
            ));
        }
        match (
            &agent.source_association,
            declaration.has_execution_metadata(),
        ) {
            (Some(source), true)
                if source.module == original.module
                    && source.operations == agent.operations
                    && source.model_wait == agent.model_wait
                    && source.helper_top_level == declaration.model_wait.is_some() => {}
            (None, false) => {}
            _ => {
                return Err(invalid(
                    "checked Agent source association has no original module provenance",
                ))
            }
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
    #[test]
    fn linked_foreign_helper_and_origin_changes_fail_even_when_functions_exist() {
        let text = source().replace("yields i64 -> i64 { yield value }", "{ value }");
        let ast = crate::check(&text, "agent-linked.spx").unwrap();
        let original = hir::resolve(&ast).unwrap();
        let foreign = hir::resolve(&crate::check(
            "module foreign; @id(\"foreign.helper\") fn foreign_wait(value:i64)->i64 {value} @id(\"foreign.main\") fn main()->i64 {0}",
            "foreign.spx").unwrap()).unwrap();
        let functions = original
            .functions
            .iter()
            .chain(
                foreign
                    .functions
                    .iter()
                    .filter(|function| function.id.as_str() == "foreign.helper"),
            )
            .map(|function| hir::LinkedScalarFunction {
                function: function.clone(),
                origin: hir::IdentityOrigin::Explicit,
            })
            .collect();
        let mut linked = hir::link_package_scalar_workspace(
            "linked".into(),
            original.entrypoint.clone(),
            functions,
        )
        .unwrap();
        linked.agents = original.agents.clone();
        hir::validate(&linked).unwrap();
        super::replay_agent_source_associations(&ast, &linked.agents).unwrap();
        assert!(linked
            .functions
            .iter()
            .any(|function| function.id.as_str() == "foreign.helper"));
        for origin_mutation in [false, true] {
            let mut forged = linked.clone();
            if origin_mutation {
                forged.agents[0].operations[0].embedded = false;
            } else {
                forged.agents[0].model_wait.as_mut().unwrap().helper_id =
                    hir::DeclarationId::new("foreign.helper");
            }
            assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
        }
        let mut foreign_module = linked.clone();
        foreign_module.agents[0]
            .source_association
            .as_mut()
            .unwrap()
            .module = "foreign".into();
        hir::validate(&foreign_module).unwrap();
        assert_eq!(
            super::replay_agent_source_associations(&ast, &foreign_module.agents)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        // A fully reminted inert carrier can be internally consistent, but
        // cannot acquire original-source provenance through codec roundtrip.
        let mut reminted = linked.clone();
        let agent = &mut reminted.agents[0];
        agent.model_wait.as_mut().unwrap().helper_id = hir::DeclarationId::new("foreign.helper");
        agent.source_association.as_mut().unwrap().model_wait = agent.model_wait.clone();
        let bytes = cache_codec::encode(&reminted).unwrap();
        let decoded: hir::ResolvedProgram = cache_codec::decode(&bytes).unwrap();
        hir::validate(&decoded).unwrap();
        assert_eq!(
            super::replay_agent_source_associations(&ast, &decoded.agents)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}
