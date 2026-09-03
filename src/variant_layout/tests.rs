use std::path::Path;

use super::{VariantFieldValueKind, VariantLayout, VariantLayoutCache, VariantTarget};
use crate::hir::{self, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
use crate::parse;

const SOURCE: &str = r#"
module test.variant_layout;
@id("choice.type")
variant Choice {
    @id("choice.none") None,
    @id("choice.flag") Flag {
        @id("choice.flag.value") value: bool,
    },
    @id("choice.pair") Pair {
        @id("choice.pair.number") number: i64,
        @id("choice.pair.enabled") enabled: bool,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const GENERIC_SOURCE: &str = r#"
module test.generic_variant_layout;
@id("choice.generic")
variant Choice<T> {
    @id("choice.generic.none") None,
    @id("choice.generic.value") Value {
        @id("choice.generic.value.value") value: T,
    },
}
@id("choice.i64")
fn choice_i64() -> Choice<i64> { Choice<i64>::Value { value: 42 } }
@id("choice.bool")
fn choice_bool() -> Choice<bool> { Choice<bool>::Value { value: true } }
@id("option.i64")
fn option_i64() -> Option<i64> { Option<i64>::Some { value: 7 } }
@id("option.bool")
fn option_bool() -> Option<bool> { Option<bool>::None {} }
@id("result.value")
fn result_value() -> Result<i64, bool> { Result<i64, bool>::Err { error: true } }
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved() -> hir::ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("variant-layout.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}

#[test]
fn concrete_generic_and_prelude_instances_have_distinct_cached_layouts() {
    let parsed = parse(GENERIC_SOURCE, Path::new("generic-variant-layout.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let native = VariantLayoutCache::build(&program, VariantTarget::Native64).unwrap();
    let wasm = VariantLayoutCache::build(&program, VariantTarget::Wasm32).unwrap();

    let choice_i64 = nominal("choice.generic", vec![ResolvedType::I64]);
    let choice_bool = nominal("choice.generic", vec![ResolvedType::Bool]);
    let option_i64 = nominal("core.option", vec![ResolvedType::I64]);
    let option_bool = nominal("core.option", vec![ResolvedType::Bool]);
    let result = nominal("core.result", vec![ResolvedType::I64, ResolvedType::Bool]);
    assert_eq!(native.layouts().len(), 5);
    assert_eq!(wasm.layouts().len(), 5);

    for cache in [&native, &wasm] {
        let integer = cache.layout(&choice_i64).unwrap();
        assert_eq!(
            (integer.payload_offset, integer.size, integer.align),
            (8, 16, 8)
        );
        assert_eq!(
            integer
                .case(&DeclarationId::new("choice.generic.value"))
                .unwrap()
                .fields[0]
                .ty,
            ResolvedType::I64
        );
        let result = cache.layout(&result).unwrap();
        assert_eq!(
            (result.payload_offset, result.size, result.align),
            (8, 16, 8)
        );
        assert_eq!(
            result.cases.iter().map(|case| case.tag).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    let native_bool = native.layout(&choice_bool).unwrap();
    let wasm_bool = wasm.layout(&choice_bool).unwrap();
    assert_eq!((native_bool.payload_offset, native_bool.size), (4, 8));
    assert_eq!((wasm_bool.payload_offset, wasm_bool.size), (4, 8));
    assert_ne!(
        native.layout(&choice_i64).unwrap().digest_hex(),
        native_bool.digest_hex()
    );
    assert_ne!(
        native.layout(&option_i64).unwrap().digest_hex(),
        native.layout(&option_bool).unwrap().digest_hex()
    );
    assert_eq!(
        native.layout(&option_i64).unwrap().digest_hex(),
        "e728ce973bb0fa9d86027841615dcd25b9a1700cb15e4fd1704da163e658d60c"
    );
    assert_eq!(
        wasm.layout(&option_i64).unwrap().digest_hex(),
        "79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda"
    );
    assert_eq!(
        native.layout(&result).unwrap().digest_hex(),
        "03ac11743e029e151b8cbc12420e899b2edfe42cbf7c68d5f6fb3ab0e043b3dc"
    );
    assert_eq!(
        wasm.layout(&result).unwrap().digest_hex(),
        "c01112f909a074343ae4eb3abde6ad70930280e4a8016c165e05f317bed9f199"
    );

    let mut confused = native.layout(&choice_i64).unwrap().clone();
    confused.instance = choice_bool;
    assert!(confused.validate(&program).is_err());
    let reversed_result = nominal("core.result", vec![ResolvedType::Bool, ResolvedType::I64]);
    assert_ne!(
        VariantLayout::for_type(&program, VariantTarget::Native64, &reversed_result)
            .unwrap()
            .digest_hex(),
        native.layout(&result).unwrap().digest_hex()
    );
    let wrong_arity = nominal("core.result", vec![ResolvedType::I64]);
    assert!(VariantLayout::for_type(&program, VariantTarget::Wasm32, &wrong_arity).is_err());
    let nested_argument = nominal("choice.generic", vec![option_i64.clone()]);
    assert!(VariantLayout::for_type(&program, VariantTarget::Wasm32, &nested_argument).is_err());

    let mut missing = native.clone();
    missing.layouts.remove(&choice_i64);
    assert!(missing.layout(&choice_i64).is_err());

    for (owner, index) in [("foreign.owner", 0), ("choice.generic", 1)] {
        let mut hostile = program.clone();
        let declaration = hostile
            .types
            .iter_mut()
            .find(|item| item.id.as_str() == "choice.generic")
            .unwrap();
        let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
            panic!("generic choice is a variant")
        };
        cases[1].fields[0].ty = ResolvedType::TypeParameter {
            owner: DeclarationId::new(owner),
            index,
        };
        assert!(VariantLayout::for_type(&hostile, VariantTarget::Native64, &choice_i64).is_err());
    }
}

#[test]
fn native64_and_wasm32_layouts_freeze_tag_payload_and_bool_profiles() {
    let program = resolved();
    let variant = DeclarationId::new("choice.type");
    let native = VariantLayout::for_variant(&program, VariantTarget::Native64, &variant)
        .expect("native variant layout");
    let wasm = VariantLayout::for_variant(&program, VariantTarget::Wasm32, &variant)
        .expect("Wasm variant layout");

    for layout in [&native, &wasm] {
        assert_eq!(layout.variant, variant);
        assert_eq!(layout.tag_size, 4);
        assert_eq!(layout.payload_offset, 8);
        assert_eq!(layout.payload_size, 16);
        assert_eq!(layout.size, 24);
        assert_eq!(layout.align, 8);
        assert_eq!(
            layout
                .cases
                .iter()
                .map(|case| (case.case.as_str(), case.tag))
                .collect::<Vec<_>>(),
            vec![("choice.none", 0), ("choice.flag", 1), ("choice.pair", 2),]
        );
        let unit = layout.case(&DeclarationId::new("choice.none")).unwrap();
        assert_eq!((unit.size, unit.align), (1, 1));
        assert!(unit.fields.is_empty());
        layout.validate(&program).unwrap();
    }

    let native_flag = native
        .case(&DeclarationId::new("choice.flag"))
        .unwrap()
        .field(&DeclarationId::new("choice.flag.value"))
        .unwrap();
    assert_eq!(
        (native_flag.offset, native_flag.size, native_flag.align),
        (0, 1, 1)
    );
    let wasm_flag = wasm
        .case(&DeclarationId::new("choice.flag"))
        .unwrap()
        .field(&DeclarationId::new("choice.flag.value"))
        .unwrap();
    assert_eq!(
        (wasm_flag.offset, wasm_flag.size, wasm_flag.align),
        (0, 4, 4)
    );
    assert_eq!(
        native.digest_hex(),
        "60ff105b799ae1a6ec24b72587901bfa1a6be6a97dad8685c75f56db9423e1e1"
    );
    assert_eq!(
        wasm.digest_hex(),
        "4a1c07d4b2011b11c43acb27aa9951b0cb6a55af24e079c73833f7047c3700e6"
    );
}

#[test]
fn direct_owned_bytes_payload_uses_target_carrier_and_remains_non_copy() {
    // Keep source/verifier admission unchanged and exercise only the
    // internal resolved-type layout substrate.
    let mut program = resolved();
    let variant = DeclarationId::new("choice.type");
    let declaration = program
        .types
        .iter_mut()
        .find(|item| item.id == variant)
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
        panic!("choice is a variant")
    };
    cases[2].fields[0].ty = ResolvedType::Bytes;
    let instance = nominal("choice.type", Vec::new());
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap()
        .return_type = instance.clone();

    let native = VariantLayout::for_type(&program, VariantTarget::Native64, &instance)
        .expect("Native64 owned-byte variant layout");
    assert_eq!(
        (
            native.payload_offset,
            native.payload_size,
            native.size,
            native.align,
        ),
        (8, 24, 32, 8)
    );
    let native_pair = native.case(&DeclarationId::new("choice.pair")).unwrap();
    assert_eq!((native_pair.size, native_pair.align), (24, 8));
    assert_eq!(
        native_pair
            .fields
            .iter()
            .map(|field| (field.value_kind, field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        vec![
            (VariantFieldValueKind::OwnedBytes, 0, 16, 8),
            (VariantFieldValueKind::Copy, 16, 1, 1),
        ]
    );
    native.validate(&program).unwrap();

    let wasm = VariantLayout::for_type(&program, VariantTarget::Wasm32, &instance)
        .expect("Wasm32 owned-byte variant layout");
    assert_eq!(
        (
            wasm.payload_offset,
            wasm.payload_size,
            wasm.size,
            wasm.align,
        ),
        (8, 16, 24, 8)
    );
    let wasm_pair = wasm.case(&DeclarationId::new("choice.pair")).unwrap();
    assert_eq!((wasm_pair.size, wasm_pair.align), (16, 8));
    assert_eq!(
        wasm_pair
            .fields
            .iter()
            .map(|field| (field.value_kind, field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        vec![
            (VariantFieldValueKind::OwnedBytes, 0, 8, 8),
            (VariantFieldValueKind::Copy, 8, 4, 4),
        ]
    );
    wasm.validate(&program).unwrap();

    for target in [VariantTarget::Native64, VariantTarget::Wasm32] {
        let cache = VariantLayoutCache::build(&program, target).unwrap();
        assert_eq!(
            cache
                .layout(&instance)
                .unwrap()
                .case(&DeclarationId::new("choice.pair"))
                .unwrap()
                .fields[0]
                .value_kind,
            VariantFieldValueKind::OwnedBytes
        );
    }

    let mut copy_confused = native;
    copy_confused.cases[2].fields[0].value_kind = VariantFieldValueKind::Copy;
    assert!(copy_confused.validate(&program).is_err());

    for algebra in [
        nominal(crate::prelude::OPTION_ID, vec![ResolvedType::Bytes]),
        nominal(
            crate::prelude::RESULT_ID,
            vec![ResolvedType::Bytes, ResolvedType::Bool],
        ),
        nominal(
            crate::prelude::RESULT_ID,
            vec![ResolvedType::I64, ResolvedType::Bytes],
        ),
    ] {
        for (target, expected) in [
            (VariantTarget::Native64, (8, 16, 24, 8)),
            (VariantTarget::Wasm32, (8, 8, 16, 8)),
        ] {
            let layout = VariantLayout::for_type(&program, target, &algebra)
                .expect("compiler-owned owned-byte algebra layout");
            assert_eq!(
                (
                    layout.payload_offset,
                    layout.payload_size,
                    layout.size,
                    layout.align,
                ),
                expected
            );
            let owned = layout
                .cases
                .iter()
                .flat_map(|case| &case.fields)
                .find(|field| field.ty == ResolvedType::Bytes)
                .expect("one direct owned-byte payload");
            assert_eq!(owned.value_kind, VariantFieldValueKind::OwnedBytes);
            layout.validate(&program).unwrap();
        }
    }

    let generic_program = hir::resolve(
        &parse(
            GENERIC_SOURCE,
            Path::new("generic-owned-byte-layout-rejection.spx"),
        )
        .unwrap(),
    )
    .unwrap();
    let source_generic_bytes = nominal("choice.generic", vec![ResolvedType::Bytes]);
    assert!(VariantLayout::for_type(
        &generic_program,
        VariantTarget::Native64,
        &source_generic_bytes,
    )
    .is_err());
}

#[test]
fn hostile_layout_and_declaration_mutations_are_rejected_independently() {
    let program = resolved();
    let variant = DeclarationId::new("choice.type");
    let canonical = VariantLayout::for_variant(&program, VariantTarget::Wasm32, &variant)
        .expect("canonical layout");

    let mut reordered = canonical.clone();
    reordered.cases.swap(0, 1);
    assert!(reordered.validate(&program).is_err());

    let mut retagged = canonical.clone();
    retagged.cases[0].tag = 1;
    assert!(retagged.validate(&program).is_err());

    let mut overlapping = canonical.clone();
    overlapping.payload_offset = 4;
    assert!(overlapping.validate(&program).is_err());

    let mut undersized = canonical.clone();
    undersized.payload_size -= 1;
    assert!(undersized.validate(&program).is_err());

    let mut misaligned = canonical.clone();
    misaligned.cases[2].fields[1].offset = 9;
    assert!(misaligned.validate(&program).is_err());

    let mut digest_confused = canonical.clone();
    digest_confused.target = VariantTarget::Native64;
    assert!(digest_confused.validate(&program).is_err());

    let mut duplicate_case = program.clone();
    let declaration = duplicate_case
        .types
        .iter_mut()
        .find(|item| item.id == variant)
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
        panic!("choice is a variant")
    };
    cases[1].id = cases[0].id.clone();
    assert!(VariantLayout::for_variant(&duplicate_case, VariantTarget::Wasm32, &variant).is_err());

    let mut noncanonical_tag = program;
    let declaration = noncanonical_tag
        .types
        .iter_mut()
        .find(|item| item.id == variant)
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut declaration.kind else {
        panic!("choice is a variant")
    };
    cases[1].index = u32::MAX;
    assert!(
        VariantLayout::for_variant(&noncanonical_tag, VariantTarget::Wasm32, &variant).is_err()
    );
}
