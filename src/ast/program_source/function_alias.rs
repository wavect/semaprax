//! Transient retained wrappers lend their exact provider payload. Admission is
//! deliberately name-equal: renamed imports keep the complete owning producer.
//! This view grants no import/wrapper authority. The producer runs the complete
//! unchanged wrapper classifier before choosing this representation; ordinary
//! source verification/resolution subsequently runs on the actual payload.
use super::{
    decl_slices::DeclarationDirectory, generated_roots::GeneratedRoots, DeclSlices, ProgramSource,
};
use crate::ast::{Function, Program};

pub(crate) struct FunctionView<'a> {
    _provider: &'a Program,
    original: &'a Function,
    _alias: &'a str,
}
impl<'a> FunctionView<'a> {
    pub(crate) fn checked(
        provider: &'a Program,
        original: &'a Function,
        alias: &'a str,
    ) -> Option<Self> {
        if alias != original.name
            || !provider
                .functions
                .iter()
                .any(|item| std::ptr::eq(item, original))
        {
            return None;
        }
        Some(Self {
            _provider: provider,
            original,
            _alias: alias,
        })
    }
}
pub(crate) enum FunctionEntry<'a> {
    Owned(usize),
    Borrowed(FunctionView<'a>),
}
pub(crate) struct FunctionDirectory<'a> {
    pub(crate) roots: GeneratedRoots,
    pub(crate) entries: Vec<FunctionEntry<'a>>,
}
impl DeclarationDirectory<Function> for FunctionDirectory<'_> {
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn get(&self, index: usize) -> Option<&Function> {
        match &self.entries[index] {
            FunctionEntry::Owned(index) => Some(&self.roots.functions[*index]),
            FunctionEntry::Borrowed(view) => Some(view.original),
        }
    }
}
pub(crate) struct TransientFunctions<'a> {
    authored: &'a Program,
    identity: super::OverlayIdentity,
    pub(crate) functions: FunctionDirectory<'a>,
}
impl<'a> TransientFunctions<'a> {
    pub(crate) fn new(authored: &'a Program, functions: FunctionDirectory<'a>) -> Self {
        Self {
            authored,
            identity: super::OverlayIdentity { _nonzero_size: 0 },
            functions,
        }
    }
}
impl super::sealed::Sealed for TransientFunctions<'_> {}
impl ProgramSource for TransientFunctions<'_> {
    fn subject(&self) -> super::Subject<'_> {
        super::Subject::Projected(&self.identity)
    }
    fn path(&self) -> &str {
        &self.authored.path
    }
    fn module(&self) -> &str {
        &self.authored.module
    }
    fn module_uses(&self) -> &[crate::ast::ModuleUse] {
        &[]
    }
    fn permits(&self) -> &[String] {
        &self.authored.permits
    }
    fn types(&self) -> DeclSlices<'_, crate::ast::TypeDeclaration> {
        DeclSlices::append(&self.authored.types, &self.functions.roots.types)
            .expect("producer checked complete type count")
    }
    fn interfaces(&self) -> &[crate::ast::InterfaceDeclaration] {
        &self.authored.interfaces
    }
    fn protocols(&self) -> &[crate::ast::ProtocolDeclaration] {
        &self.authored.protocols
    }
    fn implementations(&self) -> &[crate::ast::ProtocolImplementation] {
        &[]
    }
    fn session_protocols(&self) -> &[crate::ast::SessionProtocolDeclaration] {
        &self.authored.session_protocols
    }
    fn agents(&self) -> &[crate::ast::AgentDeclaration] {
        &self.authored.agents
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        DeclSlices::projected(&self.authored.functions, &self.functions)
            .expect("producer checked complete function count")
    }
}
