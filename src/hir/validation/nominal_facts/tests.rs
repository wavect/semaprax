use super::*;

const SOURCE: &str = r#"
module test.nominal_facts;
@id("facts.class") class Class { @id("facts.class.value") value: i64, }
@id("facts.record") record Record { @id("facts.record.value") value: i64, }
@id("facts.variant") variant Variant {
    @id("facts.variant.value") Value { @id("facts.variant.field") value: i64, },
}
@id("facts.owned") record Owned { @id("facts.owned.payload") payload: Bytes, }
@id("facts.generic") record Generic<T> { @id("facts.generic.value") value: T, }
@id("app.main") fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let source = crate::parse(SOURCE, std::path::Path::new("nominal-facts.spx")).unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn nominal(identity: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(identity),
        arguments,
    }
}

#[test]
fn nominal_facts_borrow_exact_retained_values_and_keep_wire_unchanged() {
    let program = program();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let validator = HirValidator::new(&program).unwrap();
    assert!(validator.nominal_facts.entries.get().is_none());
    for identity in [
        "facts.class",
        "facts.record",
        "facts.variant",
        "facts.owned",
    ] {
        let ty = nominal(identity, Vec::new());
        let expected = program.declarations.type_facts(&ty).unwrap();
        let actual = validator.borrowed_type_facts(&ty).unwrap().unwrap();
        assert_eq!(actual.as_ref(), &expected);
        let Cow::Borrowed(actual) = actual else {
            panic!("zero-argument retained facts were cloned")
        };
        assert!(std::ptr::eq(
            actual,
            &program.declarations.type_facts_by_id[&ty.identity_key()]
        ));
    }
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), wire);
    assert!(validator.nominal_facts.owned_capacity() > 0);
}

#[test]
fn nominal_facts_keep_generic_missing_and_unknown_fallbacks_exact() {
    let mut program = program();
    let missing = nominal("facts.record", Vec::new());
    program
        .declarations
        .type_facts_by_id
        .remove(&missing.identity_key());
    // An invalid zero-argument generic cache entry must not become eligible.
    let empty_generic = nominal("facts.generic", Vec::new());
    let forged = program.declarations.type_facts(&ResolvedType::I64).unwrap();
    program
        .declarations
        .type_facts_by_id
        .insert(empty_generic.identity_key(), forged);
    let lazy = Lookup::default();
    let ((), overflow, used) = crate::bounded_output::with_limit_usage(0, || {
        assert!(lazy.get(&program, &empty_generic).unwrap().is_none());
        assert!(lazy
            .get(&program, &nominal("facts.unknown", Vec::new()))
            .unwrap()
            .is_none());
        assert!(lazy.entries.get().is_none());
    });
    assert!(!overflow);
    assert_eq!(used, 0);
    let validator = HirValidator::new(&program).unwrap();
    let generic_i64 = nominal("facts.generic", vec![ResolvedType::I64]);
    let generic_u8 = nominal("facts.generic", vec![ResolvedType::U8]);
    for ty in [
        missing,
        empty_generic,
        generic_i64.clone(),
        generic_u8.clone(),
        nominal(crate::prelude::VEC_ID, vec![ResolvedType::I64]),
        nominal(crate::prelude::BOX_ID, vec![ResolvedType::U8]),
        nominal("facts.unknown", Vec::new()),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("facts.generic"),
            index: 0,
        },
    ] {
        let expected = program.declarations.type_facts(&ty);
        let actual = validator.borrowed_type_facts(&ty).unwrap();
        assert_eq!(actual.as_deref(), expected.as_ref());
        assert!(!matches!(actual, Some(Cow::Borrowed(_))));
    }
    assert_ne!(
        program
            .declarations
            .type_facts(&generic_i64)
            .unwrap()
            .layout_key,
        program
            .declarations
            .type_facts(&generic_u8)
            .unwrap()
            .layout_key
    );
}

#[test]
fn nominal_facts_do_not_recompute_unused_malformed_declarations_or_alias_generic_metadata() {
    let mut program = program();
    let record = nominal("facts.record", Vec::new());
    let key = record.identity_key();
    let expected = program.declarations.type_facts_by_id[&key].clone();
    // The retained entry still exists, but independently reconstructing this
    // hostile declaration's recursive facts must fail. Preparation only borrows.
    program
        .declarations
        .record_fields
        .get_mut(&DeclarationId::new("facts.record"))
        .unwrap()[0]
        .ty = record.clone();
    let generic = nominal("facts.generic", Vec::new());
    program
        .declarations
        .type_facts_by_id
        .insert(generic.identity_key(), expected.clone());
    program
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "facts.generic")
        .unwrap()
        .type_parameters
        .clear();
    let lookup = Lookup::default();
    assert_eq!(lookup.get(&program, &record).unwrap(), Some(&expected));
    assert!(lookup.get(&program, &generic).unwrap().is_none());
    assert!(program.declarations.recompute_type_facts(&record).is_none());
}

