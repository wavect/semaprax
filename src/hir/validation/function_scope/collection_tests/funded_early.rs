//! Funded early-profile, lifecycle and effect-key controls against the complete originals.
use super::*;
use crate::hir::declared_collection::funded_early;
use std::collections::BTreeSet;

#[allow(dead_code, reason = "complete saved nested outcome admission tree")]
#[rustfmt::skip]
mod before_nested_outcome;

fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}
fn roots(program: &ResolvedProgram) -> Vec<ResolvedType> {
    let mut roots = types(program);
    roots.extend(program.types.iter().map(|item| nominal(item.id.as_str())));
    roots
}
fn forged_programs() -> Vec<ResolvedProgram> {
    let clean = program();
    // An existing `<id>#invariant` declaration must refuse every record/variant profile.
    let mut invariant = clean.clone();
    let ids = invariant
        .declarations
        .declarations
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for id in ids {
        let declaration = invariant.declarations.declarations[&id].clone();
        let key = DeclarationId::new(format!("{id}#invariant"));
        invariant.declarations.declarations.insert(key, declaration);
    }
    // Hostile cached type facts are never consulted by the funded profiles.
    let mut cached = clean.clone();
    let mut facts = cached.declarations.type_facts(&ResolvedType::I64).unwrap();
    facts.copy = false;
    facts.layout_key = "arbitrary hostile cached layout".repeat(32);
    cached
        .declarations
        .type_facts_by_id
        .insert("i64".into(), facts);
    // A self-referential record and a missing record field type reach lifecycle cycles/errors.
    let (mut cycle, mut missing) = (clean.clone(), clean.clone());
    for (forged, target) in [
        (&mut cycle, None),
        (&mut missing, Some("missing.lifecycle")),
    ] {
        for item in &mut forged.types {
            let own = item.id.clone();
            if let ResolvedTypeDeclarationKind::Record { fields } = &mut item.kind {
                if let Some(field) = fields.first_mut() {
                    field.ty = target.map_or_else(|| nominal(own.as_str()), nominal);
                    if let Some(indexed) = forged
                        .declarations
                        .record_fields
                        .get_mut(&own)
                        .and_then(|fields| fields.first_mut())
                    {
                        indexed.ty = field.ty.clone();
                    }
                }
            }
        }
    }
    vec![clean, invariant, cached, cycle, missing]
}
fn observe(limit: usize, run: impl FnOnce() -> Result<bool, Diagnostic>) -> Observed {
    let ((verdict, metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = run();
                let verdict = result.as_ref().ok().copied();
                if verdict.is_none() {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                verdict
            })
        });
    (verdict, metrics, refused, paid, receipt.is_some())
}
fn check(expected: bool, run: impl Fn() -> Result<bool, Diagnostic>, label: &str) -> usize {
    let (verdict, actual, refused, paid, receipt) = observe(LIMIT, &run);
    assert_eq!(verdict, Some(expected), "{label}");
    assert!(!refused && !receipt && !actual.invalid, "{label}");
    assert_eq!(actual.live, 0, "{label}");
    assert!(
        actual.allocated <= paid && actual.highwater <= paid,
        "{label}: actual {actual:?}, paid {paid}"
    );
    let (verdict, exact, refused, exact_paid, receipt) = observe(paid, &run);
    assert_eq!(verdict, Some(expected), "{label}");
    assert!(!refused && !receipt && !exact.invalid, "{label}");
    assert_eq!(
        (exact.live, exact_paid, exact.allocated),
        (0, paid, actual.allocated),
        "{label}"
    );
    if paid > 0 {
        let (verdict, short, refused, short_paid, receipt) = observe(paid - 1, &run);
        assert!(
            verdict.is_none() && refused && receipt && !short.invalid,
            "{label}"
        );
        assert_eq!(short.live, 0, "{label}");
        assert!(short_paid < paid, "{label}");
    }
    paid
}
#[test]
fn funded_early_profiles_match_original_receivers_and_pay_each_constructor() {
    for program in forged_programs() {
        let index = &program.declarations;
        for ty in roots(&program) {
            let label = ty.identity_key();
            check(
                crate::hir::owned_collection_record::admitted(&ty, index),
                || funded_early::record_admitted(&ty, index),
                &label,
            );
            check(
                crate::hir::collection_outcome::runtime_admitted(index, &ty),
                || funded_early::runtime_admitted(index, &ty),
                &label,
            );
        }
    }
}
#[test]
fn early_profile_constructors_are_actually_paid_before_construction() {
    let program = program();
    let index = &program.declarations;
    let boxed = nominal("owned.box");
    let choice = nominal("choice.type");
    assert!(
        check(
            false,
            || funded_early::record_admitted(&boxed, index),
            "record"
        ) > 0
    );
    let (verdict, actual, refused, paid, receipt) =
        observe(0, || funded_early::runtime_admitted(index, &choice));
    assert_eq!(verdict, Some(false));
    assert!(!refused && !receipt && !actual.invalid);
    assert_eq!(
        (actual.allocated, actual.allocations, actual.live, paid),
        (0, 0, 0, 0)
    );
    let saved = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/ad38990eb7f7c65c1a349d900a5ab3436d89c06f447249d0ea844e3e08f4962e"
    ));
    let (admission, _) = saved
        .split_once(
            "\n/// Every cleanup/layout use must bind the exact active case and payload field.",
        )
        .unwrap();
    let restored = admission
        .replace(
            "mod profile;\npub(crate) use profile::{function_requires_profile_by, program_requires_profile};\n",
            "",
        )
        .replace(
            "super::super::owned_collection_record",
            "crate::hir::owned_collection_record",
        );
    assert_eq!(
        restored,
        include_str!("funded_early/before_nested_outcome.rs")
    );
    let (old_verdict, old) = measure(|| {
        crate::hir::collection_outcome::admitted(index, &choice)
            || crate::hir::collection_outcome::owned_admitted(index, &choice)
            || before_nested_outcome::admitted(index, &choice)
    });
    assert!(!old_verdict && !old.invalid);
    assert!(old.allocated > 0 && old.allocations > 0);
    assert_eq!(old.live, 0);
    assert_eq!(old.allocated, old.freed);
    // The original canonical-key spelling is the identity looked up, not an approximation.
    assert!(!funded_early::invariant_present(index, &DeclarationId::new("owned.box")).unwrap());
    let mut forged = program.clone();
    let declaration = forged.declarations.declarations[&DeclarationId::new("owned.box")].clone();
    forged
        .declarations
        .declarations
        .insert(DeclarationId::new("owned.box#invariant"), declaration);
    assert!(funded_early::invariant_present(
        &forged.declarations,
        &DeclarationId::new("owned.box")
    )
    .unwrap());
}
#[test]
fn funded_lifecycle_collector_matches_original_cycles_errors_and_pays_nodes() {
    for program in forged_programs() {
        for ty in roots(&program) {
            let (mut expected_set, mut actual_set) = (BTreeSet::new(), BTreeSet::new());
            let expected = extend_resolved_lifecycle_effects(&program, &ty, &mut expected_set);
            let actual = super::super::lifecycle::extend_funded(&program, &ty, &mut actual_set);
            assert_eq!(
                actual.as_ref().err().map(Diagnostic::json),
                expected.as_ref().err().map(Diagnostic::json)
            );
            assert_eq!(actual_set, expected_set);
            if expected.is_ok() && ty.nominal_id().is_some() {
                let run = || -> Result<bool, Diagnostic> {
                    let mut set = BTreeSet::new();
                    super::super::lifecycle::extend_funded(&program, &ty, &mut set)?;
                    Ok(set == expected_set)
                };
                assert!(check(true, run, &ty.identity_key()) > 0);
            }
        }
    }
}
#[test]
fn funded_declared_effect_keys_match_borrowed_collect_and_pay_each_new_node() {
    let effects = ["host.echo", "net.read", "host.echo", "", "net.read"]
        .map(str::to_owned)
        .to_vec();
    let expected = effects.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let paid = check(
        true,
        || Ok(super::super::lifecycle::declared_effects(&effects)? == expected),
        "effects",
    );
    assert!(paid > 0);
    let none = Vec::<String>::new();
    let (verdict, metrics, refused, paid, receipt) = observe(LIMIT, || {
        Ok(super::super::lifecycle::declared_effects(&none)?.is_empty())
    });
    assert_eq!(verdict, Some(true));
    assert!(!refused && !receipt && !metrics.invalid);
    assert_eq!((metrics.allocations, metrics.allocated, paid), (0, 0, 0));
}

