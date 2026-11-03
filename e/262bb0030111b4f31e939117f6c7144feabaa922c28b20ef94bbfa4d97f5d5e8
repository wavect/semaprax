//! Lexical missing-declaration recovery and separate annotation refusal views.
//!
//! Names borrow immutable authored source. This directory never supplies a type,
//! binding, capture, ownership state, or backend admission. Ordinary source
//! verifier storage has no physical payer; these error-only allocations remain
//! ordinary storage, not paid replay authority.

use std::collections::BTreeMap;

use crate::ast::{Expr, ExprKind, Function, Type};
use crate::diagnostic::Diagnostic;

#[derive(Default)]
struct Scope<'p> {
    parent: Option<usize>,
    names: BTreeMap<&'p str, (usize, bool, Option<&'p Type>)>,
}

/// A declaration refusal explains an expected-type failure; it never replaces
/// the independently checked value or any binding/ownership metadata.
#[derive(Clone, Copy)]
pub(super) struct RefusalView<'p> {
    pub(super) origin: usize,
    pub(super) declared: &'p Type,
}

#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
    pub(super) diagnostics: usize,
    reads: usize,
}

#[derive(Default)]
pub(super) struct Recovery<'p> {
    unknown_declarations: bool,
    scopes: Vec<Scope<'p>>,
    last_read: Option<usize>,
    reads: usize,
    // Opaque borrowed expression identities are invocation-local lookup keys.
    // They are never dereferenced, iterated to select diagnostics, or emitted.
    // Only reads of real annotation-refused bindings acquire entries.
    expressions: BTreeMap<*const Expr, RefusalView<'p>>,
    #[cfg(test)]
    pub(super) visited: usize,
}

impl<'p> Recovery<'p> {
    /// Recheck each affected declaration once in this invocation, only after an
    /// existing unknown-type refusal. Equal byte spans alone are insufficient
    /// for externally constructed source trees or duplicate parameters.
    pub(super) fn signature(
        program: &'p dyn crate::ast::program_source::ProgramSource,
        current: &'p Function,
        variables: &std::collections::HashMap<String, super::binding::Binding>,
        types: &super::type_table::TypeTable<'_>,
        diagnostics: &[Diagnostic],
    ) -> Self {
        let mut recovery = Self::default();
        if !diagnostics
            .iter()
            .any(callee_components::unknown_component_receipt)
        {
            return recovery;
        }
        recovery.unknown_declarations = true;
        let mut parameters = None;
        let mut scratch = Vec::new();
        for parameter in &current.params {
            if types.needs_drop(&parameter.ty)
                || variables.get(&parameter.name).is_none_or(|binding| {
                    binding.ty != parameter.ty || binding.mode != parameter.mode
                })
                || !diagnostics.iter().any(|diagnostic| {
                    callee_components::unknown_component_receipt(diagnostic)
                        && diagnostic.span == Some(parameter.span)
                })
            {
                continue;
            }
            let parameters = parameters.get_or_insert_with(|| {
                current
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect()
            });
            scratch.clear();
            super::declared_type::check_declared_type(
                program,
                &parameter.ty,
                parameter.span,
                types,
                parameters,
                &mut scratch,
            );
            if let Some(origin) = scratch
                .iter()
                .filter(|refusal| callee_components::unknown_component_receipt(refusal))
                .find_map(|refusal| {
                    diagnostics.iter().position(|original| {
                        original.code == refusal.code
                            && original.severity == refusal.severity
                            && original.message == refusal.message
                            && original.path == refusal.path
                            && original.span == refusal.span
                            && original.help == refusal.help
                    })
                })
            {
                recovery.ensure(0);
                recovery.scopes[0]
                    .names
                    .insert(parameter.name.as_str(), (origin, true, None));
            }
        }
        recovery
    }

