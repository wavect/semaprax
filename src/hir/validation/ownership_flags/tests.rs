use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let owner = include_str!("../function_scope/tests.rs");
    let source = owner
        .split_once("const CONTRACT_SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    crate::hir::resolve(&crate::check(source, "ownership-copy.spx").unwrap()).unwrap()
}
fn compare(program: &ResolvedProgram, ty: &ResolvedType) {
    let current = HirValidator::new(program).unwrap();
    let old = HirValidator::new(program).unwrap();
    for ownership in [
        OwnershipMode::Value,
        OwnershipMode::Borrow,
        OwnershipMode::Own,
        OwnershipMode::Shared,
    ] {
        assert_eq!(
            current.expected_ownership(ty, ownership),
            old.expected_ownership_before(ty, ownership)
        );
        assert_eq!(
            current.is_owned_resource(ty, ownership),
            old.is_owned_resource_before(ty, ownership)
        );
    }
}
#[test]
fn complete_saved_copy_receivers_keep_all_modes_cache_precedence_and_corpus_types() {
    let program = program();
    for ty in [
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(257),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::I64),
        },
        ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes],
            result: Box::new(ResolvedType::Str),
        },
    ] {
        compare(&program, &ty);
    }
    for function in &program.functions {
        compare(&program, &function.return_type);
        compare(&program, &function.body.ty);
        for param in &function.params {
            compare(&program, &param.ty);
        }
    }
    let mut poisoned = program.clone();
    let mut facts = poisoned
        .declarations
        .type_facts(&ResolvedType::I64)
        .unwrap();
    facts.copy = !facts.copy;
    poisoned
        .declarations
        .type_facts_by_id
        .insert("i64".into(), facts);
    compare(&poisoned, &ResolvedType::I64);
}
#[test]
fn primitive_copy_queries_remove_actual_layout_acquisition_and_authenticate_sticky_zero() {
    let program = program();
    let current = HirValidator::new(&program).unwrap();
    let old = HirValidator::new(&program).unwrap();
    let (((), actual), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                for _ in 0..17 {
                    assert_eq!(
                        current
                            .expected_ownership(&ResolvedType::I64, OwnershipMode::Own)
                            .unwrap(),
                        OwnershipMode::Value
                    );
                    assert!(!current
                        .is_owned_resource(&ResolvedType::I64, OwnershipMode::Own)
                        .unwrap());
                }
            })
        });
    assert!(!refused && paid == 0 && first.is_none());
    assert_eq!(actual.allocations, 0);
    assert_eq!(actual.allocated, 0);
    assert!(!actual.invalid && actual.live == 0);
    let ((), saved) = measure(|| {
        for _ in 0..17 {
            assert_eq!(
                old.expected_ownership_before(&ResolvedType::I64, OwnershipMode::Own)
                    .unwrap(),
                OwnershipMode::Value
            );
            assert!(!old
                .is_owned_resource_before(&ResolvedType::I64, OwnershipMode::Own)
                .unwrap());
        }
    });
    assert!(saved.allocated > actual.allocated);
    assert!(!saved.invalid && saved.live == 0 && saved.allocated == saved.freed);
    let (((), sticky), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            measure(|| {
                assert!(current
                    .expected_ownership(&ResolvedType::I64, OwnershipMode::Own)
                    .is_err());
                assert!(current
                    .is_owned_resource(&ResolvedType::I64, OwnershipMode::Own)
                    .is_err());
            })
        });
    assert!(refused && paid == 0 && first.unwrap().requested == 1);
    assert_eq!(sticky.allocated, 0);
    assert!(!sticky.invalid && sticky.live == 0);
}
#[test]
fn original_dispatch_is_nested_unwind_safe_and_complete_methods_stay_frozen() {
    assert!(!original_selected());
    let payload: Box<dyn std::any::Any + Send> = Box::new("ownership control unwind");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _one = original_guard();
        {
            let _two = original_guard();
            assert!(original_selected());
        }
        assert!(original_selected());
        std::panic::resume_unwind(payload);
    }));
    assert!(result.is_err() && !original_selected());
    drop(result);
    let before = include_str!("before.rs");
    for (name, old) in [
        (
            "expected_ownership",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/expected_ownership.before.txt"
            )),
        ),
        (
            "is_owned_resource",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/is_owned_resource.before.txt"
            )),
        ),
    ] {
        assert!(before.contains(&old.replace(
            &format!("fn {name}("),
            &format!("pub(in crate::hir::validation) fn {name}_before(")
        )));
    }
}

