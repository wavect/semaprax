//! Actual constructor, whole immediate-old constructor and ordinary HIR owners.
use super::*;
use crate::ast::program_source::function_alias::presentation::{FunctionCell, FunctionRef};
use crate::ast::program_source::function_alias::{FunctionEntry, FunctionView};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;
use sha2::{Digest, Sha256};
const LIMIT: usize = 64 * 1024 * 1024;
const APP: &str = r#"module app;
use function @id("left.value") from left as left;
use function @id("right.value") from right as right;
@id("app.main") fn main()->i64{left()+right()}"#;
fn programs() -> [Program; 3] {
    [
        crate::parse(APP, "app.spx").unwrap(),
        crate::parse(
            "module left; @id(\"left.value\") fn left()->i64{20}",
            "left.spx",
        )
        .unwrap(),
        crate::parse(
            "module right; @id(\"right.value\") fn right()->i64{22}",
            "right.spx",
        )
        .unwrap(),
    ]
}
#[test]
fn duplicate_finite_defaults_keep_whole_hir_graph_identities_and_source_membership() {
    let programs = programs();
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_scalar_signature::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        let view = super::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .unwrap();
        assert!(view.functions.roots.functions.is_empty());
        assert_eq!(view.functions.roots.functions.capacity(), 0);
        assert_eq!(view.functions().len(), old.functions().len());
        assert_eq!(
            view.functions().profile_iter().count(),
            programs[0].functions.len()
        );
        for ordinal in 0..2 {
            let actual = view.function_refs().get(ordinal + 1).unwrap();
            let expected = old.function_refs().get(ordinal + 1).unwrap();
            assert!(actual.is_signature());
            assert!(actual.same_ordinary_declaration(expected.payload()));
            assert!(std::ptr::eq(
                actual.payload(),
                &programs[ordinal + 1].functions[0]
            ));
            assert!(view.function_refs().contains_presentation(&actual));
            assert!(!view
                .function_refs()
                .contains_unaliased_payload(actual.payload()));
        }
        let a = view.function_refs().get(1).unwrap();
        let b = view.function_refs().get(2).unwrap();
        assert!(std::ptr::eq(a.body(), b.body()));
        assert!(!a.same_presentation(&b));
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let expected = crate::hir::identity_resolution::resolve_source_nominal(
            &old,
            layout.presentation(&programs[0], &old).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        let actual = crate::hir::identity_resolution::resolve_source_nominal(
            &view,
            layout.presentation(&programs[0], &view).unwrap(),
            crate::workspace_graph::reserve_builder_structure,
        )
        .unwrap()
        .0;
        assert_eq!(expected, actual);
        assert_eq!(
            crate::graph::to_hir_json(&expected, "same-revision").unwrap(),
            crate::graph::to_hir_json(&actual, "same-revision").unwrap()
        );
        let left = actual
            .functions
            .iter()
            .find(|f| f.id.as_str() == "left.value")
            .unwrap();
        let right = actual
            .functions
            .iter()
            .find(|f| f.id.as_str() == "right.value")
            .unwrap();
        assert_ne!(left.body.id, right.body.id);
        assert_ne!(left.id, right.id);
        crate::hir::validate(&actual).unwrap();
    });
    assert!(!refused && first.is_none());
}
fn profile(program: &dyn ProgramSource) -> [bool; 9] {
    [
        crate::map_ops::program_uses(program),
        crate::stdin_stream_ops::program_uses(program),
        crate::source_verify::program_uses_record_iterator(program),
        crate::list_ops::program_uses_list(program),
        crate::iterator_ops::program_uses_iterator(program),
        crate::box_ops::program_uses_owned_payload(program),
        crate::prelude::program_uses_box(program),
        crate::prelude::program_uses_vec(program),
        crate::prelude::program_uses_vec_v3(program),
    ]
}
#[test]
fn all_eight_finite_defaults_keep_profiles_and_every_effective_field() {
    for ty in [
        Type::I64,
        Type::I32,
        Type::Char,
        Type::U8,
        Type::Usize,
        Type::F32,
        Type::F64,
        Type::Bool,
    ] {
        let mut programs = programs();
        // Typed producer control. Provider body is deliberately immaterial to
        // signature projection; complete provider validation is a later route.
        for p in &mut programs[1..] {
            p.functions[0].return_type = ty.clone();
            p.functions[0].body = crate::ast::Expr {
                kind: crate::ast::ExprKind::Call {
                    name: "map_remove".to_owned(),
                    type_arguments: vec![Type::Bytes],
                    args: vec![],
                },
                span: crate::ast::Span::default(),
            };
            let discarded_contract = p.functions[0].body.clone();
            p.functions[0].requires.push(discarded_contract);
        }
        let authored = index_authored(&programs).unwrap();
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let registry = LayoutRegistry::new(&programs, &authored);
            let mut owner = Receiver::acquire().unwrap();
            let old = super::before_scalar_signature::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            let view = super::construct_transient(
                &programs[0],
                &authored,
                &programs,
                &registry,
                None,
                &mut owner,
            )
            .unwrap();
            assert_eq!(profile(&old), profile(&view));
            assert_eq!(
                crate::prelude::selected_for_program(&old),
                crate::prelude::selected_for_program(&view)
            );
            for ordinal in 1..3 {
                let f = view.function_refs().get(ordinal).unwrap();
                assert!(f.is_signature());
                assert_eq!(f.materialize(), *old.functions().get(ordinal).unwrap());
            }
        });
        assert!(!refused && first.is_none());
    }
}
#[test]
fn current_constructor_exact_short_zero_and_drop_keep_real_invoice() {
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
                    super::before_scalar_signature::construct_transient(
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
fn complete_old_constructor_and_actual_dispatch_remain_independent_and_same_width() {
    let old = include_str!("before_scalar_signature.rs");
    let glue = "use super::alias::owned_growth_tests;";
    assert_eq!(old.matches(glue).count(), 1);
    let restored=old.replace(glue,"#[cfg(test)]\n#[path = \"alias/owned_growth_tests.rs\"]\npub(super) mod owned_growth_tests;");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "434640ce68332a1d96840ae10b8e4a500799a567f6d75cc903a6e272196a24b2"
    );
    #[allow(dead_code)]
    struct OldView<'a> {
        _provider: &'a Program,
        original: &'a Function,
        _alias: &'a str,
    }
    #[allow(dead_code)]
    enum OldEntry<'a> {
        Owned(usize),
        Borrowed(OldView<'a>),
    }
    #[allow(dead_code)]
    struct OldRef<'a> {
        payload: &'a Function,
        alias: Option<&'a &'a str>,
    }
    use std::mem::size_of;
    assert_eq!(size_of::<Option<&Program>>(), size_of::<&Program>());
    assert_eq!(size_of::<FunctionView<'_>>(), size_of::<OldView<'_>>());
    assert_eq!(size_of::<OldEntry<'_>>(), size_of::<OldView<'_>>());
    assert_eq!(size_of::<FunctionEntry<'_>>(), size_of::<OldEntry<'_>>());
    // Every entry-bearing production invoice uses the actual element type:
    // directory backing, output slots, and the unchanged Vec carrier. Neither
    // the historical baseline nor a discarded tag receives artificial padding.
    assert_eq!(
        size_of::<Option<FunctionView<'_>>>(),
        size_of::<Option<OldView<'_>>>()
    );
    assert_eq!(
        size_of::<crate::ast::program_source::function_alias::scalar_signature::ScalarSignature<'_>>(
        ),
        size_of::<OldView<'_>>()
    );
    assert_eq!(
        size_of::<Vec<FunctionEntry<'_>>>(),
        size_of::<Vec<OldEntry<'_>>>()
    );
    for capacity in [0, 1, 2, 16, 492, 1024] {
        let old = std::alloc::Layout::array::<OldEntry<'_>>(capacity).unwrap();
        let current = std::alloc::Layout::array::<FunctionEntry<'_>>(capacity).unwrap();
        assert_eq!(old, current);
    }

    assert_eq!(size_of::<FunctionRef<'_>>(), size_of::<OldRef<'_>>());
    assert_eq!(
        size_of::<FunctionCell<'_>>(),
        size_of::<(Option<&TypeDeclaration>, OldRef<'_>)>()
    );
    let program = programs();
    use std::mem::size_of_val;
    assert_eq!(
        size_of_val(&program[0].functions().profile_iter()),
        size_of_val(&program[0].functions().iter())
    );
    let old_dispatch = include_str!("../../../constructor_failure/before_scalar_dispatch.rs");
    let header = "#![allow(dead_code, reason = \"Whole immediate-old selector, hooks and invariant forwarding are an independent cfg-test oracle.\")]\nuse super::*;\n";
    let restored = old_dispatch.strip_prefix(header).unwrap();
    assert_eq!(restored.matches("super::super::super::").count(), 16);
    let restored = restored.replace("super::super::super::", "super::super::");
    let glue = "borrowed_imports::typed::before_scalar_signature::construct_transient(";
    assert_eq!(restored.matches(glue).count(), 1);
    let restored = restored.replace(glue, "borrowed_imports::typed::construct_transient(");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "955f367930ff21369599457a8fabdf8330144ca000920d5ec594cb48a445f84a"
    );
    // Whole immediate old actual selector remains compiled, with only the
    // independently compiled constructor namespace substituted in its body.
    let _: fn(&str)->bool = crate::workspace_graph::constructor_failure::typed::before_scalar_dispatch::may_borrow_wrapper;
}

