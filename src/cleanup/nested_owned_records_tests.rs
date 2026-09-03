use super::{
    cleanup_shape_profile, field_liveness_shapes_equal, FieldLiveness, FieldLivenessShape,
    LivenessFlagId, BYTES_DROP_LIFECYCLE_ID, MAX_CLEANUP_OWNED_LEAVES, MAX_CLEANUP_SHAPE_DEPTH,
    MAX_CLEANUP_VISITED_FIELDS,
};
use crate::hir::DeclarationId;

fn bytes_leaf(index: usize) -> FieldLivenessShape {
    FieldLivenessShape::Leaf {
        flag: LivenessFlagId(u32::try_from(index).unwrap()),
        lifecycle: DeclarationId::new(BYTES_DROP_LIFECYCLE_ID),
    }
}

fn record(name: &str, fields: Vec<FieldLivenessShape>) -> FieldLivenessShape {
    FieldLivenessShape::Record {
        declaration: DeclarationId::new(name),
        fields: fields
            .into_iter()
            .enumerate()
            .map(|(index, shape)| FieldLiveness {
                field: DeclarationId::new(format!("{name}.field.{index}")),
                field_index: u32::try_from(index).unwrap(),
                shape,
            })
            .collect(),
    }
}

fn nested_record(depth: usize) -> FieldLivenessShape {
    let mut shape = bytes_leaf(0);
    for index in (0..depth).rev() {
        shape = record(&format!("record.{index}"), vec![shape]);
    }
    shape
}

#[test]
fn nested_owned_bytes_profile_is_closed_and_legacy_flat_shape_stays_outside() {
    let flat = cleanup_shape_profile(&record("flat", vec![bytes_leaf(0)])).unwrap();
    assert!(!flat.has_nested_owned_bytes);
    assert_eq!(flat.maximum_record_depth, 1);

    let nested = cleanup_shape_profile(&nested_record(2)).unwrap();
    assert!(nested.has_nested_owned_bytes);
    assert_eq!(nested.owned_leaves, 1);
    assert_eq!(nested.visited_fields, 2);
    assert_eq!(nested.maximum_record_depth, 2);
}

#[test]
fn exact_structural_limits_admit_and_plus_one_rejects() {
    let depth_exact = nested_record(MAX_CLEANUP_SHAPE_DEPTH);
    assert_eq!(
        cleanup_shape_profile(&depth_exact)
            .unwrap()
            .maximum_record_depth,
        MAX_CLEANUP_SHAPE_DEPTH
    );
    assert!(cleanup_shape_profile(&nested_record(MAX_CLEANUP_SHAPE_DEPTH + 1)).is_err());

    let leaves_exact = record(
        "leaves.outer.exact",
        vec![record(
            "leaves.inner.exact",
            (0..MAX_CLEANUP_OWNED_LEAVES).map(bytes_leaf).collect(),
        )],
    );
    assert_eq!(
        cleanup_shape_profile(&leaves_exact).unwrap().owned_leaves,
        MAX_CLEANUP_OWNED_LEAVES
    );
    let leaves_plus_one = record(
        "leaves.outer.plus-one",
        vec![record(
            "leaves.inner.plus-one",
            (0..=MAX_CLEANUP_OWNED_LEAVES).map(bytes_leaf).collect(),
        )],
    );
    assert!(cleanup_shape_profile(&leaves_plus_one).is_err());

    let mut exact_inner_fields = (0..MAX_CLEANUP_VISITED_FIELDS - 1)
        .map(|_| FieldLivenessShape::NoDrop)
        .collect::<Vec<_>>();
    exact_inner_fields[0] = bytes_leaf(0);
    let fields_exact = record(
        "fields.outer.exact",
        vec![record("fields.inner.exact", exact_inner_fields)],
    );
    assert_eq!(
        cleanup_shape_profile(&fields_exact).unwrap().visited_fields,
        MAX_CLEANUP_VISITED_FIELDS
    );
    let mut plus_one_inner_fields = (0..MAX_CLEANUP_VISITED_FIELDS)
        .map(|_| FieldLivenessShape::NoDrop)
        .collect::<Vec<_>>();
    plus_one_inner_fields[0] = bytes_leaf(0);
    let fields_plus_one = record(
        "fields.outer.plus-one",
        vec![record("fields.inner.plus-one", plus_one_inner_fields)],
    );
    assert!(cleanup_shape_profile(&fields_plus_one).is_err());
}

#[test]
fn declaration_order_and_full_stable_path_mutations_never_compare_equal() {
    let canonical = record(
        "outer",
        vec![
            record("inner", vec![bytes_leaf(0)]),
            FieldLivenessShape::NoDrop,
        ],
    );
    let mut reordered = canonical.clone();
    let FieldLivenessShape::Record { fields, .. } = &mut reordered else {
        unreachable!()
    };
    fields.swap(0, 1);
    assert!(!field_liveness_shapes_equal(&reordered, &canonical).unwrap());

    let mut renamed = canonical.clone();
    let FieldLivenessShape::Record { fields, .. } = &mut renamed else {
        unreachable!()
    };
    let FieldLivenessShape::Record { fields, .. } = &mut fields[0].shape else {
        unreachable!()
    };
    fields[0].field = DeclarationId::new("inner.mutated");
    assert!(!field_liveness_shapes_equal(&renamed, &canonical).unwrap());
}
