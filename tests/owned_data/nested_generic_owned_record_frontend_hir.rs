use std::path::Path;

use semaprax::cleanup::{FieldLiveness, FieldLivenessShape, BYTES_DROP_LIFECYCLE_ID};
use semaprax::hir::{self, DeclarationId, ResolvedType};
use semaprax::{parse, verify};

const SOURCE: &str = r#"
module test.nested_generic_owned_record_frontend_hir;

@id("nested.generic.pair")
record Pair<T, U> {
  @id("nested.generic.pair.left") left: T,
  @id("nested.generic.pair.right") right: U,
}

@id("nested.generic.box")
record Box<T> {
  @id("nested.generic.box.value") value: T,
  @id("nested.generic.box.stamp") stamp: bool,
}

@id("nested.generic.consume-box")
fn consume_box(value: own Box<Pair<Bytes, bool>>) -> i64 { 0 }

@id("nested.generic.consume-pair")
fn consume_pair(value: own Pair<Box<Bytes>, i64>) -> i64 { 0 }

@id("app.main") fn main() -> i64 { 0 }
"#;

fn resolved() -> hir::ResolvedProgram {
    let parsed = parse(
        SOURCE,
        Path::new("nested-generic-owned-record-frontend-v1.spx"),
    )
    .expect("nested generic fixture parses");
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "nested generic fixture verifies: {diagnostics:?}"
    );
    hir::resolve(&parsed).expect("nested generic fixture resolves and replays")
}

fn nominal(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(declaration),
        arguments,
    }
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function {id}"))
}

fn slot_shape<'a>(
    function: &'a hir::ResolvedFunction,
    ty: &ResolvedType,
) -> &'a FieldLivenessShape {
    &function
        .cleanup
        .slots
        .iter()
        .find(|slot| slot.ty == *ty)
        .unwrap_or_else(|| panic!("missing cleanup slot for {}", ty.identity_key()))
        .shape
}

fn record_fields(shape: &FieldLivenessShape) -> &[FieldLiveness] {
    let FieldLivenessShape::Record { fields, .. } = shape else {
        panic!("expected a record cleanup shape, got {shape:?}")
    };
    fields
}

fn assert_bytes_leaf(shape: &FieldLivenessShape) {
    assert!(matches!(
        shape,
        FieldLivenessShape::Leaf { lifecycle, .. }
            if lifecycle.as_str() == BYTES_DROP_LIFECYCLE_ID
    ));
}

