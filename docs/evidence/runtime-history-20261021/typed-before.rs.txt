//! Explicit original receiver authority; old One/Many helpers remain independent.
use super::receiving::{CoreFailure, Receiver, Setup};
use crate::diagnostic::Diagnostic;
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "selected diagnostic stays inline after refusal"
)]
pub(in crate::workspace_graph) enum Failure {
    One(Diagnostic),
    Many(Vec<Diagnostic>),
    Setup(Setup),
}
impl Failure {
    pub(in crate::workspace_graph) fn one(diagnostic: Diagnostic) -> Self {
        Self::One(diagnostic)
    }
}
impl From<Vec<Diagnostic>> for Failure {
    fn from(report: Vec<Diagnostic>) -> Self {
        Self::Many(report)
    }
}
impl From<Setup> for Failure {
    fn from(failure: Setup) -> Self {
        Self::Setup(failure)
    }
}
impl From<Diagnostic> for Failure {
    fn from(primary: Diagnostic) -> Self {
        Self::One(primary)
    }
}
pub(in crate::workspace_graph) fn construct_transient_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::expected_projection::CoreProviderProofs<'a>,
    owner: &mut Receiver,
) -> Result<super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use super::super::expected_projection::{borrowed_imports, borrowed_leaf};
    use borrowed_leaf::Input;
    owner.entry::<Input<'a>>()?;
    if borrowed_leaf::eligible(program) {
        borrowed_leaf::construct(program)
            .map(Input::Borrowed)
            .map_err(Failure::from)
    } else if borrowed_imports::eligible(program, authored, programs) {
        borrowed_imports::typed::construct_using(
            program,
            authored,
            programs,
            registry,
            Some(core),
            owner,
        )
        .map(Input::Borrowed)
    } else {
        // Explicit legacy dependency: its Vec producers are not recertified.
        Input::construct_transient_core(program, authored, programs, registry, core)
            .map_err(Failure::from)
    }
}
/// A conservative negative prefilter, never provider or wrapper authority.
/// Reserved wrappers and ordinary function imports can trigger a full probe;
/// a positive still executes the unchanged full producer and classifier.
/// Both cache modes pay and perform this same allocation-free selection.
#[allow(clippy::too_many_arguments)]
pub(in crate::workspace_graph) fn construct_selected_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::expected_projection::CoreProviderProofs<'a>,
    allow_transient: bool,
    owner: &mut Receiver,
) -> Result<super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use crate::ast::ModuleUse;
    use std::mem::size_of;
    #[cfg(test)]
    if SCALAR_SIGNATURE_BEFORE.with(std::cell::Cell::get) {
        return before_scalar_dispatch::construct_selected_core(
            program,
            authored,
            programs,
            registry,
            core,
            allow_transient,
            owner,
        );
    }
    #[cfg(test)]
    if ORIGINAL_ALIAS_DISPATCH.with(std::cell::Cell::get) {
        return if allow_transient {
            before_scalar_dispatch::construct_transient_alias_core(
                program, authored, programs, registry, core, owner,
            )
        } else {
            construct_transient_core(program, authored, programs, registry, core, owner)
        };
    }
    // One real forwarding Result, in addition to the selected full producer.
    owner.entry::<super::super::expected_projection::borrowed_leaf::Input<'a>>()?;
    owner.reserve(
        2 * size_of::<&crate::ast::Program>()
            + 2 * size_of::<&std::collections::BTreeMap<&str, super::super::AuthoredDeclaration<'_>>>()
            + 2 * size_of::<&[crate::ast::Program]>()
            + 2 * size_of::<&super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>>()
            + 2 * size_of::<&mut super::super::expected_projection::CoreProviderProofs<'_>>()
            + 2 * size_of::<&mut Receiver>()
            + size_of::<[usize; 8]>() // len, multiplication operands/result and receiving arguments
            + size_of::<[bool; 5]>() // allow argument/formal, any return/receiver, dispatch
            + 2 * size_of::<&Vec<ModuleUse>>()
            + 2 * size_of::<std::slice::Iter<'_, ModuleUse>>()
            + 2 * size_of::<&mut std::slice::Iter<'_, ModuleUse>>()
            + size_of::<&mut ()>(), // mutable borrow of the zero-capture predicate
    )?;
    // Reserve all bounded scan roles before the first identity observation.
    // An early positive does not refund controls for later source entries.
    const PER_IMPORT_BYTES: usize = 2 * size_of::<Option<&ModuleUse>>()
        + 2 * size_of::<&ModuleUse>()
        + 2 * size_of::<&String>()
        + 12 * size_of::<&str>() // id return/receiver, predicate argument/formal, both prefix pairs
        + 6 * size_of::<bool>(); // prefix results, OR, helper return/receiver and predicate
    owner.reserve(program.module_uses.len().saturating_mul(
        PER_IMPORT_BYTES + 2 * size_of::<&crate::ast::ModuleUseKind>() + 2 * size_of::<bool>(),
    ))?;
    let possible = program.module_uses.iter().any(|import| {
        may_borrow_wrapper(import.persistent_id.as_str())
            || import.kind == crate::ast::ModuleUseKind::Function
    });
    if allow_transient && possible {
        construct_transient_alias_core(program, authored, programs, registry, core, owner)
    } else {
        construct_transient_core(program, authored, programs, registry, core, owner)
    }
}

