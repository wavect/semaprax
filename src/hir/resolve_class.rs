//! Class inheritance lowering plus the resolver diagnostic helper.
//!
//! Method lookup along an inheritance chain, implicit prefix upcast
//! admission, and expression ownership classification.

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

use super::ids::DeclarationId;
use super::nodes::{DeclarationKind, OwnershipMode, ResolvedType};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn error(
        &self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
    ) -> Diagnostic {
        Diagnostic::error(code, message, span).at_path(&self.program.path)
    }

    /// Class Inheritance v1: finds the nearest ancestor of `start` (inclusive)
    /// declaring a method named `method`, returning the declaring class and
    /// the method's stable identity.
    pub(super) fn resolve_method_in_chain(
        &self,
        start: &DeclarationId,
        method: &str,
        span: Span,
    ) -> Result<(DeclarationId, DeclarationId), Diagnostic> {
        let mut chain = vec![start.clone()];
        chain.extend(self.declarations.class_ancestors(start));
        for class in &chain {
            if let Some(declaration) = self
                .declarations
                .declarations()
                .find(|decl| {
                    decl.kind == DeclarationKind::Function
                        && decl.name == method
                        && decl.owner.as_ref() == Some(class)
                })
                .map(|decl| decl.id.clone())
            {
                return Ok((class.clone(), declaration));
            }
        }
        Err(self.error(
            "SPX-H001",
            format!("unresolved method `{method}` on class `{start}`"),
            span,
        ))
    }

    /// Class Inheritance v1: consumes `receiver` (a whole value of some
    /// descendant of `holder`) as a `holder`-typed value. Exact-type receivers
    /// pass through unchanged; descendants are consumed through the same
    /// prefix-upcast block a declared-type binding uses, which requires the
    /// child-declared suffix to introduce no cleanup leaves.
    /// Class Inheritance v1: admits an implicit upcast from class `child` to
    /// its ancestor `parent`. The prefix must be exactly the ancestor's
    /// effective layout, and the child-declared suffix must be cleanup-inert:
    /// consuming the child transfers its inherited leaves into the
    /// ancestor-typed result, so owned suffix state would otherwise leak.
    pub(super) fn check_upcast_admissible(
        &self,
        child: &DeclarationId,
        parent: &DeclarationId,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !self.declarations.class_extends(child, parent) {
            return Err(self.error(
                "SPX-T232",
                format!("`{child}` does not inherit from `{parent}`"),
                span,
            ));
        }
        let child_fields = self.declarations.record_fields(child).ok_or_else(|| {
            self.error("SPX-H006", format!("class `{child}` has no fields"), span)
        })?;
        let parent_fields = self.declarations.record_fields(parent).ok_or_else(|| {
            self.error("SPX-H006", format!("class `{parent}` has no fields"), span)
        })?;
        if child_fields.len() < parent_fields.len()
            || child_fields[..parent_fields.len()]
                .iter()
                .zip(parent_fields.iter())
                .any(|(child_field, parent_field)| child_field.id != parent_field.id)
        {
            return Err(self.error(
                "SPX-H006",
                format!("class `{child}` prefix disagrees with ancestor `{parent}`"),
                span,
            ));
        }
        for field in &child_fields[parent_fields.len()..] {
            let drops = self
                .declarations
                .type_facts(&field.ty)
                .is_some_and(|facts| facts.needs_drop);
            if drops {
                return Err(self.error(
                    "SPX-T233",
                    format!(
                        "upcast from `{child}` to `{parent}` would discard owned field `{}`; only cleanup-inert child fields are admitted in this slice",
                        field.name
                    ),
                    span,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn expression_ownership(
        &self,
        ty: &ResolvedType,
        non_copy_mode: OwnershipMode,
        span: Span,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy_mode
                }
            })
            .ok_or_else(|| {
                self.error(
                    "SPX-H004",
                    format!(
                        "semantic facts are unavailable for type `{}`",
                        ty.identity_key()
                    ),
                    span,
                )
            })
    }
}