#[test]
fn full_nominal_fixture_keeps_missing_generic_forged_cache_and_unknown_copy_routes() {
    let owner = include_str!("../nominal_facts/tests.rs");
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let mut program =
        crate::hir::resolve(&crate::parse(source, "ownership-nominal.spx").unwrap()).unwrap();
    let nominal = |id: &str, arguments| ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    };
    let missing = nominal("facts.record", Vec::new());
    program
        .declarations
        .type_facts_by_id
        .remove(&missing.identity_key());
    let empty_generic = nominal("facts.generic", Vec::new());
    let forged = program.declarations.type_facts(&ResolvedType::I64).unwrap();
    program
        .declarations
        .type_facts_by_id
        .insert(empty_generic.identity_key(), forged);
    for ty in [
        missing,
        empty_generic,
        nominal("facts.class", Vec::new()),
        nominal("facts.variant", Vec::new()),
        nominal("facts.owned", Vec::new()),
        nominal("facts.generic", vec![ResolvedType::I64]),
        nominal("facts.generic", vec![ResolvedType::U8]),
        nominal(crate::prelude::VEC_ID, vec![ResolvedType::I64]),
        nominal(crate::prelude::BOX_ID, vec![ResolvedType::U8]),
        nominal("facts.unknown", Vec::new()),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("facts.generic"),
            index: 0,
        },
    ] {
        compare(&program, &ty);
    }
}

#[test]
fn full_original_ownership_methods_and_all_hooked_sources_are_exactly_bound() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ownership-copy-flags/source-provenance.json"
    )))
    .unwrap();
    let sources = [
        (
            "src/hir/validation.rs",
            ci_str!("../../validation.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/validation.rs.before.txt"
            )),
            "9e878bead4d3d748764a55e10c51edd93146f1998d5eafd896205b31f1067ad6",
        ),
        (
            "src/hir/validation/nominal_facts.rs",
            ci_str!("../nominal_facts.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/nominal_facts.rs.before.txt"
            )),
            "f85363333cd6d13d361f1e41c967d7033d2934627d30758d8188b66ff6a9a873",
        ),
        (
            "src/hir/validation/identity_directory/oracle_entries.rs",
            ci_str!("../identity_directory/oracle_entries.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/oracle_entries.rs.before.txt"
            )),
            "3b42c48e7dd26991eb4e773952d69c7934118f2fea666ff8b687947e4fa6ece1",
        ),
        (
            "src/hir/validation/call_parameters.rs",
            ci_str!("../call_parameters.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/ownership-copy-flags/call_parameters.rs.before.txt"
            )),
            "3be63c3a568b23436a6e5897669266202ea71033003b963891de196094b9b4aa",
        ),
    ];
    for (path, current, original, digest) in sources {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            digest
        );
        assert_eq!(
            proof["original_sources"][path]["sha256"].as_str().unwrap(),
            digest
        );
        let mut projected = current.to_owned();
        for op in proof["full_current_projection"][path].as_array().unwrap() {
            let from = op["from"].as_str().unwrap();
            let to = op["to"].as_str().unwrap();
            assert_eq!(projected.matches(from).count(), 1, "{}", op["name"]);
            projected = projected.replace(from, to);
        }
        assert_eq!(projected, original);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                ci_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ownership-copy-flags/expected_ownership.before.txt"
                ))
                .as_bytes()
            ))
        ),
        "70d1af41ec7068192db12503d5db6ac92922ce6dcef36a635721844b30b7aac5"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                ci_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/ownership-copy-flags/is_owned_resource.before.txt"
                ))
                .as_bytes()
            ))
        ),
        "bffbf162d46f6c7e15339f00332f69018c07d44b738736ebb453b215ac70c5e4"
    );
}

