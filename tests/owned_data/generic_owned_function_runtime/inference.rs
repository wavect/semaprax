//! Omitted bounded generic vectors keep the explicit owned-call runtime proof.
use super::*;
use std::fmt::Write as _;

const SCALARS: [(&str, &str); 8] = [
    ("i64", "7"),
    ("i32", "7i32"),
    ("u8", "7u8"),
    ("usize", "7usize"),
    ("char", "'x'"),
    ("f32", "1.5f32"),
    ("f64", "1.5"),
    ("bool", "true"),
];

fn source(inferred: bool, allowed: bool) -> String {
    let arguments = |ty: &str| {
        if inferred {
            String::new()
        } else {
            format!("<{ty}>")
        }
    };
    let mut source = String::from(
        r#"
module test.generic_owned_inference_runtime;
@id("inference.pair") record Pair<T,U>{
 @id("inference.pair.payload") payload:T,
 @id("inference.pair.marker") marker:U,
}
@id("inference.relay") fn relay<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T>{value}
@id("inference.reject") fn reject<T>(value:own Pair<Bytes,T>,allowed:bool)->Pair<Bytes,T> requires allowed {value}
"#,
    );
    let mut calls = Vec::new();
    for (ty, literal) in SCALARS {
        let arguments = arguments(ty);
        writeln!(
            source,
            r#"
@id("inference.make.{ty}") fn make_{ty}()->Pair<Bytes,{ty}>{{
 let input=[9u8];
 Pair<Bytes,{ty}>{{payload:bytes_copy(array_as_slice(input)),marker:{literal}}}
}}
@id("inference.consume.{ty}") fn consume_{ty}(value:own Pair<Bytes,{ty}>)->i64{{
 match own value{{Pair{{payload,marker}}=>if byte_len(bytes_as_slice(payload))==1usize&&marker=={literal}{{1}}else{{0}},}}
}}
@id("inference.run.{ty}") fn run_{ty}()->i64{{
 let input = make_{ty}();
 let relayed = relay{arguments}(input);
 let accepted = reject{arguments}(relayed,{allowed});
 consume_{ty}(accepted)
}}
"#
        )
        .unwrap();
        calls.push(format!("run_{ty}()"));
    }
    writeln!(
        source,
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    )
    .unwrap();
    source
}

#[test]
fn inferred_owned_calls_match_explicit_instances_and_settle_on_every_engine() {
    let explicit_text = source(false, true);
    let inferred_text = source(true, true);
    let explicit = semaprax::check(&explicit_text, "inference-explicit.spx").unwrap();
    let inferred = semaprax::check(&inferred_text, "inference-omitted.spx").unwrap();
    let explicit_hir = hir::resolve(&explicit).unwrap();
    let inferred_hir = hir::resolve(&inferred).unwrap();
    hir::validate(&explicit_hir).unwrap();
    hir::validate(&inferred_hir).unwrap();
    assert_eq!(
        inferred_hir.function_instances,
        explicit_hir.function_instances
    );
    assert_eq!(
        inferred_hir
            .function_instances
            .iter()
            .filter(|instance| instance.template.as_str() == "inference.relay")
            .count(),
        SCALARS.len()
    );
    let canonical = semaprax::format::canonical(&inferred);
    let reparsed = semaprax::check(&canonical, "inference-canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let graph = semaprax::graph::to_json(&inferred).unwrap();
    semaprax::graph::verify_json(&inferred, &graph).unwrap();

    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    let success = Expected::Value(SCALARS.len() as i64);
    run_interpreter_source("inferred owned calls", &inferred_text, success);
    if clang {
        run_native(&inferred, success);
    }
    if node {
        run_wasm_source(&inferred, &explicit_text, success);
    }

    let failure_text = source(true, false);
    let failure = semaprax::check(&failure_text, "inference-failure.spx").unwrap();
    let expected = Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure");
    run_interpreter_source("inferred owned failure", &failure_text, expected);
    if clang {
        run_native(&failure, expected);
    }
    if node {
        run_wasm_source(&failure, &source(false, false), expected);
    }
}