#[test]
fn nominal_facts_preserve_stale_cached_fact_rejection() {
    let mut program = program();
    let ty = nominal("facts.record", Vec::new());
    let key = ty.identity_key();
    program
        .declarations
        .type_facts_by_id
        .get_mut(&key)
        .unwrap()
        .layout_key
        .push_str(".stale");
    let validator = HirValidator::new(&program).unwrap();
    let borrowed = validator.borrowed_type_facts(&ty).unwrap().unwrap();
    assert_eq!(
        borrowed.as_ref(),
        &program.declarations.type_facts(&ty).unwrap()
    );
    assert_ne!(
        borrowed.as_ref(),
        &program.declarations.recompute_type_facts(&ty).unwrap()
    );
    let error = validate_core(&program).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "record `facts.record` has invalid or stale recursive type facts"
    );
}

#[test]
fn nominal_facts_repeated_lookup_and_shared_clone_need_no_temporary_key_budget() {
    let program = program();
    let ty = nominal("facts.record", Vec::new());
    let lookup = Lookup::default();
    let expected = lookup.get(&program, &ty).unwrap().unwrap();
    let capacity = lookup.owned_capacity();
    let ((), overflow, used) = crate::bounded_output::with_limit_usage(0, || {
        let cloned = lookup.clone();
        assert_eq!(cloned.owned_capacity(), capacity);
        assert!(Arc::ptr_eq(
            lookup.entries.get().unwrap().as_ref().unwrap(),
            cloned.entries.get().unwrap().as_ref().unwrap()
        ));
        for _ in 0..64 {
            assert!(std::ptr::eq(
                cloned.get(&program, &ty).unwrap().unwrap(),
                expected
            ));
        }
    });
    assert!(!overflow);
    assert_eq!(used, 0);
}

#[test]
fn nominal_facts_preparation_charges_metadata_and_refuses_one_short_or_floor() {
    let program = program();
    let ty = nominal("facts.record", Vec::new());
    let (lookup, overflow, used) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let lookup = Lookup::default();
        assert!(lookup.get(&program, &ty).unwrap().is_some());
        lookup
    });
    assert!(!overflow);
    assert!(used >= lookup.owned_capacity());
    assert!(used > 0);
    let (result, overflow) =
        crate::bounded_output::with_limit(used, || Lookup::default().get(&program, &ty));
    assert!(!overflow);
    assert!(result.unwrap().is_some());
    let (result, overflow) =
        crate::bounded_output::with_limit(used - 1, || Lookup::default().get(&program, &ty));
    assert!(overflow);
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    let (result, overflow) = crate::bounded_output::with_limit(used, || {
        assert!(crate::bounded_output::set_active_floor(1));
        Lookup::default().get(&program, &ty)
    });
    assert!(overflow);
    assert_eq!(result.unwrap_err().code, "SPX-H006");
}

#[test]
fn nominal_facts_zero_argument_key_matches_canonical_bytes_without_identity_clone() {
    for identity in ["x", "colon:id", "unicode.λ", "long.identity.with.parts"] {
        let ty = nominal(identity, Vec::new());
        let ResolvedType::Nominal { declaration, .. } = &ty else {
            unreachable!()
        };
        assert_eq!(nominal_key(declaration).unwrap(), ty.identity_key());
    }
}

#[test]
fn nominal_facts_private_forecast_covers_prelude_and_metadata_key_coexistence() {
    let prelude = crate::prelude::declarations()
        .iter()
        .filter(|declaration| declaration.type_parameters.is_empty())
        .map(|declaration| declaration.stable_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        prelude,
        crate::private_capacity_contract::ZERO_ARGUMENT_FACT_PRELUDE_IDS
    );
    let source = crate::parse(SOURCE, std::path::Path::new("nominal-facts.spx")).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let ty = nominal("facts.record", Vec::new());
    let lookup = Lookup::default();
    assert!(lookup.get(&program, &ty).unwrap().is_some());
    let maximum_key = program
        .types
        .iter()
        .filter(|declaration| eligible(&program, declaration))
        .map(|declaration| nominal_key(&declaration.id).unwrap().capacity())
        .max()
        .unwrap();
    let upper = crate::private_capacity_contract::nominal_facts_lookup_upper(&source).unwrap();
    assert!(lookup.owned_capacity() + maximum_key <= upper);
}
