//! Exact transient type-site views; generated paths remain independently owned.

use std::borrow::Cow;
use std::cmp::Ordering;

use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{expected_projection::checked_builder_sum, reserve_builder_structure};

#[derive(Debug)]
pub(super) struct TypeSite<'a> {
    owner: Cow<'a, str>,
    expression: Option<Cow<'a, str>>,
    path: String,
    target: &'a str,
}

impl<'a> TypeSite<'a> {
    pub(super) fn push(
        out: &mut Vec<Self>,
        owner: &'a str,
        expression: Option<&'a str>,
        path: &str,
        target: &'a str,
    ) -> Result<(), Vec<Diagnostic>> {
        let bytes = checked_builder_sum(std::mem::size_of::<Self>(), path.len())?;
        reserve_builder_structure(bytes)?;
        out.push(Self {
            owner: Cow::Borrowed(owner),
            expression: expression.map(Cow::Borrowed),
            path: path.to_owned(),
            target,
        });
        Ok(())
    }

    // A generated closure owner must be copied before its temporary identity dies.
    #[cfg(test)]
    fn push_owned_owner(
        out: &mut Vec<Self>,
        owner: &str,
        expression: Option<&'a str>,
        path: &str,
        target: &'a str,
    ) -> Result<(), Vec<Diagnostic>> {
        let bytes = checked_builder_sum(std::mem::size_of::<Self>(), path.len())?;
        let bytes = checked_builder_sum(bytes, owner.len())?;
        reserve_builder_structure(bytes)?;
        out.push(Self {
            owner: Cow::Owned(owner.to_owned()),
            expression: expression.map(Cow::Borrowed),
            path: path.to_owned(),
            target,
        });
        Ok(())
    }

    fn push_owned_expression(
        out: &mut Vec<Self>,
        owner: &'a str,
        expression: &str,
        path: &str,
        target: &'a str,
    ) -> Result<(), Vec<Diagnostic>> {
        let bytes = checked_builder_sum(std::mem::size_of::<Self>(), path.len())?;
        let bytes = checked_builder_sum(bytes, expression.len())?;
        reserve_builder_structure(bytes)?;
        out.push(Self {
            owner: Cow::Borrowed(owner),
            expression: Some(Cow::Owned(expression.to_owned())),
            path: path.to_owned(),
            target,
        });
        Ok(())
    }

    pub(super) fn projection(&self) -> (&str, &str, &str, &str) {
        (
            self.owner.as_ref(),
            self.expression.as_deref().unwrap_or(&self.path),
            &self.path,
            self.target,
        )
    }
}

/// A fused view's authored type argument belongs to its inner call, not the
/// retained outer HIR node. Copy that temporary identity only into actual sites.
pub(super) fn collect_reconstructed_sites<'a>(
    owner: &'a crate::hir::DeclarationId,
    ty: &'a crate::hir::ResolvedType,
    path: &str,
    expression: &str,
    imported: &std::collections::BTreeSet<&str>,
    out: &mut Vec<TypeSite<'a>>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    if imported.contains(declaration.as_str()) {
        TypeSite::push_owned_expression(
            out,
            owner.as_str(),
            expression,
            path,
            declaration.as_str(),
        )?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        collect_reconstructed_sites(
            owner,
            argument,
            &crate::bounded_output::budgeted_format(format_args!("{path}.argument.{index}")),
            expression,
            imported,
            out,
        )?;
    }
    Ok(())
}