#[test]
fn generic_copy_fallback_pays_complete_backing_drop_exact_short_and_sticky() {
    let owner = include_str!("../nominal_facts/tests.rs");
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let program =
        crate::hir::resolve(&crate::parse(source, "ownership-generic.spx").unwrap()).unwrap();
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("facts.generic"),
        arguments: vec![ResolvedType::I64],
    };
    let observe = |limit| {
        let validator = HirValidator::new(&program).unwrap();
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = validator.expected_ownership(&ty, OwnershipMode::Own);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = crate::bounded_output::active_remaining();
                    drop(validator.expected_ownership(&ty, OwnershipMode::Own));
                    drop(validator.is_owned_resource(&ty, OwnershipMode::Own));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, crate::bounded_output::active_remaining());
                } else {
                    assert_eq!(result.as_ref().unwrap(), &OwnershipMode::Value);
                }
                drop(result);
            })
        })
    };
    let (((), wide), refused, required, first) = observe(usize::MAX);
    assert!(!refused && first.is_none() && required > 0);
    assert!(!wide.invalid && wide.live == 0 && wide.allocated == wide.freed);
    assert!(wide.allocated <= required && wide.highwater <= required);
    let (((), exact), refused, used, first) = observe(required);
    assert!(!refused && used == required && first.is_none());
    assert_eq!(exact.allocated, wide.allocated);
    assert!(!exact.invalid && exact.live == 0 && exact.allocated == exact.freed);
    for limit in [0, required - 1] {
        let (((), short), refused, used, first) = observe(limit);
        assert!(refused && first.is_some() && used <= limit);
        assert!(!short.invalid && short.live == 0 && short.allocated == short.freed);
        assert!(short.allocated <= used && short.highwater <= used);
    }
    let old = HirValidator::new(&program).unwrap();
    let ((), physical) = measure(|| {
        assert_eq!(
            old.expected_ownership_before(&ty, OwnershipMode::Own)
                .unwrap(),
            OwnershipMode::Value
        )
    });
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}

#[test]
fn clone_payer_literal_backing_cursor_once_exact_short_and_sticky_drop() {
    use crate::cleanup_plan::ReplayClonePayload;
    use std::mem::size_of;
    let ty = ResolvedType::Function {
        parameters: vec![
            ResolvedType::Nominal {
                declaration: DeclarationId::new("copy:é\0.args"),
                arguments: vec![ResolvedType::I64, ResolvedType::Bool],
            },
            ResolvedType::I64,
        ],
        result: Box::new(ResolvedType::Nominal {
            declaration: DeclarationId::new("copy:é\0.result"),
            arguments: Vec::new(),
        }),
    };
    let payload = 5 * size_of::<ResolvedType>();
    assert_eq!(ty.clone_payload_bytes(), Some(payload));
    let (_, old) = measure(|| {
        let copy = ty.clone();
        assert_eq!(copy, ty);
        drop(copy);
    });
    assert!(!old.invalid && old.live == 0);
    assert_eq!(old.allocated, payload);
    assert_eq!(old.freed, payload);
    let setup = size_of::<Payer>() + 514 * size_of::<Option<(&ResolvedType, usize)>>();
    let exact = setup + 2 * payload;
    let observe = |limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut payer = match Payer::new() {
                    Ok(payer) => payer,
                    Err(error) => {
                        drop(error);
                        return false;
                    }
                };
                let result = (|| {
                    let first = payer.clone_type(&ty)?;
                    assert_eq!(first, ty);
                    let remaining = crate::bounded_output::active_remaining().unwrap();
                    let second = payer.clone_type(&ty)?;
                    assert_eq!(second, ty);
                    assert_eq!(
                        remaining - crate::bounded_output::active_remaining().unwrap(),
                        payload
                    );
                    drop((first, second));
                    Ok::<_, Diagnostic>(())
                })();
                if let Err(error) = result {
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = crate::bounded_output::active_remaining();
                    let retry = payer.clone_type(&ResolvedType::I64).unwrap_err();
                    assert_eq!(retry.code, "SPX-H006");
                    assert_eq!(remaining, crate::bounded_output::active_remaining());
                    assert_eq!(before, crate::test_allocation_observer::current_metrics());
                    drop((error, retry));
                    false
                } else {
                    true
                }
            })
        })
    };
    let ((ok, wide), refused, paid, first) = observe(usize::MAX);
    assert!(ok && !refused && first.is_none());
    assert_eq!(paid, exact);
    assert_eq!(wide.allocated, 2 * payload);
    assert_eq!(wide.allocated, wide.freed);
    assert!(!wide.invalid && wide.live == 0);
    let ((ok, same), refused, paid, first) = observe(exact);
    assert!(ok && !refused && first.is_none() && paid == exact);
    assert_eq!(same, wide);
    for limit in [0, setup - 1, setup + payload - 1, exact - 1] {
        let ((ok, short), refused, paid, first) = observe(limit);
        assert!(!ok && refused && first.is_some() && paid <= limit);
        assert!(!short.invalid && short.live == 0 && short.allocated <= paid);
        assert_eq!(short.allocated, short.freed);
    }
}

