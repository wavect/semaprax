//! Declared-type lookup for source verification.
//!
//! The table is built from the parsed AST alone, so these fixtures are only
//! parsed. That is deliberate: the table has to answer for hostile and
//! half-written source (inheritance cycles, recursive records, oversized
//! aggregates) without hanging or reporting an admitted shape.

use std::path::Path;

use super::*;

fn parsed(source: &str, path: &str) -> Program {
    crate::parse(source, Path::new(path)).expect("fixture parses")
}

fn named(name: &str) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: Vec::new(),
    }
}

fn generic(name: &str, arguments: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments,
    }
}

/// Declared child-first so the source order of the chain is the reverse of the
/// ancestry order the merged field list must use.
const INHERITANCE: &str = r#"module test.type_table_inheritance;

@id("t.puppy")
class Puppy : Dog {
    @id("t.puppy.cute")
    cute: i64,

    @id("t.puppy.score")
    fn score(self: Puppy) -> i64
{
        self.cute
    }
}

@id("t.dog")
class Dog : Animal {
    @id("t.dog.bark")
    bark: i64,

    @id("t.dog.describe")
    fn describe(self: Dog) -> i64
{
        self.bark
    }
}

@id("t.animal")
class Animal {
    @id("t.animal.legs")
    legs: i64,

    @id("t.animal.describe")
    fn describe(self: Animal) -> i64
{
        self.legs
    }

