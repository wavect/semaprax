//! Internal compiler input. Borrowed declarations retain authored order and
//! identity; generated overlays are not verification or resolution authority.
use super::{
    AgentDeclaration, ExprKind, Function, InterfaceDeclaration, ModuleUse, Program,
    ProtocolDeclaration, ProtocolImplementation, SessionProtocolDeclaration, Type, TypeDeclaration,
};

mod decl_slices;
pub(crate) mod function_alias;
pub(crate) use decl_slices::DeclSlices;
#[cfg(test)]
mod decl_slices_original;

/// A concrete address distinguishes different presentations of the same AST.
/// The byte prevents distinct empty overlays from sharing a ZST identity.
pub(crate) struct OverlayIdentity {
    _nonzero_size: u8,
}
#[derive(Clone, Copy)]
pub(crate) enum Subject<'a> {
    Owned(&'a Program),
    Projected(&'a OverlayIdentity),
}
impl Subject<'_> {
    pub(crate) fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Owned(a), Self::Owned(b)) => std::ptr::eq(a, b),
            (Self::Projected(a), Self::Projected(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }
}
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Program {}
    impl Sealed for super::SyntheticProgramView<'_> {}
    impl Sealed for super::checked_key::ReadySource<'_> {}
}
/// Only the owned AST and compiler-owned presentation implement this input.
/// A view carries no verdict; every source pass and resolver still runs.
pub(crate) trait ProgramSource: sealed::Sealed {
    fn subject(&self) -> Subject<'_>;
    fn path(&self) -> &str;
    fn module(&self) -> &str;
    fn module_uses(&self) -> &[ModuleUse];
    fn permits(&self) -> &[String];
    fn types(&self) -> DeclSlices<'_, TypeDeclaration>;
    fn interfaces(&self) -> &[InterfaceDeclaration];
    fn protocols(&self) -> &[ProtocolDeclaration];
    fn implementations(&self) -> &[ProtocolImplementation];
    fn session_protocols(&self) -> &[SessionProtocolDeclaration];
    fn agents(&self) -> &[AgentDeclaration];
    fn functions(&self) -> DeclSlices<'_, Function>;
}
impl ProgramSource for Program {
    fn subject(&self) -> Subject<'_> {
        Subject::Owned(self)
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn module(&self) -> &str {
        &self.module
    }
    fn module_uses(&self) -> &[ModuleUse] {
        &self.module_uses
    }
    fn permits(&self) -> &[String] {
        &self.permits
    }
    fn types(&self) -> DeclSlices<'_, TypeDeclaration> {
        DeclSlices::one(&self.types)
    }
    fn interfaces(&self) -> &[InterfaceDeclaration] {
        &self.interfaces
    }
    fn protocols(&self) -> &[ProtocolDeclaration] {
        &self.protocols
    }
    fn implementations(&self) -> &[ProtocolImplementation] {
        &self.implementations
    }
    fn session_protocols(&self) -> &[SessionProtocolDeclaration] {
        &self.session_protocols
    }
    fn agents(&self) -> &[AgentDeclaration] {
        &self.agents
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        DeclSlices::one(&self.functions)
    }
}

#[path = "program_source/generated_roots.rs"]
pub(crate) mod generated_roots;
use generated_roots::GeneratedRoots;
pub(crate) mod receiving_roots;
use receiving_roots::{GeneratedOwner, ReceivingRoots};

/// A producer-paid generated overlay after the exact authored slices.
/// Authored bodies and declaration backing are never cloned to lend this input.
pub(crate) struct SyntheticProgramView<'a> {
    authored: &'a Program,
    generated: GeneratedOwner<'a>,
    identity: OverlayIdentity,
}
impl<'a> SyntheticProgramView<'a> {
    pub(crate) fn into_overlay_owner(self) -> (&'a Program, GeneratedRoots) {
        (self.authored, self.generated.into_plain())
    }

    pub(crate) fn with_generated_owner(
        authored: &'a Program,
        generated: GeneratedRoots,
    ) -> Option<Self> {
        authored.types.len().checked_add(generated.types.len())?;
        authored
            .functions
            .len()
            .checked_add(generated.functions.len())?;
        Some(Self {
            authored,
            generated: GeneratedOwner::Plain(generated),
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }

    pub(crate) fn with_receiving_owner(
        authored: &'a Program,
        generated: ReceivingRoots<'a>,
    ) -> Option<Self> {
        authored.types.len().checked_add(generated.types().len())?;
        authored
            .functions
            .len()
            .checked_add(generated.functions().len())?;
        Some(Self {
            authored,
            generated: GeneratedOwner::Receiving(generated),
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }
    pub(crate) fn into_receiving_owner(self) -> (&'a Program, GeneratedOwner<'a>) {
        (self.authored, self.generated)
    }

    pub(crate) fn new(authored: &'a Program, generated_functions: Vec<Function>) -> Option<Self> {
        // Keep the leaf-main constructor closed. Imported prototypes use the
        // separately classified and paid imported-overlay producer.
        if generated_functions.iter().any(|function| {
            function.name != "main"
                || !function.params.is_empty()
                || !function.type_parameters.is_empty()
                || !function.requires.is_empty()
                || !function.ensures.is_empty()
                || function.return_type != Type::I64
                || function.yields.is_some()
                || function.follows.is_some()
                || !function.effects.is_empty()
                || !matches!(function.body.kind, ExprKind::Int(0))
        }) {
            return None;
        }
        authored
            .functions
            .len()
            .checked_add(generated_functions.len())?;
        Some(Self {
            authored,
            generated: GeneratedOwner::Plain(GeneratedRoots::plain(
                Vec::new(),
                generated_functions,
            )),
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }
}
impl ProgramSource for SyntheticProgramView<'_> {
    fn subject(&self) -> Subject<'_> {
        Subject::Projected(&self.identity)
    }
    fn path(&self) -> &str {
        &self.authored.path
    }
    fn module(&self) -> &str {
        &self.authored.module
    }
    // Project validation authenticates these sidecars before presentation.
    // This is exactly the old clone-and-clear projection, not a permission.
    fn module_uses(&self) -> &[ModuleUse] {
        &[]
    }
    fn permits(&self) -> &[String] {
        &self.authored.permits
    }
    fn types(&self) -> DeclSlices<'_, TypeDeclaration> {
        DeclSlices::append(&self.authored.types, self.generated.types())
            .expect("constructor checked declaration count")
    }
    fn interfaces(&self) -> &[InterfaceDeclaration] {
        &self.authored.interfaces
    }
    fn protocols(&self) -> &[ProtocolDeclaration] {
        &self.authored.protocols
    }
    fn implementations(&self) -> &[ProtocolImplementation] {
        &[]
    }
    fn session_protocols(&self) -> &[SessionProtocolDeclaration] {
        &self.authored.session_protocols
    }
    fn agents(&self) -> &[AgentDeclaration] {
        &self.authored.agents
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        DeclSlices::append(&self.authored.functions, self.generated.functions())
            .expect("constructor checked declaration count")
    }
}

/// Immutable owning cache source; wire encoding remains the historical Program.
pub(crate) mod checked_key;
