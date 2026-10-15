//! Full previous collectors, exact path membership and paid receiving destruction.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};
const LIMIT: usize = 64 * 1024 * 1024;

fn root(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}
fn program(depth: usize) -> ResolvedProgram {
    let mut source = crate::hir::iterative_validator_tests::SOURCE.to_owned();
    for n in 0..depth {
        source.push_str(&format!("\n@id(\"inline.row.{n}\") record InlineRow{n} {{ @id(\"inline.row.{n}.value\") value:i64, }}\n"));
    }
    let mut program =
        crate::hir::resolve(&crate::check(&source, "inline-lifecycle.spx").unwrap()).unwrap();
    for n in 0..depth.saturating_sub(1) {
        let item = program
            .types
            .iter_mut()
            .find(|item| item.id.as_str() == format!("inline.row.{n}"))
            .unwrap();
        let ResolvedTypeDeclarationKind::Record { fields } = &mut item.kind else {
            unreachable!()
        };
        fields[0].ty = root(&format!("inline.row.{}", n + 1));
    }
    program
}
fn fields(program: &ResolvedProgram, targets: &[ResolvedType]) -> Vec<ResolvedFieldDeclaration> {
    let item = program
        .types
        .iter()
        .find(|item| item.id.as_str() == "inline.row.0")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
        unreachable!()
    };
    targets
        .iter()
        .map(|ty| {
            let mut field = fields[0].clone();
            field.ty = ty.clone();
            field
        })
        .collect()
}
fn install(program: &mut ResolvedProgram, kind: ResolvedTypeDeclarationKind) {
    program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "inline.row.0")
        .unwrap()
        .kind = kind;
}
fn imported(program: &mut ResolvedProgram) {
    let item = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "token.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut item.kind else {
        unreachable!()
    };
    drop.kind = ResolvedResourceDropKind::Imported {
        import: DeclarationId::new("host.echo"),
        import_key: "host.echo".into(),
    };
    let item = program
        .interfaces
        .iter_mut()
        .flat_map(|interface| &mut interface.imports)
        .find(|item| item.id.as_str() == "host.echo")
        .unwrap();
    item.effects = vec![
        "z.effect".into(),
        "α.effect".into(),
        "z.effect".into(),
        "a.effect".into(),
    ];
}

#[test]
fn complete_before_collector_matches_depths_spill_cycles_arguments_and_errors() {
    for depth in [0, 1, 15, 16, 17, 33] {
        let mut program = program(depth);
        let ty = if depth == 0 {
            ResolvedType::I64
        } else {
            root("inline.row.0")
        };
        if depth > 0 {
            let last = program
                .types
                .iter_mut()
                .find(|item| item.id.as_str() == format!("inline.row.{}", depth - 1))
                .unwrap();
            let ResolvedTypeDeclarationKind::Record { fields } = &mut last.kind else {
                unreachable!()
            };
            fields[0].ty = ty.clone();
        }
        let mut current = super::super::Collector::new(&program);
        let mut old = before_inline::Collector::new(&program);
        for _ in 0..3 {
            assert_eq!(current.extend(&ty), old.extend(&ty));
            assert!(current.iter().eq(old.iter()));
        }
        for ty in [
            root("missing.root"),
            ResolvedType::String,
            ResolvedType::Nominal {
                declaration: DeclarationId::new("inline.row.0"),
                arguments: vec![ResolvedType::I64],
            },
        ] {
            assert_eq!(current.extend(&ty), old.extend(&ty));
            assert!(current.iter().eq(old.iter()));
        }
    }
}

#[test]
fn record_class_variant_and_partial_first_diagnostics_keep_full_original_order() {
    for shape in 0..3 {
        for missing in [false, true] {
            let mut program = program(17);
            imported(&mut program);
            // A branch retains a completed effect before the first missing type;
            // an active root cycle must close before the later missing branch.
            let targets = [
                root("token.type"),
                root("inline.row.0"),
                root(if missing {
                    "missing.first.α\0"
                } else {
                    "inline.row.1"
                }),
                root("missing.second"),
            ];
            let ordered = fields(&program, &targets);
            let kind = match shape {
                0 => ResolvedTypeDeclarationKind::Record {
                    fields: ordered.into(),
                },
                1 => ResolvedTypeDeclarationKind::Class {
                    fields: ordered.into(),
                    methods: Vec::new(),
                },
                _ => ResolvedTypeDeclarationKind::Variant {
                    cases: vec![ResolvedVariantCaseDeclaration {
                        id: DeclarationId::new("inline.case"),
                        name: "Case".into(),
                        index: 0,
                        fields: ordered,
                        span: Span::default(),
                    }]
                    .into(),
                },
            };
            install(&mut program, kind);
            let ty = root("inline.row.0");
            let mut current = super::super::Collector::new(&program);
            let mut old = before_inline::Collector::new(&program);
            for _ in 0..2 {
                let expected = old.extend(&ty);
                let actual = current.extend(&ty);
                assert_eq!(actual, expected);
                assert!(current.iter().eq(old.iter()));
                assert_eq!(
                    current.iter().cloned().collect::<Vec<_>>(),
                    vec!["a.effect", "z.effect", "α.effect"]
                );
                assert!(actual.unwrap_err().message.contains(if missing {
                    "missing.first.α\0"
                } else {
                    "missing.second"
                }));
            }
        }
    }
    let mut program = program(1);
    imported(&mut program);
    let item = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "token.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut item.kind else {
        unreachable!()
    };
    let ResolvedResourceDropKind::Imported { import, .. } = &mut drop.kind else {
        unreachable!()
    };
    *import = DeclarationId::new("missing.import");
    let ty = root("token.type");
    assert_eq!(
        super::super::Collector::new(&program).extend(&ty),
        before_inline::Collector::new(&program).extend(&ty)
    );
}

