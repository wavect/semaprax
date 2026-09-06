//! Nested reconstruction and changed nominal results settle across all engines.
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

fn source(failure: bool, bypass: bool, choose: bool) -> String {
    let mut source = String::from(
        r#"
module test.nested_composition_runtime;
@id("c.pair") record Pair<A,B> { @id("c.payload") payload:A, @id("c.marker") marker:B, }
@id("c.wrap") record Wrap<A> { @id("c.value") value:A, }
@id("c.relay") fn relay<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T> {value}
@id("c.compose") fn compose<T>(value:own Pair<Bytes,T>,choose:bool)->Pair<Bytes,T> {
    relay<T>(if choose {
        match own value { Pair {payload,marker} => Pair<Bytes,T> {payload:payload,marker:marker}, }
    } else {value})
}
@id("c.wrap_owned") fn wrap_owned<T>(value:own Pair<Bytes,T>)->Wrap<Pair<Bytes,T>> {
    match own value {Pair {payload,marker} => Wrap<Pair<Bytes,T>> {value:Pair<Bytes,T> {payload:payload,marker:marker}},}
}
@id("c.rebuild") fn rebuild<T>(value:own Wrap<Pair<Bytes,T>>)->Wrap<Pair<Bytes,T>> {
    match own value {Wrap {value:Pair {payload,marker}} => Wrap<Pair<Bytes,T>> {value:Pair<Bytes,T> {payload:payload,marker:marker}},}
}
@id("c.observe") fn observe<T>(value:own Wrap<Pair<Bytes,T>>)->Wrap<Pair<Bytes,T>> {
    let observed=match borrow value {Wrap {value:Pair {payload:borrowed_payload,marker}} => marker,};
    match own value {Wrap {value:Pair {payload,marker:_}} => Wrap<Pair<Bytes,T>> {value:Pair<Bytes,T> {payload:payload,marker:observed}},}
}
@id("c.update") fn update<T>(value:own Pair<Wrap<Bytes>,T>,next:T)->Pair<Wrap<Bytes>,T> {
    let alias=value;
    alias with {marker:next}
}
@id("c.reject") fn reject<T>(value:own Wrap<Pair<Bytes,T>>,allowed:bool)->Wrap<Pair<Bytes,T>> requires allowed {value}
"#,
    );
    let mut calls = Vec::new();
    for (ty, literal) in SCALARS {
        let pipeline = if bypass {
            format!("Wrap<Pair<Bytes,{ty}>> {{value:value}}")
        } else {
            format!("observe<{ty}>(rebuild<{ty}>(wrap_owned<{ty}>(compose<{ty}>(value,choose))))")
        };
        let update = if bypass {
            "nested".to_owned()
        } else {
            format!("update<{ty}>(nested,{literal})")
        };
        writeln!(source,r#"
@id("c.consume.{ty}") fn consume_{ty}(value:own Wrap<Pair<Bytes,{ty}>>)->i64 {{
    match own value {{Wrap {{value:Pair {{payload,marker}}}} =>
        if marker=={literal} && byte_len(bytes_as_slice(payload))==1usize {{1}} else {{0}},}}
}}
@id("c.consume_nested.{ty}") fn consume_nested_{ty}(value:own Pair<Wrap<Bytes>,{ty}>)->i64 {{
    match own value {{Pair {{payload:Wrap {{value:payload}},marker}} =>
        if marker=={literal} && byte_len(bytes_as_slice(payload))==1usize {{1}} else {{0}},}}
}}
@id("c.run.{ty}") fn run_{ty}(choose:bool,allowed:bool)->i64 {{
    let input=[9u8];
    let value=Pair<Bytes,{ty}> {{payload:bytes_copy(array_as_slice(input)),marker:{literal}}};
    let observed=consume_{ty}(reject<{ty}>({pipeline},allowed));
    let nested=Pair<Wrap<Bytes>,{ty}> {{payload:Wrap<Bytes> {{value:bytes_copy(array_as_slice(input))}},marker:{literal}}};
    observed+consume_nested_{ty}({update})
}}
"#).unwrap();
        calls.push(format!("run_{ty}({choose},{})", !failure));
    }
    writeln!(
        source,
        "@id(\"app.main\") fn main()->i64 {{{}}}",
        calls.join("+")
    )
    .unwrap();
    source
}

#[test]
fn nested_composition_all_scalars_preserve_values_and_cleanup_on_all_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    for (failure, choose) in [(false, true), (false, false), (true, true)] {
        let text = source(failure, false, choose);
        let parsed = semaprax::check(&text, "nested-composition-runtime.spx").unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        hir::validate(&resolved).unwrap();
        let graph = semaprax::graph::to_json(&parsed).unwrap();
        semaprax::graph::verify_json(&parsed, &graph).unwrap();
        let expected = if failure {
            Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
        } else {
            Expected::Value(16)
        };
        run_interpreter_source("nested-composition", &text, expected);
        if clang {
            run_native(&parsed, expected);
        }
        if node {
            run_wasm_source(&parsed, &source(failure, true, choose), expected);
        }
    }
}
