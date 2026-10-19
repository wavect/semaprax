//! Complete original transient constructor and late-refusal publication controls.
use super::super::super::{construct, LayoutRegistry};
use super::{sources, BOX_CALLER, BOX_PROVIDER, VEC_CALLER, VEC_PROVIDER};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;
#[path = "original_construct.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
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
            assert!(refused && first.is_some() && used < paid);
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

// The complete pre-provider-proof constructor owns the original publication check.
// Formatting and provider reuse are independent, ordered full-source inverses.
fn publication_constructor_input(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    assert_eq!(current, include_str!("../../../borrowed_imports.rs"));
    assert_eq!(
        digest(current),
        "9b70751b6d20536b964bf0739ba74e312f0fb1be03ef37037054cf274ca29a99"
    );
    let records: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../docs/evidence/provider-proof-backing-714177/whole-source-inverses.json"
    ))
    .unwrap();
    let record = records["whole_sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"] == "src/workspace_graph/expected_projection/borrowed_imports.rs")
        .unwrap();
    let edits = record["inverse"].as_array().unwrap();
    assert_eq!(edits.len(), 7);
    let mut restored = ci_str!("../../../borrowed_imports.rs").to_owned();
    assert_eq!(
        digest(&restored),
        "cbea3d04790517cf7584fe1163000314dbf514990b5cd5ed4429564e59f23675"
    );
    // The existing CI receiver has already restored both complete prefix edits.
    assert_eq!(edits[1]["before"].as_str().unwrap(), restored);
    for edit in &edits[2..] {
        let after = edit["current"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(restored.len(), 14329);
    assert_eq!(
        digest(&restored),
        "54599fda2371ac9e76b897812e8d717828d542db25dceffa2d1161b403a2bc38"
    );
    restored
}

#[test]
fn full_original_constructor_and_only_sticky_publication_changes_stay_source_bound() {
    use sha2::{Digest, Sha256};
    let original =
        ci_str!("../../../../../../docs/evidence/transient-publication/original-construct.rs.txt");
    let saved = crate::workspace_graph::expected_projection::defaults::project_original_receiver(
        "src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs",
        ci_str!("original_construct.rs"),
    );
    let saved = saved
        .strip_prefix("use crate::workspace_graph::expected_projection::borrowed_imports::*;\n\n")
        .unwrap();
    assert_eq!(
        saved.replace("crate::workspace_graph::", "super::super::"),
        original
    );
    let compiled = include_str!("../../../borrowed_imports.rs");
    let current = publication_constructor_input(compiled);
    for bad in [
        compiled[..compiled.len() - 1].to_owned(),
        format!("{compiled}\n"),
        compiled.replacen("construct_using", "foreign_construct_using", 1),
        current.clone(),
    ] {
        assert!(std::panic::catch_unwind(|| publication_constructor_input(&bad)).is_err());
    }
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
