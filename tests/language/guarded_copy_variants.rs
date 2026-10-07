//! Copy Variant Guards v1: fallthrough, ownership-neutral guards, and settlement.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{
    emit_copy_variant_module, emit_module, InternalStringOptions,
};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"
module test.guarded_copy_variants;
@id("guards.loop") fn looped() -> i64 {
    let selected = Option<i64>::Some { value: 7 };
    let mut i = 0;
    let mut out = "";
    while i < 3 {
        let piece = match selected {
            Option::Some { value: n } if n > 8 => "wrong",
            Option::Some { value: n } if n == 7 && i < 2 => "a",
            Option::Some { value: n } => "bc",
            Option::None {} => "wrong",
        };
        out = string_concat(out, piece);
        i = i + 1;
        0
    }
    let reused = match selected { Option::Some { value: n } => n, Option::None {} => 0, };
    string_len(out) * 10 + i + reused
}
@id("guards.skipped") fn skipped() -> i64 {
    let selected = Option<i64>::None {};
    let text = match selected {
        Option::Some { value: n } if n / 0 == 0 => "wrong",
        Option::Some { value: n } => "wrong",
        Option::None {} => "ok",
    };
    string_len(text)
}
@id("guards.lazy") fn lazy() -> i64 {
    let selected = Option<i64>::Some { value: 7 };
    match selected {
        Option::Some { value: n } if false && n / 0 == 0 => 1000,
        Option::Some { value: n } if true || n / 0 == 0 => n,
        Option::Some { value: n } => 1000,
        Option::None {} => 1000,
    }
}
@id("guards.result") fn result_case() -> i64 {
    let selected = Result<i64, bool>::Err { error: true };
    let text = match selected {
        Result::Ok { value: n } if n + 9223372036854775807 > 0 => "wrong",
        Result::Ok { value: n } => "wrong",
        Result::Err { error: failed } if !failed => "wrong",
        Result::Err { error: failed } => "right",
    };
    string_len(text)
}
@id("guards.indexed") fn indexed() -> i64 {
    let array = [0u8, 255u8, 255u8];
    let bytes = array_as_slice(array);
    let length = byte_len(bytes);
    let mut index = 0usize;
    let mut total = 0usize;
    while index <= length {
        total = total + match byte_get(bytes, index) {
            Option::Some { value: byte } if byte == 255u8 => 1usize,
            Option::Some { value: byte } => 0usize,
            Option::None {} => 0usize,
        };
        index = index + 1usize;
        0
    }
    if total == 2usize { 2 } else { 0 }
}
@id("guards.failure") fn failure() -> i64 {
    let kept = "keep";
    let selected = Option<i64>::Some { value: 7 };
    let text = match selected {
        Option::Some { value: n } if n + 9223372036854775807 > 0 => "unpublished",
        Option::Some { value: n } => "fallback",
        Option::None {} => "other",
    };
    string_len(kept) + string_len(text)
}
@id("app.main") fn main() -> i64 { looped() }
"#;
const CASES: &[(&str, &str)] = &[
    ("guards.loop", "ok|50"),
    ("guards.skipped", "ok|2"),
    ("guards.lazy", "ok|7"),
    ("guards.result", "ok|5"),
    ("guards.indexed", "ok|2"),
    ("guards.failure", "semaprax.arithmetic.v1|1"),
];
const WASM_CASES: &[&str] = &[
    "guards.loop",
    "guards.skipped",
    "guards.lazy",
    "guards.result",
    "guards.indexed",
    "guards.failure",
];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn guarded_copy_variants_round_trip_and_resolve() {
    let program = parse(SOURCE, Path::new("guarded-copy-variants.spx")).unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "{:?}",
        verify::verify(&program)
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("guarded-copy-variants-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    assert!(graph.contains("\"guard\""));
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn guarded_copy_variants_reference_interpreter() {
    let canonical =
        format::canonical(&parse(SOURCE, Path::new("guarded-copy-variants.spx")).unwrap());
    let fixture = Fixture::new(&canonical);
    for (id, expected) in CASES {
        // `semaprax run` retries a program the canonical profile refuses
        // (user functions over `string`) on the internal String profile.
        let options = InterpreterOptions::default();
        let result = interpreter::interpret(&fixture.source, id, &[], &options)
            .or_else(|_| {
                interpreter::internal_strings::interpret(&fixture.source, id, &[], &options)
            })
            .unwrap();
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            assert_eq!(outcome["kind"], "failed", "{}", result.envelope);
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn guarded_copy_variants_native_settlement() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("guarded-copy-variants-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MIN);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn guarded_copy_variants_wasm_settlement() {
    if !command_available("node") || WASM_CASES.is_empty() {
        return;
    }
    let program = parse(SOURCE, Path::new("guarded-copy-variants-wasm.spx")).unwrap();
    let selected = WASM_CASES
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact =
        emit_copy_variant_module(&program, &selected, InternalStringOptions::default()).unwrap();
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    let script = fixture.write(
        "probe.mjs",
        format!(
            r#"import {{readFileSync}} from 'node:fs';
import {{instantiate}} from './program.mjs';
const api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for(const id of {}){{
  let first;
  for(let repeat=0;repeat<3;repeat++){{
    const outcome=api.call(id);
    const observed=outcome.kind==='success'?`ok|${{outcome.value}}`:`${{outcome.domain}}|${{outcome.code}}`;
    if(first===undefined)first=observed;else if(first!==observed)throw new Error(id);
  }}
  process.stdout.write(`${{id}}|${{first}}\n`);
}}
"#,
            serde_json::to_string(WASM_CASES).unwrap()
        ),
    );
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = CASES
        .iter()
        .filter(|(id, _)| WASM_CASES.contains(id))
        .map(|(id, observation)| format!("{id}|{observation}\n"))
        .collect::<String>();
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    fixture.cleanup();
}

#[test]
fn guarded_variants_preserve_fallback_and_profile_diagnostics() {
    let missing = "module t; @id(\"t.main\") fn main()->i64 { let x=Option<i64>::Some { value: 7 }; match x { Option::Some { value: n } if n>0 => n, Option::None {} => 0, } }";
    let wrong_type = missing.replace("n>0", "n").replace(
        "Option::None {} =>",
        "Option::Some { value: other } => 1, Option::None {} =>",
    );
    let unsupported = wrong_type.replace("if n =>", "if string_len(\"temporary\")>0 =>");
    let unreachable = missing.replace(
        "Option::Some { value: n } if",
        "Option::Some { value: first } => first, Option::Some { value: n } if",
    );
    let owned = "module t; @id(\"t.main\") fn main()->i64 { let x=Option<string>::Some { value: \"owned\" }; match own x { Option::Some { value: text } if true => string_len(text), Option::None {} => 0, } }";
    for (source, code) in [
        (missing, "SPX-M101"),
        (&wrong_type, "SPX-T256"),
        (&unsupported, "SPX-T254"),
        (&unreachable, "SPX-M102"),
        (owned, "SPX-T254"),
    ] {
        let program = parse(source, Path::new("guarded-variant-refusal.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap_or_else(|| panic!("expected {code}: {diagnostics:?}"));
        assert!(diagnostic.span.is_some());
        assert_eq!(
            diagnostic.path.as_deref(),
            Some("guarded-variant-refusal.spx")
        );
        assert!(hir::resolve(&program).is_err());
    }
}

fn first_match(expression: &mut hir::ResolvedExpr) -> &mut hir::ResolvedExpr {
    match &mut expression.kind {
        hir::ResolvedExprKind::Block { statements, .. } => statements
            .iter_mut()
            .find_map(|statement| match statement {
                hir::ResolvedStatement::Let { value, .. }
                    if matches!(value.kind, hir::ResolvedExprKind::Match { .. }) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("match let retained"),
        _ => panic!("block retained"),
    }
}

#[test]
fn guarded_variant_hir_and_cleanup_edges_fail_closed() {
    let parsed = parse(SOURCE, Path::new("guarded-variant-hostile.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "guards.skipped")
        .unwrap();
    assert!(function.cleanup_plan.edges.iter().any(|edge| matches!(
        edge.condition,
        semaprax::cleanup_plan::EdgeCondition::BooleanResult(_, false)
    )));
    let foreign_value = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "guards.lazy")
        .unwrap()
        .result_id
        .clone();
    for mutation in 0..4 {
        let mut hostile = resolved.clone();
        let function = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "guards.skipped")
            .unwrap();
        let hir::ResolvedExprKind::Match { arms, .. } = &mut first_match(&mut function.body).kind
        else {
            unreachable!()
        };
        match mutation {
            0 => arms[0].guard.as_mut().unwrap().ty = hir::ResolvedType::I64,
            1 => {
                arms.remove(1);
            }
            2 => {
                let id = arms[0].value.id.clone();
                arms[0].guard.as_mut().unwrap().id = id;
            }
            3 => {
                let guard = arms[0].guard.as_mut().unwrap();
                guard.kind = hir::ResolvedExprKind::Place(hir::Place {
                    root: foreign_value.clone(),
                    projections: Vec::new(),
                });
            }
            _ => unreachable!(),
        }
        assert_eq!(
            hir::validate(&hostile).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
    }
    let mut hostile = resolved;
    let function = hostile
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "guards.skipped")
        .unwrap();
    let edge = function
        .cleanup_plan
        .edges
        .iter_mut()
        .find(|edge| {
            matches!(
                edge.condition,
                semaprax::cleanup_plan::EdgeCondition::BooleanResult(_, false)
            )
        })
        .unwrap();
    let semaprax::cleanup_plan::EdgeCondition::BooleanResult(_, value) = &mut edge.condition else {
        unreachable!()
    };
    *value = true;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn copy_variant_wasm_profile_is_explicit_and_closed() {
    let program = parse(SOURCE, Path::new("guarded-copy-profile.spx")).unwrap();
    let selected = vec!["guards.loop".to_owned(), "guards.indexed".to_owned()];
    assert_eq!(
        emit_module(&program, &selected, InternalStringOptions::default())
            .unwrap_err()
            .code,
        "SPX-W111"
    );
    let artifact =
        emit_copy_variant_module(&program, &selected, InternalStringOptions::default()).unwrap();
    let descriptor: Value = serde_json::from_str(artifact.descriptor()).unwrap();
    assert_eq!(descriptor["profile"], "copy-variants-v1");
    let mut reversed = selected.clone();
    reversed.reverse();
    let reordered =
        emit_copy_variant_module(&program, &reversed, InternalStringOptions::default()).unwrap();
    assert_eq!(artifact.wasm_bytes(), reordered.wasm_bytes());
    assert_eq!(artifact.descriptor(), reordered.descriptor());
    assert_eq!(artifact.runtime_source(), reordered.runtime_source());
    let unselected = r#"
@id("unselected.owned") fn unselected() -> i64 {
    let owned = Option<string>::Some { value: "excluded" };
    match own owned { Option::Some { value: text } => string_len(text), Option::None {} => 0, }
}
"#;
    let extended = parse(
        &format!("{SOURCE}{unselected}"),
        Path::new("guarded-copy-unselected.spx"),
    )
    .unwrap();
    let unchanged =
        emit_copy_variant_module(&extended, &selected, InternalStringOptions::default()).unwrap();
    assert_eq!(artifact.wasm_bytes(), unchanged.wasm_bytes());
    assert_eq!(artifact.descriptor(), unchanged.descriptor());
    assert_eq!(artifact.runtime_source(), unchanged.runtime_source());
    assert_eq!(
        emit_copy_variant_module(
            &extended,
            &["unselected.owned".to_owned()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    let forged = SOURCE.replace("n == 7 && i < 2", "n + 1");
    let forged = parse(&forged, Path::new("guarded-copy-bad-guard.spx")).unwrap();
    assert_eq!(
        emit_copy_variant_module(&forged, &selected, InternalStringOptions::default())
            .unwrap_err()
            .code,
        "SPX-T256"
    );
}
