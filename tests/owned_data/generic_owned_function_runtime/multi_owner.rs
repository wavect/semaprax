//! Two independently staged owners, explicit reconstruction and replacement.
use super::*;
use std::fmt::Write as _;

fn source(update: bool, failure: bool, bypass: bool) -> String {
    let mut source = String::from(
        r#"
module test.generic_multi_owner_runtime;
@id("o.pair") record Pair<A,B> {@id("o.left") left:A,@id("o.right") right:B,}
@id("o.wrap") record Wrap<A> {@id("o.value") value:A,}
@id("o.combine") fn combine<T>(left:own Pair<Bytes,T>,right:own Wrap<Bytes>)->Pair<Pair<Bytes,T>,Wrap<Bytes>> {
    Pair<Pair<Bytes,T>,Wrap<Bytes>>{left:left,right:right}
}
@id("o.replace") fn replace<T>(base:own Pair<Wrap<Bytes>,T>,next:own Wrap<Bytes>)->Pair<Wrap<Bytes>,T> {
    let alias=base;alias with {left:next}
}
@id("o.guard") fn guard(value:own Wrap<Bytes>,allowed:bool)->Wrap<Bytes> requires allowed {value}
"#,
    );
    let scalars = [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'x'"),
        ("f32", "1.5f32"),
        ("f64", "1.5f64"),
        ("bool", "true"),
    ];
    let mut calls = Vec::new();
    for (ty, literal) in scalars {
        let (input, output, pattern, condition) = if update {
            (format!("Pair<Wrap<Bytes>,{ty}> {{left:Wrap<Bytes>{{value:bytes_copy(array_as_slice(first))}},right:{literal}}}"),
             format!("Pair<Wrap<Bytes>,{ty}>"),
             "Pair {left:Wrap {value:second},right:marker}",
             "byte_len(bytes_as_slice(second))==2usize".to_owned())
        } else {
            (format!("Pair<Bytes,{ty}> {{left:bytes_copy(array_as_slice(first)),right:{literal}}}"),
             format!("Pair<Pair<Bytes,{ty}>,Wrap<Bytes>>"),
             "Pair {left:Pair {left:first,right:marker},right:Wrap {value:second}}",
             "byte_len(bytes_as_slice(first))==1usize && byte_len(bytes_as_slice(second))==2usize".to_owned())
        };
        let transfer = if bypass {
            if update {
                format!("Pair<Wrap<Bytes>,{ty}> {{left:guard(next,allowed),right:{literal}}}")
            } else {
                format!(
                    "Pair<Pair<Bytes,{ty}>,Wrap<Bytes>> {{left:base,right:guard(next,allowed)}}"
                )
            }
        } else {
            format!(
                "{}<{ty}>(base,guard(next,allowed))",
                if update { "replace" } else { "combine" }
            )
        };
        writeln!(
            source,
            r#"
@id("o.consume.{ty}") fn consume_{ty}(value:own {output})->i64 {{
    match own value {{{pattern}=>if marker=={literal} && {condition} {{1}} else {{0}},}}
}}
@id("o.run.{ty}") fn run_{ty}(allowed:bool)->i64 {{
    let first=[9u8];let second=[8u8,7u8];
    let base={input};
    let next=Wrap<Bytes>{{value:bytes_copy(array_as_slice(second))}};
    consume_{ty}({transfer})
}}
"#
        )
        .unwrap();
        calls.push(format!("run_{ty}({})", !failure));
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
fn multi_owner_all_scalars_settle_reconstruction_update_and_second_argument_failure() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    for update in [false, true] {
        for failure in [false, true] {
            let text = source(update, failure, false);
            let parsed = semaprax::check(&text, "multi-owner-runtime.spx").unwrap();
            let resolved = hir::resolve(&parsed).unwrap();
            hir::validate(&resolved).unwrap();
            let graph = semaprax::graph::to_json(&parsed).unwrap();
            semaprax::graph::verify_json(&parsed, &graph).unwrap();
            let expected = if failure {
                Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
            } else {
                Expected::Value(8)
            };
            run_interpreter_source("multi-owner", &text, expected);
            if clang {
                run_native(&parsed, expected);
            }
            if node {
                run_wasm_source(&parsed, &source(update, failure, true), expected);
            }
        }
    }
}
