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
@id("guards.index_bounds") fn index_bounds() -> i64 {
    let empty = [];
    let empty_view = array_as_slice(empty);
    let array = [255u8];
    let view = array_as_slice(array);
    let zero = match byte_get(empty_view, 0usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let empty_high = match byte_get(empty_view, 4294967296usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let empty_maximum = match byte_get(empty_view, 18446744073709551615usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let high = match byte_get(view, 4294967296usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let high_bit = match byte_get(view, 9223372036854775808usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let maximum = match byte_get(view, 18446744073709551615usize) { Option::Some { value: byte } => 0, Option::None {} => 1, };
    let length = if byte_len(empty_view) == 0usize { 1 } else { 0 };
    zero + empty_high + empty_maximum + high + high_bit + maximum + length
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
@id("guards.constructed") fn constructed() -> i64 {
    let mut i = 0;
    let mut total = 0;
    let mut out = "";
    while i < 3 {
        let selected = Option<i64>::Some { value: i };
        let piece = match selected {
            Option::Some { value: n } if n == 1 => "ab",
            Option::Some { value: n } => "x",
            Option::None {} => "bad",
        };
        total = total + match selected { Option::Some { value: n } => n, Option::None {} => 1000, };
        out = string_concat(out, piece);
        i = i + 1;
        0
    }
    string_len(out) * 10 + total
}
@id("guards.constructed-nested") fn constructed_nested() -> i64 {
    let mut i = 0;
    let mut total = 0;
    while i < 2 {
        let mut j = 0;
        while j < 2 {
            let piece = match (Result<bool, i64>::Err { error: i + j }) {
                Result::Ok { value: yes } => "bad",
                Result::Err { error: n } if n == 0 => "z",
                Result::Err { error: n } => "qq",
            };
            total = total + string_len(piece);
            j = j + 1;
            0
        }
        i = i + 1;
        0
    }
    total
}
@id("guards.constructed-inspect") fn constructed_inspect() -> i64 {
    let mut i = 0;
    let mut total = 0;
    while i < 3 {
        let piece = match (Option<i64>::Some { value: string_len("abc") }) {
            Option::Some { value: n } if n == 3 => "x",
            Option::Some { value: n } => "bad",
            Option::None {} => "bad",
        };
        total = total + string_len(piece);
        i = i + 1;
        0
    }
    total
}
@id("guards.constructed-condition") fn constructed_condition() -> i64 {
    let mut i = 0;
    while match (Option<i64>::Some { value: i }) { Option::Some { value: n } => n < 3, Option::None {} => false, } {
        i = i + 1;
        0
    }
    i
}
@id("guards.constructed-failure") fn constructed_failure() -> i64 {
    let mut out = "kept";
    let mut i = 0;
    while i < 3 {
        let local = "iteration";
        let piece = match (Option<i64>::Some { value: i }) {
            Option::Some { value: n } if n == 1 && n / 0 == 1 => "bad",
            Option::Some { value: n } => "x",
            Option::None {} => "bad",
        };
        out = string_concat(out, piece);
        i = i + 1;
        0
    }
    string_len(out)
}
@id("guards.constructed-operand-failure") fn constructed_operand_failure() -> i64 {
    let kept = "kept";
    while true {
        let piece = match (Option<i64>::Some { value: 1 / 0 }) {
            Option::Some { value: n } => "bad",
            Option::None {} => "bad",
        };
        0
    }
    string_len(kept)
}
@id("guards.constructed-for") fn constructed_for() -> i64 {
    let mut values = vec_with_capacity<i64>(2usize);
    values = vec_push<i64>(values, 1);
    values = vec_push<i64>(values, 3);
    let mut out = "";
    let mut total = 0;
    for item in values {
        let piece = match (Option<i64>::Some { value: item }) {
            Option::Some { value: n } if n == 1 => "a",
            Option::Some { value: n } => "b",
            Option::None {} => "bad",
        };
        total = total + item;
        out = string_concat(out, piece);
        0
    }
    total + string_len(out)
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
    ("guards.index_bounds", "ok|7"),
    ("guards.constructed", "ok|43"),
    ("guards.constructed-nested", "ok|7"),
    ("guards.constructed-inspect", "ok|3"),
    ("guards.constructed-condition", "ok|3"),
    ("guards.constructed-failure", "semaprax.arithmetic.v1|1"),
    (
        "guards.constructed-operand-failure",
        "semaprax.arithmetic.v1|1",
    ),
    ("guards.constructed-for", "ok|6"),
];
const WASM_CASES: &[&str] = &[
    "guards.loop",
    "guards.skipped",
    "guards.lazy",
    "guards.result",
    "guards.indexed",
    "guards.failure",
    "guards.index_bounds",
    "guards.constructed",
    "guards.constructed-nested",
    "guards.constructed-inspect",
    "guards.constructed-condition",
    "guards.constructed-failure",
    "guards.constructed-operand-failure",
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
    let calloc_observer = r#"
static void *fixture_calloc(size_t n, size_t size) {
    REQUIRE(n != 0 && size != 0 && n <= SIZE_MAX / size);
    void *pointer = fixture_malloc(n * size);
    memset(pointer, 0, n * size);
    return pointer;
}
#define calloc fixture_calloc
"#;
    let mut probe = format!(
        "{}\n{}\n{calloc_observer}\n{generated}\n#undef malloc\n#undef free\n#undef calloc\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let validation = if let Some(value) = observation.strip_prefix("ok|") {
            format!("REQUIRE(token==0 && value==INT64_C({value}));")
        } else {
            let (domain, code) = observation.rsplit_once('|').unwrap();
            format!("REQUIRE(token!=0); const struct spx_normalized_status *selected=spx_status_resolve(&context,token); REQUIRE(selected!=NULL && selected->code=={code} && strcmp(selected->domain_id,\"{domain}\")==0);")
        };
        probe.push_str(&format!(
            r#"for (int repeat=0; repeat<3; ++repeat) {{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ if(repeat==0) (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MIN);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        if(repeat==0) (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    {validation}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    for (uint32_t slot=0; slot<SPX_VEC_AUTHORITY_CAPACITY; ++slot) REQUIRE(!context.vec_authority[slot].live);
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
    // The active None case still has a type with a non-Copy Bytes payload.
    let owned = Option<Bytes>::None {};
    0
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

#[test]
fn loop_copy_construction_preserves_closed_source_and_wasm_boundaries() {
    let prefix = "module t; permit { unsafe } @id(\"t.main\") fn main()->i64 { while false { ";
    for (body, code) in [
        ("let x=Option<string>::None {}; 0", "SPX-T252"),
        ("let x=Option<i64>::Some { value: { @audit(\"loop\") unsafe { 0 } 0 } }; 0", "SPX-T252"),
        ("let x=Option<i64>::Some { value: 1 }; match x { Option::Some { wrong: n } => n, Option::None {} => 0, }", "SPX-M104"),
        ("match (Option<i64>::Some { value: 1 }) { Option::Some { value: n } if n>0 => n, Option::None {} => 0, }", "SPX-M101"),
        ("match (Option<i64>::Some { value: 1 }) { Option::Some { value: n } if { true } => n, Option::Some { value: n } => n, Option::None {} => 0, }", "SPX-T254"),
    ] {
        let source = format!("{prefix}{body} }} 0 }}");
        let program = parse(&source, Path::new("loop-construction-refusal.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        let diagnostic = diagnostics.iter().find(|d| d.code == code).unwrap_or_else(|| panic!("expected {code}: {diagnostics:?}"));
        assert!(diagnostic.span.is_some());
        assert_eq!(diagnostic.path.as_deref(), Some("loop-construction-refusal.spx"));
        assert!(hir::resolve(&program).is_err());
    }
    let program = parse(SOURCE, Path::new("loop-construction-profile.spx")).unwrap();
    assert_eq!(
        emit_copy_variant_module(
            &program,
            &["guards.constructed-for".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    assert_eq!(
        emit_module(
            &program,
            &["guards.constructed".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
}

fn loop_constructor_mut(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "guards.constructed")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    let body = statements
        .iter_mut()
        .find_map(|s| match s {
            hir::ResolvedStatement::While { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop block")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("constructor let")
    };
    value
}

#[test]
fn loop_copy_construction_rejects_forged_hir_types_owners_and_members() {
    let program =
        hir::resolve(&parse(SOURCE, Path::new("loop-construction-hostile.spx")).unwrap()).unwrap();
    for mutation in 0..5 {
        let mut hostile = program.clone();
        let value = loop_constructor_mut(&mut hostile);
        match mutation {
            0 => value.ownership = hir::OwnershipMode::Own,
            1 => {
                let hir::ResolvedType::Nominal { arguments, .. } = &mut value.ty else {
                    unreachable!()
                };
                arguments[0] = hir::ResolvedType::String;
            }
            2 => {
                let hir::ResolvedExprKind::ConstructVariant { case, .. } = &mut value.kind else {
                    unreachable!()
                };
                *case = hir::DeclarationId::new("core.option.none");
            }
            3 => {
                let hir::ResolvedExprKind::ConstructVariant { fields, .. } = &mut value.kind else {
                    unreachable!()
                };
                fields[0].field = hir::DeclarationId::new("foreign.field");
            }
            4 => {
                let hir::ResolvedExprKind::ConstructVariant { fields, .. } = &mut value.kind else {
                    unreachable!()
                };
                fields[0].value.ownership = hir::OwnershipMode::Own;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            hir::validate(&hostile).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
    }
}
