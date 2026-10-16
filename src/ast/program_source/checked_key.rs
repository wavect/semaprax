//! Compiler-owned key state. Pending ownership cannot be read as a source.
//! Publication follows transfer of the actual current parsed program.
use super::generated_roots::GeneratedRoots;
use super::{DeclSlices, OverlayIdentity, ProgramSource, Subject};
use crate::ast::{
    AgentDeclaration, Function, InterfaceDeclaration, ModuleUse, Program, ProtocolDeclaration,
    ProtocolImplementation, SessionProtocolDeclaration, TypeDeclaration,
};
use crate::workspace_graph::current_source::CurrentParsedSource;
use std::sync::{Arc, OnceLock};

pub(crate) enum CheckedSource {
    Owned(Program),
    Deferred(DeferredSource),
}
pub(crate) struct DeferredSource {
    generated: GeneratedRoots,
    source_slot: usize,
    path_allocation: usize,
    module_allocation: usize,
    owner: OnceLock<Arc<CurrentParsedSource>>,
    identity: OverlayIdentity,
}
pub(crate) enum ReadySource<'a> {
    Owned(&'a Program),
    Shared {
        authored: &'a Program,
        key: &'a DeferredSource,
    },
}
impl CheckedSource {
    /// The caller has acquired the exact replacement backing before passing
    /// these directories. The private workspace slot survives the final move.
    pub(crate) fn deferred(
        program: &Program,
        generated: GeneratedRoots,
        source_slot: usize,
    ) -> Self {
        Self::Deferred(DeferredSource {
            generated,
            source_slot,
            path_allocation: program.path.as_ptr() as usize,
            module_allocation: program.module.as_ptr() as usize,
            owner: OnceLock::new(),
            identity: OverlayIdentity { _nonzero_size: 0 },
        })
    }
    pub(crate) fn ready(&self) -> Option<ReadySource<'_>> {
        match self {
            Self::Owned(program) => Some(ReadySource::Owned(program)),
            Self::Deferred(key) => Some(ReadySource::Shared {
                authored: key.owner.get()?.program(),
                key,
            }),
        }
    }
    pub(crate) fn owned(&self) -> Option<&Program> {
        match self {
            Self::Owned(program) => Some(program),
            Self::Deferred(_) => None,
        }
    }
    pub(crate) fn needs_current_owner(&self) -> bool {
        matches!(self, Self::Deferred(key) if key.owner.get().is_none())
    }
    /// Only the actual current restored parsed owner can complete this key.
    /// Equal previous source bytes never authorize a different AST allocation.
    pub(crate) fn can_bind(&self, source_slot: usize, owner: &CurrentParsedSource) -> bool {
        match self {
            Self::Owned(_) => true,
            Self::Deferred(key) if key.owner.get().is_some() => false,
            Self::Deferred(key) => {
                let program = owner.program();
                key.source_slot == source_slot
                    && key.path_allocation == program.path.as_ptr() as usize
                    && key.module_allocation == program.module.as_ptr() as usize
                    && match owner.original_ordinals() {
                        Some(positions) => DeclSlices::selected(
                            &program.functions,
                            positions,
                            &key.generated.functions,
                        )
                        .is_some(),
                        None => program
                            .functions
                            .len()
                            .checked_add(key.generated.functions.len())
                            .is_some(),
                    }
                    && program
                        .types
                        .len()
                        .checked_add(key.generated.types.len())
                        .is_some()
            }
        }
    }
    pub(crate) fn bind(
        &self,
        source_slot: usize,
        owner: &Arc<CurrentParsedSource>,
    ) -> Result<(), ()> {
        match self {
            Self::Owned(_) => Ok(()),
            Self::Deferred(key) if self.can_bind(source_slot, owner) => {
                key.owner.set(Arc::clone(owner)).map_err(|_| ())
            }
            Self::Deferred(_) => Err(()),
        }
    }
    #[cfg(test)]
    pub(crate) fn materialize(&self) -> Program {
        let ready = self.ready().expect("only completed keys are observed");
        Program {
            path: ready.path().to_owned(),
            module: ready.module().to_owned(),
            module_uses: ready.module_uses().to_vec(),
            permits: ready.permits().to_vec(),
            types: ready.types().iter().cloned().collect(),
            interfaces: ready.interfaces().to_vec(),
            protocols: ready.protocols().to_vec(),
            implementations: ready.implementations().to_vec(),
            session_protocols: ready.session_protocols().to_vec(),
            agents: ready.agents().to_vec(),
            functions: ready.functions().iter().cloned().collect(),
        }
    }
}
impl ProgramSource for ReadySource<'_> {
    fn subject(&self) -> Subject<'_> {
        match self {
            Self::Owned(p) => p.subject(),
            Self::Shared { key, .. } => Subject::Projected(&key.identity),
        }
    }
    fn path(&self) -> &str {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.path,
        }
    }
    fn module(&self) -> &str {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.module,
        }
    }
    fn module_uses(&self) -> &[ModuleUse] {
        match self {
            Self::Owned(p) => &p.module_uses,
            Self::Shared { .. } => &[],
        }
    }
    fn permits(&self) -> &[String] {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.permits,
        }
    }
    fn types(&self) -> DeclSlices<'_, TypeDeclaration> {
        match self {
            Self::Owned(p) => p.types(),
            Self::Shared { authored, key } => {
                DeclSlices::append(&authored.types, &key.generated.types)
                    .expect("completion checked type length")
            }
        }
    }
    fn interfaces(&self) -> &[InterfaceDeclaration] {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.interfaces,
        }
    }
    fn protocols(&self) -> &[ProtocolDeclaration] {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.protocols,
        }
    }
    fn implementations(&self) -> &[ProtocolImplementation] {
        match self {
            Self::Owned(p) => &p.implementations,
            Self::Shared { .. } => &[],
        }
    }
    fn session_protocols(&self) -> &[SessionProtocolDeclaration] {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.session_protocols,
        }
    }
    fn agents(&self) -> &[AgentDeclaration] {
        match self {
            Self::Owned(p) | Self::Shared { authored: p, .. } => &p.agents,
        }
    }
    fn functions(&self) -> DeclSlices<'_, Function> {
        match self {
            Self::Owned(p) => p.functions(),
            Self::Shared { authored, key } => {
                match key.owner.get().expect("completed key").original_ordinals() {
                    Some(positions) => DeclSlices::selected(
                        &authored.functions,
                        positions,
                        &key.generated.functions,
                    )
                    .expect("completion checked ordered selection"),
                    None => DeclSlices::append(&authored.functions, &key.generated.functions)
                        .expect("completion checked function lengths"),
                }
            }
        }
    }
    fn function_refs(&self) -> super::FunctionSlices<'_> {
        match self {
            Self::Owned(p) => p.function_refs(),
            Self::Shared { authored, key } => {
                match key.owner.get().expect("completed key").original_ordinals() {
                    Some(positions) => super::FunctionSlices::selected(
                        &authored.functions,
                        positions,
                        &key.generated.functions,
                    )
                    .expect("completion checked ordered selection"),
                    None => {
                        super::FunctionSlices::append(&authored.functions, &key.generated.functions)
                            .expect("completion checked function lengths")
                    }
                }
            }
        }
    }
}
mod encode;
