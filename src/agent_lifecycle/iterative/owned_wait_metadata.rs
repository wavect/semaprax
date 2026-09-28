//! Borrowed source-checked Outcome facts. No owner, grant or restore authority.
use super::*;
use crate::hir::{
    DeclarationId, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
};

pub(crate) struct OwnedWaitOutcomeMetadataV8<'a> {
    pub(crate) id: &'a DeclarationId,
    pub(crate) bytes_field: &'a DeclarationId,
    pub(crate) status_field: &'a DeclarationId,
    declaration: &'a ResolvedTypeDeclaration,
}
impl<'a> OwnedWaitOutcomeMetadataV8<'a> {
    pub(crate) fn fields(&self) -> impl Iterator<Item = (&'a DeclarationId, &'a ResolvedType)> {
        let ResolvedTypeDeclarationKind::Record { fields } = &self.declaration.kind else {
            unreachable!("checked lifecycle Outcome is a record")
        };
        fields.iter().map(|field| (&field.id, &field.ty))
    }
}
pub(super) fn outcome(lifecycle: &CompiledAgentLifecycle) -> OwnedWaitOutcomeMetadataV8<'_> {
    let shape = &lifecycle.binding.outcome;
    let declaration = lifecycle
        .program
        .types
        .iter()
        .find(|d| d.id == shape.record)
        .expect("checked lifecycle Outcome declaration");
    OwnedWaitOutcomeMetadataV8 {
        id: &shape.record,
        bytes_field: &shape.bytes_field,
        status_field: &shape.scalar_field,
        declaration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_frame_v8_outcome_metadata_borrows_actual_declared_field_order() {
        let source = super::super::tests::source("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }");
        let compiled = compile_agent_lifecycle_v2(
            &source,
            "outcome-metadata.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        let metadata = compiled.owned_wait_outcome_v8();
        assert_eq!(metadata.id.as_str(), "fixture.agent.type.outcome");
        assert_eq!(
            metadata.bytes_field.as_str(),
            "fixture.agent.type.outcome.value"
        );
        assert_eq!(
            metadata.status_field.as_str(),
            "fixture.agent.type.outcome.status"
        );
        assert_eq!(
            metadata
                .fields()
                .map(|(id, ty)| (id.as_str(), ty))
                .collect::<Vec<_>>(),
            vec![
                ("fixture.agent.type.outcome.value", &ResolvedType::Bytes),
                ("fixture.agent.type.outcome.status", &ResolvedType::I64)
            ]
        );
        let declaration = compiled
            .inner
            .program
            .types
            .iter()
            .find(|d| d.id == *metadata.id)
            .unwrap();
        let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
            panic!()
        };
        assert!(metadata
            .fields()
            .zip(fields)
            .all(|((id, ty), field)| std::ptr::eq(id, &field.id) && std::ptr::eq(ty, &field.ty)));
    }
}