#[test]
fn clone_payer_keeps_deep_census_boundary_and_failed_clone_sticky() {
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    for depth in [1, 32, 514, 515] {
        let mut ty = ResolvedType::I64;
        for _ in 1..depth {
            ty = ResolvedType::Nominal {
                declaration: DeclarationId::new("deep:é\0.copy"),
                arguments: vec![ty],
            };
        }
        let ((accepted, metrics), refused, paid, receipt) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut payer = Payer::new().unwrap();
                    let result = payer.clone_type(&ty);
                    let accepted = result.is_ok();
                    if depth <= 514 {
                        assert_eq!(result.as_ref().unwrap(), &ty);
                    } else {
                        let error = result.as_ref().unwrap_err();
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.message, "ownership type flags layout overflowed");
                        let before = crate::test_allocation_observer::current_metrics();
                        drop(payer.clone_type(&ResolvedType::I64).unwrap_err());
                        assert_eq!(before, crate::test_allocation_observer::current_metrics());
                    }
                    drop(result);
                    accepted
                })
            });
        assert_eq!(accepted, depth <= 514);
        assert_eq!(refused, depth > 514);
        assert_eq!(receipt.is_some(), depth > 514);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated <= paid);
        assert_eq!(metrics.allocated, metrics.freed);
        if depth > 514 {
            assert_eq!(paid, std::mem::size_of::<Payer>() + cursor);
            assert_eq!(metrics.allocated, 0);
            assert_eq!(receipt.unwrap().requested, usize::MAX);
        }
    }
}

#[test]
fn lazy_flags_payer_keeps_zero_limit_primitives_and_full_forged_cache_precedence() {
    let mut index = DeclarationIndex::default();
    let types = [
        ResolvedType::I64,
        ResolvedType::ArrayU8(12),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::Bool),
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("cache:é\0.copy"),
            arguments: vec![ResolvedType::I64],
        },
    ];
    for ty in &types {
        index.type_facts_by_id.insert(
            ty.identity_key(),
            TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "literal-hostile-cache-layout".to_owned(),
            },
        );
    }
    for ty in &types {
        let old = index.type_flags(ty);
        let ((result, metrics), refused, paid, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                measure(|| {
                    index
                        .type_flags_paid_lazy(
                            ty,
                            Payer::authenticate,
                            || -> Result<Payer, Diagnostic> {
                                panic!("fast receiver must not construct a payer")
                            },
                        )
                        .unwrap()
                })
            });
        assert_eq!(result, old);
        assert!(!refused && first.is_none() && paid == 0);
        assert_eq!(metrics, crate::test_allocation_observer::Metrics::default());
    }
    let unknown = ResolvedType::Nominal {
        declaration: DeclarationId::new("unknown:é\0.copy"),
        arguments: vec![ResolvedType::I64],
    };
    let ((result, metrics), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| index.type_flags_paid_lazy(&unknown, Payer::authenticate, Payer::new))
        });
    assert!(result.is_err() && refused && paid == 0);
    assert_eq!(first.unwrap().requested, std::mem::size_of::<Payer>());
    assert_eq!(metrics, crate::test_allocation_observer::Metrics::default());
}

