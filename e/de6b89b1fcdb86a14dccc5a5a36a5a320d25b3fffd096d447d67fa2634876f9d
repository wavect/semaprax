//! Borrowed nominal lookup for metadata; the emitted declaration view stays local.
use crate::hir::items::{ItemSource, View};
use crate::workspace_graph::source_schema::*;

pub(super) struct Types<'a> {
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
}
impl<'a> Types<'a> {
    pub(super) fn new(
        module: &'a WorkspaceResolvedModule,
        modules: &'a [WorkspaceResolvedModule],
        facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    ) -> Result<Self, Vec<Diagnostic>> {
        reserve_builder_structure(std::mem::size_of::<Self>())?;
        Ok(Self {
            module,
            modules,
            facts,
        })
    }
}
impl ItemSource<hir::ResolvedTypeDeclaration> for Types<'_> {
    fn view(&self) -> View<'_, hir::ResolvedTypeDeclaration> {
        self.module.types.view()
    }
    fn find_in_context(
        &self,
        predicate: &mut dyn FnMut(&hir::ResolvedTypeDeclaration) -> bool,
    ) -> Option<&hir::ResolvedTypeDeclaration> {
        let mut candidates = self
            .modules
            .iter()
            .flat_map(|owner| owner.types.iter().map(move |item| (owner, item)))
            .filter(|(_, item)| predicate(item));
        let (owner, item) = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        let fact = self.facts.get(item.id.as_str())?;
        let kind = match item.kind {
            hir::ResolvedTypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
            hir::ResolvedTypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            hir::ResolvedTypeDeclarationKind::Class { .. } => hir::DeclarationKind::Class,
            hir::ResolvedTypeDeclarationKind::Resource { .. } => hir::DeclarationKind::Resource,
        };
        (fact.kind == kind
            && fact.origin != hir::IdentityOrigin::CompilerOwned
            && fact.owner.is_none()
            && fact.path.as_deref() == Some(owner.path.as_str())
            && fact.module.as_deref() == Some(owner.module.as_str())
            && self
                .modules
                .iter()
                .filter(|candidate| {
                    candidate.path == owner.path || candidate.module == owner.module
                })
                .count()
                == 1)
            .then_some(item)
    }
}
#[cfg(test)]
#[path = "main8a/tests.rs"]
mod tests;