    // Begin exact producer-origin parameter receiver.
    pub(super) fn signature_origin(
        program: &'p dyn crate::ast::program_source::ProgramSource,
        current: &'p Function,
        variables: &std::collections::HashMap<String, super::binding::Binding>,
        types: &super::type_table::TypeTable<'_>,
        diagnostics: &[Diagnostic],
        origin: &super::declaration::SignatureOrigin<'p>,
        result_type: Option<&Type>,
    ) -> Option<Self> {
        let template = origin.template_for(program, current)?;
        let mut recovery = Self::default();
        if !diagnostics
            .iter()
            .any(callee_components::unknown_component_receipt)
        {
            return Some(recovery);
        }
        recovery.unknown_declarations = true;
        let mut parameters = None;
        let mut scratch = Vec::new();
        for (parameter, actual) in template.params.iter().zip(&current.params) {
            if types.needs_drop(&actual.ty)
                || variables
                    .get(&parameter.name)
                    .is_none_or(|binding| binding.ty != actual.ty || binding.mode != actual.mode)
                || !diagnostics.iter().any(|diagnostic| {
                    callee_components::unknown_component_receipt(diagnostic)
                        && diagnostic.span == Some(parameter.span)
                })
            {
                continue;
            }
            let parameters = parameters.get_or_insert_with(|| {
                template
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect()
            });
            scratch.clear();
            super::declared_type::check_declared_type(
                program,
                &parameter.ty,
                parameter.span,
                types,
                parameters,
                &mut scratch,
            );
            if let Some(origin) = scratch
                .iter()
                .filter(|refusal| callee_components::unknown_component_receipt(refusal))
                .find_map(|refusal| {
                    diagnostics.iter().position(|original| {
                        original.code == refusal.code
                            && original.severity == refusal.severity
                            && original.message == refusal.message
                            && original.path == refusal.path
                            && original.span == refusal.span
                            && original.help == refusal.help
                    })
                })
            {
                recovery.ensure(0);
                recovery.scopes[0]
                    .names
                    .insert(parameter.name.as_str(), (origin, true, None));
            }
        }
        // The reserved result name has its own actual postcondition carrier;
        // it never becomes available in a requires clause or ordinary body.
        if let Some(index) = result_type
            .filter(|ty| std::ptr::eq(*ty, &current.return_type))
            .and_then(|_| origin.return_refusal(program, current, types, diagnostics))
        {
            recovery.ensure(0);
            recovery.scopes[0]
                .names
                .insert("result", (index, true, None));
        }
        Some(recovery)
    }
    // End exact producer-origin parameter receiver.

    pub(super) fn clear(&mut self, scope: usize, name: &str) {
        if let Some(row) = self.scopes.get_mut(scope) {
            row.names.remove(name);
        }
    }

    /// Scope ordinals are reused after a branch or block ends. Reset the row
    /// before entering that ordinal; a former sibling must never be an ancestor.
    /// Before the first refused declaration there are no inherited facts, so
    /// successful programs do not acquire a directory merely to fork scopes.
    pub(super) fn fork(&mut self, parent: Option<usize>, child: usize) {
        if self.scopes.is_empty() {
            return;
        }
        self.ensure(child);
        self.scopes[child] = Scope {
            parent,
            names: BTreeMap::new(),
        };
    }

    fn ensure(&mut self, scope: usize) {
        if self.scopes.len() <= scope {
            self.scopes.resize_with(scope + 1, Scope::default);
        }
    }

    /// Only an absent checked binding/function reaches this lookup. Existing
    /// availability, loan, move, and function-value diagnostics run first.
    pub(super) fn reads_refused(&mut self, scope: usize, name: &str) -> bool {
        self.read(scope, name, false).is_some()
    }

    #[cfg(test)]
    pub(super) fn reads_known_refused(&mut self, scope: usize, name: &str) -> bool {
        self.read(scope, name, true).is_some()
    }

    /// Keep the full actual value for annotation refusals. Unknown signature
    /// types keep their existing narrower inference behavior in this batch.
    pub(super) fn reads_value(&mut self, scope: usize, name: &str, expression: &Expr) -> bool {
        match self.read(scope, name, true) {
            Some((origin, Some(declared))) => {
                self.expressions.insert(
                    std::ptr::from_ref(expression),
                    RefusalView { origin, declared },
                );
                false
            }
            Some((_, None)) => true,
            None => false,
        }
    }

    pub(super) fn begin(&mut self, expression: &Expr) {
        self.expressions.remove(&std::ptr::from_ref(expression));
    }