#[test]
fn complete_original_paid_entry_and_lazy_fast_gates_stay_source_exact() {
    use sha2::{Digest, Sha256};
    let original = ci_str!("../../../../docs/evidence/ownership-clone-census/paid.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "12f1218a6606519c1facdfb622abee4fd9eb6e2a35e2c2a1a91c91cff7343d86"
    );
    let current = ci_str!("../../declaration_index/flags/paid.rs");
    let start = current
        .find("    /// Invocation-local lazy state:")
        .unwrap();
    let end = current[start..]
        .find("    // Keeping this constructor out of the fast receiving frame")
        .unwrap()
        + start;
    let mut projected = format!("{}{}", &current[..start], &current[end..]);
    // Later nominal inventory receivers add registrations only. Keep their
    // exact text bound before replaying the complete earlier paid receiver.
    for registration in [
        "#[path = \"../nominal_paid_facts.rs\"]\nmod nominal_constructor;\n",
        "\nmod inventory_constructor;\npub(crate) use inventory_constructor::{\n    identity as inventory_identity, substitute as inventory_substitute, InventoryPayer,\n};\n",
    ] {
        assert_eq!(projected.matches(registration).count(), 1);
        projected = projected.replacen(registration, "", 1);
    }
    assert_eq!(projected, original);
    let gates = |body: &str| {
        let start = body.find("        if matches!(\n").unwrap();
        let end = body[start..]
            .find("        self.type_flags_paid_construct")
            .unwrap()
            + start;
        body[start..end].to_owned()
    };
    assert_eq!(gates(&current[start..end]), gates(original));
}

#[test]
fn leaf_clone_reuses_held_identities_without_a_census_cursor_and_keeps_sticky_zero() {
    let leaves = [
        ResolvedType::Never,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
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
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("leaf:é\0.owner"),
            index: u32::MAX,
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("leaf:é\0.nominal"),
            arguments: Vec::new(),
        },
    ];
    let observe = |original, limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = (|| {
                    let mut payer = Payer::new()?;
                    for ty in &leaves {
                        let value = if original {
                            payer.clone_type_before(ty)?
                        } else {
                            payer.clone_type(ty)?
                        };
                        assert_eq!(&value, ty);
                        drop(value);
                    }
                    Ok::<_, Diagnostic>(())
                })();
                if result.is_err() {
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = crate::bounded_output::active_remaining();
                    assert!(Payer::authenticate().is_err());
                    assert_eq!(remaining, crate::bounded_output::active_remaining());
                    assert_eq!(before, crate::test_allocation_observer::current_metrics());
                }
                let accepted = result.is_ok();
                drop(result);
                accepted
            })
        })
    };
    let ((old_ok, old), old_refused, old_paid, old_first) = observe(true, usize::MAX);
    let ((ok, current), refused, paid, first) = observe(false, usize::MAX);
    assert!(old_ok && ok && !old_refused && !refused && old_first.is_none() && first.is_none());
    assert_eq!(
        old_paid - paid,
        514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>()
    );
    assert_eq!(paid, std::mem::size_of::<Payer>());
    assert_eq!(old, current);
    assert!(!current.invalid && current.live == 0 && current.allocated == 0 && current.freed == 0);
    let ((old_ok, old), refused, old_paid, first) = observe(true, paid);
    assert!(!old_ok && refused && old_paid == paid);
    assert_eq!(
        first.unwrap().requested,
        514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>()
    );
    assert!(!old.invalid && old.live == 0 && old.allocated == 0);
    for limit in [0, paid - 1, paid] {
        let ((ok, allocation), refused, used, first) = observe(false, limit);
        assert_eq!(ok, limit == paid);
        assert_eq!(refused, limit != paid);
        assert_eq!(first.is_some(), limit != paid);
        assert!(used <= limit && !allocation.invalid && allocation.live == 0);
        assert_eq!((allocation.allocated, allocation.freed), (0, 0));
    }
}