    @id("t.animal.label")
    fn label(self: Animal) -> i64
{
        1
    }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn field_names(fields: &[FieldDeclaration]) -> Vec<String> {
    fields.iter().map(|field| field.name.clone()).collect()
}

#[test]
fn effective_class_fields_are_merged_root_ancestor_first() {
    let program = parsed(INHERITANCE, "type-table-inheritance.spx");
    let types = TypeTable::new(&program);

    // Construction and projection see inherited members as a prefix of the
    // declared ones, in ancestry order. Reversing this silently reorders every
    // class literal's field list.
    assert_eq!(
        field_names(effective_record_fields(&types, &named("Puppy")).expect("class fields")),
        vec!["legs".to_owned(), "bark".to_owned(), "cute".to_owned()]
    );
    assert_eq!(
        field_names(effective_record_fields(&types, &named("Dog")).expect("class fields")),
        vec!["legs".to_owned(), "bark".to_owned()]
    );
    // `record_fields` deliberately stays declaration-local; the two answers
    // must not be conflated.
    assert_eq!(
        field_names(
            types
                .record_fields(&named("Puppy"))
                .expect("declared fields")
        ),
        vec!["cute".to_owned()]
    );
    // A declared field lookup is also declaration-local.
    assert!(types.declared_field("Puppy", "cute").is_some());
    assert!(types.declared_field("Puppy", "legs").is_none());
}

#[test]
fn method_resolution_prefers_the_nearest_declaring_class() {
    let program = parsed(INHERITANCE, "type-table-inheritance.spx");
    let types = TypeTable::new(&program);

    // `Dog` overrides `describe`, so a `Puppy` receiver must reach the `Dog`
    // body, not the `Animal` one it also inherits.
    assert_eq!(
        resolve_class_method(&types, "Puppy", "describe").map(|(owner, _)| owner),
        Some("Dog")
    );
    assert_eq!(
        resolve_class_method(&types, "Animal", "describe").map(|(owner, _)| owner),
        Some("Animal")
    );
    // A method only the root declares is still reachable from the leaf.
    assert_eq!(
        resolve_class_method(&types, "Puppy", "label").map(|(owner, _)| owner),
        Some("Animal")
    );
    assert!(resolve_class_method(&types, "Puppy", "absent").is_none());
    assert!(resolve_class_method(&types, "Absent", "label").is_none());
}

#[test]
fn class_ancestry_is_transitive_and_a_cycle_terminates() {
    let program = parsed(INHERITANCE, "type-table-inheritance.spx");
    let types = TypeTable::new(&program);
    assert!(types.class_extends("Puppy", "Animal"));
    assert!(types.class_extends("Puppy", "Dog"));
    assert!(!types.class_extends("Animal", "Puppy"));
    assert!(!types.class_extends("Puppy", "Puppy"));

    let cyclic = parsed(
        r#"module test.type_table_cycle;

@id("t.ping")
class Ping : Pong {
    @id("t.ping.value")
    value: i64,
}

@id("t.pong")
class Pong : Ping {
    @id("t.pong.value")
    value: i64,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#,
        "type-table-cycle.spx",
    );
    let types = TypeTable::new(&cyclic);
    // Building the table over a cyclic chain must terminate and refuse to
    // publish a merged field list, and the ancestry query must terminate too.
    assert!(effective_record_fields(&types, &named("Ping")).is_none());
    assert!(!types.class_extends("Ping", "Absent"));
    assert!(resolve_class_method(&types, "Ping", "absent").is_none());
}

const SHAPES: &str = r#"module test.type_table_shapes;

@id("t.plain")
record Plain {
    @id("t.plain.count")
    count: i64,
}

@id("t.buffer")
record Buffer {
    @id("t.buffer.payload")
    payload: Bytes,
    @id("t.buffer.count")
    count: i64,
}

@id("t.wrap")
record Wrap {
    @id("t.wrap.inner")
    inner: Buffer,
}

@id("t.label")
record Label {
    @id("t.label.text")
    text: string,
}

@id("t.token")
resource Token {
    @id("t.token.drop")
    drop trivial;
}

@id("t.holder")
record Holder {
    @id("t.holder.token")
    token: Token,
}

@id("t.cell")
record Cell<T> {
    @id("t.cell.value")
    value: T,
}

@id("t.packet")
variant Packet {
    @id("t.packet.empty")
    Empty,
    @id("t.packet.full")
    Full {
        @id("t.packet.full.payload")
        payload: Bytes,
    },
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

#[test]
fn drop_authority_is_broader_than_authored_resource_membership() {
    let program = parsed(SHAPES, "type-table-shapes.spx");
    let types = TypeTable::new(&program);

    // Compiler-owned `Bytes` and `string` carry destruction authority without
    // being authored resources. Collapsing the two predicates would either
    // demand ownership modes on byte records or drop cleanup for them.
    assert!(types.needs_drop(&named("Buffer")));
    assert!(!types.contains_resource(&named("Buffer")));
    assert!(types.contains_owned_bytes(&named("Buffer")));
    assert!(!types.contains_string(&named("Buffer")));

    assert!(types.needs_drop(&named("Label")));
    assert!(!types.contains_resource(&named("Label")));
    assert!(!types.contains_owned_bytes(&named("Label")));
    assert!(types.contains_string(&named("Label")));

    assert!(types.needs_drop(&named("Holder")));
    assert!(types.contains_resource(&named("Holder")));
    assert!(!types.contains_owned_bytes(&named("Holder")));

    for predicate in [
        types.needs_drop(&named("Plain")),
        types.contains_resource(&named("Plain")),
        types.contains_owned_bytes(&named("Plain")),
        types.contains_string(&named("Plain")),
    ] {
        assert!(!predicate, "a scalar record owns nothing");
    }

    // Only a `resource` declaration is opaque; an aggregate holding one is not.
    assert!(types.is_opaque_resource(&named("Token")));
    assert!(!types.is_opaque_resource(&named("Holder")));

    // Nesting is transitive in both directions.
    assert!(types.needs_drop(&named("Wrap")));
    assert!(types.contains_owned_bytes(&named("Wrap")));
}

#[test]
fn owned_byte_aggregates_are_classified_flat_nested_or_outside_the_profile() {
    let program = parsed(SHAPES, "type-table-shapes.spx");
    let types = TypeTable::new(&program);

    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("Buffer")),
        NestedOwnedRecordAdmission::Admitted
    );
    assert!(types.is_flat_owned_byte_record(&named("Buffer")));

    // A record whose owned bytes sit one level down is admitted by the nested
    // profile but is not flat; the exact-pattern rules read both answers.
    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("Wrap")),
        NestedOwnedRecordAdmission::Admitted
    );
    assert!(types.is_nested_owned_byte_record(&named("Wrap")));
    assert!(!types.is_flat_owned_byte_record(&named("Wrap")));

    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("Plain")),
        NestedOwnedRecordAdmission::NoOwnedBytes
    );
    // `string`, authored resources, generic instances, and variants are all
    // outside the record profile rather than "no owned bytes".
    for outside in [
        named("Label"),
        named("Holder"),
        named("Packet"),
        named("Absent"),
        generic("Cell", vec![Type::I64]),
    ] {
        assert_eq!(
            classify_nested_owned_byte_record(&types, &outside),
            NestedOwnedRecordAdmission::OutsideProfile,
            "{outside:?}"
        );
    }

    // The variant profile is separate and admits the flat authored shape.
    assert!(types.is_flat_owned_byte_variant(&named("Packet")));
    assert!(!types.is_flat_owned_byte_variant(&named("Buffer")));
}

#[test]
fn a_self_recursive_record_is_reported_recursive_rather_than_traversed() {
    let program = parsed(
        r#"module test.type_table_recursive;

@id("t.node")
record Node {
    @id("t.node.payload")
    payload: Bytes,
    @id("t.node.next")
    next: Node,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#,
        "type-table-recursive.spx",
    );
    let types = TypeTable::new(&program);
    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("Node")),
        NestedOwnedRecordAdmission::Recursive
    );
    // The drop predicate must also terminate on the same cycle.
    assert!(types.needs_drop(&named("Node")));
}

fn chain_source(depth: usize) -> String {
    let mut source = String::from("module test.type_table_depth;\n\n");
    for index in 0..depth {
        source.push_str(&format!("@id(\"t.r{index}\")\nrecord R{index} {{\n"));
        if index + 1 == depth {
            source.push_str(&format!(
                "    @id(\"t.r{index}.payload\")\n    payload: Bytes,\n"
            ));
        } else {
            source.push_str(&format!(
                "    @id(\"t.r{index}.next\")\n    next: R{next},\n",
                next = index + 1
            ));
        }
        source.push_str("}\n\n");
    }
    source.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    source
}

