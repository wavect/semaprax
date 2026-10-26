//! Complete original transient constructor and late-refusal publication controls.
use super::super::super::{construct, LayoutRegistry};
use super::{sources, BOX_CALLER, BOX_PROVIDER, VEC_CALLER, VEC_PROVIDER};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;
#[path = "original_construct.rs"]
mod original_construct;

#[test]
fn last_alias_and_synthetic_main_refusal_never_publish_partial_generated_roots() {
    for (caller, provider) in [(VEC_CALLER, VEC_PROVIDER), (BOX_CALLER, BOX_PROVIDER)] {
        for synthetic_main in [false, true] {
            let caller = if synthetic_main {
                caller.replace("fn main()", "fn inert()")
            } else {
                caller.to_owned()
            };
            let programs = sources(&caller, provider);
            let authored = index_authored(&programs).unwrap();
            let registry = LayoutRegistry::new(&programs, &authored);
            drop(registry.get(&programs[0], &authored, &programs).unwrap());
            // Warm ordinary immutable tables before the complete observation.
            drop(construct(&programs[0], &authored, &programs, &registry).unwrap());
            let observe = |limit, before| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let result = if before {
                            original_construct::construct(
                                &programs[0],
                                &authored,
                                &programs,
                                &registry,
                            )
                        } else {
                            construct(&programs[0], &authored, &programs, &registry)
                        };
                        let outcome = match &result {
                            Ok(view) => (
                                true,
                                programs[0].module_uses.iter().all(|import| {
                                    if import.kind != crate::ast::ModuleUseKind::Function {
                                        return true;
                                    }
                                    view.functions().iter().any(|function| {
                                        function.stable_id == import.persistent_id
                                            && function.name == import.alias
                                    })
                                }) && (!synthetic_main
                                    || view
                                        .functions()
                                        .iter()
                                        .any(|function| function.name == "main")),
                                false,
                            ),
                            Err(errors) => (
                                false,
                                false,
                                errors.len() == 1 && errors[0].code == "SPX-G171",
                            ),
                        };
                        drop(result);
                        outcome
                    })
                })
            };
            let ((wide, metrics), refused, paid, first) = observe(usize::MAX, false);
            assert_eq!(wide, (true, true, false));
            assert!(
                !refused && first.is_none() && paid > 0 && !metrics.invalid && metrics.live == 0
            );
            assert_eq!(metrics.allocated, metrics.freed);
            let ((old_wide, old_metrics), old_refused, old_paid, old_first) =
                observe(usize::MAX, true);
            assert_eq!(old_wide, wide);
            assert!(!old_refused && old_first.is_none());
            assert_eq!(old_paid, paid);
            assert_eq!(old_metrics, metrics);
            let ((exact, exact_metrics), refused, used, first) = observe(paid, false);
            assert_eq!(exact, wide);
            assert!(!refused && first.is_none() && used == paid);
            assert_eq!(exact_metrics, metrics);
            let ((old_short, old_metrics), refused, used, first) = observe(paid - 1, true);
            // Preserve the full original defective receiver as a counterexample.
            assert_eq!(old_short, (true, false, false));
            assert!(refused && first.is_some() && used <= paid - 1);
            assert!(!old_metrics.invalid && old_metrics.live == 0);
            assert_eq!(old_metrics.allocated, old_metrics.freed);
            for limit in [paid - 1, 0] {
                let ((outcome, metrics), refused, used, first) = observe(limit, false);
                assert_eq!(outcome, (false, false, true));
                assert!(refused && first.is_some() && used <= limit);
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
                let remaining = first.unwrap();
                assert!(remaining.requested > remaining.remaining);
            }
        }
    }
}

#[test]
fn full_original_constructor_and_only_sticky_publication_changes_stay_source_bound() {
    use sha2::{Digest, Sha256};
    let original = include_str!(
        "../../../../../../docs/evidence/transient-publication/original-construct.rs.txt"
    );
    let saved = crate::workspace_graph::expected_projection::defaults::project_original_receiver(
        "src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs",
        include_str!("original_construct.rs"),
    );
    let saved = saved
        .strip_prefix("use crate::workspace_graph::expected_projection::borrowed_imports::*;\n\n")
        .unwrap();
    assert_eq!(
        saved.replace("crate::workspace_graph::", "super::super::"),
        original
    );
    let current = include_str!("../../../borrowed_imports.rs");
    let start = current
        .find("pub(in crate::workspace_graph) fn construct<'a>(")
        .unwrap();
    let body = &current[start..current[start..].find("\n#[cfg(test)]").unwrap() + start];
    let mut restored = body.to_owned();
    assert_eq!(
        restored
            .matches("        reserve_builder_structure(0)?;\n")
            .count(),
        4
    );
    restored = restored.replace("        reserve_builder_structure(0)?;\n", "");
    let footer = "    // Infallible bounded string helpers record refusal and return empty text.\n    // A generated view must never publish that partial construction as success.\n    reserve_builder_structure(0)?;\n";
    assert_eq!(restored.matches(footer).count(), 1);
    restored = restored.replace(footer, "");
    assert_eq!(restored, original);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "051b0ab47e03d9e0fac140f6199e87b1375510f16327f148387fe6414e7c6d10"
    );
}
