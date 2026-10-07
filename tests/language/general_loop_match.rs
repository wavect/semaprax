//! General Copy loop matches, private helper boundaries and scoped guards.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{
    emit_copy_variant_module, emit_general_loop_match_module, emit_module,
    emit_string_replacement_module, InternalStringOptions,
};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"
module test.general_loop_match;
@id("choice.type") variant Choice { @id("choice.a") A, @id("choice.b") B, }
@id("helper.choose") fn make_choice(index:i64)->Option<i64> { if index%2==0 { Option<i64>::Some {value:index} } else { Option<i64>::None {} } }
@id("helper.read") fn consume_choice(choice:Option<i64>)->i64 { match choice { Option::Some {value:n} => n, Option::None {} => 10, } }
@id("helper.positive") fn positive(value:i64)->bool { value>=0 }
@id("helper.required") fn requires_positive(value:i64)->bool requires value>0 { true }
@id("helper.pair") fn pair(first:string,second:i64)->bool { string_len(first)>second }
@id("helper.text") fn nonempty(text:string)->bool { string_len(text)>0 }
@id("general.boundary") fn call_boundary()->i64 {
 let mut index=0; let mut total=0;
 while index<4 { let choice=make_choice(index); total=total+consume_choice(choice); index=index+1; 0 } total
}
@id("general.call") fn guarded_call()->i64 {
 let choice=Option<i64>::Some {value:5}; let mut index=0; let mut total=0;
 while index<3 { total=total+match choice { Option::Some {value:n} if positive(n) => n, Option::Some {value:n} => -1, Option::None {} => 0, }; index=index+1; 0 } total
}
@id("general.block") fn guarded_block()->i64 {
 let choice=Option<i64>::Some {value:2}; let held="held"; let mut index=0; let mut total=0;
 while index<4 { let text=match choice { Option::Some {value:n} if { let temporary=string_concat("é","\u{0}"); string_len(temporary)==3 && positive(n) } => "ok", Option::Some {value:n} => "x", Option::None {} => "", }; total=total+string_len(text); index=index+1; 0 } total+string_len(held)-4
}
@id("general.false") fn guarded_false()->i64 {
 let choice=Option<i64>::Some {value:2}; let mut index=0; let mut total=0;
 while index<2 { let text=match choice { Option::Some {value:n} if string_len(string_concat("a","b"))==99 => "wrong", Option::Some {value:n} => "N", Option::None {} => "", }; total=total+string_len(text); index=index+1; 0 } total
}
@id("general.wrong-case") fn wrong_case()->i64 {
 let choice=Option<i64>::None {}; let mut index=0; let mut total=0;
 while index<2 { total=total+match choice { Option::Some {value:n} if 1/0==0 => 99, Option::Some {value:n} => 1, Option::None {} => 7, }; index=index+1; 0 } total
}
@id("general.failure") fn guard_failure()->i64 {
 let choice=Option<i64>::Some {value:2}; let held="held"; let mut index=0;
 while index<2 { let text=match choice { Option::Some {value:n} if string_len(string_concat("a","b"))>0 && 1/0==0 => "wrong", Option::Some {value:n} => "N", Option::None {} => "", }; index=index+string_len(text); 0 } string_len(held)
}
@id("general.contract") fn guard_contract()->i64 {
 let choice=Option<i64>::Some {value:0}; let held="held"; let mut index=0;
 while index<2 { let text=match choice { Option::Some {value:n} if nonempty("temporary") && requires_positive(n) && 1/0==0 => "wrong", Option::Some {value:n} => "N", Option::None {} => "", }; index=index+string_len(text); 0 } string_len(held)
}
@id("general.wildcard") fn guarded_wildcard()->i64 {
 let choice=Option<i64>::Some {value:2}; let mut index=0; let mut total=0;
 while index<2 { let text=match choice { _ if string_len(string_concat("a","b"))==2 => "yes", Option::Some {value:n} => "N", Option::None {} => "", }; total=total+string_len(text); index=index+1; 0 } total
}
@id("general.or") fn guarded_or()->i64 {
 let choice=Choice::A {}; let mut index=0; let mut total=0;
 while index<3 { let text=match choice { Choice::A {} | Choice::B {} if positive(1) => "four", _ => "", }; total=total+string_len(text); index=index+1; 0 } total
}
@id("general.lazy") fn guarded_lazy()->i64 {
 let choice=Option<i64>::Some {value:2}; let mut index=0; let mut total=0;
 while index<2 { total=total+match choice { Option::Some {value:n} if nonempty("x") || requires_positive(0) => 9, Option::Some {value:n} => 0, Option::None {} => 0, }; index=index+1; 0 } total
}
@id("general.nested") fn guarded_nested()->i64 {
 let choice=Option<i64>::Some {value:2}; let mut index=0; let mut total=0;
 while index<2 { let text=match choice { Option::Some {value:n} if consume_choice(make_choice(n))>=0 && match make_choice(n) { Option::Some {value:inner} if positive(inner) => true, Option::Some {value:inner} => false, Option::None {} => false, } => string_concat("a","b"), Option::Some {value:n} => "N", Option::None {} => "", }; total=total+string_len(text); index=index+1; 0 } total
}
@id("general.operand-failure") fn operand_failure()->i64 {
 let mut index=0; while index<1 { let number=match make_choice(1/0) { Option::Some {value:n} if nonempty("never") => n, _ => 0, }; index=index+number; 0 } index
}
@id("general.result") fn variant_result()->i64 {
 let choice=Option<i64>::Some {value:2}; let mut index=0; let mut total=0;
 while index<3 { let output=match choice { Option::Some {value:n} if positive(n) => Option<i64>::Some {value:n+1}, _ => Option<i64>::None {}, }; total=total+consume_choice(output); index=index+1; 0 } total
}
@id("general.scalar-result") fn scalar_variant_result()->i64 {
 let mut index=0; let mut total=0;
 while index<2 { let output=match index { 0 if positive(index) => make_choice(2), _ => Option<i64>::None {}, }; total=total+consume_choice(output); index=index+1; 0 } total
}
@id("general.staging-failure") fn staging_failure()->i64 {
 let choice=Option<i64>::Some {value:2}; let held="held";
 while false || true { let number=match choice { Option::Some {value:n} if pair(string_concat("first","!"),1/0) => n, _ => 0, }; let unused=number; 0 } string_len(held)
}
@id("general.for") fn for_case()->i64 {
 let mut values=vec_with_capacity<i64>(2usize); values=vec_push<i64>(values,0); values=vec_push<i64>(values,2); let finished=values; let mut total=0;
 for value in finished { total=total+match make_choice(value) { Option::Some {value:n} if positive(n) => n, _ => 0, }; 0 } total
}
@id("app.main") fn main()->i64 { 0 }
"#;
const CASES: &[(&str, &str)] = &[
    ("general.boundary", "ok|22"),
    ("general.call", "ok|15"),
    ("general.block", "ok|8"),
    ("general.false", "ok|2"),
    ("general.wrong-case", "ok|14"),
    ("general.failure", "semaprax.arithmetic.v1|4"),
    ("general.contract", "semaprax.contract.v1|1"),
    ("general.wildcard", "ok|6"),
    ("general.or", "ok|12"),
    ("general.lazy", "ok|18"),
    ("general.nested", "ok|4"),
    ("general.operand-failure", "semaprax.arithmetic.v1|4"),
    ("general.result", "ok|9"),
    ("general.scalar-result", "ok|12"),
    ("general.staging-failure", "semaprax.arithmetic.v1|4"),
    ("general.for", "ok|2"),
];
const WASM_CASES: &[&str] = &[
    "general.boundary",
    "general.call",
    "general.block",
    "general.false",
    "general.wrong-case",
    "general.failure",
    "general.contract",
    "general.wildcard",
    "general.or",
    "general.lazy",
    "general.nested",
    "general.operand-failure",
    "general.result",
    "general.scalar-result",
    "general.staging-failure",
];
fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn general_loop_match_source_graph_and_roundtrip() {
    let program = parse(SOURCE, Path::new("general-loop-match.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("general-loop-match.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let document: Value = serde_json::from_str(&graph::to_json(&reparsed).unwrap()).unwrap();
    let resolved = hir::resolve(&reparsed).unwrap();
    hir::validate(&resolved).unwrap();
    assert!(document.to_string().contains("guard"));
    let block = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "general.block")
        .unwrap();
    assert!(block.cleanup_plan.regions.len() > 2);
    assert!(block
        .cleanup_plan
        .exits
        .iter()
        .any(|exit| !exit.finalize_in_order.is_empty()));
}

#[test]
fn general_loop_match_reference_interpreter() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("general-loop-match.spx")).unwrap());
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
fn general_loop_match_native_settlement() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("general-loop-match-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let calloc_observer = r#"static void *fixture_calloc(size_t n,size_t size) { REQUIRE(n!=0 && size!=0 && n<=SIZE_MAX/size); void *pointer=fixture_malloc(n*size); memset(pointer,0,n*size); return pointer; }
#define calloc fixture_calloc
"#;
    let mut probe = format!(
        "{}\n{}\n{calloc_observer}\n{generated}\n#undef calloc\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
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
            r#"{{ for(unsigned repeat=0;repeat<3;repeat++) {{
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
}} }}
"#
        ));
        for _ in 0..3 {
            expected.push_str(&format!("{id}|{observation}\n"));
        }
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn general_loop_match_wasm_settlement() {
    if !command_available("node") || WASM_CASES.is_empty() {
        return;
    }
    let program = parse(SOURCE, Path::new("general-loop-match-wasm.spx")).unwrap();
    let selected = WASM_CASES
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact =
        emit_general_loop_match_module(&program, &selected, InternalStringOptions::default())
            .unwrap();
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
fn general_loop_match_preserves_stable_source_refusals() {
    let prefix =
        "module t; @id(\"t.inspect\") fn inspect(text:string)->bool { string_len(text)>0 } ";
    for (body,code) in [
        ("let choice=Option<i64>::Some {value:1}; match choice { _ if true => 1, }", "SPX-M101"),
        ("let choice=Option<i64>::Some {value:1}; match choice { Option::Some {value:n} if n => n, _ => 0, }", "SPX-T256"),
        ("let choice=Option<string>::Some {value:\"owned\"}; match own choice { Option::Some {value:text} if true => string_len(text), Option::None {} => 0, }", "SPX-T254"),
        ("let held=\"outer\"; let choice=Option<i64>::Some {value:1}; match choice { _ if inspect(held) => 1, _ => 0, }", "SPX-T254"),
        ("let choice=Option<i64>::Some {value:1}; match choice { Option::Some {wrong:n} if true => n, _ => 0, }", "SPX-M104"),
    ] {
        let source=format!("{prefix}@id(\"t.main\") fn main()->i64 {{ {body} }}");
        let program=parse(&source,Path::new("general-match-refusal.spx")).unwrap();
        let diagnostics=verify::verify(&program);
        let diagnostic=diagnostics.iter().find(|d|d.code==code).unwrap_or_else(||panic!("expected {code}: {diagnostics:?}"));
        assert!(diagnostic.span.is_some());
        assert_eq!(diagnostic.path.as_deref(),Some("general-match-refusal.spx"));
        assert!(hir::resolve(&program).is_err());
    }
}

#[test]
fn general_loop_match_wasm_selection_is_explicit_and_closed() {
    let program = parse(SOURCE, Path::new("general-match-profiles.spx")).unwrap();
    for id in [
        "general.block",
        "general.wildcard",
        "general.or",
        "general.boundary",
        "general.result",
        "general.scalar-result",
    ] {
        assert_eq!(
            emit_module(&program, &[id.into()], InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
        assert_eq!(
            emit_copy_variant_module(&program, &[id.into()], InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    assert_eq!(
        emit_string_replacement_module(
            &program,
            &["general.block".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    assert_eq!(
        emit_general_loop_match_module(
            &program,
            &["general.for".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    let selected = vec!["general.block".into()];
    let first =
        emit_general_loop_match_module(&program, &selected, InternalStringOptions::default())
            .unwrap();
    let second =
        emit_general_loop_match_module(&program, &selected, InternalStringOptions::default())
            .unwrap();
    assert_eq!(first.wasm_bytes(), second.wasm_bytes());
    assert_eq!(first.descriptor(), second.descriptor());
    assert!(first.descriptor().contains("general-loop-match-v1"));
    assert_eq!(
        emit_general_loop_match_module(
            &program,
            &["helper.choose".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
}

fn first_match_mut(expression: &mut hir::ResolvedExpr) -> &mut hir::ResolvedExpr {
    if matches!(expression.kind, hir::ResolvedExprKind::Match { .. }) {
        return expression;
    }
    match &mut expression.kind {
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    hir::ResolvedStatement::Let { value, .. }
                        if matches!(value.kind, hir::ResolvedExprKind::Match { .. }) =>
                    {
                        return value
                    }
                    hir::ResolvedStatement::While { body, .. } => return first_match_mut(body),
                    _ => {}
                }
            }
            first_match_mut(tail)
        }
        _ => panic!("fixture Match not found"),
    }
}
#[test]
fn general_loop_match_rejects_forged_guards_and_settlement() {
    let parsed = parse(SOURCE, Path::new("general-match-hostile.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).unwrap();
    for mutation in 0..4 {
        let mut hostile = program.clone();
        let function = hostile
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "general.block")
            .unwrap();
        match mutation {
            0 => {
                let hir::ResolvedExprKind::Match { arms, .. } =
                    &mut first_match_mut(&mut function.body).kind
                else {
                    unreachable!()
                };
                arms[0].guard.as_mut().unwrap().ty = hir::ResolvedType::I64;
            }
            1 => {
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
                let semaprax::cleanup_plan::EdgeCondition::BooleanResult(_, value) =
                    &mut edge.condition
                else {
                    unreachable!()
                };
                *value = true;
            }
            2 => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| !exit.finalize_in_order.is_empty())
                    .unwrap();
                exit.finalize_in_order.clear();
            }
            3 => {
                let hir::ResolvedExprKind::Match { arms, .. } =
                    &mut first_match_mut(&mut function.body).kind
                else {
                    unreachable!()
                };
                arms.remove(1);
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

#[test]
fn general_loop_match_rejects_forged_outer_owner_consumption() {
    let mut program =
        hir::resolve(&parse(SOURCE, Path::new("general-guard-owner.spx")).unwrap()).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "general.contract")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        unreachable!()
    };
    let owner = statements
        .iter()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::Let { binding, .. } if binding.name == "held" => {
                Some(binding.id.clone())
            }
            _ => None,
        })
        .unwrap();
    let hir::ResolvedExprKind::Match { arms, .. } = &mut first_match_mut(&mut function.body).kind
    else {
        unreachable!()
    };
    fn replace(expression: &mut hir::ResolvedExpr, owner: &hir::ValueId) -> bool {
        match &mut expression.kind {
            hir::ResolvedExprKind::Call { args, .. }
                if args
                    .first()
                    .is_some_and(|arg| arg.ty == hir::ResolvedType::String) =>
            {
                args[0].kind = hir::ResolvedExprKind::Place(hir::Place {
                    root: owner.clone(),
                    projections: Vec::new(),
                });
                true
            }
            hir::ResolvedExprKind::Binary { left, right, .. } => {
                replace(left, owner) || replace(right, owner)
            }
            _ => false,
        }
    }
    assert!(replace(arms[0].guard.as_mut().unwrap(), &owner));
    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn general_loop_match_guards_retain_declared_read_authority() {
    let source = r#"module t;
permit { process.args.read }
@id("t.read") fn read()->bool uses { process.args.read } { args_len()==0usize }
@id("t.main") fn main()->i64 uses { process.args.read } {
 while false { let choice=Option<i64>::Some {value:1}; let ignored=match choice { _ if read() => 1, _ => 0, }; 0 } 0
}
"#;
    let program = parse(source, Path::new("guard-read-authority.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    assert_eq!(
        emit_general_loop_match_module(
            &program,
            &["t.main".into()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    let missing = source.replace(
        "fn main()->i64 uses { process.args.read }",
        "fn main()->i64",
    );
    let diagnostics =
        verify::verify(&parse(&missing, Path::new("guard-read-authority.spx")).unwrap());
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-E102"),
        "{diagnostics:?}"
    );
    let write = source
        .replace("process.args.read", "process.stdout.write")
        .replace("args_len()==0usize", "true");
    let diagnostics = verify::verify(&parse(&write, Path::new("guard-write-refusal.spx")).unwrap());
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.code == "SPX-T252")
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert!(diagnostic.span.is_some());
    assert_eq!(diagnostic.path.as_deref(), Some("guard-write-refusal.spx"));
}
