//! Nonidentity type mappings retain checked values and whole-owner transfers.
use super::*;
use std::fmt::Write as _;

const SCALARS: [(&str, &str); 8] = [
    ("i64", "7"),
    ("i32", "7i32"),
    ("u8", "7u8"),
    ("usize", "7usize"),
    ("char", "'x'"),
    ("f32", "1.5f32"),
    ("f64", "1.5f64"),
    ("bool", "true"),
];

fn source(failure: Option<usize>, bypass: bool) -> String {
    let mut source = String::from(
        r#"
module test.explicit_forwarding_runtime;
@id("mapping.pair") record Pair<A, B> {
  @id("mapping.pair.payload") payload: A,
  @id("mapping.pair.marker") marker: B,
}
@id("mapping.callee")
fn callee<A, B>(value: own Pair<Bytes, B>, marker: A, allowed: bool) -> Pair<Bytes, B>
  requires allowed
{ value }
@id("mapping.caller")
fn caller<T>(value: own Pair<Bytes, T>, allowed: bool) -> Pair<Bytes, T> {
  callee<bool, T>(value, true, allowed)
}
@id("mapping.first") fn first<A, B>(left: A, right: B) -> A { left }
@id("mapping.permute") fn permute<A, B>(left: A, right: B) -> B { first<B, A>(right, left) }
@id("mapping.repeat") fn repeat<A>(value: A) -> A { first<A, A>(value, value) }
@id("mapping.concrete") fn concrete<A>(value: A) -> i64 { first<i64, A>(7, value) }
"#,
    );
    let mut calls = Vec::new();
    for (index, (ty, literal)) in SCALARS.iter().enumerate() {
        let transfer = if bypass {
            "value".to_owned()
        } else {
            format!("caller<{ty}>(value, allowed)")
        };
        writeln!(source, r#"
@id("mapping.consume.{ty}") fn consume_{ty}(value: own Pair<Bytes, {ty}>) -> i64 {{
  match own value {{ Pair {{ payload: payload, marker: marker }} =>
    if marker == {literal} && byte_len(bytes_as_slice(payload)) == 1usize {{ 1 }} else {{ 0 }}, }}
}}
@id("mapping.run.{ty}") fn run_{ty}(allowed: bool) -> i64 {{
  let input = [9u8];
  let value = Pair<Bytes, {ty}> {{ payload: bytes_copy(array_as_slice(input)), marker: {literal}, }};
  consume_{ty}({transfer})
}}
"#).unwrap();
        if failure.is_none() || failure == Some(index) {
            calls.push(format!("run_{ty}({})", failure.is_none()));
        }
    }
    writeln!(source, "@id(\"app.main\") fn main() -> i64 {{ if permute<i64, bool>(3, true) && repeat<i64>(7) == 7 && concrete<bool>(false) == 7 {{ {} }} else {{ 0 }} }}", calls.join(" + ")).unwrap();
    source
}

#[test]
fn explicit_forwarding_eight_owned_scalars_and_scalar_maps_settle_on_all_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(
            clang && node,
            "explicit forwarding corpus requires clang and Node"
        );
    }
    for failure in std::iter::once(None).chain((0..8).map(Some)) {
        let text = source(failure, false);
        let parsed = semaprax::check(&text, "explicit-forwarding-runtime.spx").unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        hir::validate(&resolved).unwrap();
        assert_eq!(
            resolved
                .function_instances
                .iter()
                .filter(|i| i.template.as_str() == "mapping.callee")
                .count(),
            8
        );
        let graph = semaprax::graph::to_json(&parsed).unwrap();
        assert!(graph.contains("semaprax.graph.v35"));
        semaprax::graph::verify_json(&parsed, &graph).unwrap();
        let expected = if failure.is_some() {
            Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
        } else {
            Expected::Value(8)
        };
        run_interpreter_source("explicit-forwarding", &text, expected);
        if clang {
            run_native(&parsed, expected);
        }
        if node {
            run_wasm_source(&parsed, &source(failure, true), expected);
        }
    }
}
