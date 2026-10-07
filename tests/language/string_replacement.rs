//! General whole mutable String replacement and canonical owner settlement.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{
    emit_copy_variant_module, emit_module, emit_string_replacement_module, InternalStringOptions,
};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"
module test.string_replacement;
@id("helper.rebuild") fn rebuild(input: own string) -> string { string_concat(input, "!") }
@id("helper.reject") fn reject(input: own string) -> string requires string_len(input) < 2 { input }
@id("replace.literal") fn literal() -> i64 { let mut text="old"; text="é\u{0}x"; string_len(text) }
@id("replace.named") fn named() -> i64 { let mut text="old"; let next="new"; let before=string_len(next); text=next; string_len(text)+before }
@id("replace.branch") fn branch() -> i64 {
    let mut text="old"; let held="held"; let mut index=0;
    while index<4 { text=if index%2==0 { string_concat("a", "b") } else { "c" }; index=index+1; 0 }
    string_len(text)*10+string_len(held)
}
@id("replace.match") fn matched() -> i64 {
    let mut text="old"; let mut index=0;
    while index<3 { text=match index { 0 => "a", 1 => "bb", _ => "ccc", }; index=index+1; 0 }
    string_len(text)
}
@id("replace.reverse") fn reversed() -> i64 { let mut text="x"; text=string_concat("p",text); string_len(text) }
@id("replace.other") fn other_owner() -> i64 { let mut text="old"; let next="a"; text=rebuild(next); string_len(text) }
@id("replace.owned") fn owned() -> i64 { let mut text="a"; text=rebuild(text); string_len(text) }
@id("replace.mixed") fn mixed() -> i64 {
    let mut text="a"; let mut index=0;
    while index<4 { text=if index%2==0 { rebuild(text) } else { "b" }; index=index+1; 0 }
    string_len(text)
}
@id("replace.nested") fn nested() -> i64 {
    let mut text="old"; let mut other="other";
    text={ text="middle"; other=if true { "x" } else { "z" }; string_concat(text, "!") };
    string_len(text)*10+string_len(other)
}
@id("replace.slice") fn sliced() -> i64 { let mut text="abc"; text=string_slice(text,1,string_len(text)); string_len(text) }
@id("replace.zero") fn zero() -> i64 { let mut text="old"; while false { text="new"; 0 } string_len(text) }
@id("replace.failure") fn failure() -> i64 {
    let mut text="old"; let held="held";
    text={ let temporary="temp"; let crash=1/0; string_concat(temporary,"new") };
    string_len(text)+string_len(held)
}
@id("replace.condition-failure") fn condition_failure() -> i64 {
    let mut text="old"; text=if 1/0==0 { "x" } else { "y" }; string_len(text)
}
@id("replace.contract") fn contract() -> i64 { let mut text="old"; text=reject(text); string_len(text) }
@id("replace.text-failure") fn text_failure() -> i64 { let mut text="old"; text=string_slice(text,0,99); string_len(text) }
@id("replace.legacy") fn legacy() -> i64 { let mut text="a"; text=string_concat(text,"b"); string_len(text) }
"#;
const CASES: &[(&str, &str)] = &[
    ("replace.literal", "ok|4"),
    ("replace.named", "ok|6"),
    ("replace.branch", "ok|14"),
    ("replace.match", "ok|3"),
    ("replace.reverse", "ok|2"),
    ("replace.other", "ok|2"),
    ("replace.owned", "ok|2"),
    ("replace.mixed", "ok|1"),
    ("replace.nested", "ok|71"),
    ("replace.slice", "ok|2"),
    ("replace.zero", "ok|3"),
    ("replace.failure", "semaprax.arithmetic.v1|1"),
    ("replace.condition-failure", "semaprax.arithmetic.v1|1"),
    ("replace.contract", "semaprax.contract.v1|1"),
    ("replace.text-failure", "semaprax.text.v1|1"),
    ("replace.legacy", "ok|2"),
];
const WASM_CASES: &[&str] = &[
    "replace.literal",
    "replace.named",
    "replace.branch",
    "replace.match",
    "replace.reverse",
    "replace.other",
    "replace.owned",
    "replace.mixed",
    "replace.nested",
    "replace.zero",
    "replace.failure",
    "replace.condition-failure",
    "replace.contract",
    "replace.legacy",
];
fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_replacement_source_graph_and_roundtrip() {
    let program = parse(SOURCE, Path::new("string-replacement.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("string-replacement.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let document: Value = serde_json::from_str(&graph::to_json(&reparsed).unwrap()).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v68");
    assert_eq!(
        document["string_replacement"]["schema"],
        "semaprax.string-replacement.v1"
    );
    let resolved = hir::resolve(&reparsed).unwrap();
    hir::validate(&resolved).unwrap();
    let replaced = resolved
        .functions
        .iter()
        .filter(|function| {
            function.id.as_str().starts_with("replace.") && function.id.as_str() != "replace.legacy"
        })
        .collect::<Vec<_>>();
    for function in replaced {
        assert_eq!(
            function.cleanup_plan.schema,
            semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16,
            "{}",
            function.id
        );
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|b| &b.transitions)
            .any(|t| matches!(
                t,
                semaprax::cleanup_plan::CleanupTransition::ReserveRenewal { .. }
            )));
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|b| &b.transitions)
            .any(|t| matches!(t, semaprax::cleanup_plan::CleanupTransition::Renew { .. })));
    }
    assert_eq!(
        resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "replace.legacy")
            .unwrap()
            .cleanup_plan
            .schema,
        semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2
    );
}