impl PartialEq for TypeSite<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.projection() == other.projection()
    }
}
impl Eq for TypeSite<'_> {}
impl PartialOrd for TypeSite<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TypeSite<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.projection().cmp(&other.projection())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::TypeSite;
    use crate::{bounded_output, hir};

    #[test]
    fn borrowed_type_sites_preserve_all_fields_and_nested_occurrences() {
        let owner = hir::DeclarationId::new("app.main");
        let expression = hir::workspace_expression_identity(&owner, "body.tail");
        let nominal = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("lib.item"),
            arguments: vec![hir::ResolvedType::Nominal {
                declaration: hir::DeclarationId::new("lib.item"),
                arguments: Vec::new(),
            }],
        };
        let mut sites = Vec::new();
        super::super::collect_resolved_type_sites(
            owner.as_str(),
            &nominal,
            "body.tail.type",
            Some(&expression),
            &BTreeSet::from(["lib.item"]),
            &mut sites,
        )
        .unwrap();
        assert_eq!(sites.len(), 2);
        sites.sort();
        assert_eq!(
            sites.iter().map(TypeSite::projection).collect::<Vec<_>>(),
            vec![
                (
                    "app.main",
                    expression.as_str(),
                    "body.tail.type",
                    "lib.item"
                ),
                (
                    "app.main",
                    expression.as_str(),
                    "body.tail.type.argument.0",
                    "lib.item"
                ),
            ]
        );
        let hir::ResolvedType::Nominal { declaration, .. } = &nominal else {
            unreachable!()
        };
        assert!(std::ptr::eq(
            sites[0].projection().0.as_ptr(),
            owner.as_str().as_ptr()
        ));
        assert!(std::ptr::eq(
            sites[0].projection().1.as_ptr(),
            expression.as_ptr()
        ));
        assert!(std::ptr::eq(
            sites[0].projection().3.as_ptr(),
            declaration.as_str().as_ptr()
        ));
        TypeSite::push(
            &mut sites,
            owner.as_str(),
            Some(&expression),
            "body.tail.type",
            declaration.as_str(),
        )
        .unwrap();
        sites.sort();
        assert_eq!(sites.len(), 3);
        assert_eq!(
            sites[0].projection(),
            sites[1].projection(),
            "duplicate sites retain their multiplicity"
        );
    }

    #[test]
    fn closure_type_sites_keep_the_authoritative_owner_and_copy_temporary_owners_explicitly() {
        let source = "module test.type_site_closure;\n@id(\"app.main\") fn main()->i64 { let callback = fn(value:i64)->i64 { value }; callback(1) }\n";
        let parsed = crate::parse(source, Path::new("type-site-closure.spx")).unwrap();
        let mut program = hir::resolve(&parsed).unwrap();
        let main = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("main block")
        };
        let closure = statements[0].value_mut();
        let hir::ResolvedExprKind::Closure { parameters, .. } = &mut closure.kind else {
            panic!("closure binding")
        };
        // Reconstruction-only hostile HIR: this does not admit nominal closure parameters.
        parameters[0].ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("lib.item"),
            arguments: Vec::new(),
        };
        let mut sites = Vec::new();
        super::super::collect_resolved_expression_type_sites(
            &main.id,
            closure,
            "body.s0.value",
            &BTreeSet::from(["lib.item"]),
            &mut sites,
        )
        .unwrap();
        assert_eq!(sites.len(), 1);
        let path = "body.s0.value.closure.param.0";
        assert_eq!(
            sites[0].projection(),
            ("app.main", closure.id.as_str(), path, "lib.item")
        );
        let expected_owner = hir::closure::closure_id(&closure.id).as_str().to_owned();
        assert_ne!(sites[0].projection().0, expected_owner);

        let bytes = std::mem::size_of::<TypeSite<'_>>() + path.len() + expected_owner.len();
        let mut owned = Vec::new();
        {
            let temporary_owner = hir::closure::closure_id(&closure.id);
            let (result, overflow, consumed) = bounded_output::with_limit_usage(bytes, || {
                TypeSite::push_owned_owner(
                    &mut owned,
                    temporary_owner.as_str(),
                    Some(closure.id.as_str()),
                    path,
                    "lib.item",
                )
            });
            result.unwrap();
            assert!(!overflow);
            assert_eq!(consumed, bytes);
        }
        assert_eq!(
            owned[0].projection(),
            (
                expected_owner.as_str(),
                closure.id.as_str(),
                path,
                "lib.item"
            )
        );
        let temporary_owner = hir::closure::closure_id(&closure.id);
        let mut refused = Vec::new();
        let (error, overflow, consumed) = bounded_output::with_limit_usage(bytes - 1, || {
            TypeSite::push_owned_owner(
                &mut refused,
                temporary_owner.as_str(),
                Some(closure.id.as_str()),
                path,
                "lib.item",
            )
        });
        assert_eq!(error.unwrap_err()[0].code, "SPX-G171");
        assert!(overflow);
        assert_eq!(consumed, refusal_diagnostic_bytes(bytes - 1));
        assert!(refused.is_empty());
    }

    #[test]
    fn borrowed_type_site_paths_are_charged_once_and_refuse_before_allocation() {
        let mut sites = Vec::new();
        let path = "function.app.main.param.0";
        let bytes = std::mem::size_of::<TypeSite<'_>>() + path.len();
        let (result, overflow, consumed) = bounded_output::with_limit_usage(bytes, || {
            TypeSite::push(&mut sites, "app.main", None, path, "lib.item")
        });
        result.unwrap();
        assert!(!overflow);
        assert_eq!(consumed, bytes);
        let projected = sites[0].projection();
        assert_eq!(projected, ("app.main", path, path, "lib.item"));
        assert!(
            std::ptr::eq(projected.1.as_ptr(), projected.2.as_ptr()),
            "the signature fallback uses the same owned path"
        );
        let mut refused = Vec::new();
        let (error, overflow, consumed) = bounded_output::with_limit_usage(bytes - 1, || {
            TypeSite::push(&mut refused, "app.main", None, path, "lib.item")
        });
        assert_eq!(error.unwrap_err()[0].code, "SPX-G171");
        assert!(overflow);
        assert_eq!(consumed, refusal_diagnostic_bytes(bytes - 1));
        assert!(refused.is_empty());
    }

    fn refusal_diagnostic_bytes(remaining: usize) -> usize {
        // Refusing the site does not copy its path/owner, but reporting the
        // refusal still charges every diagnostic append, including discarded
        // partial text. Keep that distinct from the refused site allocation.
        // The report is formed after the site's own refusal, so reproduce that
        // sticky state first: since 87612d690 a refused budget admits no
        // further required append, and 74d6ff96d then borrows the static
        // default-limit message instead of charging formatted text.
        let (_, _, bytes) = bounded_output::with_limit_usage(remaining, || {
            assert!(!bounded_output::reserve_active_required(remaining + 1));
            crate::workspace_graph::diagnostics::limit_error(
                "builder_bytes",
                crate::workspace_graph::active_builder_limit(),
            )
        });
        bytes
    }
    #[test]
    fn fused_vec_field_sites_use_inner_call_identity_and_nested_type_paths() {
        let parsed = crate::parse(
            "module app; @id(\"app.main\") fn main()->i64 {0}",
            Path::new("fused-type-site.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let mut expression = main.body.clone();
        let outer_identity = expression.id.clone();
        expression.kind = hir::ResolvedExprKind::VecFieldRead {
            origin: crate::hir::VecFieldReadOrigin::Explicit,
            element: hir::ResolvedType::Nominal {
                declaration: hir::DeclarationId::new("lib.row"),
                arguments: vec![hir::ResolvedType::Nominal {
                    declaration: hir::DeclarationId::new("lib.leaf"),
                    arguments: vec![],
                }],
            },
            field: hir::DeclarationId::new("lib.row.text"),
            bytes: true,
            args: vec![],
        };
        let imported = BTreeSet::from(["lib.row", "lib.leaf"]);
        let inner = hir::workspace_expression_identity(&main.id, "body.tail.arg.0");
        let mut sites = Vec::new();
        super::super::collect_resolved_expression_type_sites(
            &main.id,
            &expression,
            "body.tail",
            &imported,
            &mut sites,
        )
        .unwrap();
        assert_eq!(
            expression.id, outer_identity,
            "replay never rekeys the retained outer HIR"
        );
        assert_eq!(
            sites.iter().map(TypeSite::projection).collect::<Vec<_>>(),
            vec![
                (
                    "app.main",
                    inner.as_str(),
                    "body.tail.arg.0.type_argument.0",
                    "lib.row"
                ),
                (
                    "app.main",
                    inner.as_str(),
                    "body.tail.arg.0.type_argument.0.argument.0",
                    "lib.leaf"
                ),
            ]
        );
        assert!(
            sites
                .iter()
                .all(|site| site.projection().1 != outer_identity.as_str()),
            "outer-identity edge substitution cannot match the independent inner sites"
        );
        let hir::ResolvedExprKind::VecFieldRead { bytes, .. } = &mut expression.kind else {
            unreachable!()
        };
        *bytes = false;
        let mut ordinary = Vec::new();
        super::super::collect_resolved_expression_type_sites(
            &main.id,
            &expression,
            "body.tail",
            &imported,
            &mut ordinary,
        )
        .unwrap();
        assert_eq!(
            ordinary[0].projection(),
            (
                "app.main",
                outer_identity.as_str(),
                "body.tail.type_argument.0",
                "lib.row"
            )
        );
        assert!(std::ptr::eq(
            ordinary[0].projection().1.as_ptr(),
            expression.id.as_str().as_ptr()
        ));
    }

    #[test]
    fn owned_type_site_identity_is_charged_before_exact_and_one_short_copy() {
        let identity = "declaration:8:app.main:expression:15:body.tail.arg.0";
        let path = "body.tail.arg.0.type_argument.0";
        let bytes = std::mem::size_of::<TypeSite<'_>>() + path.len() + identity.len();
        let mut sites = Vec::new();
        let (result, overflow, consumed) = bounded_output::with_limit_usage(bytes, || {
            TypeSite::push_owned_expression(&mut sites, "app.main", identity, path, "lib.row")
        });
        result.unwrap();
        assert!(!overflow);
        assert_eq!(consumed, bytes);
        assert_eq!(
            sites[0].projection(),
            ("app.main", identity, path, "lib.row")
        );
        assert!(!std::ptr::eq(
            sites[0].projection().1.as_ptr(),
            identity.as_ptr()
        ));
        let mut refused = Vec::new();
        let (result, overflow, _) = bounded_output::with_limit_usage(bytes - 1, || {
            TypeSite::push_owned_expression(&mut refused, "app.main", identity, path, "lib.row")
        });
        assert!(result.is_err());
        assert!(overflow);
        assert!(refused.is_empty());
    }

    #[test]
    fn budgeted_reconstruction_matches_canonical_identity_on_delimiter_and_unicode_inputs() {
        for (owner, path) in [
            ("", ""),
            ("a:b", "tail:arg.0"),
            ("é命", "body.é:0"),
            ("0123456789", "01234567890123456789"),
        ] {
            let owner = hir::DeclarationId::new(owner);
            assert_eq!(
                hir::budgeted_workspace_expression_identity(&owner, path),
                hir::workspace_expression_identity(&owner, path)
            );
        }
    }
}
