use super::*;
use std::fmt::Write as _;

const SCALARS: [(&str, &str); 8] = [
    ("bool", "true"),
    ("i64", "7"),
    ("i32", "7i32"),
    ("u8", "7u8"),
    ("usize", "7usize"),
    ("char", "'x'"),
    ("f32", "1.5f32"),
    ("f64", "1.5f64"),
];

fn source(selected_shape: usize, failure: Option<usize>, bypass: bool) -> String {
    let mut source = String::from(
        r#"module test.generic_owned_complete_matrix;
@id("matrix.pair") record Pair<T, U> {
  @id("matrix.pair.payload") payload: T,
  @id("matrix.pair.marker") marker: U,
}
@id("matrix.box") record Box<T> { @id("matrix.box.value") value: T, }
"#,
    );
    let mut calls = Vec::new();
    for (shape, generic) in [
        "Pair<Bytes, T>",
        "Box<Pair<Bytes, T>>",
        "Pair<Box<Bytes>, T>",
    ]
    .into_iter()
    .enumerate()
    {
        if shape != selected_shape {
            continue;
        }
        writeln!(source, r#"
@id("matrix.leaf.{shape}")
fn leaf_{shape}<T>(value: own {generic}, allowed: bool) -> {generic}
  requires allowed
{{ value }}
@id("matrix.middle.{shape}")
fn middle_{shape}<T>(value: own {generic}, allowed: bool) -> {generic} {{ leaf_{shape}<T>(value, allowed) }}
@id("matrix.outer.{shape}")
fn outer_{shape}<T>(value: own {generic}, allowed: bool) -> {generic} {{ middle_{shape}<T>(value, allowed) }}
"#).unwrap();
        for (scalar, (ty, literal)) in SCALARS.iter().enumerate() {
            let concrete = generic.replace('T', ty);
            let bytes = "bytes_copy(array_as_slice(input))";
            let (constructor, pattern) = match shape {
                0 => (
                    format!("Pair<Bytes, {ty}> {{ payload: {bytes}, marker: {literal}, }}"),
                    "Pair { payload: payload, marker: marker }",
                ),
                1 => (
                    format!("Box<Pair<Bytes, {ty}>> {{ value: Pair<Bytes, {ty}> {{ payload: {bytes}, marker: {literal}, }}, }}"),
                    "Box { value: Pair { payload: payload, marker: marker } }",
                ),
                _ => (
                    format!("Pair<Box<Bytes>, {ty}> {{ payload: Box<Bytes> {{ value: {bytes}, }}, marker: {literal}, }}"),
                    "Pair { payload: Box { value: payload }, marker: marker }",
                ),
            };
            let transfer = if bypass {
                "value".to_owned()
            } else {
                format!("outer_{shape}<{ty}>(value, allowed)")
            };
            writeln!(
                source,
                r#"
@id("matrix.consume.{shape}.{ty}")
fn consume_{shape}_{ty}(value: own {concrete}) -> i64 {{
  match own value {{ {pattern} =>
    if marker == {literal} && byte_len(bytes_as_slice(payload)) == 1usize {{ 1 }} else {{ 0 }}, }}
}}
@id("matrix.run.{shape}.{ty}")
fn run_{shape}_{ty}(allowed: bool) -> i64 {{
  let input = [9u8];
  let value = {constructor};
  consume_{shape}_{ty}({transfer})
}}
"#
            )
            .unwrap();
            if failure.is_none() || failure == Some(scalar) {
                calls.push(format!(
                    "run_{shape}_{ty}({})",
                    if failure.is_some() { "false" } else { "true" }
                ));
            }
        }
    }
    writeln!(
        source,
        "@id(\"app.main\") fn main() -> i64 {{ {} }}",
        calls.join(" + ")
    )
    .unwrap();
    source
}

#[test]
fn all_copy_scalars_flat_and_both_nested_forwarding_settle_on_three_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(
            clang && node,
            "the generic ownership corpus requires clang and Node"
        );
    }
    let cases = (0..3).flat_map(|shape| {
        std::iter::once((shape, None))
            .chain((0..SCALARS.len()).map(move |scalar| (shape, Some(scalar))))
    });
    for (shape, failure) in cases {
        let text = source(shape, failure, false);
        let parsed = parse(&text, Path::new("generic-owned-complete-matrix.spx")).unwrap();
        let diagnostics = verify::verify(&parsed);
        assert!(
            diagnostics.iter().all(|d| !d.severity.is_error()),
            "{failure:?}: {diagnostics:?}"
        );
        if failure.is_none() {
            let canonical = semaprax::format::canonical(&parsed);
            let reparsed =
                parse(&canonical, Path::new("generic-owned-complete-matrix.spx")).unwrap();
            assert_eq!(canonical, semaprax::format::canonical(&reparsed));
            assert_eq!(
                semaprax::graph::to_json(&parsed).unwrap(),
                semaprax::graph::to_json(&parsed).unwrap()
            );
        }
        let program = hir::resolve(&parsed).unwrap();
        hir::validate(&program).unwrap();
        assert_eq!(program.function_instances.len(), 24);
        for instance in &program.function_instances {
            let schema = if instance.template.as_str().ends_with(".0") {
                "semaprax.cleanup-plan.v2"
            } else {
                "semaprax.cleanup-plan.v7"
            };
            assert_eq!(instance.function.cleanup_plan.schema, schema);
            assert_eq!(
                instance.function.params[0].ownership,
                hir::OwnershipMode::Own
            );
            assert_eq!(
                instance.function.params[0].ty,
                instance.function.return_type
            );
        }
        let expected = if failure.is_some() {
            Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
        } else {
            Expected::Value(8)
        };
        run_interpreter_source("all-copy-scalar-matrix", &text, expected);
        if clang {
            run_native(&parsed, expected);
        }
        if node {
            run_wasm_source(&parsed, &source(shape, failure, true), expected);
        }
    }
}
