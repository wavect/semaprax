//! Actual affine receiver and typed constructor entry; old helpers stay callable.
use super::super::{Receiver, Setup};
use crate::ast::{Type, TypeDeclarationKind};
use crate::bounded_output::with_limit_usage_refusal;
use crate::diagnostic::Diagnostic;
use crate::test_allocation_observer::{current_metrics, measure};
use crate::workspace_graph::constructor_failure::{typed, typed::Failure};
use crate::workspace_graph::expected_projection::borrowed_imports::{
    constructor_test_default_expr_inline, constructor_test_inline_defaults,
    constructor_test_typed_defaults, constructor_test_typed_import_stub,
};
const LIMIT: usize = 8 * 1024 * 1024;
fn primary() -> Diagnostic {
    Diagnostic::io_static("SPX-G172", "selected semantic primary")
}
fn setup(failure: impl Into<Failure>) -> Setup {
    match failure.into() {
        Failure::Setup(failure) => failure,
        _ => panic!("expected inline setup refusal"),
    }
}
#[test]
fn typed_constructor_receiver_first_budget_and_foreign_restoration_preserve_original_owner() {
    let exact = Receiver::setup_bytes().unwrap();
    let ((), refused, _, first) = with_limit_usage_refusal(exact, || {
        let mut owner = Receiver::acquire().unwrap();
        let (((), physical), child_refused, child_used, child_first) =
            with_limit_usage_refusal(7, || {
                measure(|| {
                    assert_eq!(
                        setup(owner.reserve(usize::MAX).unwrap_err()),
                        Setup::Detached
                    );
                    assert_eq!(setup(owner.physical()), Setup::Detached);
                })
            });
        assert!(!child_refused && child_used == 0 && child_first.is_none());
        assert_eq!(physical.allocated, 0);
        let ((), physical) = measure(|| {
            assert_eq!(setup(owner.reserve(0).unwrap_err()), Setup::Detached);
            assert_eq!(setup(owner.physical()), Setup::Detached);
            drop(owner);
        });
        assert_eq!(physical.allocated, 0);
    });
    assert!(refused && first.is_some());
    let ((), refused, _, first) = with_limit_usage_refusal(exact, || {
        let mut owner = Receiver::acquire().unwrap();
        let first = setup(owner.reserve(1).unwrap_err());
        assert!(matches!(first, Setup::Budget { .. }));
        let (((), physical), child_refused, child_used, child_first) =
            with_limit_usage_refusal(7, || {
                measure(|| {
                    assert_eq!(setup(owner.reserve(0).unwrap_err()), first);
                    assert_eq!(setup(owner.physical()), first);
                })
            });
        assert!(!child_refused && child_used == 0 && child_first.is_none());
        assert_eq!(physical.allocated, 0);
        assert_eq!(setup(owner.reserve(0).unwrap_err()), first);
        drop(owner);
    });
    let first = first.unwrap();
    assert!(refused);
    assert_eq!(
        (first.requested, first.remaining, first.floor),
        (Receiver::reservation_controls() + 1, 0, 0)
    );
}
#[test]
fn typed_constructor_selected_semantic_and_many_keep_all_fields_pointer_and_drop() {
    let exact = Receiver::setup_bytes().unwrap();
    let (((), physical), refused, _, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let mut owner = Receiver::acquire().unwrap();
            let pointer = owner.report.as_ptr();
            assert!(owner.reserve(1).is_err());
            let stop = current_metrics().allocated;
            let report = owner
                .receive_typed(Failure::One(primary()))
                .into_legacy_unfunded();
            assert_eq!(report.as_ptr(), pointer);
            assert_eq!(report.as_slice(), [primary()].as_slice());
            drop(report);
            assert_eq!(current_metrics().allocated, stop);
        })
    });
    assert!(refused && first.is_some());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    let reports = vec![
        Diagnostic::io("SPX-G173", "first.λ\0🙂")
            .at_path("λ.spx")
            .with_help("dynamic help"),
        primary(),
    ];
    let expected = reports.clone();
    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let owner = Receiver::acquire().unwrap();
        let pointer = reports.as_ptr();
        let capacity = reports.capacity();
        let ((), physical) = measure(|| {
            let report = owner
                .receive_typed(Failure::Many(reports))
                .into_legacy_unfunded();
            assert_eq!(report, expected);
            assert_eq!(report.as_ptr(), pointer);
            assert_eq!(report.capacity(), capacity);
            drop(report);
        });
        assert_eq!(physical.allocated, 0);
    });
    assert!(!refused && first.is_none());
}
#[test]
fn typed_default_fields_match_complete_old_and_every_byte_refusal_has_no_later_allocation() {
    let mut program = crate::parse("module typed.defaults; @id(\"row\") record Row { @id(\"a\") a:i64,@id(\"b\") b:i64,@id(\"c\") c:i64,@id(\"d\") d:i64,@id(\"e\") e:i64,@id(\"f\") f:i64,@id(\"g\") g:i64,@id(\"h\") h:i64,@id(\"i\") i:i64, } @id(\"main\") fn main()->i64 {0}", "typed-defaults.spx").unwrap();
    let mut row = program.types.remove(0);
    let all = match &row.kind {
        TypeDeclarationKind::Record { fields } => fields.clone(),
        _ => unreachable!(),
    };
    let ty = Type::Named {
        name: "Row".to_owned(),
        arguments: vec![],
    };
    for width in [0, 1, 4, 5, 9] {
        if let TypeDeclarationKind::Record { fields } = &mut row.kind {
            *fields = all[..width].to_vec();
        }
        let declarations = [("Row", &row)];
        let old = constructor_test_inline_defaults::default_expr(&ty, &declarations, false).unwrap();
        let (((), physical), refused, exact, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut owner = Receiver::acquire().unwrap();
                let actual =
                    constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner).unwrap();
                assert_eq!(actual, old);
                drop(actual);
                drop(owner);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !physical.invalid
                && physical.live == 0
                && physical.allocated == physical.freed
                && physical.allocated <= exact
        );
        for limit in 0..=exact {
            let ((success, physical), refused, used, first) =
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut owner = match Receiver::acquire() {
                            Ok(owner) => owner,
                            Err(_) => return false,
                        };
                        let actual =
                            constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner);
                        let stop = current_metrics().allocated;
                        let success = actual.is_ok();
                        if let Err(error) = &actual {
                            assert!(matches!(error, Failure::Setup(Setup::Budget { .. })));
                        }
                        drop(actual);
                        drop(owner);
                        assert_eq!(current_metrics().allocated, stop);
                        success
                    })
                });
            assert_eq!(success, limit == exact);
            assert_eq!(refused, limit < exact);
            assert_eq!(first.is_some(), limit < exact);
            assert!(
                !physical.invalid
                    && physical.live == 0
                    && physical.allocated == physical.freed
                    && physical.allocated <= used
            );
        }
    }
}
#[test]
fn typed_static_defaults_and_stub_forwarding_keep_complete_old_six_fields_and_values() {
    let mut program = crate::parse(
        "module typed.stub; @id(\"main\") fn main()->i64 {0}",
        "typed-stub.spx",
    )
    .unwrap();
    for ty in [
        Type::Never,
        Type::Str,
        Type::SliceU8,
        Type::StringMap,
        Type::Bytes,
        Type::ArrayU8(4),
        Type::I64,
        Type::String,
    ] {
        program.functions[0].return_type = ty;
        let function = &program.functions[0];
        let old = constructor_test_default_expr_inline(function, &[], false, false);
        let (((), physical), refused, used, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut owner = Receiver::acquire().unwrap();
                let actual =
                    constructor_test_typed_import_stub::default_expr(function, &[], false, false, &mut owner);
                match (actual, &old) {
                    (Ok(actual), Ok(old)) => assert_eq!(&actual, old),
                    (Err(Failure::One(actual)), Err(old)) => {
                        assert_eq!(std::slice::from_ref(&actual), old.diagnostics())
                    }
                    _ => panic!("complete old stub differs"),
                }
                drop(owner);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !physical.invalid
                && physical.live == 0
                && physical.allocated == physical.freed
                && physical.allocated <= used
        );
    }
}
#[test]
fn typed_core_overlay_matches_complete_old_success_and_late_semantic_failure_through_drop() {
    use crate::workspace_graph::expected_projection::{CoreProviderProofs, private_layout};
    let mut programs = [
        "module typed.app; use function @id(\"helper\") from typed.provider as increment; @id(\"main\") fn main()->i64 {increment(41)}",
        "module typed.provider; @id(\"helper\") fn helper(value:i64)->i64 {value+1}",
    ].map(|s| crate::parse(s,"typed-core.spx").unwrap());
    for late in [false, true] {
        if late {
            programs[1].functions[0].return_type = Type::Str;
        }
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let registry = private_layout::registry::LayoutRegistry::new(&programs, &authored);
        // The full old receiver needs a live ordinary owner too.
        let (old, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
            crate::workspace_graph::constructor_failure::construct_transient_core(
                &programs[0],
                &authored,
                &programs,
                &registry,
                &mut core,
            )
        });
        assert!(!refused && first.is_none());
        let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
                let mut owner = Receiver::acquire().unwrap();
                let current = typed::construct_transient_core(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    &mut core,
                    &mut owner,
                );
                match (&current, &old) {
                    (Ok(actual), Ok(old)) => {
                        assert!(actual.source().types().iter().eq(old.source().types().iter()));
                        assert!(actual.source().functions().iter().eq(old.source().functions().iter()));
                    }
                    (Err(Failure::One(actual)), Err(old)) => {
                        assert_eq!(std::slice::from_ref(actual), old.diagnostics())
                    }
                    _ => panic!("full old core differs"),
                }
                drop(current);
                drop(owner);
                drop(core);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    }
}
#[test]
fn typed_helper_budget_reaches_original_same_child_prepaid_final_report_without_late_backing() {
    use crate::workspace_graph::constructor_failure::attempt;
    use crate::workspace_graph::diagnostics::{self, constructor_report_before as before};
    let _context = diagnostics::invocation_scope();
    let operation = || {
        attempt::run(usize::MAX, || {
            let mut owner = Receiver::acquire()?;
            match constructor_test_typed_defaults::default_expr(&Type::I64, &[], false, &mut owner) {
                Ok(value) => {
                    drop(value);
                    drop(owner);
                    Ok(())
                }
                Err(failure) => Err(owner.receive_typed(failure)),
            }
        })
    };
    let ((), refused, exact, first) = with_limit_usage_refusal(LIMIT, || drop(operation()));
    assert!(!refused && first.is_none());
    let ((), refused, _, first) = with_limit_usage_refusal(exact - 1, || drop(operation()));
    assert!(refused && first.is_some());
    let expected = before::live_builder_refusal(usize::MAX, first);
    let (((), physical), refused, used, repeated) = with_limit_usage_refusal(exact - 1, || {
        measure(|| {
            let attempt = operation();
            let stop = current_metrics().allocated;
            let (result, publication) = attempt.into_parts();
            assert!(matches!(
                result,
                Err(super::super::CoreFailure::Setup(Setup::Budget { .. }))
            ));
            let report = publication.expect("same original child sealed its held receipt");
            assert_eq!(report.as_slice(), std::slice::from_ref(&expected));
            drop(report);
            drop(result);
            assert_eq!(current_metrics().allocated, stop);
        })
    });
    assert!(refused && repeated == first);
    assert!(
        !physical.invalid
            && physical.live == 0
            && physical.allocated == physical.freed
            && physical.allocated <= used
    );
}

