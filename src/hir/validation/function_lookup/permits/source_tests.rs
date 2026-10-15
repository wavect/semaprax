//! Current source membership and the complete original BTree receiver.
use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn source_permits_match_complete_old_membership_without_backing_in_both_lookup_modes() {
    let mut program = crate::hir::resolve(
        &crate::check(
            "module permits.source; @id(\"main\") fn main()->i64 { 0 }",
            "source-permits.spx",
        )
        .unwrap(),
    )
    .unwrap();
    program.permits = ["é.echo", "host.echo", "", "é.echo", "host.echo.extra", "λ"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    for borrowed in [false, true] {
        let lookup = authenticate(&program, borrowed.then_some(&directory)).unwrap();
        let old = lookup.permits(&program);
        let independent_tree =
            Permits::Legacy(program.permits.iter().map(String::as_str).collect());
        for limit in [0, 1] {
            let (((), actual), refused, used, receipt) =
                crate::bounded_output::with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let current = lookup.permits_borrowed(&program);
                        for key in ["é.echo", "host.echo", "", "λ", "é", "host", "missing"] {
                            assert_eq!(current.contains(key), old.contains(key));
                            assert_eq!(current.contains(key), independent_tree.contains(key));
                        }
                        drop(current);
                    })
                });
            assert!(!refused && receipt.is_none());
            assert_eq!(used, 0);
            assert_eq!(actual.allocated, 0);
            assert_eq!(actual.freed, 0);
            assert_eq!(actual.live, 0);
            assert!(!actual.invalid);
        }
        let (((), actual), refused, used, receipt) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                measure(|| {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                    let current = lookup.permits_borrowed(&program);
                    assert!(current.contains("é.echo"));
                    drop(current);
                    assert!(!crate::bounded_output::reserve_active_required(0));
                })
            });
        assert!(refused && receipt.is_some());
        assert_eq!(used, 0);
        assert_eq!(actual.allocated, 0);
        assert_eq!(actual.live, 0);
        assert!(!actual.invalid);
        drop(independent_tree);
        drop(old);
        drop(lookup);
    }
}

#[test]
fn current_source_permits_preserve_full_saved_function_diagnostics_and_effect_order() {
    let base = crate::hir::resolve(
        &crate::check(
            "module permits.order; @id(\"main\") fn main()->i64 { 0 }",
            "permit-order.spx",
        )
        .unwrap(),
    )
    .unwrap();
    for case in 0..3 {
        let mut program = base.clone();
        program.permits = vec!["λ.echo".into(), "host.echo".into(), "λ.echo".into()];
        program.functions[0].effects = match case {
            0 => vec!["λ.echo".into(), "host.echo".into()],
            _ => vec!["missing.z".into(), "missing.a".into()],
        };
        if case == 2 {
            program.functions[0].return_type = ResolvedType::Unit;
        }
        let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
        for borrowed in [false, true] {
            let mut current =
                HirValidator::with_directory(&program, borrowed.then_some(&directory)).unwrap();
            let mut original = current.clone();
            let function = &program.functions[0];
            let execution = FunctionExecutionId::Monomorphic(function.id.clone());
            let expected = original.validate_function_owned_reference(function, &execution);
            let actual = current.validate_function(function, &execution);
            assert_eq!(actual, expected);
            match case {
                0 => actual.unwrap(),
                1 => {
                    let diagnostic = actual.unwrap_err();
                    assert!(diagnostic.message.contains("missing.z"));
                    assert!(!diagnostic.message.contains("missing.a"));
                }
                _ => assert!(actual.unwrap_err().message.contains("unit result")),
            }
        }
    }
}

#[test]
fn complete_lookup_and_permit_sources_have_only_named_borrowed_membership_hooks() {
    let projected = super::super::paid_custody::project_current(
        "src/hir/validation/function_lookup.rs",
        ci_str!("../../function_lookup.rs"),
    );
    let current = projected.as_str();
    let addition = "    pub(super) fn permits_borrowed(&self, program: &'a ResolvedProgram) -> Permits<'a> {\n        match self {\n            Self::Borrowed { directory, .. } => Permits::Borrowed((*directory).permits()),\n            Self::Legacy(_) => Permits::Source(&program.permits),\n        }\n    }\n    // Complete original BTree receiver for independently compiled saved validators.\n    #[cfg(test)]\n";
    assert_eq!(current.matches(addition).count(), 1);
    assert_eq!(current.replacen(addition, "", 1), ci_str!("../../../../../docs/evidence/function-scope-backing-isolation/function_lookup.rs.before.txt"));
    let current = ci_str!("../permits.rs");
    let variant = "    Source(&'a [String]),\n    #[cfg(test)]\n";
    let arm = "            Self::Source(permits) => permits.iter().any(|permit| permit == effect),\n            #[cfg(test)]\n";
    assert_eq!(current.matches(variant).count(), 1);
    assert_eq!(current.matches(arm).count(), 1);
    assert_eq!(current.matches("#[cfg(test)]\nuse super::*;").count(), 1);
    let original = current
        .strip_suffix("\n#[cfg(test)]\nmod source_tests;\n")
        .unwrap()
        .replacen(variant, "", 1)
        .replacen(arm, "", 1)
        .replacen("#[cfg(test)]\nuse super::*;", "use super::*;", 1);
    assert_eq!(
        original,
        ci_str!(
            "../../../../../docs/evidence/function-scope-backing-isolation/permits.rs.before.txt"
        )
    );
}
