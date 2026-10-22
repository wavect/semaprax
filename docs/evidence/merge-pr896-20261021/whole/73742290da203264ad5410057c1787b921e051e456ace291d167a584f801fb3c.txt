//! Standalone string continuation of the closed ordinary-import view.
use super::*;
use crate::ast::program_source::function_alias::FunctionEntry;
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;
use sha2::{Digest, Sha256};
const LIMIT: usize = 64 * 1024 * 1024;
const APP: &str = r#"module app;
use function @id("ops.world") from test.string_ops as world;
use function @id("ops.combine") from test.string_ops as combine;
@id("app.main") fn main()->i64 {let message=combine(world(),"!");string_len(message)}"#;
// Use the whole existing admitted example, including its independent main.
const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/string_ops.spx"
));
fn programs() -> [Program; 2] {
    [
        crate::parse(APP, "app.spx").unwrap(),
        crate::parse(PROVIDER, "text.spx").unwrap(),
    ]
}
#[test]
fn imported_string_examples_keep_complete_effective_hir_graph_and_distinct_default_ids() {
    let programs = programs();
    // The extension stores no String, extra variant or mode field in an entry.
    // The genuine pre-String layout witness remains in scalar_signature_tests.
    assert_eq!(
        std::mem::size_of::<FunctionEntry<'_>>(),
        std::mem::size_of::<crate::ast::program_source::function_alias::FunctionView<'_>>()
    );
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_string_signature::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let current = super::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert_eq!(old.functions.roots.functions.len(), 2);
        assert!(current.functions.roots.functions.is_empty());
        assert_eq!(current.functions.roots.functions.capacity(), 0);
        for i in 1..3 {
            let actual = current.function_refs().get(i).unwrap();
            let expected = old.function_refs().get(i).unwrap();
            assert!(actual.is_signature());
            assert!(actual.same_ordinary_declaration(expected.payload()));
            assert!(
                matches!(&actual.body().kind,ExprKind::String(value) if value.is_empty() && value.capacity()==0)
            );
            assert!(current.function_refs().contains_presentation(&actual));
            assert!(!current
                .function_refs()
                .contains_unaliased_payload(actual.payload()));
        }
        let world = current.function_refs().get(1).unwrap();
        let combine = current.function_refs().get(2).unwrap();
        assert!(std::ptr::eq(world.body(), combine.body()));
        assert!(!world.same_presentation(&combine));
        assert_eq!(
            crate::prelude::selected_for_program(&old),
            crate::prelude::selected_for_program(&current)
        );
        assert_eq!(
            crate::prelude::declarations_for_program(&old),
            crate::prelude::declarations_for_program(&current)
        );
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let expected = crate::hir::identity_resolution::resolve_source_nominal(
            &old,
            layout.presentation(&programs[0], &old).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        let actual = crate::hir::identity_resolution::resolve_source_nominal(
            &current,
            layout.presentation(&programs[0], &current).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        assert_eq!(expected, actual);
        crate::hir::validate(&actual).unwrap();
        assert_eq!(
            crate::graph::to_hir_json(&expected, "same-string-revision").unwrap(),
            crate::graph::to_hir_json(&actual, "same-string-revision").unwrap()
        );
        let a = actual
            .functions
            .iter()
            .find(|f| f.id.as_str() == "ops.world")
            .unwrap();
        let b = actual
            .functions
            .iter()
            .find(|f| f.id.as_str() == "ops.combine")
            .unwrap();
        assert_ne!(a.body.id, b.body.id);
        // Full provider validation is independent: its original nonempty functions
        // remain checked, rather than inheriting the importing stub's finite body.
        let provider = crate::hir::resolve(&programs[1]).unwrap();
        crate::hir::validate(&provider).unwrap();
    });
    assert!(!refused && first.is_none());
}
#[test]
fn string_parameters_with_primitive_results_preserve_all_metadata_and_profiles() {
    // Parse a complete admitted revision: both the declaration and its caller
    // now use a real String-read i64 result, not a mismatched AST return type.
    let original =
        "fn combine(left: string, right: string) -> string\n{\n    string_concat(left, right)\n}";
    assert_eq!(PROVIDER.matches(original).count(), 1);
    let provider = PROVIDER.replace(original,
        "fn combine(left: string, right: string) -> i64\n{\n    string_len(left) + string_len(right)\n}");
    let app = APP.replace(
        "let message=combine(world(),\"!\");string_len(message)",
        "combine(world(),\"!\")",
    );
    assert_ne!(app, APP);
    let programs = [
        crate::parse(&app, "app.spx").unwrap(),
        crate::parse(&provider, "text.spx").unwrap(),
    ];
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_string_signature::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let current = super::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        for i in 1..3 {
            assert!(current
                .function_refs()
                .get(i)
                .unwrap()
                .same_ordinary_declaration(old.functions().get(i).unwrap()));
        }
        assert_eq!(
            crate::prelude::selected_for_program(&old),
            crate::prelude::selected_for_program(&current)
        );
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let expected = crate::hir::identity_resolution::resolve_source_nominal(
            &old,
            layout.presentation(&programs[0], &old).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        let actual = crate::hir::identity_resolution::resolve_source_nominal(
            &current,
            layout.presentation(&programs[0], &current).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        assert_eq!(expected, actual);
        crate::hir::validate(&actual).unwrap();
        assert_eq!(
            crate::graph::to_hir_json(&expected, "same-read-revision").unwrap(),
            crate::graph::to_hir_json(&actual, "same-read-revision").unwrap()
        );
        let provider = crate::hir::resolve(&programs[1]).unwrap();
        crate::hir::validate(&provider).unwrap();
    });
    assert!(!refused && first.is_none());
}
#[test]
fn complete_string_constructor_exact_short_zero_and_drop_keep_original_work_and_fees() {
    let programs = programs();
    let authored = index_authored(&programs).unwrap();
    let observe = |current, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                let mut owner = match Receiver::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                let result = if current {
                    super::construct_transient(
                        &programs[0],
                        &authored,
                        &programs,
                        &registry,
                        None,
                        &mut owner,
                    )
                } else {
                    super::before_string_signature::construct_transient(
                        &programs[0],
                        &authored,
                        &programs,
                        &registry,
                        None,
                        &mut owner,
                    )
                };
                let ok = result.is_ok();
                drop(result);
                drop(owner);
                drop(registry);
                ok
            })
        })
    };
    for current in [false, true] {
        let ((ok, metrics), refused, paid, first) = observe(current, LIMIT);
        assert!(ok && !refused && first.is_none());
        assert!(paid > 0);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        for (limit, success) in [(paid, true), (paid - 1, false), (0, false)] {
            let ((ok, metrics), refused, _, first) = observe(current, limit);
            assert_eq!(ok, success);
            assert_eq!(refused, !success);
            assert_eq!(first.is_some(), !success);
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        }
    }
}
#[test]
fn unsynthesizable_defaults_and_wrong_provider_keep_complete_ordered_diagnostics() {
    use crate::ast::program_source::function_alias::scalar_signature::ScalarSignature;
    for ty in [Type::Never, Type::Str, Type::SliceU8, Type::ArrayU8(0)] {
        let mut programs = programs();
        programs[1]
            .functions
            .iter_mut()
            .find(|f| f.name == "world")
            .unwrap()
            .return_type = ty.clone();
        assert!(
            ScalarSignature::checked(&programs[0], &programs[1].functions[1], "combine").is_none()
        );
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut owner = Receiver::acquire().unwrap();
            let old = super::before_string_signature::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .err()
            .unwrap();
            let current = super::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .err()
            .unwrap();
            let (Failure::One(old), Failure::One(current)) = (old, current) else {
                panic!("ordinary default diagnostic expected")
            };
            assert_eq!(old, current);
            assert_eq!(
                old.code,
                if ty == Type::Never {
                    "SPX-G172"
                } else {
                    "SPX-G173"
                }
            );
        });
        assert!(!refused && first.is_none());
    }
}
#[test]
fn owned_string_parameter_keeps_full_owning_fallback_and_pre_string_oracles_are_complete() {
    let mut programs = programs();
    programs[1]
        .functions
        .iter_mut()
        .find(|f| f.name == "combine")
        .unwrap()
        .params[0]
        .mode = crate::ast::ParamMode::Own;
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_string_signature::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let current = super::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert!(matches!(
            current.functions.entries[1],
            FunctionEntry::Owned(_)
        ));
        assert!(current
            .function_refs()
            .get(2)
            .unwrap()
            .same_ordinary_declaration(old.functions().get(2).unwrap()));
    });
    assert!(!refused && first.is_none());
    let old = include_str!("before_string_signature.rs");
    assert_eq!(
        old.matches("ScalarSignature::checked_before_string(")
            .count(),
        1
    );
    let old = old.replace(
        "ScalarSignature::checked_before_string(",
        "ScalarSignature::checked(",
    );
    let glue = "use super::alias::owned_growth_tests;";
    assert_eq!(old.matches(glue).count(), 1);
    let old=old.replace(glue,"#[cfg(test)]\n#[path = \"alias/owned_growth_tests.rs\"]\npub(super) mod owned_growth_tests;");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "9c277a8ecb1a8b99f6f03031899a4d7cf413583e9cba285a98f8edfa0cfa9b84"
    );
    let predicate = include_str!("../../../../ast/program_source/function_alias/before_string.rs");
    let header="//! Complete immediate-old finite admission predicate.\nuse super::scalar_signature::ScalarSignature;\nimpl<'a> ScalarSignature<'a> {\n";
    let method = predicate
        .strip_prefix(header)
        .unwrap()
        .strip_suffix("\n}\n")
        .unwrap();
    assert_eq!(method.matches("fn checked_before_string(").count(), 1);
    let method = method.replace("fn checked_before_string(", "fn checked(");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(method.as_bytes()))
        ),
        "b07ed8c0f579e2838cd64fded12ea4d80b747fe9f6ae4b023ff4d842b8ad74ec"
    );
}

