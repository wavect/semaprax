//! Borrow immutable HIR identities and copy only selected call owners.

use std::collections::BTreeSet;

use crate::{diagnostic::Diagnostic, hir};

use super::{checked_builder_sum, reserve_builder_structure};

pub(super) fn imported_call_sites<'a>(
    resolved: &'a hir::ResolvedProgram,
    target_ids: &BTreeSet<&str>,
) -> Result<Vec<(String, &'a str, &'a str)>, Vec<Diagnostic>> {
    let mut actual = Vec::new();
    hir::visit_workspace_call_sites(resolved, &mut |owner, expression, target| {
        if target_ids.contains(target.as_str()) {
            let bytes = checked_builder_sum(
                std::mem::size_of::<(String, &str, &str)>(),
                owner.as_str().len(),
            )?;
            // Charge before copying: closure owners are temporary walker values.
            reserve_builder_structure(bytes)?;
            actual.push((owner.as_str().to_owned(), expression, target.as_str()));
        }
        Ok::<(), Vec<Diagnostic>>(())
    })?;
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::imported_call_sites;
    use crate::{bounded_output, hir};

    fn resolved(body: &str) -> hir::ResolvedProgram {
        let source = format!(
            "module test.selected_calls;\n@id(\"lib.imported\") fn imported(value:i64)->i64{{value}}\n@id(\"app.local\") fn local(value:i64)->i64{{value}}\n@id(\"app.main\") fn main()->i64{{{body}}}\n"
        );
        let program = crate::parse(&source, Path::new("selected-calls.spx")).unwrap();
        hir::resolve(&program).unwrap()
    }

    #[test]
    fn selected_imports_borrow_sites_preserve_occurrences_and_skip_local_calls() {
        let program = resolved("local(1) + imported(2) + imported(3)");
        let targets = BTreeSet::from(["lib.imported"]);
        let sites = imported_call_sites(&program, &targets).unwrap();
        assert_eq!(sites.len(), 2);
        assert!(sites
            .iter()
            .all(|(owner, _, target)| owner == "app.main" && *target == "lib.imported"));
        assert_ne!(sites[0].1, sites[1].1);
        let retained = hir::workspace_call_sites(&program);
        let selected = retained
            .into_iter()
            .filter(|(_, _, target)| targets.contains(target.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            sites
                .iter()
                .map(|(owner, expression, target)| (owner.as_str(), *expression, *target))
                .collect::<Vec<_>>(),
            selected
                .iter()
                .map(|(owner, expression, target)| (
                    owner.as_str(),
                    expression.as_str(),
                    target.as_str()
                ))
                .collect::<Vec<_>>()
        );
        let mut matched = 0;
        hir::visit_workspace_call_sites(&program, &mut |_, expression, target| {
            for (_, borrowed_expression, borrowed_target) in &sites {
                if *borrowed_expression == expression {
                    matched += 1;
                    assert!(std::ptr::eq(
                        borrowed_expression.as_ptr(),
                        expression.as_ptr()
                    ));
                    assert!(std::ptr::eq(
                        borrowed_target.as_ptr(),
                        target.as_str().as_ptr()
                    ));
                }
            }
            Ok::<(), ()>(())
        })
        .unwrap();
        assert_eq!(matched, 2);
    }

    #[test]
    fn selected_closure_calls_keep_the_private_owner_after_the_visit() {
        let program = resolved("let callback = fn(value:i64)->i64 { imported(value) + imported(value) }; local(7) + callback(1)");
        let closure = hir::closure::inventory(&program)[0];
        let owner = hir::closure::closure_id(&closure.id);
        let targets = BTreeSet::from(["lib.imported"]);
        // The visitor constructs one temporary closure owner before copying
        // the two selected owners. Its formatted text remains charged too.
        let bytes = owner.as_str().len()
            + 2 * (std::mem::size_of::<(String, &str, &str)>() + owner.as_str().len());
        let (sites, overflow, consumed) =
            bounded_output::with_limit_usage(bytes, || imported_call_sites(&program, &targets));
        let sites = sites.unwrap();
        assert_eq!(sites.len(), 2);
        assert!(!overflow);
        assert_eq!(consumed, bytes);
        assert!(sites
            .iter()
            .all(|(caller, _, target)| caller == owner.as_str() && *target == "lib.imported"));
        assert_ne!(sites[0].1, sites[1].1);
        let selected = hir::workspace_call_sites(&program)
            .into_iter()
            .filter(|(_, _, target)| target.as_str() == "lib.imported")
            .collect::<Vec<_>>();
        assert_eq!(
            sites
                .iter()
                .map(|(caller, expression, target)| (caller.as_str(), *expression, *target))
                .collect::<Vec<_>>(),
            selected
                .iter()
                .map(|(caller, expression, target)| (
                    caller.as_str(),
                    expression.as_str(),
                    target.as_str()
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn selected_call_storage_is_charged_exactly_and_refuses_before_copying() {
        let local = resolved("local(1) + local(2)");
        let targets = BTreeSet::from(["lib.imported"]);
        let (sites, overflow, consumed) =
            bounded_output::with_limit_usage(0, || imported_call_sites(&local, &targets));
        assert!(sites.unwrap().is_empty());
        assert!(!overflow);
        assert_eq!(consumed, 0);

        let imported = resolved("local(1) + imported(2) + imported(3)");
        let bytes = 2 * (std::mem::size_of::<(String, &str, &str)>() + "app.main".len());
        let (sites, overflow, consumed) =
            bounded_output::with_limit_usage(bytes, || imported_call_sites(&imported, &targets));
        assert_eq!(sites.unwrap().len(), 2);
        assert!(!overflow);
        assert_eq!(consumed, bytes);
        let (error, overflow, consumed) = bounded_output::with_limit_usage(bytes - 1, || {
            imported_call_sites(&imported, &targets)
        });
        let error = error.unwrap_err();
        assert_eq!(error[0].code, "SPX-G171");
        assert_eq!(
            error[0].message,
            format!(
                "Workspace Semantic Graph `builder_bytes` exceeds {}",
                super::super::super::active_builder_limit()
            )
        );
        assert!(overflow);
        // A refusal also formats a bounded diagnostic. Measure that output
        // independently with exactly the bytes remaining after the first site;
        // even a discarded partial diagnostic keeps its allocation debit.
        let (_, _, diagnostic_bytes) = bounded_output::with_limit_usage(bytes / 2 - 1, || {
            crate::workspace_graph::diagnostics::limit_error(
                "builder_bytes",
                super::super::super::active_builder_limit(),
            )
        });
        assert_eq!(
            consumed,
            bytes / 2 + diagnostic_bytes,
            "only the first site and bounded refusal diagnostic are debited"
        );
    }
}
