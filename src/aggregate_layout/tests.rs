use std::path::Path;

use super::{
    align_up, owned_bytes_size_align, AggregateFieldValueKind, AggregateLayout,
    AggregateLayoutCache, AggregateTarget,
};
use crate::hir::{
    self, DeclarationId, ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::parse;

const SOURCE: &str = r#"
module test.aggregate_layout;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("inner.type")
record Inner {
    @id("inner.flag")
    flag: bool,
    @id("inner.token")
    token: Token,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.count")
    count: i64,
    @id("outer.inner")
    inner: Inner,
}

@id("empty.type")
record Empty {}

@id("array-holder.type")
record ArrayHolder {
    @id("array-holder.bytes")
    bytes: [u8; 7],
    @id("array-holder.zero")
    zero: [u8; 0],
}

@id("generic-pair.type")
record GenericPair<T, U> {
    @id("generic-pair.left")
    left: T,
    @id("generic-pair.right")
    right: U,
}

@id("generic-box.type")
record GenericBox<T> {
    @id("generic-box.value")
    value: T,
}

@id("mono-wrapper.type")
record MonoWrapper {
    @id("mono-wrapper.value")
    value: GenericBox<Bytes>,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> hir::ResolvedProgram {
    hir::resolve(&parse(SOURCE, Path::new("aggregate-layout.spx")).unwrap()).unwrap()
}

fn replace_array_holder_fields(
    program: &mut hir::ResolvedProgram,
    field_count: usize,
    bytes_fields: usize,
) {
    let holder = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "array-holder.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut holder.kind else {
        unreachable!()
    };
    let prototype = fields[0].clone();
    *fields = (0..field_count)
        .map(|index| {
            let mut field = prototype.clone();
            field.id = DeclarationId::new(format!("array-holder.f{index}"));
            field.index = u32::try_from(index).unwrap();
            field.ty = if index < bytes_fields {
                ResolvedType::Bytes
            } else {
                ResolvedType::Bool
            };
            field
        })
        .collect();
}

fn replace_mono_wrapper_field(program: &mut hir::ResolvedProgram, ty: ResolvedType) {
    let wrapper = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "mono-wrapper.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut wrapper.kind else {
        unreachable!()
    };
    fields[0].ty = ty;
}

#[test]
fn native_and_wasm_layouts_have_frozen_offsets_and_digests() {
    let program = program();
    let record = DeclarationId::new("outer.type");
    let native = AggregateLayout::for_record(&program, AggregateTarget::Native64, &record).unwrap();
    assert_eq!((native.size, native.align), (32, 8));
    assert_eq!(
        native
            .fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        vec![
            ("outer.flag", 0, 1, 1),
            ("outer.count", 8, 8, 8),
            ("outer.inner", 16, 16, 8),
        ]
    );
    assert_eq!(
        native.digest_hex(),
        "695de29a8ad639fd9272ebaa04faf75d64205881d37f06f93ad4a339e3c36c1f"
    );
    native.validate(&program).unwrap();

    let wasm = AggregateLayout::for_record(&program, AggregateTarget::Wasm32, &record).unwrap();
    assert_eq!((wasm.size, wasm.align), (24, 8));
    assert_eq!(
        wasm.fields
            .iter()
            .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        vec![
            ("outer.flag", 0, 4, 4),
            ("outer.count", 8, 8, 8),
            ("outer.inner", 16, 8, 4),
        ]
    );
    assert_eq!(
        wasm.digest_hex(),
        "4e2d57512c9a80b6c4bfecc371373e52f1c1045206f7c63d94ebdb579cc5b11d"
    );
    wasm.validate(&program).unwrap();

    let empty = DeclarationId::new("empty.type");
    let native_empty =
        AggregateLayout::for_record(&program, AggregateTarget::Native64, &empty).unwrap();
    let wasm_empty =
        AggregateLayout::for_record(&program, AggregateTarget::Wasm32, &empty).unwrap();
    assert_eq!((native_empty.size, native_empty.align), (1, 1));
    assert_eq!((wasm_empty.size, wasm_empty.align), (1, 1));
    assert!(native_empty.fields.is_empty());
    assert!(wasm_empty.fields.is_empty());
    assert_eq!(
        native_empty.digest_hex(),
        "13d181500c46e00b711fd2374705971496e00a04f0cbacc546bff1e2339e3140"
    );
    assert_eq!(
        wasm_empty.digest_hex(),
        "2cef34f3db54e52a15b8d8e123867b6ba592de0d1f6fc49597a98714f03b3f1d"
    );
    native_empty.validate(&program).unwrap();
    wasm_empty.validate(&program).unwrap();
    let mut zero_sized = native_empty;
    zero_sized.size = 0;
    assert!(zero_sized.validate(&program).is_err());
}

