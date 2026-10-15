//! Complete current and original admission controls with full old substitution.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
fn summary(value: NestedOwnedRecordAdmission) -> (u8, usize, usize) {
    match value {
        NestedOwnedRecordAdmission::Admitted(f) => (0, f.byte_leaves, f.visited_fields),
        NestedOwnedRecordAdmission::NoOwnedBytes => (1, 0, 0),
        NestedOwnedRecordAdmission::OutsideProfile => (2, 0, 0),
        NestedOwnedRecordAdmission::Recursive => (3, 0, 0),
        NestedOwnedRecordAdmission::LimitExceeded => (4, 0, 0),
    }
}
fn parity(index: &DeclarationIndex, root: &ResolvedType) {
    assert_eq!(
        summary(classify_nested_owned_byte_record(index, root)),
        summary(leaf_saved::classify_nested_owned_byte_record(index, root))
    );
    super::classifier_tests::leaf_parity(index, root);
}
#[test]
fn classifier_leaf_roots_keep_all_original_profile_results_with_literal_zero_backing() {
    let index = DeclarationIndex::default();
    use ResolvedType::*;
    let owner = DeclarationId::new("leaf.hostile.完全");
    let mut roots = vec![
        Never,
        Unit,
        I64,
        I32,
        Char,
        U8,
        Usize,
        ArrayU8(u32::MAX),
        F32,
        F64,
        Bool,
        String,
        OnceFunction,
        OnceFunctionI64,
        OnceFunctionI64Pair,
        MutFunctionI64,
        Bytes,
        Str,
        SliceU8,
        StringMap,
        TypeParameter {
            owner: owner.clone(),
            index: u32::MAX,
        },
    ];
    roots.push(Function {
        parameters: vec![
            Nominal {
                declaration: owner,
                arguments: vec![Bytes; 16]
            };
            16
        ],
        result: Box::new(I64),
    });
    for root in roots {
        parity(&index, &root);
        let expected = summary(leaf_saved::classify_nested_owned_byte_record(&index, &root));
        #[allow(
            clippy::drop_non_drop,
            reason = "End this borrowed proof at the measured lifetime boundary."
        )]
        let (_, old) =
            measure(|| drop(leaf_saved::classify_nested_owned_byte_record(&index, &root)));
        let (((), current), refused, used, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert_eq!(
                    summary(classify_nested_owned_byte_record(&index, &root)),
                    expected
                );
            })
        });
        assert!(!refused && used == 0 && first.is_none());
        assert!(
            !current.invalid
                && current.allocated == 0
                && current.allocations == 0
                && current.live == 0
        );
        assert!(!old.invalid && old.allocated > 0 && old.live == 0);
        assert_eq!(old.allocated, old.freed);
    }
}
const SOURCE: &str = r#"module leaf.caller;
@id("leaf.plain") record Plain { @id("leaf.plain.bytes") bytes:Bytes, @id("leaf.plain.count") count:i64, }
@id("leaf.generic") record Generic<T> { @id("leaf.generic.value") value:T, }
@id("leaf.main") fn main()->i64 {0}
"#;
#[test]
fn real_nested_record_caller_preserves_full_old_limits_cycles_and_scalar_field_drop() {
    let ast = crate::parse(SOURCE, "leaf-caller.spx").unwrap();
    let mut index = crate::hir::resolve(&ast).unwrap().declarations;
    let plain = DeclarationId::new("leaf.plain");
    let generic = DeclarationId::new("leaf.generic");
    let root = ResolvedType::Nominal {
        declaration: plain.clone(),
        arguments: vec![],
    };
    parity(&index, &root);
    #[allow(
        clippy::drop_non_drop,
        reason = "End this borrowed proof at the measured lifetime boundary."
    )]
    let (_, old) = measure(|| drop(leaf_saved::classify_nested_owned_byte_record(&index, &root)));
    #[allow(
        clippy::drop_non_drop,
        reason = "End this borrowed proof at the measured lifetime boundary."
    )]
    let (_, current) = measure(|| drop(classify_nested_owned_byte_record(&index, &root)));
    assert!(!old.invalid && !current.invalid && old.live == 0 && current.live == 0);
    assert_eq!(old.allocated, old.freed);
    assert_eq!(current.allocated, current.freed);
    assert!(
        current.allocated < old.allocated,
        "actual scalar-field substitution caller"
    );
    for arguments in [
        vec![],
        vec![ResolvedType::I64],
        vec![ResolvedType::Bytes],
        vec![ResolvedType::String],
        vec![ResolvedType::I64, ResolvedType::Bytes],
    ] {
        parity(
            &index,
            &ResolvedType::Nominal {
                declaration: generic.clone(),
                arguments,
            },
        );
    }
    for leaf in [
        ResolvedType::Bytes,
        ResolvedType::I64,
        ResolvedType::String,
        ResolvedType::Never,
    ] {
        let mut nested = leaf;
        for _ in 0..65 {
            nested = ResolvedType::Nominal {
                declaration: generic.clone(),
                arguments: vec![nested],
            };
            parity(&index, &nested);
        }
    }
    let field = index.record_fields[&plain][0].clone();
    for count in [0, 1, 256, 257, 4096, 4097] {
        let mut fields = vec![field.clone(); count];
        for field in &mut fields {
            field.ty = ResolvedType::I64;
        }
        if let Some(first) = fields.first_mut() {
            first.ty = ResolvedType::Bytes;
        }
        index.record_fields.insert(plain.clone(), fields);
        parity(&index, &root);
    }
    let mut recursive = field.clone();
    recursive.ty = root.clone();
    index.record_fields.insert(plain.clone(), vec![recursive]);
    parity(&index, &root);
    for ty in [
        ResolvedType::TypeParameter {
            owner: generic.clone(),
            index: 0,
        },
        ResolvedType::TypeParameter {
            owner: plain.clone(),
            index: u32::MAX,
        },
    ] {
        let mut malformed = field.clone();
        malformed.ty = ty;
        index.record_fields.insert(plain.clone(), vec![malformed]);
        parity(&index, &root);
    }
}