#[test]
fn string_replacement_reference_interpreter() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("string-replacement.spx")).unwrap());
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
fn string_replacement_native_settlement() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("string-replacement-native.spx")).unwrap();
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

#[test]
fn string_replacement_wasm_settlement() {
    if !command_available("node") || WASM_CASES.is_empty() {
        return;
    }
    let program = parse(SOURCE, Path::new("string-replacement-wasm.spx")).unwrap();
    let selected = WASM_CASES
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact =
        emit_string_replacement_module(&program, &selected, InternalStringOptions::default())
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
fn string_replacement_preserves_closed_boundaries() {
    for (body, code) in [
        ("let text=\"x\"; text=\"y\"; 0", "SPX-U101"),
        ("let mut text=\"x\"; text=1; 0", "SPX-U102"),
        ("let mut bytes=bytes_zeroed(1usize); bytes=bytes_zeroed(2usize); 0", "SPX-U105"),
        ("let mut text=\"x\"; let next=\"y\"; text=next; string_len(next)", "SPX-O101"),
        ("let mut text=\"x\"; let next=\"y\"; text=match 0 { 0 => next, _ => \"z\", }; string_len(next)", "SPX-O101"),
        ("let mut text=\"x\"; let view=string_as_str(text); text=\"y\"; string_len(string_from_str(view))", "SPX-T265"),
    ] {
        let source=format!("module refused; @id(\"r.main\") fn main()->i64 {{ {body} }}");
        let program=parse(&source,Path::new("replacement-refused.spx")).unwrap();
        let errors=verify::verify(&program);
        let diagnostic=errors.iter().find(|e|e.code==code).unwrap_or_else(||panic!("{code}: {errors:?}"));
        assert!(diagnostic.span.is_some());
        assert_eq!(diagnostic.path.as_deref(),Some("replacement-refused.spx"));
        assert!(hir::resolve(&program).is_err());
    }
    let program = parse(SOURCE, Path::new("replacement-profile.spx")).unwrap();
    for emit in [emit_module, emit_copy_variant_module] {
        let error = emit(
            &program,
            &["replace.literal".into()],
            InternalStringOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W111");
        assert_eq!(
            error.message,
            "whole String replacement requires the explicit string-replacement-v1 profile"
        );
    }
    for id in ["replace.slice", "replace.text-failure"] {
        assert_eq!(
            emit_string_replacement_module(
                &program,
                &[id.into()],
                InternalStringOptions::default()
            )
            .unwrap_err()
            .code,
            "SPX-W116"
        );
    }
}

#[test]
fn string_replacement_hostile_hir_and_plan_fail_closed() {
    let program = parse(SOURCE, Path::new("replacement-hostile.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    for mutation in 0..6 {
        let mut hostile = resolved.clone();
        let function = hostile
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "replace.literal")
            .unwrap();
        match mutation {
            0 => function.cleanup_plan.schema = semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15,
            1 => {
                for block in &mut function.cleanup_plan.blocks {
                    block.transitions.retain(|t| {
                        !matches!(
                            t,
                            semaprax::cleanup_plan::CleanupTransition::ReserveRenewal { .. }
                        )
                    });
                }
            }
            2 => {
                for transition in function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.transitions)
                {
                    if let semaprax::cleanup_plan::CleanupTransition::Renew {
                        at,
                        source,
                        destination,
                    } = transition
                    {
                        *transition = semaprax::cleanup_plan::CleanupTransition::Transfer {
                            at: at.clone(),
                            source: source.clone(),
                            destination: destination.clone(),
                        };
                    }
                }
            }
            3 => {
                if let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind {
                    if let hir::ResolvedStatement::Let { mutable, .. } = &mut statements[0] {
                        *mutable = false;
                    }
                }
            }
            4 => {
                if let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind {
                    if let hir::ResolvedStatement::Assign { value, .. } = &mut statements[1] {
                        value.ownership = hir::OwnershipMode::Borrow;
                    }
                }
            }
            5 => {
                for transition in function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.transitions)
                {
                    if let semaprax::cleanup_plan::CleanupTransition::Renew {
                        source,
                        destination,
                        ..
                    } = transition
                    {
                        *source = destination.clone();
                    }
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            hir::validate(&hostile).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
        assert!(semaprax::codegen::emit_hir_c(&hostile).is_err());
    }
}