#[test]
fn fixed_byte_arrays_have_target_independent_size_n_alignment_one() {
    let program = program();
    let record = DeclarationId::new("array-holder.type");
    for target in [AggregateTarget::Native64, AggregateTarget::Wasm32] {
        let layout = AggregateLayout::for_record(&program, target, &record).unwrap();
        assert_eq!((layout.size, layout.align), (7, 1));
        assert_eq!(
            layout
                .fields
                .iter()
                .map(|field| (field.field.as_str(), field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            vec![
                ("array-holder.bytes", 0, 7, 1),
                ("array-holder.zero", 7, 0, 1),
            ]
        );
        layout.validate(&program).unwrap();
    }
}

#[test]
fn direct_owned_bytes_layout_matches_existing_carriers_without_becoming_copy() {
    assert_eq!(owned_bytes_size_align(AggregateTarget::Native64), (16, 8));
    assert_eq!(owned_bytes_size_align(AggregateTarget::Wasm32), (8, 8));

    // Source admission intentionally remains closed. Mutating resolved
    // type facts isolates this internal layout substrate from the parser,
    // verifier, public ABI, Project, and backend profiles.
    let mut program = program();
    let holder = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "array-holder.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut holder.kind else {
        panic!("array holder is a record")
    };
    fields[0].ty = ResolvedType::Bytes;
    fields[1].ty = ResolvedType::U8;
    let instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("array-holder.type"),
        arguments: Vec::new(),
    };
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap()
        .return_type = instance.clone();

    let native = AggregateLayout::for_type(&program, AggregateTarget::Native64, &instance)
        .expect("Native64 owned-byte record layout");
    assert_eq!((native.size, native.align), (24, 8));
    assert_eq!(
        native
            .fields
            .iter()
            .map(|field| (
                field.field.as_str(),
                field.value_kind,
                field.offset,
                field.size,
                field.align,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "array-holder.bytes",
                AggregateFieldValueKind::OwnedBytes,
                0,
                16,
                8,
            ),
            ("array-holder.zero", AggregateFieldValueKind::Copy, 16, 1, 1,),
        ]
    );
    native.validate(&program).unwrap();

    let wasm = AggregateLayout::for_type(&program, AggregateTarget::Wasm32, &instance)
        .expect("Wasm32 owned-byte record layout");
    assert_eq!((wasm.size, wasm.align), (16, 8));
    assert_eq!(
        wasm.fields
            .iter()
            .map(|field| (field.value_kind, field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        vec![
            (AggregateFieldValueKind::OwnedBytes, 0, 8, 8),
            (AggregateFieldValueKind::Copy, 8, 4, 4),
        ]
    );
    wasm.validate(&program).unwrap();

    for target in [AggregateTarget::Native64, AggregateTarget::Wasm32] {
        let cache = AggregateLayoutCache::build(&program, target).unwrap();
        assert_eq!(
            cache.layout(&instance).unwrap().fields[0].value_kind,
            AggregateFieldValueKind::OwnedBytes
        );
    }

    let mut copy_confused = native;
    copy_confused.fields[0].value_kind = AggregateFieldValueKind::Copy;
    assert!(copy_confused.validate(&program).is_err());
}

#[test]
fn concrete_generic_owned_bytes_layout_substitutes_before_target_layout() {
    let program = program();
    let scalars = [
        ResolvedType::I64,
        ResolvedType::U8,
        ResolvedType::I32,
        ResolvedType::Usize,
        ResolvedType::Char,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
    ];
    let mut digests = std::collections::BTreeSet::new();
    for scalar in scalars {
        let instance = ResolvedType::Nominal {
            declaration: DeclarationId::new("generic-pair.type"),
            arguments: vec![ResolvedType::Bytes, scalar.clone()],
        };
        for (target, expected_size, expected_offset) in [
            (AggregateTarget::Native64, 24, 16),
            (AggregateTarget::Wasm32, 16, 8),
        ] {
            let layout = AggregateLayout::for_type(&program, target, &instance)
                .expect("concrete generic owned-byte layout");
            assert_eq!((layout.size, layout.align), (expected_size, 8));
            assert_eq!(
                layout
                    .fields
                    .iter()
                    .map(|field| (field.ty.clone(), field.value_kind, field.offset))
                    .collect::<Vec<_>>(),
                vec![
                    (ResolvedType::Bytes, AggregateFieldValueKind::OwnedBytes, 0),
                    (
                        scalar.clone(),
                        AggregateFieldValueKind::Copy,
                        expected_offset
                    ),
                ]
            );
            layout.validate(&program).unwrap();
            assert!(digests.insert(layout.digest_hex().to_owned()));

            let mut substituted = layout;
            substituted.fields[1].ty = if scalar == ResolvedType::Bool {
                ResolvedType::I64
            } else {
                ResolvedType::Bool
            };
            assert!(substituted.validate(&program).is_err());
        }
    }
}

#[test]
fn nested_concrete_generic_layouts_substitute_recursively_and_remain_distinct() {
    let program = program();
    let pair_bytes_bool = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic-pair.type"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
    };
    let box_pair = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic-box.type"),
        arguments: vec![pair_bytes_bool],
    };
    let box_bytes = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic-box.type"),
        arguments: vec![ResolvedType::Bytes],
    };
    let pair_box = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic-pair.type"),
        arguments: vec![box_bytes, ResolvedType::I64],
    };

    for (target, expected_size) in [
        (AggregateTarget::Native64, 24),
        (AggregateTarget::Wasm32, 16),
    ] {
        let outer_box = AggregateLayout::for_type(&program, target, &box_pair)
            .expect("Box<Pair<Bytes,bool>> layout");
        assert_eq!((outer_box.size, outer_box.align), (expected_size, 8));
        assert_eq!(outer_box.fields.len(), 1);
        assert_eq!(
            outer_box.fields[0].value_kind,
            AggregateFieldValueKind::Aggregate
        );
        assert_eq!(outer_box.fields[0].offset, 0);
        outer_box.validate(&program).unwrap();

        let outer_pair = AggregateLayout::for_type(&program, target, &pair_box)
            .expect("Pair<Box<Bytes>,i64> layout");
        assert_eq!((outer_pair.size, outer_pair.align), (expected_size, 8));
        assert_eq!(
            outer_pair
                .fields
                .iter()
                .map(|field| (field.value_kind, field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            [
                (AggregateFieldValueKind::Aggregate, 0, expected_size - 8, 8),
                (AggregateFieldValueKind::Copy, expected_size - 8, 8, 8),
            ]
        );
        outer_pair.validate(&program).unwrap();
        assert_ne!(outer_box.digest, outer_pair.digest);

        let mut forged_child = outer_box;
        forged_child.fields[0].nested_digest[0] ^= 1;
        assert!(forged_child.validate(&program).is_err());

        let mut forged_carrier = outer_pair;
        forged_carrier.fields[0].value_kind = AggregateFieldValueKind::OwnedBytes;
        assert!(forged_carrier.validate(&program).is_err());
    }
}

#[test]
fn nested_concrete_generic_layout_admission_is_bounded_and_record_only() {
    let program = program();
    let box_id = DeclarationId::new("generic-box.type");
    let boxed = |argument| ResolvedType::Nominal {
        declaration: box_id.clone(),
        arguments: vec![argument],
    };

    let resource_argument = boxed(ResolvedType::Nominal {
        declaration: DeclarationId::new("token.type"),
        arguments: Vec::new(),
    });
    assert!(
        AggregateLayout::for_type(&program, AggregateTarget::Native64, &resource_argument).is_err()
    );

    let mut at_limit = ResolvedType::Bytes;
    for _ in 0..64 {
        at_limit = boxed(at_limit);
    }
    AggregateLayout::for_type(&program, AggregateTarget::Wasm32, &at_limit)
        .expect("64 record levels are admitted");
    let plus_one = boxed(at_limit);
    assert!(AggregateLayout::for_type(&program, AggregateTarget::Wasm32, &plus_one).is_err());

    let nested_holder = boxed(ResolvedType::Nominal {
        declaration: DeclarationId::new("array-holder.type"),
        arguments: Vec::new(),
    });
    let mut leaf_limit = program.clone();
    replace_array_holder_fields(&mut leaf_limit, 256, 256);
    AggregateLayout::for_type(&leaf_limit, AggregateTarget::Wasm32, &nested_holder)
        .expect("root plus descendant admits exactly 256 owned leaves");
    let mut leaf_plus_one = program.clone();
    replace_array_holder_fields(&mut leaf_plus_one, 257, 257);
    assert!(
        AggregateLayout::for_type(&leaf_plus_one, AggregateTarget::Wasm32, &nested_holder).is_err()
    );

    let mut field_limit = program.clone();
    replace_array_holder_fields(&mut field_limit, 4_095, 1);
    AggregateLayout::for_type(&field_limit, AggregateTarget::Wasm32, &nested_holder)
        .expect("root plus descendant admits exactly 4096 visited fields");
    let mut field_plus_one = program.clone();
    replace_array_holder_fields(&mut field_plus_one, 4_096, 1);
    assert!(
        AggregateLayout::for_type(&field_plus_one, AggregateTarget::Wasm32, &nested_holder)
            .is_err()
    );

    let mut cyclic = program.clone();
    let holder = cyclic
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "array-holder.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut holder.kind else {
        unreachable!()
    };
    fields[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("array-holder.type"),
        arguments: Vec::new(),
    };
    let cyclic_argument = boxed(ResolvedType::Nominal {
        declaration: DeclarationId::new("array-holder.type"),
        arguments: Vec::new(),
    });
    assert!(
        AggregateLayout::for_type(&cyclic, AggregateTarget::Native64, &cyclic_argument).is_err()
    );
}