#[test]
fn imported_lifecycle_effect_payloads_are_funded_deduplicated_and_drop_complete() {
    let mut program = program();
    let token = function(&program, "token.consume").params[0].ty.clone();
    let token_id = token.nominal_id().unwrap().clone();
    let import_id = DeclarationId::new("host.echo");
    let import = program
        .interfaces
        .iter_mut()
        .flat_map(|interface| &mut interface.imports)
        .find(|import| import.id == import_id)
        .unwrap();
    import.effects = ["host.echo", "net.完全", "", "host.echo", "net.完全"]
        .map(str::to_owned)
        .to_vec();
    let resource = program
        .types
        .iter_mut()
        .find(|item| item.id == token_id)
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource {
        drop: resource_drop,
    } = &mut resource.kind
    else {
        panic!("resource fixture")
    };
    resource_drop.kind = ResolvedResourceDropKind::Imported {
        import: import_id,
        import_key: "unchanged collector ignores import-key metadata".to_owned(),
    };
    let mut expected = BTreeSet::new();
    extend_resolved_lifecycle_effects(&program, &token, &mut expected).unwrap();
    assert_eq!(expected.len(), 3);
    check(
        true,
        || {
            let mut actual = BTreeSet::new();
            super::super::lifecycle::extend_funded(&program, &token, &mut actual)?;
            super::super::lifecycle::extend_funded(&program, &token, &mut actual)?;
            Ok(actual == expected)
        },
        "imported effects and duplicate payloads",
    );
    program.interfaces.clear();
    let mut expected_set = BTreeSet::new();
    let expected_error =
        extend_resolved_lifecycle_effects(&program, &token, &mut expected_set).unwrap_err();
    let expected_json = expected_error.json();
    let ((), metrics) = measure(|| {
        let mut actual_set = BTreeSet::new();
        let error =
            super::super::lifecycle::extend_funded(&program, &token, &mut actual_set).unwrap_err();
        assert_eq!(actual_set, expected_set);
        let json = error.json();
        assert_eq!(json, expected_json);
        drop(json);
        drop(error);
    });
    assert_eq!(metrics.live, 0);
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, metrics.freed);
    let mut actual_set = BTreeSet::new();
    assert_eq!(
        super::super::lifecycle::extend_funded(&program, &token, &mut actual_set)
            .unwrap_err()
            .json(),
        expected_json
    );
}