fn observe(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    limit: usize,
    old: bool,
) -> ((bool, Metrics), bool, usize, bool) {
    let ((ok, metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut current = super::super::Collector::new(program);
            let mut previous = before_inline::Collector::new(program);
            let mut ok = true;
            for _ in 0..3 {
                let result = if old {
                    previous.extend(ty)
                } else {
                    current.extend(ty)
                };
                if result.is_err() {
                    ok = false;
                    drop(result);
                    break;
                }
            }
            drop(current);
            drop(previous);
            ok
        })
    });
    ((ok, metrics), refused, used, first.is_some())
}

#[test]
fn inline_and_paid_tree_spill_have_independent_exact_short_zero_and_full_drop_controls() {
    for depth in [0, 1, 15, 16, 17, 33] {
        let program = program(depth);
        let ty = if depth == 0 {
            ResolvedType::I64
        } else {
            root("inline.row.0")
        };
        for old in [false, true] {
            let ((ok, actual), refused, paid, first) = observe(&program, &ty, LIMIT, old);
            assert!(ok && !refused && !first && !actual.invalid);
            assert_eq!(actual.live, 0);
            assert_eq!(actual.allocated, actual.freed);
            assert!(actual.allocated <= paid && actual.highwater <= paid);
            if !old && depth <= 16 {
                assert_eq!(actual.allocated, 0);
            }
            if !old && depth > 16 {
                assert!(actual.allocated > 0);
            }
            let ((ok, exact), refused, used, first) = observe(&program, &ty, paid, old);
            assert!(ok && !refused && !first && used == paid && !exact.invalid && exact.live == 0);
            if paid > 0 {
                for limit in [0, paid - 1] {
                    let ((ok, actual), refused, used, first) = observe(&program, &ty, limit, old);
                    assert!(
                        !ok && refused
                            && first
                            && used <= limit
                            && !actual.invalid
                            && actual.live == 0
                    );
                    assert_eq!(actual.allocated, actual.freed);
                }
            }
        }
    }
}

#[test]
fn borrowed_equal_keys_and_sticky_inline_or_spill_insertion_preserve_membership() {
    let ids = (0..33)
        .map(|n| DeclarationId::new(&format!("α.{n}\0")))
        .collect::<Vec<_>>();
    let equal = ids
        .iter()
        .map(|id| DeclarationId::new(id.as_str()))
        .collect::<Vec<_>>();
    let (((), actual), refused, paid) = with_limit_usage(LIMIT, || {
        measure(|| {
            let mut active = Active::new().unwrap();
            for id in &ids {
                active.insert(id).unwrap();
            }
            for id in &equal {
                assert!(active.contains(id));
            }
            for id in equal.iter().rev() {
                active.remove(id);
                assert!(!active.contains(id));
            }
            assert!(active.inline.iter().all(Option::is_none) && active.spill.is_empty());
            drop(active);
        })
    });
    assert!(!refused && !actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    for width in [0, 16, 17] {
        let mut active = Active::new().unwrap();
        for id in &ids[..width] {
            active.insert(id).unwrap();
        }
        let (((), actual), refused, used, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!crate::bounded_output::reserve_active_required(1));
                assert!(active.insert(&ids[width]).is_err());
                assert!(!active.contains(&ids[width]));
                assert!(!crate::bounded_output::reserve_active_required(0));
            })
        });
        assert!(
            refused && first.is_some() && used == 0 && !actual.invalid && actual.allocated == 0
        );
    }
}

#[test]
fn complete_original_and_all_source_audit_receivers_are_byte_exact() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/lifecycle-inline-active-path/whole-source-inverses.json"
    )))
    .unwrap();
    let inputs=[
        ci_str!("../../completed.rs"),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/hir_module_boundaries.rs")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
    ];
    let originals = [
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/completed.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/hir_module_boundaries.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/lifecycle-inline-active-path/interop_source_inventory.rs.before.txt"
        )),
    ];
    let rows = packet.as_array().unwrap();
    assert_eq!(rows.len(), inputs.len());
    for ((row, current), original) in rows.iter().zip(inputs).zip(originals) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["original_sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
    assert_eq!(
        ci_str!("../before_inline.rs").to_owned() + "\n#[cfg(test)]\nmod tests;\n",
        originals[0]
    );
    let mut legacy = ci_str!("../../../lifecycle.rs").to_owned();
    for (current, original) in [
        (
            "#[cfg(test)]\npub(super) fn extend_funded(",
            "pub(super) fn extend_funded(",
        ),
        ("#[cfg(test)]\nfn collect<'a>(", "fn collect<'a>("),
    ] {
        assert_eq!(legacy.matches(current).count(), 1);
        legacy = legacy.replacen(current, original, 1);
    }

    let binding: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/lifecycle-inline-active-path/source-binding.json"
    )))
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(legacy.as_bytes()))
        ),
        binding["original_lifecycle_sha256"].as_str().unwrap()
    );
    // Actual compile-time authority inventories include every new child exactly once.
    for input in &inputs[1..] {
        for name in [
            "completed/inline_path.rs",
            "completed/inline_path/tests.rs",
            "completed/before_inline.rs",
        ] {
            assert_eq!(input.matches(name).count(), 1);
        }
    }
}
