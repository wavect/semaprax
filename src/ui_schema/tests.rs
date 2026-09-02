use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-ui-schema-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn options_reject_out_of_bounds_values() {
    assert!(UiSchemaOptions::new(512).is_err());
    assert!(UiSchemaOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
    assert!(UiSchemaOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
    assert_eq!(UiSchemaOptions::default().max_bytes, DEFAULT_MAX_BYTES);
}

#[test]
fn domain_digest_is_domain_separated() {
    let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
    let second = domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc");
    assert_ne!(first, second);
    assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
}

#[test]
fn canonical_texts_are_stable_and_verifier_friendly() {
    let fields = [("x", "i64", 0u32, 8u32, 8u32), ("flag", "bool", 8, 1, 1)];
    assert_eq!(
        state_shape_layout_text(&fields, 16, 8),
        "{\"fields\":[{\"index\":0,\"name\":\"x\",\"type\":\"i64\",\"offset\":0,\
\"size_bytes\":8,\"align_bytes\":8},{\"index\":1,\"name\":\"flag\",\"type\":\"bool\",\
\"offset\":8,\"size_bytes\":1,\"align_bytes\":1}],\"size_bytes\":16,\"align_bytes\":8}"
    );
    assert_eq!(
        action_signature_text(&[("left", "i64"), ("right", "i64")], "i64"),
        "{\"parameters\":[{\"name\":\"left\",\"type\":\"i64\"},\
{\"name\":\"right\",\"type\":\"i64\"}],\"result\":{\"type\":\"i64\"}}"
    );
}

/// State-shape facts must equal the checked Native64 compiler layouts,
/// including the trailing-padding size of a mixed i64/bool record.
#[test]
fn state_shape_facts_come_from_the_checked_layouts() {
    let source = r#"
module test.shapes;

@id("shapes.point")
record Point {
    @id("shapes.point.x")
    x: i64,
    @id("shapes.point.y")
    y: i64,
    @id("shapes.point.flag")
    flag: bool,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    let text = std::fs::read_to_string(&path).unwrap();
    let program = parse(&text, &path).expect("parses");
    let resolved = hir::resolve(&program).expect("resolves");
    let shape = project_state_shape(&resolved, "shapes.point").expect("shape");

    let declaration = resolved
        .types
        .iter()
        .find(|candidate| candidate.id.as_str() == "shapes.point")
        .expect("resolved record");
    let checked = aggregate_layout::AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Native64,
        &declaration.id,
    )
    .expect("checked layout");
    assert_eq!(
        (shape.size_bytes, shape.align_bytes),
        (checked.size, checked.align)
    );
    assert_eq!(shape.fields.len(), checked.fields.len());
    for (field, fact) in shape.fields.iter().zip(checked.fields.iter()) {
        assert_eq!(
            (field.offset, field.size_bytes, field.align_bytes),
            (fact.offset, fact.size, fact.align)
        );
    }
    assert_eq!(shape.name, "Point");
    assert_eq!(
        shape
            .fields
            .iter()
            .map(|field| field.ty)
            .collect::<Vec<_>>(),
        vec!["i64", "i64", "bool"]
    );
    // Mixed scalar padding: two i64 plus one bool pad to the record align.
    assert_eq!(shape.size_bytes, 24);
    assert_eq!(shape.align_bytes, 8);
    cleanup(&path);
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Widened-scalar state-shape facts must equal the checked Native64
/// compiler layouts for every admitted scalar kind, including mixed
/// padding and the four-byte char representation.
#[test]
fn widened_state_shape_facts_come_from_the_checked_layouts() {
    let source = r#"
module test.widened.shapes;

@id("shapes.tensor")
record Tensor {
    @id("shapes.tensor.a")
    a: i64,
    @id("shapes.tensor.b")
    b: i32,
    @id("shapes.tensor.c")
    c: u8,
    @id("shapes.tensor.d")
    d: f32,
    @id("shapes.tensor.e")
    e: f64,
    @id("shapes.tensor.g")
    g: char,
    @id("shapes.tensor.h")
    h: bool,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    let text = std::fs::read_to_string(&path).unwrap();
    let program = parse(&text, &path).expect("parses");
    let resolved = hir::resolve(&program).expect("resolves");
    let shape = project_state_shape(&resolved, "shapes.tensor").expect("shape");

    let declaration = resolved
        .types
        .iter()
        .find(|candidate| candidate.id.as_str() == "shapes.tensor")
        .expect("resolved record");
    let checked = aggregate_layout::AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Native64,
        &declaration.id,
    )
    .expect("checked layout");
    assert_eq!(
        (shape.size_bytes, shape.align_bytes),
        (checked.size, checked.align)
    );
    assert_eq!(shape.fields.len(), checked.fields.len());
    for (field, fact) in shape.fields.iter().zip(checked.fields.iter()) {
        assert_eq!(
            (field.offset, field.size_bytes, field.align_bytes),
            (fact.offset, fact.size, fact.align)
        );
    }
    assert_eq!(shape.name, "Tensor");
    assert_eq!(
        shape
            .fields
            .iter()
            .map(|field| field.ty)
            .collect::<Vec<_>>(),
        vec!["i64", "i32", "u8", "f32", "f64", "char", "bool"]
    );
    // Mixed scalar padding: i64, i32, u8, f32 then an aligned f64, char,
    // bool pad to the record alignment.
    assert_eq!(shape.size_bytes, 40);
    assert_eq!(shape.align_bytes, 8);
    assert_eq!(
        shape
            .fields
            .iter()
            .map(|field| (field.offset, field.size_bytes, field.align_bytes))
            .collect::<Vec<_>>(),
        vec![
            (0, 8, 8),
            (8, 4, 4),
            (12, 1, 1),
            (16, 4, 4),
            (24, 8, 8),
            (32, 4, 4),
            (36, 1, 1)
        ]
    );
    cleanup(&path);
}