// Authenticate each forged field's complete metadata outside the observed lifetime.
fn add_record(index: &mut DeclarationIndex, id: &str, types: Vec<ResolvedType>) -> ResolvedType {
    let id = DeclarationId::new(id);
    let mut declaration = index.declarations[&DeclarationId::new("owned.box")].clone();
    declaration.id = id.clone();
    index.declarations.insert(id.clone(), declaration);
    index.type_parameters.insert(id.clone(), Vec::new());
    let template = index.record_fields[&DeclarationId::new("owned.box")][0].clone();
    let template_metadata = index.declarations[&template.id].clone();
    let fields: Vec<crate::hir::ResolvedFieldDeclaration> = types
        .into_iter()
        .enumerate()
        .map(|(position, ty)| {
            let mut field = template.clone();
            field.id = DeclarationId::new(format!("{id}.field.{position}"));
            field.name = format!("field_{position}");
            field.index = position.try_into().unwrap();
            field.ty = ty;
            let mut metadata = template_metadata.clone();
            metadata.id = field.id.clone();
            metadata.owner = Some(id.clone());
            metadata.name = field.name.clone();
            index.declarations.insert(field.id.clone(), metadata);
            field
        })
        .collect();
    index.record_fields.insert(id.clone(), fields);
    ResolvedType::Nominal {
        declaration: id,
        arguments: Vec::new(),
    }
}

#[test]
fn funded_early_profiles_preserve_exact_depth_leaf_and_field_limits() {
    for depth in [64, 65] {
        let mut program = collection_program();
        let mut root = vector_of(ResolvedType::I64);
        for level in 0..depth {
            root = add_record(
                &mut program.declarations,
                &format!("boundary.depth.{depth}.{level}"),
                vec![root],
            );
        }
        let expected = crate::hir::owned_collection_record::admitted(&root, &program.declarations);
        assert_eq!(expected, depth == 64);
        check(
            expected,
            || funded_early::record_admitted(&root, &program.declarations),
            "exact depth",
        );
    }
    for leaves in [256, 257] {
        let mut program = collection_program();
        let root = add_record(
            &mut program.declarations,
            &format!("boundary.leaves.{leaves}"),
            vec![vector_of(ResolvedType::I64); leaves],
        );
        let expected = crate::hir::owned_collection_record::admitted(&root, &program.declarations);
        assert_eq!(expected, leaves == 256);
        check(
            expected,
            || funded_early::record_admitted(&root, &program.declarations),
            "exact leaves",
        );
    }
    for fields in [4096, 4097] {
        let mut program = collection_program();
        let mut types = vec![ResolvedType::I64; fields];
        types[0] = vector_of(ResolvedType::I64);
        let root = add_record(
            &mut program.declarations,
            &format!("boundary.fields.{fields}"),
            types,
        );
        let expected = crate::hir::owned_collection_record::admitted(&root, &program.declarations);
        assert_eq!(expected, fields == 4096);
        check(
            expected,
            || funded_early::record_admitted(&root, &program.declarations),
            "exact fields",
        );
    }
}
