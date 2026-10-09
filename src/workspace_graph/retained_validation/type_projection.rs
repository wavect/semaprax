//! Exact transient type-site views; generated paths remain independently owned.

use std::borrow::Cow;
use std::cmp::Ordering;

use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{expected_projection::checked_builder_sum, reserve_builder_structure};

#[derive(Debug)]
pub(super) struct TypeSite<'a> {
    owner: Cow<'a, str>,
    expression: Option<&'a str>,
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
            expression,
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
            expression,
            path: path.to_owned(),
            target,
        });
        Ok(())
    }

    pub(super) fn projection(&self) -> (&str, &str, &str, &str) {
        (
            self.owner.as_ref(),
            self.expression.unwrap_or(&self.path),
            &self.path,
            self.target,
        )
    }
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
        assert_eq!(consumed, 0);
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
        assert_eq!(consumed, 0);
        assert!(refused.is_empty());
    }
}
