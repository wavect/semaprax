//! Authored generic variants preserve conditional owner settlement on all engines.
use super::*;
use std::fmt::Write as _;
fn source(data: bool, fail: Option<usize>, bypass: bool) -> String {
    let mut source = String::from(
        r#"
module test.generic_authored_variant_runtime;
@id("v.choice") variant Choice<P,T> {
 @id("v.data") Data { @id("v.payload") payload:P, @id("v.marker") marker:T, },
 @id("v.empty") Empty { @id("v.empty.marker") marker:T, },
}
@id("v.relay") fn relay<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{value}
@id("v.rebuild") fn rebuild<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{
 match own value {
  Choice::Data {payload,marker}=>Choice<Bytes,T>::Data {payload:payload,marker:marker},
  Choice::Empty {marker}=>Choice<Bytes,T>::Empty {marker:marker},
 }
}
@id("v.compose") fn compose<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{
 relay<T>(match own value {
  Choice::Data {payload,marker}=>Choice<Bytes,T>::Data {payload:payload,marker:marker},
  Choice::Empty {marker}=>Choice<Bytes,T>::Empty {marker:marker},
 })
}
@id("v.select") fn select<T>(value:own Choice<Bytes,T>,choose:bool)->Choice<Bytes,T>{relay<T>(if choose {rebuild<T>(value)} else {value})}
@id("v.observe") fn observe<T>(value:own Choice<Bytes,T>)->T{
 match borrow value {Choice::Data{payload,marker}=>marker,Choice::Empty{marker}=>marker,}
}
@id("v.reject") fn reject<T>(value:own Choice<Bytes,T>,allowed:bool)->Choice<Bytes,T> requires allowed {value}
"#,
    );
    let mut calls = Vec::new();
    for (index, (ty, literal)) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'x'"),
        ("f32", "1.5f32"),
        ("f64", "1.5"),
        ("bool", "true"),
    ]
    .into_iter()
    .enumerate()
    {
        let pipeline = if bypass {
            format!("make_{ty}()")
        } else {
            format!("reject<{ty}>(select<{ty}>(compose<{ty}>(make_{ty}()),choose),allowed)")
        };
        writeln!(source,r#"
@id("v.make.{ty}") fn make_{ty}()->Choice<Bytes,{ty}>{{
 let input=[9u8,8u8];
 if {data} {{ Choice<Bytes,{ty}>::Data {{ payload:bytes_copy(array_as_slice(input)),marker:{literal} }} }} else {{ Choice<Bytes,{ty}>::Empty {{ marker:{literal} }} }}
}}
@id("v.consume.{ty}") fn consume_{ty}(value:own Choice<Bytes,{ty}>)->i64{{
 match own value{{Choice::Data{{payload,marker}}=>if byte_len(bytes_as_slice(payload))==2usize && marker=={literal}{{1}}else{{0}},Choice::Empty{{marker}}=>if marker=={literal}{{1}}else{{0}},}}
}}
@id("v.run.{ty}") fn run_{ty}(choose:bool,allowed:bool)->i64{{
 if choose {{ let observed=observe<{ty}>({pipeline}); if observed=={literal}{{1}}else{{0}} }} else {{ consume_{ty}({pipeline}) }}
}}
"#).unwrap();
        let allowed = fail != Some(index);
        calls.push(format!("run_{ty}(true,{allowed})"));
        calls.push(format!("run_{ty}(false,{allowed})"));
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
fn authored_variants_all_scalars_cases_joins_and_failures_settle_on_every_engine() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    for data in [true, false] {
        for fail in std::iter::once(None).chain((0..8).map(Some)) {
            let text = source(data, fail, false);
            let parsed = semaprax::check(&text, "authored-variant-runtime.spx").unwrap();
            let canonical = semaprax::format::canonical(&parsed);
            let reparsed = semaprax::check(&canonical, "authored-variant-canonical.spx").unwrap();
            assert_eq!(canonical, semaprax::format::canonical(&reparsed));
            let resolved = hir::resolve(&parsed).unwrap();
            hir::validate(&resolved).unwrap();
            let graph = semaprax::graph::to_json(&parsed).unwrap();
            semaprax::graph::verify_json(&parsed, &graph).unwrap();
            let expected = if fail.is_some() {
                Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
            } else {
                Expected::Value(16)
            };
            run_interpreter_source("authored variants", &text, expected);
            if clang {
                run_native(&parsed, expected);
            }
            if node {
                run_wasm_source(&parsed, &source(data, fail, true), expected);
            }
        }
    }
}