#[test]
fn monomorphic_wrapper_authenticates_one_complete_nested_generic_budget() {
    let program = program();
    let wrapper = ResolvedType::Nominal {
        declaration: DeclarationId::new("mono-wrapper.type"),
        arguments: Vec::new(),
    };
    let boxed = |argument| ResolvedType::Nominal {
        declaration: DeclarationId::new("generic-box.type"),
        arguments: vec![argument],
    };

    let mut exact_depth = ResolvedType::Bytes;
    for _ in 0..63 {
        exact_depth = boxed(exact_depth);
    }
    let mut depth_limit = program.clone();
    replace_mono_wrapper_field(&mut depth_limit, exact_depth.clone());
    AggregateLayout::for_type(&depth_limit, AggregateTarget::Wasm32, &wrapper)
        .expect("monomorphic root plus generic descendants admits depth 64");
    let mut depth_plus_one = program.clone();
    replace_mono_wrapper_field(&mut depth_plus_one, boxed(exact_depth));
    assert!(AggregateLayout::for_type(&depth_plus_one, AggregateTarget::Wasm32, &wrapper).is_err());

    let nested_holder = boxed(ResolvedType::Nominal {
        declaration: DeclarationId::new("array-holder.type"),
        arguments: Vec::new(),
    });
    let mut leaf_limit = program.clone();
    replace_array_holder_fields(&mut leaf_limit, 256, 256);
    replace_mono_wrapper_field(&mut leaf_limit, nested_holder.clone());
    AggregateLayout::for_type(&leaf_limit, AggregateTarget::Native64, &wrapper)
        .expect("monomorphic root plus generic descendant admits 256 leaves");
    let mut leaf_plus_one = program.clone();
    replace_array_holder_fields(&mut leaf_plus_one, 257, 257);
    replace_mono_wrapper_field(&mut leaf_plus_one, nested_holder.clone());
    assert!(
        AggregateLayout::for_type(&leaf_plus_one, AggregateTarget::Native64, &wrapper).is_err()
    );

    let mut field_limit = program.clone();
    replace_array_holder_fields(&mut field_limit, 4_094, 1);
    replace_mono_wrapper_field(&mut field_limit, nested_holder.clone());
    AggregateLayout::for_type(&field_limit, AggregateTarget::Wasm32, &wrapper)
        .expect("wrapper, generic box, and descendant admit 4096 total fields");
    let mut field_plus_one = program;
    replace_array_holder_fields(&mut field_plus_one, 4_095, 1);
    replace_mono_wrapper_field(&mut field_plus_one, nested_holder);
    assert!(AggregateLayout::for_type(&field_plus_one, AggregateTarget::Wasm32, &wrapper).is_err());
}