/// A positive is only a reason to run the unchanged full classifier.
/// All current Vec/Box wrapper identities belong to these two families.
pub(in crate::workspace_graph) fn may_borrow_wrapper(id: &str) -> bool {
    id.starts_with("std.collections.vec.") || id.starts_with("std.mem.box.")
}

#[cfg(test)]
thread_local! {
    static ORIGINAL_ALIAS_DISPATCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Complete original producers and the exact previous two-way dispatch remain
/// compiled independently. This is a dispatch oracle, not an old whole engine.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_original_alias_dispatch<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL_ALIAS_DISPATCH.with(|old| old.set(self.0));
        }
    }
    let restore = Restore(ORIGINAL_ALIAS_DISPATCH.with(|old| old.replace(true)));
    let result = run();
    drop(restore);
    result
}

/// The caller admits this only for an uncached transient resolution. A cache
/// or invariant-rewriting route keeps the complete original constructor above.
pub(in crate::workspace_graph) fn construct_transient_alias_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::expected_projection::CoreProviderProofs<'a>,
    owner: &mut Receiver,
) -> Result<super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use super::super::expected_projection::{borrowed_imports, borrowed_leaf::Input};
    owner.entry::<Input<'a>>()?;
    // Before admission scans or forwarding arguments: the new boundary has
    // one real Result in addition to the selected old/new producer's Result.
    owner.reserve(
        std::mem::size_of::<&crate::ast::Program>()
            + std::mem::size_of::<
                &std::collections::BTreeMap<&str, super::super::AuthoredDeclaration<'_>>,
            >()
            + std::mem::size_of::<&[crate::ast::Program]>()
            + std::mem::size_of::<
                &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
            >()
            + std::mem::size_of::<&mut super::super::expected_projection::CoreProviderProofs<'_>>()
            + std::mem::size_of::<&mut Receiver>()
            + 2 * std::mem::size_of::<bool>(),
    )?;
    if !borrowed_imports::eligible(program, authored, programs) {
        return construct_transient_core(program, authored, programs, registry, core, owner);
    }
    // Invariant passes may require a writable whole Program. Preserve their
    // complete ordinary producer rather than materializing views later.
    owner.reserve(
        std::mem::size_of::<std::slice::Iter<'_, crate::ast::Program>>()
            + programs.len()
                * (std::mem::size_of::<Option<&crate::ast::Program>>()
                    + std::mem::size_of::<&crate::ast::Program>()),
    )?;
    for candidate in programs {
        // The backing length already fits an allocated TypeDeclaration Vec;
        // these smaller per-item controls cannot overflow that source bound.
        owner.reserve(
            std::mem::size_of::<std::slice::Iter<'_, crate::ast::TypeDeclaration>>()
                + candidate.types.len()
                    * (std::mem::size_of::<Option<&crate::ast::TypeDeclaration>>()
                        + 2 * std::mem::size_of::<&crate::ast::TypeDeclaration>()
                        + std::mem::size_of::<&[crate::ast::Expr]>()
                        + std::mem::size_of::<bool>()),
        )?;
        for declaration in &candidate.types {
            if !declaration.invariants().is_empty() {
                return construct_transient_core(
                    program, authored, programs, registry, core, owner,
                );
            }
        }
    }
    borrowed_imports::typed::construct_transient(
        program,
        authored,
        programs,
        registry,
        Some(core),
        owner,
    )
    .map(Input::Transient)
}
impl Receiver {
    /// Semantic selection precedes a secondary receiving/sticky association.
    pub(in crate::workspace_graph) fn receive_typed(self, failure: Failure) -> CoreFailure {
        match failure {
            Failure::One(primary) => self.receive(super::Failure::One(primary)),
            Failure::Many(report) => self.receive(super::Failure::Many(report)),
            Failure::Setup(refusal) => {
                drop(self);
                CoreFailure::Setup(refusal)
            }
        }
    }
}

#[cfg(test)]
#[path = "../function_alias/before_core.rs"]
pub(in crate::workspace_graph) mod before_function_alias_core;

#[cfg(test)]
#[path = "before_scalar_dispatch.rs"]
pub(in crate::workspace_graph) mod before_scalar_dispatch;

#[cfg(test)]
thread_local! {
    static SCALAR_SIGNATURE_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// The entire immediate-old actual selector and its original producer choices.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_original_scalar_dispatch<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCALAR_SIGNATURE_BEFORE.with(|value| value.set(self.0));
        }
    }
    let restore = Restore(SCALAR_SIGNATURE_BEFORE.with(|value| value.replace(true)));
    let result = run();
    drop(restore);
    result
}