#[test]
fn nested_concrete_generic_instances_substitute_before_hir_and_cleanup() {
    let program = resolved();
    let pair_bytes_bool = nominal(
        "nested.generic.pair",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    let box_pair = nominal("nested.generic.box", vec![pair_bytes_bool.clone()]);
    let box_bytes = nominal("nested.generic.box", vec![ResolvedType::Bytes]);
    let pair_box = nominal(
        "nested.generic.pair",
        vec![box_bytes.clone(), ResolvedType::I64],
    );

    let consume_box = function(&program, "nested.generic.consume-box");
    assert_eq!(consume_box.params[0].ty, box_pair);
    let outer = record_fields(slot_shape(consume_box, &box_pair));
    assert_eq!(outer[0].field.as_str(), "nested.generic.box.value");
    assert_eq!(outer[1].field.as_str(), "nested.generic.box.stamp");
    let nested_pair = record_fields(&outer[0].shape);
    assert_eq!(
        nested_pair
            .iter()
            .map(|field| field.field.as_str())
            .collect::<Vec<_>>(),
        ["nested.generic.pair.left", "nested.generic.pair.right"]
    );
    assert_bytes_leaf(&nested_pair[0].shape);
    assert!(matches!(nested_pair[1].shape, FieldLivenessShape::NoDrop));
    assert!(matches!(outer[1].shape, FieldLivenessShape::NoDrop));

    let consume_pair = function(&program, "nested.generic.consume-pair");
    assert_eq!(consume_pair.params[0].ty, pair_box);
    let outer = record_fields(slot_shape(consume_pair, &pair_box));
    assert_eq!(outer[0].field.as_str(), "nested.generic.pair.left");
    assert_eq!(outer[1].field.as_str(), "nested.generic.pair.right");
    let nested_box = record_fields(&outer[0].shape);
    assert_eq!(
        nested_box
            .iter()
            .map(|field| field.field.as_str())
            .collect::<Vec<_>>(),
        ["nested.generic.box.value", "nested.generic.box.stamp"]
    );
    assert_bytes_leaf(&nested_box[0].shape);
    assert!(matches!(nested_box[1].shape, FieldLivenessShape::NoDrop));
    assert!(matches!(outer[1].shape, FieldLivenessShape::NoDrop));

    hir::validate(&program).expect("canonical nested substitution replays independently");
}

#[test]
fn hostile_nested_type_arguments_and_cleanup_carriers_fail_closed() {
    let program = resolved();
    let pair_bytes_bool = nominal(
        "nested.generic.pair",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    let box_pair = nominal("nested.generic.box", vec![pair_bytes_bool]);

    let mut wrong_argument = program.clone();
    let function = wrong_argument
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.generic.consume-box")
        .unwrap();
    function.params[0].ty = nominal(
        "nested.generic.box",
        vec![nominal(
            "nested.generic.pair",
            vec![ResolvedType::Bytes, ResolvedType::I64],
        )],
    );
    assert_eq!(hir::validate(&wrong_argument).unwrap_err().code, "SPX-H006");

    let mut wrong_leaf = program.clone();
    let function = wrong_leaf
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.generic.consume-box")
        .unwrap();
    let inventory = function
        .cleanup
        .slots
        .iter_mut()
        .find(|slot| slot.ty == box_pair)
        .unwrap();
    let FieldLivenessShape::Record { fields, .. } = &mut inventory.shape else {
        unreachable!()
    };
    let FieldLivenessShape::Record { fields, .. } = &mut fields[0].shape else {
        unreachable!()
    };
    fields.swap(0, 1);
    assert_eq!(hir::validate(&wrong_leaf).unwrap_err().code, "SPX-H006");

    let mut wrong_plan = program.clone();
    let function = wrong_plan
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.generic.consume-pair")
        .unwrap();
    let slot = function.cleanup_plan.slots.first_mut().unwrap();
    let FieldLivenessShape::Record { fields, .. } = &mut slot.field_liveness_shape else {
        unreachable!()
    };
    let FieldLivenessShape::Record { fields, .. } = &mut fields[0].shape else {
        unreachable!()
    };
    fields[0].shape = FieldLivenessShape::NoDrop;
    assert_eq!(hir::validate(&wrong_plan).unwrap_err().code, "SPX-H006");
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let parsed = parse(
        source,
        Path::new("nested-generic-owned-record-hostile-v1.spx"),
    )
    .expect("hostile source parses");
    verify::verify(&parsed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn nested_box_source(depth: usize) -> String {
    let mut ty = String::from("Bytes");
    for _ in 0..depth {
        ty = format!("Box<{ty}>");
    }
    format!(
        r#"module test.nested.generic.depth;
record Box<T> {{ value: T, }}
fn consume(value: own {ty}) -> i64 {{ 0 }}
fn main() -> i64 {{ 0 }}
"#
    )
}

#[test]
fn nested_generic_argument_depth_uses_one_global_bound() {
    assert!(error_codes(&nested_box_source(64)).is_empty());
    let plus_one = error_codes(&nested_box_source(65));
    assert_eq!(plus_one, ["SPX-T223"]);
}

#[test]
fn nested_generic_storage_keeps_nonrecord_noncopy_and_nonconcrete_shapes_closed() {
    let cases = [
        (
            r#"module test.nested.generic.string;
record Box<T> { value: T, }
fn reject(value: own Box<string>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T223",
        ),
        (
            r#"module test.nested.generic.class;
class Cell<T> { value: T, }
record Box<T> { value: T, }
fn reject(value: own Box<Cell<Bytes>>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T223",
        ),
        (
            r#"module test.nested.generic.variant;
variant Choice<T> { Value { value: T, }, }
record Box<T> { value: T, }
fn reject(value: own Box<Choice<Bytes>>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T223",
        ),
        (
            r#"module test.nested.generic.resource;
resource Token { drop trivial; }
record Box<T> { value: T, }
fn reject(value: own Box<Token>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T223",
        ),
        (
            r#"module test.nested.generic.nonconcrete;
record Pair<T, U> { left: T, right: U, }
record Box<T> { value: T, }
fn reject<T>(value: own Box<Pair<Bytes, T>>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T224",
        ),
    ];
    for (source, expected) in cases {
        let codes = error_codes(source);
        assert!(
            codes.contains(&expected),
            "missing {expected} for hostile nested generic shape: {codes:?}\n{source}"
        );
    }
}
