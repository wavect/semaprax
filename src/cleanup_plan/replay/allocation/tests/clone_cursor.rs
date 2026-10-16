//! Ordinary flat clones pay their real backing, never an unused type cursor.
use super::*;
use crate::bounded_output::set_active_floor;

fn flat<T: ClonePayload + Clone + std::fmt::Debug + PartialEq>(
    function: &ResolvedFunction,
    value: &T,
    payload: usize,
) {
    assert!(!T::NEEDS_TYPE_CURSOR);
    assert_eq!(value.clone_payload_bytes(), Some(payload));
    let text = function.id.as_str().len() + 512;
    for floor in [0, 37] {
        for available in [0, text - 1, text + payload, text + payload + 1] {
            let (((), metrics), refused, used, receipt) =
                with_limit_usage_refusal(available + floor, || {
                    assert!(set_active_floor(floor));
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let result = budget.reserve_clone(function, value, "flat payload");
                        assert!(!budget.builder_clone_cursor_paid);
                        assert!(!budget.selected() && budget.physical.is_none());
                        if available >= text + payload {
                            result.unwrap();
                            let copy = value.clone();
                            assert_eq!(&copy, value);
                            drop(copy);
                        } else {
                            let error = result.unwrap_err();
                            assert!(!error.code.is_empty());
                            let before = current_metrics();
                            assert!(budget.reserve_clone(function, value, "retry").is_err());
                            assert_eq!(current_metrics(), before);
                            drop(error);
                        }
                        drop(budget);
                    })
                });
            assert!(!metrics.invalid);
            assert_eq!(metrics.allocated, metrics.freed);
            assert_eq!(metrics.live, 0);
            assert_eq!(refused, available < text + payload);
            if refused {
                assert_eq!(used, 0);
                assert_eq!(metrics.allocated, 0);
                let receipt = receipt.unwrap();
                assert_eq!(receipt.floor, floor);
                assert_eq!(receipt.requested, text);
            } else {
                assert!(receipt.is_none());
                assert_eq!(used, text + payload);
                assert_eq!(metrics.allocated, text + payload);
            }
        }
    }
    if payload != 0 {
        let (((), metrics), refused, used, receipt) =
            with_limit_usage_refusal(text + payload - 1, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let error = budget
                        .reserve_clone(function, value, "short payload")
                        .unwrap_err();
                    assert!(!budget.builder_clone_cursor_paid);
                    let before = current_metrics();
                    assert!(budget.reserve_clone(function, value, "retry").is_err());
                    assert_eq!(current_metrics(), before);
                    drop((error, budget));
                })
            });
        assert!(refused && !metrics.invalid);
        assert_eq!(receipt.unwrap().requested, payload);
        assert_eq!(used, text);
        assert_eq!(metrics.allocated, text);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
    }
}

#[test]
fn ordinary_flat_clone_exact_zero_short_and_floor_cover_real_owned_backing() {
    let program = program();
    let function = &program.functions[0];
    let shared = DeclarationId::new("cursor.shared.é");
    flat(function, &shared, 0);
    let instance = FunctionInstanceId::derive(&shared, &[ResolvedType::I64]);
    let bytes = instance.as_str().len();
    flat(function, &instance, bytes);
    let pairs = vec![(Some(shared.clone()), instance.clone()); 3];
    flat(
        function,
        &pairs,
        pairs.len() * std::mem::size_of_val(&pairs[0]) + 3 * bytes,
    );
    let borrowed = std::rc::Rc::new(ResolvedType::Function {
        parameters: vec![ResolvedType::I64; 3],
        result: Box::new(ResolvedType::I64),
    });
    flat(function, &borrowed, 0);
    assert_eq!(std::rc::Rc::strong_count(&borrowed), 1);
}

// Keep the earlier scalar census transition through the full before producer.
// The current scalar-to-compound transition below pays real recursive storage.
#[test]
fn flat_clone_does_not_prepay_or_suppress_a_later_recursive_cursor() {
    let program = program();
    let function = &program.functions[0];
    let text = function.id.as_str().len() + 512;
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    for available in [text + cursor - 1, text + cursor] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(available, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .reserve_clone(function, &function.id, "shared first")
                    .unwrap();
                assert!(!budget.builder_clone_cursor_paid);
                let result =
                    budget.reserve_clone_before(function, &ResolvedType::I64, "recursive next");
                if available == text + cursor {
                    result.unwrap();
                    assert!(budget.builder_clone_cursor_paid);
                    let before = current_metrics();
                    budget
                        .reserve_clone_before(function, &ResolvedType::I64, "reuse")
                        .unwrap();
                    assert_eq!(current_metrics(), before);
                } else {
                    let error = result.unwrap_err();
                    assert!(!budget.builder_clone_cursor_paid);
                    let before = current_metrics();
                    assert!(budget
                        .reserve_clone(function, &function.id, "flat retry")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    drop(error);
                }
                drop(budget);
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocated, text);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
        assert_eq!(refused, available < text + cursor);
        if refused {
            assert_eq!(used, text);
            assert_eq!(receipt.unwrap().requested, cursor);
        } else {
            assert_eq!(used, text + cursor);
            assert!(receipt.is_none());
        }
    }
}