#[test]
fn exact_reconstruction_rejects_reorder_overlap_undersize_and_alignment_mutations() {
    let program = program();
    let canonical = AggregateLayout::for_record(
        &program,
        AggregateTarget::Native64,
        &DeclarationId::new("outer.type"),
    )
    .unwrap();

    let mut reordered = canonical.clone();
    reordered.fields.swap(0, 1);
    assert!(reordered.validate(&program).is_err());

    let mut overlapping = canonical.clone();
    overlapping.fields[1].offset = 0;
    assert!(overlapping.validate(&program).is_err());

    let mut undersized = canonical.clone();
    undersized.size -= 1;
    assert!(undersized.validate(&program).is_err());

    let mut misaligned = canonical;
    misaligned.fields[1].align = 4;
    assert!(misaligned.validate(&program).is_err());
}

#[test]
fn unknown_duplicate_recursive_and_imported_resource_inputs_fail_closed() {
    let mut hostile_program = program();
    assert!(AggregateLayout::for_record(
        &hostile_program,
        AggregateTarget::Native64,
        &DeclarationId::new("missing.type")
    )
    .is_err());

    let duplicate = hostile_program.types[1].clone();
    hostile_program.types.push(duplicate);
    assert!(AggregateLayout::for_record(
        &hostile_program,
        AggregateTarget::Native64,
        &DeclarationId::new("inner.type")
    )
    .is_err());

    hostile_program.types.pop();
    let inner = hostile_program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "inner.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut inner.kind else {
        unreachable!()
    };
    fields[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("inner.type"),
        arguments: Vec::new(),
    };
    assert!(AggregateLayout::for_record(
        &hostile_program,
        AggregateTarget::Native64,
        &DeclarationId::new("inner.type")
    )
    .is_err());

    let mut imported_program = program();
    let token = imported_program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "token.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut token.kind else {
        unreachable!()
    };
    drop.kind = ResolvedResourceDropKind::Imported {
        import: DeclarationId::new("host.drop"),
        import_key: "host.drop".to_owned(),
    };
    assert!(AggregateLayout::for_record(
        &imported_program,
        AggregateTarget::Wasm32,
        &DeclarationId::new("outer.type")
    )
    .is_err());

    assert!(align_up(u32::MAX, 8).is_err());
}