    /// Blocks return their checked tail. Follow only that transparent chain,
    /// never statements or sibling arms, and never rescan a whole function.
    pub(super) fn view(&self, mut expression: &Expr) -> Option<RefusalView<'p>> {
        loop {
            if let Some(view) = self.expressions.get(&std::ptr::from_ref(expression)) {
                return Some(*view);
            }
            let ExprKind::Block { tail, .. } = &expression.kind else {
                return None;
            };
            expression = tail;
        }
    }

    pub(super) fn explains(&self, expression: &Expr, expected: &Type) -> bool {
        self.view(expression)
            .is_some_and(|view| view.declared == expected)
    }

    pub(super) fn explains_acceptance(
        &self,
        expression: &Expr,
        accepts: impl FnOnce(&Type) -> bool,
    ) -> bool {
        self.view(expression)
            .is_some_and(|view| accepts(view.declared))
    }

    pub(super) fn inherit_binding(&mut self, scope: usize, name: &'p str, value: &Expr) {
        if let Some(view) = self.view(value) {
            self.refused_annotation(scope, name, view.declared, view.origin);
        }
    }

    fn read(
        &mut self,
        mut scope: usize,
        name: &str,
        known: bool,
    ) -> Option<(usize, Option<&'p Type>)> {
        while let Some(row) = self.scopes.get(scope) {
            #[cfg(test)]
            {
                self.visited += 1;
            }
            if let Some(&(origin, declaration_known, declared)) = row.names.get(name) {
                // A missing declaration never outranks a real function value
                // or checked binding. Known markers are inference-only.
                if declaration_known != known {
                    return None;
                }
                self.last_read = Some(origin);
                self.reads += 1;
                return Some((origin, declared));
            }
            let Some(parent) = row.parent else {
                break;
            };
            scope = parent;
        }
        None
    }

    pub(super) fn checkpoint(&self, diagnostics: &[Diagnostic]) -> Checkpoint {
        Checkpoint {
            diagnostics: diagnostics.len(),
            reads: self.reads,
        }
    }

    /// Publish only after the complete initializer. A continuation captures the
    /// diagnostic/read frontier before evaluation, including nested blocks;
    /// source spans or old unrelated diagnostics cannot manufacture this fact.
    pub(super) fn refused_let(
        &mut self,
        scope: usize,
        name: &'p str,
        checkpoint: Checkpoint,
        diagnostics: &[Diagnostic],
    ) {
        self.publish(scope, name, checkpoint, diagnostics, false);
    }

    pub(super) fn refused_annotation(
        &mut self,
        scope: usize,
        name: &'p str,
        declared: &'p Type,
        origin: usize,
    ) {
        self.ensure(scope);
        self.scopes[scope]
            .names
            .insert(name, (origin, true, Some(declared)));
    }

    /// Resolve one refused child before any later continuation can append errors.
    pub(super) fn refusal_origin(&self, checkpoint: Checkpoint, diagnostics: &[Diagnostic]) -> Option<usize> {
        diagnostics.iter().enumerate().skip(checkpoint.diagnostics)
            .find(|(_, diagnostic)| diagnostic.severity.is_error())
            .map(|(index, _)| index)
            .or_else(|| (self.reads > checkpoint.reads).then_some(self.last_read).flatten())
    }

    pub(super) fn refused_origin(&mut self, scope: usize, name: &'p str, origin: usize) {
        self.ensure(scope);
        self.scopes[scope].names.insert(name, (origin, false, None));
    }

    fn publish(
        &mut self,
        scope: usize,
        name: &'p str,
        checkpoint: Checkpoint,
        diagnostics: &[Diagnostic],
        known: bool,
    ) {
        let origin = diagnostics
            .iter()
            .enumerate()
            .skip(checkpoint.diagnostics)
            .find(|(_, diagnostic)| diagnostic.severity.is_error())
            .map(|(index, _)| index)
            .or_else(|| {
                (self.reads > checkpoint.reads)
                    .then_some(self.last_read)
                    .flatten()
            });
        if let Some(origin) = origin {
            self.ensure(scope);
            self.scopes[scope].names.insert(name, (origin, known, None));
        }
    }
}

mod callee_components;

#[cfg(test)]
mod before_parameter_signature;