#[test]
fn cursor_trait_defaults_and_type_containers_remain_conservative() {
    struct FuturePayload;
    impl ClonePayload for FuturePayload {
        fn clone_payload_bytes(&self) -> Option<usize> {
            Some(0)
        }
    }
    fn classified<T: ClonePayload>(expected: bool) {
        assert_eq!(T::NEEDS_TYPE_CURSOR, expected);
    }
    classified::<FuturePayload>(true);
    classified::<ResolvedType>(true);
    classified::<StagedCopyResultSource>(true);
    classified::<Option<ResolvedType>>(true);
    classified::<Vec<ResolvedType>>(true);
    classified::<(DeclarationId, ResolvedType)>(true);
    classified::<(ResolvedType, DeclarationId)>(true);
    classified::<CleanupPlace>(false);
    classified::<crate::hir::ControlTarget>(false);
    classified::<ExprSkeletonPath>(false);
    classified::<SkeletonPath>(false);
}

mod before;

#[test]
fn recursive_clone_keeps_complete_original_verdict_and_debit_frontiers() {
    let program = program();
    let function = &program.functions[0];
    let text = function.id.as_str().len() + 512;
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    let value = ResolvedType::Function {
        parameters: vec![ResolvedType::I64, ResolvedType::String],
        result: Box::new(ResolvedType::Bytes),
    };
    let payload = 3 * std::mem::size_of::<ResolvedType>();
    assert_eq!(value.clone_payload_bytes(), Some(payload));
    for limit in [
        0,
        text - 1,
        text + cursor - 1,
        text + cursor,
        text + cursor + payload - 1,
        text + cursor + payload,
    ] {
        let run = |old: bool| {
            with_limit_usage_refusal(limit, || {
                let mut budget = ReplayBudget::new();
                let result = if old {
                    budget.reserve_clone_before(function, &value, "complete recursive")
                } else {
                    budget.reserve_clone(function, &value, "complete recursive")
                };
                let cursor_paid = budget.builder_clone_cursor_paid;
                let remaining = budget.remaining;
                let retry = if old {
                    budget.reserve_clone_before(function, &value, "recursive retry")
                } else {
                    budget.reserve_clone(function, &value, "recursive retry")
                };
                (result, retry, cursor_paid, remaining)
            })
        };
        assert_eq!(run(false), run(true));
    }
}

#[test]
fn complete_clone_sources_restore_the_original_bodies_and_recursive_control() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/replay-clone-cursor-v1");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let path = record["path"].as_str().unwrap();
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        let expected = record["sha256"].as_str().unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            expected
        );
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restore = |source: &str| -> Option<String> {
            let mut restored = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if restored.matches(new).count() != 1 {
                    return None;
                }
                restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(restored)
        };
        assert_eq!(
            restore(&current).as_deref(),
            Some(original.as_str()),
            "{path}"
        );
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen("clone_payload_bytes", "changed_clone_payload_bytes", 1))
                .as_deref(),
            Some(original.as_str())
        );
        if path.ends_with("physical_budget.rs") {
            let start = original
                .find("    #[track_caller]\n    pub(super) fn reserve_clone")
                .unwrap();
            let end = start + original[start + 1..].find("\n    #[track_caller]").unwrap() + 1;
            let method = original[start..end].strip_suffix('\n').unwrap();
            let expected_control = format!(
                "use super::*;\n\nimpl ReplayBudget {{\n{}\n}}\n",
                method.replace("fn reserve_clone<", "fn reserve_clone_before<")
            );
            assert_eq!(include_str!("clone_cursor/before.rs"), expected_control);
        }
    }
}

// Historical scalar census control. Its complete implementation remains bound
// by the earlier whole-source archive; current leaf clones use the tests below.
pub(super) fn before_scalar_clone(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    value: &ResolvedType,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.reserve_clone_before(function, value, phase)
}

