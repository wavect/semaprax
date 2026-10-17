//! Owning tests use the authenticated held-source context without dropping builtins
//! or manufacturing a declaration index from standalone HIR.
use super::*;
pub(crate) struct SourceRecordReplayFixture(WorkspaceGraphBuild);
impl SourceRecordReplayFixture {
    pub(crate) fn new(source: &str) -> Self {
        Self::try_new(source).unwrap()
    }
    pub(crate) fn try_new(source: &str) -> Result<Self, Vec<Diagnostic>> {
        let sources = [
            ("flat.spx", source),
            (
                "support.spx",
                "module support.flat;@id(\"support.ready\") fn ready()->i64{0}",
            ),
        ]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&crate::parse(source, path).unwrap()),
        })
        .collect();
        build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
            .map(|(build, _)| Self(build))
    }
    fn module(&self) -> &WorkspaceResolvedModule {
        self.0
            .hir
            .modules
            .iter()
            .find(|module| module.path == "flat.spx")
            .unwrap()
    }
    pub(crate) fn functions(&self) -> hir::items::View<'_, hir::ResolvedFunction> {
        hir::items::ItemSource::view(&self.module().functions)
    }
    pub(crate) fn function_mut(&mut self, id: &str) -> &mut hir::ResolvedFunction {
        self.0
            .hir
            .modules
            .iter_mut()
            .find(|module| module.path == "flat.spx")
            .unwrap()
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }
    /// Positive receiving controls borrow the actual prepared production
    /// directory, including its independently rebuilt canonical Vec type.
    /// The legacy callback below remains available for its closed negatives.
    pub(crate) fn with_prepared_context<T>(
        &self,
        apply: impl FnOnce(&dyn crate::cleanup::ReplayContext) -> T,
    ) -> T {
        let anchor = self
            .module()
            .functions
            .first()
            .expect("source fixture function");
        let preparation = crate::cleanup_plan::SelectedReplayPreparation::new(
            anchor,
            crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES),
        )
        .unwrap();
        source_schema::with_prepared_test_context(
            self.module(),
            &self.0.hir.modules,
            &self.0.hir.declarations,
            preparation,
            |context, _held_preparation| {
                assert!(
                    (0..context.type_count()).any(|i| context
                        .type_at(i)
                        .is_some_and(|ty| ty.id.as_str() == crate::prelude::VEC_ID)),
                    "prepared current directory retains the canonical builtin Vec"
                );
                apply(context)
            },
        )
        .unwrap()
    }
    /// Complete non-static-text controls use the production owned-data linker,
    /// not the closed literal-source receiver. Its roots are exact retained
    /// members; provider closure, builtin types and declaration index are all
    /// rebuilt and validated by the existing linker before the callback.
    pub(crate) fn with_linked_context<T>(
        &self,
        apply: impl FnOnce(&dyn crate::cleanup::ReplayContext) -> T,
    ) -> T {
        let module = self.module();
        let roots = module.functions.iter()
            .map(|function| function.id.as_str().to_owned())
            .collect::<Vec<_>>();
        let program = self.0.linked_owned_data_api_program_with_roots(
            &module.module, &roots,
        ).unwrap();
        apply(&program)
    }
    pub(crate) fn with_context<T>(
        &self,
        apply: impl FnOnce(&dyn crate::cleanup::ReplayContext) -> T,
    ) -> T {
        source_schema::with_test_context(self.module(), &self.0.hir.declarations, apply).unwrap()
    }
}