#[test]
fn typed_constructor_complete_sources_original_helpers_and_live_caller_are_exactly_bound() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-typed-propagation-825/source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (
            "docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/00.before"
            )),
        ),
        (
            "src/workspace_graph.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/01.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/02.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/attempt.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/attempt.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/03.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/receiving.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/04.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/report.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/report.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/05.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/borrowed_imports.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/borrowed_imports.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/06.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/defaults.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/defaults.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/07.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/import_stub.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/import_stub.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/08.before"
            )),
        ),
        (
            "tests/workspace/semantic_graph/source_inventory.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/workspace/semantic_graph/source_inventory.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/09.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/typed.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/receiving/typed.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/10.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/typed/tests.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/receiving/typed/tests.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/11.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/typed.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/typed.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/12.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/borrowed_imports/typed.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/borrowed_imports/typed.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/13.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/typed.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/defaults/typed.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/14.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/typed_fields.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/defaults/typed_fields.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/15.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/import_stub/typed.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/import_stub/typed.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/16.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/inline.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/defaults/inline.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/17.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/defaults/inline_fields.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/defaults/inline_fields.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/18.before"
            )),
        ),
        (
            "src/workspace_graph/expected_projection/borrowed_imports/inline.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/expected_projection/borrowed_imports/inline.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/19.before"
            )),
        ),
        (
            "src/workspace_graph/constructor_failure/receiving/before.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/workspace_graph/constructor_failure/receiving/before.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/constructor-typed-propagation-825/original/20.before"
            )),
        ),
    ];
    let records = packet.as_array().unwrap();
    assert_eq!(records.len(), sources.len());
    for (record, (path, current, original)) in records.iter().zip(sources) {
        let current = crate::source_reflows_merge::setup_typed_input(path, current);
        let current = current.as_str();
        assert_eq!(record["source"].as_str().unwrap(), path);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            record["current_sha256"].as_str().unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["original_sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for edit in record["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(restored.matches(new).count(), 1, "{path}");
            restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original, "{path}");
    }
    let caller = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/workspace_graph.rs"
    ));
    assert!(caller.contains("constructor_failure::typed::construct_transient_core("));
    assert!(caller.contains(".receive_typed(failure)"));
}