#[test]
fn field_lookup_uses_stable_identity() {
    let program = program();
    let layout = AggregateLayout::for_record(
        &program,
        AggregateTarget::Native64,
        &DeclarationId::new("outer.type"),
    )
    .unwrap();
    assert_eq!(
        layout
            .field(&DeclarationId::new("outer.inner"))
            .map(|field| field.offset),
        Some(16)
    );
    assert!(layout.field(&DeclarationId::new("inner.token")).is_none());
}

#[test]
fn generic_instances_bind_cache_digest_and_field_substitution_even_when_layouts_match() {
    let source = r#"
module test.generic_aggregate_layout;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.phantom") record Phantom<T> { @id("test.phantom.marker") marker: bool, }
@id("test.use_i64") fn use_i64(value: Phantom<i64>) -> bool { value.marker }
@id("test.use_bool") fn use_bool(value: Phantom<bool>) -> bool { value.marker }
@id("test.make_i64") fn make_i64(value: i64) -> Box<i64> { Box<i64> { value: value } }
@id("test.make_bool") fn make_bool(value: bool) -> Box<bool> { Box<bool> { value: value } }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program =
        hir::resolve(&parse(source, Path::new("generic-aggregate-layout.spx")).unwrap()).unwrap();
    let phantom_i64 = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.phantom"),
        arguments: vec![ResolvedType::I64],
    };
    let phantom_bool = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.phantom"),
        arguments: vec![ResolvedType::Bool],
    };
    let i64_layout =
        AggregateLayout::for_type(&program, AggregateTarget::Native64, &phantom_i64).unwrap();
    let bool_layout =
        AggregateLayout::for_type(&program, AggregateTarget::Native64, &phantom_bool).unwrap();
    assert_eq!(
        (i64_layout.size, i64_layout.align),
        (bool_layout.size, bool_layout.align)
    );
    assert_eq!(
        i64_layout
            .fields
            .iter()
            .map(|field| (field.offset, field.size, field.align))
            .collect::<Vec<_>>(),
        bool_layout
            .fields
            .iter()
            .map(|field| (field.offset, field.size, field.align))
            .collect::<Vec<_>>()
    );
    assert_ne!(phantom_i64.identity_key(), phantom_bool.identity_key());
    assert_ne!(i64_layout.digest, bool_layout.digest);

    let cache = AggregateLayoutCache::build(&program, AggregateTarget::Native64).unwrap();
    assert_eq!(cache.layout(&phantom_i64).unwrap(), &i64_layout);
    assert_eq!(cache.layout(&phantom_bool).unwrap(), &bool_layout);
    assert_eq!(
        cache
            .layouts()
            .filter(|layout| layout.record.as_str() == "test.phantom")
            .count(),
        2
    );

    let mut relabeled = i64_layout;
    relabeled.instance = phantom_bool;
    assert!(relabeled.validate(&program).is_err());

    let box_i64 = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.box"),
        arguments: vec![ResolvedType::I64],
    };
    let box_bool = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.box"),
        arguments: vec![ResolvedType::Bool],
    };
    assert_eq!(
        cache.layout(&box_i64).unwrap().fields[0].ty,
        ResolvedType::I64
    );
    assert_eq!(
        cache.layout(&box_bool).unwrap().fields[0].ty,
        ResolvedType::Bool
    );
}