#[test]
fn the_nested_record_depth_bound_admits_the_limit_and_refuses_one_deeper() {
    let at_limit = parsed(
        &chain_source(MAX_NESTED_OWNED_RECORD_DEPTH),
        "type-table-depth.spx",
    );
    let types = TypeTable::new(&at_limit);
    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("R0")),
        NestedOwnedRecordAdmission::Admitted
    );

    let one_deeper = parsed(
        &chain_source(MAX_NESTED_OWNED_RECORD_DEPTH + 1),
        "type-table-depth.spx",
    );
    let types = TypeTable::new(&one_deeper);
    assert_eq!(
        classify_nested_owned_byte_record(&types, &named("R0")),
        NestedOwnedRecordAdmission::LimitExceeded
    );
}

fn leaves_source(leaves: usize) -> String {
    let mut source =
        String::from("module test.type_table_leaves;\n\n@id(\"t.wide\")\nrecord Wide {\n");
    for index in 0..leaves {
        source.push_str(&format!(
            "    @id(\"t.wide.f{index}\")\n    f{index}: Bytes,\n"
        ));
    }
    source.push_str("}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    source
}

#[test]
fn the_owned_byte_leaf_bound_admits_the_limit_and_refuses_one_more() {
    let at_limit = parsed(
        &leaves_source(MAX_NESTED_OWNED_BYTE_LEAVES),
        "type-table-leaves.spx",
    );
    assert_eq!(
        classify_nested_owned_byte_record(&TypeTable::new(&at_limit), &named("Wide")),
        NestedOwnedRecordAdmission::Admitted
    );

    let one_more = parsed(
        &leaves_source(MAX_NESTED_OWNED_BYTE_LEAVES + 1),
        "type-table-leaves.spx",
    );
    assert_eq!(
        classify_nested_owned_byte_record(&TypeTable::new(&one_more), &named("Wide")),
        NestedOwnedRecordAdmission::LimitExceeded
    );
}

#[test]
fn the_owned_byte_prelude_carriers_are_an_exact_closed_list() {
    for admitted in [
        ("Option", vec![Type::Bytes]),
        ("Result", vec![Type::Bytes, Type::I64]),
        ("Result", vec![Type::Bytes, Type::Bool]),
        ("Result", vec![Type::I64, Type::Bytes]),
        ("Result", vec![Type::Bool, Type::Bytes]),
    ] {
        assert!(
            owned_byte_prelude_instance_is_admitted(admitted.0, &admitted.1),
            "{admitted:?}"
        );
    }
    // Two owned carriers in one instance, a non-prelude name, and the ordinary
    // copy instances are all outside the exception.
    for rejected in [
        ("Option", vec![Type::I64]),
        ("Option", vec![Type::Bytes, Type::Bytes]),
        ("Result", vec![Type::Bytes, Type::Bytes]),
        ("Result", vec![Type::Bytes, Type::Usize]),
        ("Cell", vec![Type::Bytes]),
    ] {
        assert!(
            !owned_byte_prelude_instance_is_admitted(rejected.0, &rejected.1),
            "{rejected:?}"
        );
    }
}

#[test]
fn generic_record_field_types_are_substituted_by_declaration_position() {
    let program = parsed(SHAPES, "type-table-shapes.spx");
    let types = TypeTable::new(&program);
    let declaration = types.declaration("Cell").expect("generic record declared");
    let field = types
        .declared_field("Cell", "value")
        .expect("field declared");

    assert_eq!(
        types.record_field_type(&generic("Cell", vec![Type::I64]), field),
        Some(Type::I64)
    );
    // A nested template argument is rebuilt, not flattened.
    assert_eq!(
        TypeTable::substitute_variant_type(
            declaration,
            &[Type::Bool],
            &generic("Option", vec![named("T")])
        ),
        Some(generic("Option", vec![Type::Bool]))
    );
    // Missing arguments must produce `None` rather than a half-substituted
    // type that later reads as a concrete one.
    assert_eq!(
        TypeTable::substitute_variant_type(declaration, &[], &named("T")),
        None
    );
    // A name that is not a parameter of this declaration is left alone.
    assert_eq!(
        TypeTable::substitute_variant_type(declaration, &[Type::I64], &named("Buffer")),
        Some(named("Buffer"))
    );
}

#[test]
fn field_and_case_lookups_reject_the_wrong_declaration_kind() {
    let program = parsed(SHAPES, "type-table-shapes.spx");
    let types = TypeTable::new(&program);

    assert!(types.record_fields(&named("Packet")).is_none());
    assert!(types.record_fields(&named("Token")).is_none());
    assert!(effective_record_fields(&types, &named("Packet")).is_none());
    assert!(effective_record_fields(&types, &Type::I64).is_none());

    assert_eq!(
        types
            .variant_cases(&named("Packet"))
            .expect("variant cases")
            .len(),
        2
    );
    assert!(types.variant_cases(&named("Buffer")).is_none());
}