#[test]
fn ordinary_scalar_types_clone_without_cursor_and_keep_all_backing_authenticated() {
    let program = program();
    let function = &program.functions[0];
    let types = [
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::ArrayU8(u32::MAX),
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::StringMap,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
        ResolvedType::TypeParameter {
            owner: function.id.clone(),
            index: u32::MAX,
        },
    ];
    let text = function.id.as_str().len() + 512;
    for value in &types {
        assert!(!value.clone_needs_type_cursor());
        assert_eq!(value.clone_payload_bytes(), Some(0), "complete old census");
        assert_eq!(value.clone_payload_bytes_without_type_cursor(), Some(0));
        for floor in [0, 37] {
            for available in [0, text - 1, text, text + 1] {
                let (((), metrics), refused, used, receipt) =
                    with_limit_usage_refusal(available + floor, || {
                        assert!(set_active_floor(floor));
                        measure(|| {
                            let mut budget = ReplayBudget::new();
                            let result = budget.reserve_clone(function, value, "scalar type");
                            assert!(!budget.builder_clone_cursor_paid);
                            if available >= text {
                                result.unwrap();
                                let before = current_metrics();
                                let copy = value.clone();
                                assert_eq!(&copy, value);
                                drop(copy);
                                budget
                                    .reserve_clone(function, value, "scalar reuse")
                                    .unwrap();
                                assert_eq!(current_metrics(), before);
                            } else {
                                let error = result.unwrap_err();
                                assert!(!error.code.is_empty());
                                let before = current_metrics();
                                assert!(budget
                                    .reserve_clone(function, value, "scalar retry")
                                    .is_err());
                                assert_eq!(current_metrics(), before);
                                drop(error);
                            }
                            drop(budget);
                        })
                    });
                assert!(!metrics.invalid);
                assert_eq!(metrics.live, 0);
                assert_eq!(metrics.allocated, metrics.freed);
                assert_eq!(refused, available < text);
                if refused {
                    assert_eq!(used, 0);
                    assert_eq!(metrics.allocated, 0);
                    let receipt = receipt.unwrap();
                    assert_eq!(receipt.floor, floor);
                    assert_eq!(receipt.requested, text);
                } else {
                    assert_eq!(used, text);
                    assert_eq!(metrics.allocated, text);
                    assert!(receipt.is_none());
                }
            }
        }
    }
}

#[test]
fn scalar_clone_then_compound_pays_the_full_original_cursor_and_owned_payload() {
    let program = program();
    let function = &program.functions[0];
    let value = ResolvedType::Function {
        parameters: vec![ResolvedType::I64, ResolvedType::String],
        result: Box::new(ResolvedType::Bytes),
    };
    assert!(value.clone_needs_type_cursor());
    assert_eq!(value.clone_payload_bytes_without_type_cursor(), None);
    let text = function.id.as_str().len() + 512;
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    let payload = 3 * std::mem::size_of::<ResolvedType>();
    for available in [
        text + cursor - 1,
        text + cursor + payload - 1,
        text + cursor + payload,
    ] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(available, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .reserve_clone(function, &ResolvedType::I64, "leaf first")
                    .unwrap();
                assert!(!budget.builder_clone_cursor_paid);
                let result = budget.reserve_clone(function, &value, "compound next");
                assert_eq!(budget.builder_clone_cursor_paid, available >= text + cursor);
                if available == text + cursor + payload {
                    result.unwrap();
                    let copy = value.clone();
                    assert_eq!(copy, value);
                    drop(copy);
                } else {
                    let error = result.unwrap_err();
                    let before = current_metrics();
                    assert!(budget
                        .reserve_clone(function, &ResolvedType::I64, "sticky leaf retry")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    drop(error);
                }
                drop(budget);
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, available < text + cursor + payload);
        if available < text + cursor {
            assert_eq!(used, text);
            assert_eq!(receipt.unwrap().requested, cursor);
            assert_eq!(metrics.allocated, text);
        } else if refused {
            assert_eq!(used, text + cursor);
            assert_eq!(receipt.unwrap().requested, payload);
            assert_eq!(metrics.allocated, text);
        } else {
            assert_eq!(used, text + cursor + payload);
            assert!(receipt.is_none());
            assert_eq!(metrics.allocated, text + payload);
        }
    }
}

#[test]
fn scalar_cursor_complete_sources_and_original_census_remain_bound() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/scalar-clone-cursor-v1");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let restore = |source: &str| -> Option<String> {
            let mut restored = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if restored.matches(new).count() != 1 {
                    return None;
                }
                restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(restored)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
    fn census(source: &str) -> &str {
        let implementation = source
            .split("impl ClonePayload for ResolvedType {")
            .nth(1)
            .unwrap();
        let start = implementation
            .find("    fn clone_payload_bytes(&self)")
            .unwrap();
        implementation[start..]
            .split("impl ClonePayload for StagedCopyResultSource")
            .next()
            .unwrap()
    }
    let original = std::fs::read_to_string(evidence.join("0.before.txt")).unwrap();
    let current =
        std::fs::read_to_string(root.join("src/cleanup_plan/replay/physical_clone.rs")).unwrap();
    assert_eq!(
        census(&current),
        census(&original),
        "complete original ordinary and oracle censuses remain compiled"
    );
}
