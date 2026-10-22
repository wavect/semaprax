#![allow(dead_code, reason = "Whole immediate-old selector, hooks and invariant forwarding are an independent cfg-test oracle.")]
use super::*;
/// A conservative negative prefilter, never provider or wrapper authority.
/// Only these reserved identity families can reach FunctionEntry::Borrowed;
/// a positive still executes the unchanged full producer and classifier.
/// Both cache modes pay and perform this same allocation-free selection.
#[allow(clippy::too_many_arguments)]
pub(in crate::workspace_graph) fn construct_selected_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::super::expected_projection::CoreProviderProofs<'a>,
    allow_transient: bool,
    owner: &mut Receiver,
) -> Result<super::super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use crate::ast::ModuleUse;
    use std::mem::size_of;
    #[cfg(test)]
    if ORIGINAL_ALIAS_DISPATCH.with(std::cell::Cell::get) {
        return if allow_transient {
            construct_transient_alias_core(program, authored, programs, registry, core, owner)
        } else {
            construct_transient_core(program, authored, programs, registry, core, owner)
        };
    }
    // One real forwarding Result, in addition to the selected full producer.
    owner.entry::<super::super::super::expected_projection::borrowed_leaf::Input<'a>>()?;
    owner.reserve(
        2 * size_of::<&crate::ast::Program>()
            + 2 * size_of::<&std::collections::BTreeMap<&str, super::super::super::AuthoredDeclaration<'_>>>()
            + 2 * size_of::<&[crate::ast::Program]>()
            + 2 * size_of::<&super::super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>>()
            + 2 * size_of::<&mut super::super::super::expected_projection::CoreProviderProofs<'_>>()
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
    owner.reserve(program.module_uses.len().saturating_mul(PER_IMPORT_BYTES))?;
    let possible = program
        .module_uses
        .iter()
        .any(|import| may_borrow_wrapper(import.persistent_id.as_str()));
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
    authored: &'a std::collections::BTreeMap<&'a str, super::super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::super::expected_projection::CoreProviderProofs<'a>,
    owner: &mut Receiver,
) -> Result<super::super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use super::super::super::expected_projection::{borrowed_imports, borrowed_leaf::Input};
    owner.entry::<Input<'a>>()?;
    // Before admission scans or forwarding arguments: the new boundary has
    // one real Result in addition to the selected old/new producer's Result.
    owner.reserve(
        std::mem::size_of::<&crate::ast::Program>()
            + std::mem::size_of::<
                &std::collections::BTreeMap<&str, super::super::super::AuthoredDeclaration<'_>>,
            >()
            + std::mem::size_of::<&[crate::ast::Program]>()
            + std::mem::size_of::<
                &super::super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
            >()
            + std::mem::size_of::<&mut super::super::super::expected_projection::CoreProviderProofs<'_>>()
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
    borrowed_imports::typed::before_scalar_signature::construct_transient(
        program,
        authored,
        programs,
        registry,
        Some(core),
        owner,
    )
    .map(Input::Transient)
}
