//! Condition temporaries settle before both loop outcomes and on failure.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
const SOURCE: &str = r#"
module test.string_conditions;
@id("condition.render") fn render(n:i64)->string {string_concat("n",string_from_i64(n))}
@id("condition.checked") fn checked(n:i64)->string requires n<2 {string_from_i64(n)}
@id("condition.repeat") fn repeat()->i64 {let held="owner";let mut i=0;while i<400 && string_len(render(i))>0 {i=i+1;0} i+string_len(held)}
@id("condition.false") fn skip()->i64 {let held="owner";while string_len(string_concat("",""))>0 {0} string_len(held)}
@id("condition.lazy") fn lazy()->i64 {let held="owner";while false && string_len(render(1/0))>0 {0} string_len(held)}
@id("condition.block") fn block()->i64 {let mut i=0;while {let note=render(i);string_len(note)>0} && i<3 {i=i+1;0} i}
@id("condition.nested") fn nested()->i64 {let mut i=0;let mut n=0;while i<3 && string_len(render(i))>0 {let mut j=0;while j<2 && string_len(render(j))>0 {j=j+1;n=n+1;0} i=i+1;0} n}
@id("condition.failure") fn failure()->i64 {let held="owner";while string_len(string_concat("a","b"))>0 && 1/0==0 {0} string_len(held)}
@id("condition.contract") fn contract()->i64 {let held="owner";let mut i=0;while i<3 && string_len(checked(i))>0 {i=i+1;0} i+string_len(held)}
@id("condition.text-failure") fn text_failure()->i64 {let held="owner";while string_len(string_slice(string_concat("a","b"),0,9))>0 {0} string_len(held)}
@id("app.main") fn main() -> i64 { 0 }
"#;
const CASES: &[(&str, &str)] = &[
    ("condition.repeat", "ok|405"),
    ("condition.false", "ok|5"),
    ("condition.lazy", "ok|5"),
    ("condition.block", "ok|3"),
    ("condition.nested", "ok|6"),
    ("condition.failure", "semaprax.arithmetic.v1|4"),
    ("condition.contract", "semaprax.contract.v1|1"),
    ("condition.text-failure", "semaprax.text.v1|1"),
];
fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_conditions_roundtrip_and_reject_missing_scope_finalizers() {
    let ast = parse(SOURCE, Path::new("conditions.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = format::canonical(&ast);
    let round = parse(&canonical, Path::new("conditions.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&round));
    graph::verify_json(&round, &graph::to_json(&round).unwrap()).unwrap();
    let resolved = hir::resolve(&round).unwrap();
    hir::validate(&resolved).unwrap();
    let mut hostile = resolved.clone();
    let main = hostile
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "condition.false")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("block")
    };
    let condition = statements
        .iter()
        .find_map(|s| {
            if let hir::ResolvedStatement::While { condition, .. } = s {
                Some(condition.id.clone())
            } else {
                None
            }
        })
        .unwrap();
    let plan = &mut main.cleanup_plan;
    let branch=plan.blocks.iter().find(|b|match &b.terminator {semaprax::cleanup_plan::CleanupTerminator::Branch(edges)=>edges.iter().any(|e|matches!(&plan.edges[e.0 as usize].condition,semaprax::cleanup_plan::EdgeCondition::BooleanResult(id,_) if id==&condition)),_=>false}).unwrap().id;
    let edge = plan.edges.iter().find(|edge| edge.to == branch).unwrap().id;
    let exit=plan.exits.iter_mut().find(|exit|matches!(exit.continuation,semaprax::cleanup_plan::ExitContinuation::Continue(id) if id==edge)).unwrap();
    assert!(!exit.finalize_in_order.is_empty());
    exit.finalize_in_order.clear();
    assert!(hir::validate(&hostile).is_err());
    assert!(semaprax::codegen::emit_hir_c(&hostile).is_err());
}
#[test]
fn string_condition_cannot_consume_an_enclosing_owner() {
    let source="module negative; fn consume(value:string)->bool {string_len(value)>0} fn main()->i64 {let value=\"x\";while consume(value){0} 0}";
    let ast = parse(source, Path::new("condition-negative.spx")).unwrap();
    let errors = verify::verify(&ast);
    assert!(
        errors.iter().any(|e| e.code == "SPX-T252"
            && e.message == "ownership of `value` changes inside a while condition"),
        "{errors:?}"
    );
    assert!(hir::resolve(&ast).is_err());
}
#[test]
fn string_conditions_reference_interpreter() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("string-condition.spx")).unwrap());
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
fn string_conditions_native_settlement() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("string-condition-native.spx")).unwrap();
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