fn workspace_sources(app: &str) -> Vec<crate::workspace_graph::WorkspaceSource> {
    [
        ("app.spx", app),
        (
            "left.spx",
            "module left; @id(\"left.value\") fn left()->i64{20}",
        ),
        (
            "right.spx",
            "module right; @id(\"right.value\") fn right()->i64{22}",
        ),
    ]
    .into_iter()
    .map(|(path, source)| crate::workspace_graph::WorkspaceSource {
        path: path.to_owned(),
        source: crate::format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect()
}
#[test]
fn actual_uncached_dispatch_preserves_whole_workspace_and_six_field_first_errors() {
    use crate::workspace_graph::constructor_failure::typed::with_original_scalar_dispatch;
    let old = with_original_scalar_dispatch(|| {
        crate::workspace_graph::build_owned(workspace_sources(APP))
    })
    .unwrap();
    let current = crate::workspace_graph::build_owned(workspace_sources(APP)).unwrap();
    let old = old.linked_scalar_program("app").unwrap();
    let current = current.linked_scalar_program("app").unwrap();
    assert_eq!(old, current);
    assert_eq!(
        crate::graph::to_hir_json(&old, "same-input").unwrap(),
        crate::graph::to_hir_json(&current, "same-input").unwrap()
    );
    for app in [
        APP.replace("left()+right()", "left(1)+right()"),
        APP.replace("left()+right()", "unknown()+right()"),
        APP.replace("left()+right()", "true"),
    ] {
        let old = with_original_scalar_dispatch(|| {
            crate::workspace_graph::build_owned(workspace_sources(&app))
        })
        .err()
        .expect("old source must refuse");
        let current = crate::workspace_graph::build_owned(workspace_sources(&app))
            .err()
            .expect("current source must refuse");
        assert!(!old.is_empty());
        // Diagnostic Eq includes code, severity, message, path, span and help;
        // complete ordered vectors retain the original first selection.
        assert_eq!(old, current);
    }
}
#[test]
fn dense_real_signature_payload_savings_cover_complete_new_constructor_invoice() {
    let mut provider = String::from("module dense;\n");
    let mut app = String::from("module dense_app;\n");
    for i in 0..16 {
        app.push_str(&format!(
            "use function @id(\"dense.f{i}\") from dense as f{i};\n"
        ));
        provider.push_str(&format!("@id(\"dense.f{i}\") fn f{i}("));
        for p in 0..16 {
            if p != 0 {
                provider.push(',');
            }
            provider.push_str(&format!("p{p}:i64"));
        }
        provider.push_str(")->i64{42}\n");
    }
    app.push_str("@id(\"dense.main\") fn main()->i64{0}");
    let programs = [
        crate::parse(&app, "app.spx").unwrap(),
        crate::parse(&provider, "provider.spx").unwrap(),
    ];
    let authored = index_authored(&programs).unwrap();
    let observe = |current| {
        with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                let mut owner = Receiver::acquire().unwrap();
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
                    super::before_scalar_signature::construct_transient(
                        &programs[0],
                        &authored,
                        &programs,
                        &registry,
                        None,
                        &mut owner,
                    )
                }
                .unwrap();
                if current {
                    assert_eq!(
                        result
                            .functions
                            .entries
                            .iter()
                            .enumerate()
                            .filter(|(ordinal, _)| result
                                .function_refs()
                                .get(programs[0].functions.len() + ordinal)
                                .unwrap()
                                .is_signature())
                            .count(),
                        16
                    );
                }
                drop(result);
                drop(owner);
                drop(registry);
            })
        })
    };
    let (((), old_metrics), old_refused, old_paid, old_first) = observe(false);
    let (((), new_metrics), new_refused, new_paid, new_first) = observe(true);
    assert!(!old_refused && !new_refused && old_first.is_none() && new_first.is_none());
    for metrics in [old_metrics, new_metrics] {
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
    // This is a complete constructor invoice, including cold checks and real
    // directory capacity. It grants no full-resolver/application gain claim.
    assert!(
        new_paid < old_paid,
        "complete old={old_paid}, current={new_paid}"
    );
}
#[test]
fn broader_signatures_keep_owning_fallback_and_closed_provider_membership() {
    use crate::ast::program_source::function_alias::scalar_signature::ScalarSignature;
    let mut programs = programs();
    assert!(ScalarSignature::checked(&programs[2], &programs[1].functions[0], "left").is_none());
    programs[1].functions[0].return_type = Type::Bytes;
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_scalar_signature::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .err()
        .expect("the original Bytes-returning default is unsupported");
        let current = super::construct_transient(
            &programs[0],
            &authored,
            &programs,
            &registry,
            None,
            &mut owner,
        )
        .err()
        .expect("current fallback must retain the original refusal");
        let (Failure::One(old), Failure::One(current)) = (old, current) else {
            panic!("ordinary single default refusal required");
        };
        assert_eq!(old, current);
        assert_eq!(old.code, "SPX-G173");
        assert_eq!(old.severity, crate::diagnostic::Severity::Error);
        assert_eq!(
            old.message,
            "internal byte-data types have no synthesizable workspace default"
        );
        assert_eq!(old.path, None);
        assert_eq!(old.span, None);
        assert_eq!(old.help, None);
    });
    assert!(!refused && first.is_none());
    owning_parameter_fallback_keeps_real_success_and_complete_hir_parity();
}

fn owning_parameter_fallback_keeps_real_success_and_complete_hir_parity() {
    let mut programs = programs();
    programs[0] = crate::parse(
        &APP.replace("left()", "left(bytes_zeroed(0usize))"),
        "app.spx",
    )
    .unwrap();
    programs[1] = crate::parse("module left; @id(\"left.value\") fn left(input: own Bytes)->i64{if byte_len(input)==0usize{20}else{21}}", "left.spx").unwrap();
    let authored = index_authored(&programs).unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let registry = LayoutRegistry::new(&programs, &authored);
        let mut owner = Receiver::acquire().unwrap();
        let old = super::before_scalar_signature::construct_transient(
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
            current.functions.entries[0],
            FunctionEntry::Owned(_)
        ));
        for i in 0..old.functions().len() {
            assert!(current
                .function_refs()
                .get(i)
                .unwrap()
                .same_ordinary_declaration(old.functions().get(i).unwrap()));
        }
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
    });
    assert!(!refused && first.is_none());
}