#[test]
fn complete_clone_predecessor_and_recursive_census_fallback_remain_source_bound() {
    use sha2::{Digest, Sha256};
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ownership-leaf-clone-v1/function.json"
    )))
    .unwrap();
    let body = record["body"].as_str().unwrap();
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
    );
    assert_eq!(digest, record["sha256"].as_str().unwrap());
    assert_eq!(
        digest,
        "193c8dc2803409158e0dbeb3d133ab6651eb2568a40e277d1396d913a90e9438"
    );
    let archived = body.replacen(
        "    fn clone_type(",
        "    pub(super) fn clone_type_before(",
        1,
    );
    assert_eq!(include_str!("before.rs").matches(&archived).count(), 1);
    let current = include_str!("../ownership_flags.rs");
    let hook="        // These variants own no recursive backing. Identity clones retain\n        // their existing Arc, and an empty argument vector clones to no buffer.\n        // Avoid calling the fixed-cursor census at all on this exact shape.\n        if match ty {\n            ResolvedType::Function { .. } => false,\n            ResolvedType::Nominal { arguments, .. } => arguments.is_empty(),\n            ResolvedType::Never\n            | ResolvedType::OnceFunction\n            | ResolvedType::OnceFunctionI64\n            | ResolvedType::OnceFunctionI64Pair\n            | ResolvedType::MutFunctionI64\n            | ResolvedType::Unit\n            | ResolvedType::I64\n            | ResolvedType::I32\n            | ResolvedType::Char\n            | ResolvedType::U8\n            | ResolvedType::Usize\n            | ResolvedType::ArrayU8(_)\n            | ResolvedType::F32\n            | ResolvedType::F64\n            | ResolvedType::Bool\n            | ResolvedType::String\n            | ResolvedType::Bytes\n            | ResolvedType::Str\n            | ResolvedType::SliceU8\n            | ResolvedType::StringMap\n            | ResolvedType::TypeParameter { .. } => true,\n        } {\n            return Ok(ty.clone());\n        }\n";
    let updated = body.replacen(
        "        Self::authenticate()?;\n",
        &format!("        Self::authenticate()?;\n{hook}"),
        1,
    );
    assert_eq!(current.matches(&updated).count(), 1);
}

#[test]
fn a_leaf_does_not_mark_the_later_recursive_clone_cursor_as_paid() {
    let nested = ResolvedType::Function {
        parameters: vec![ResolvedType::I64],
        result: Box::new(ResolvedType::Bool),
    };
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    let payload = 2 * std::mem::size_of::<ResolvedType>();
    for original in [false, true] {
        let (((), allocation), refused, paid, first) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut payer = Payer::new().unwrap();
                    let leaf = if original {
                        payer.clone_type_before(&ResolvedType::I64)
                    } else {
                        payer.clone_type(&ResolvedType::I64)
                    }
                    .unwrap();
                    assert_eq!(leaf, ResolvedType::I64);
                    drop(leaf);
                    let before = crate::bounded_output::active_remaining().unwrap();
                    let first = if original {
                        payer.clone_type_before(&nested)
                    } else {
                        payer.clone_type(&nested)
                    }
                    .unwrap();
                    assert_eq!(first, nested);
                    let remaining = crate::bounded_output::active_remaining().unwrap();
                    assert_eq!(
                        before - remaining,
                        payload + if original { 0 } else { cursor }
                    );
                    let second = if original {
                        payer.clone_type_before(&nested)
                    } else {
                        payer.clone_type(&nested)
                    }
                    .unwrap();
                    assert_eq!(second, nested);
                    assert_eq!(
                        remaining - crate::bounded_output::active_remaining().unwrap(),
                        payload
                    );
                    drop((first, second));
                })
            });
        assert!(!refused && first.is_none());
        assert_eq!(paid, std::mem::size_of::<Payer>() + cursor + 2 * payload);
        assert_eq!(
            (allocation.allocated, allocation.freed),
            (2 * payload, 2 * payload)
        );
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= paid);
    }
}