#[test]
fn string_import_type_and_call_refusals_keep_all_six_fields_and_first_selection() {
    for app in [
        APP.replace("combine(world(),\"!\")", "combine(1,\"!\")"),
        APP.replace("world()", "world(1)"),
        APP.replace("string_len(message)", "message"),
    ] {
        let programs = [
            crate::parse(&app, "app.spx").unwrap(),
            crate::parse(PROVIDER, "text.spx").unwrap(),
        ];
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut owner = Receiver::acquire().unwrap();
            let old = super::before_string_signature::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            let current = super::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            let layout = registry.get(&programs[0], &authored, &programs).unwrap();
            let old = crate::hir::identity_resolution::resolve_source_nominal(
                &old,
                layout.presentation(&programs[0], &old).unwrap(),
                crate::workspace_graph::reserve_builder_structure,
            )
            .err()
            .unwrap();
            let current = crate::hir::identity_resolution::resolve_source_nominal(
                &current,
                layout.presentation(&programs[0], &current).unwrap(),
                crate::workspace_graph::reserve_builder_structure,
            )
            .err()
            .unwrap();
            let (
                crate::hir::identity_resolution::Failure::Source(old),
                crate::hir::identity_resolution::Failure::Source(current),
            ) = (old, current)
            else {
                panic!("ordinary source refusal required")
            };
            assert!(!old.is_empty());
            assert_eq!(old, current);
        });
        assert!(!refused && first.is_none());
    }
}
