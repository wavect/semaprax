//! Query-local callable facts shared by one agent-context request (REF-09).
//!
//! One `AgentCallables` view is built per query from the retained, already
//! validated `ResolvedProgram`. It borrows the program's declarations, is
//! dropped with the query, and is never cached across queries or programs.
//! It replaces the program-wide membership set that each fact renderer used
//! to rebuild, so rendering R reached functions no longer scans all F
//! callable declarations R times.
//!
//! Edge rules are unchanged: the legacy `calls` field of a monomorphic
//! function fact is its authored callee set restricted to declared functions
//! and function templates; a template fact keeps its unrestricted authored
//! callee set. These are the legacy visitor's edges and are deliberately not
//! substituted by `PersistentCallIndex`, whose v2 edges also carry
//! conservative indirect and closure-body dependencies.

use super::work_counter::{record, Work};
use super::*;

pub(super) struct AgentCallables<'a> {
    pub(super) functions: BTreeMap<DeclarationId, &'a ResolvedFunction>,
    pub(super) templates: BTreeMap<DeclarationId, &'a crate::hir::ResolvedFunctionTemplate>,
}

impl<'a> AgentCallables<'a> {
    pub(super) fn new(program: &'a ResolvedProgram) -> Self {
        record(Work::CallableMembershipBuild, 1);
        record(
            Work::CallableMembershipEntry,
            program.functions.len() + program.function_templates.len(),
        );
        Self {
            functions: program
                .functions
                .iter()
                .map(|function| (function.id.clone(), function))
                .collect(),
            templates: program
                .function_templates
                .iter()
                .map(|template| (template.id.clone(), template))
                .collect(),
        }
    }

    pub(super) fn contains(&self, id: &DeclarationId) -> bool {
        self.functions.contains_key(id) || self.templates.contains_key(id)
    }

    /// The legacy `calls` field of a monomorphic function fact, derived from
    /// the function's authored call set computed once by the caller.
    pub(super) fn restrict(&self, calls: &BTreeSet<DeclarationId>) -> BTreeSet<DeclarationId> {
        calls
            .iter()
            .filter(|callee| self.contains(callee))
            .cloned()
            .collect()
    }

    /// Compute the legacy `calls` field of a monomorphic function fact.
    pub(super) fn function_calls(&self, function: &ResolvedFunction) -> BTreeSet<DeclarationId> {
        self.restrict(&function_calls(function))
    }
}
