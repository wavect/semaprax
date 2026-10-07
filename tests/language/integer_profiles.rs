//! GAP-07/GAP-10: exact conversion and checked remainder parity.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::{format, graph, hir, interpreter, parse, verify, wasm};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"module test.integer_profiles;
@id("numeric.rem32") fn rem32(a: i32, b: i32) -> i32 { a % b }
@id("numeric.rem8") fn rem8(a: u8, b: u8) -> u8 { a % b }
@id("numeric.accept255") fn accept255(value: i64) -> i64
    requires value == i64_from_u8(255u8)
    ensures result == i64_from_u8(255u8)
{ value }
@id("numeric.argument") fn argument() -> i64 { accept255(i64_from_i32(255i32)) }
@id("numeric.all") fn all() -> i64 {
    let mut byte = 0u8;
    let mut counter = 0;
    let mut failures = 0;
    while counter <= 255 {
        failures = failures + if i64_from_u8(byte) == counter && usize_from_u8(byte) == usize_from_i64(counter) { 0 } else { 1 };
        byte = if byte < 255u8 { byte + 1u8 } else { byte };
        counter = counter + 1;
        0
    }
    let signed = i64_from_i32(-2147483648i32) == -2147483648 && i64_from_i32(2147483647i32) == 2147483647 && i64_from_i32(-1i32) == -1 && i64_from_i32(0i32) == 0 && i64_from_i32(1i32) == 1;
    let sizes = i64_from_usize(0usize) == 0 && i64_from_usize(9223372036854775807usize) == 9223372036854775807 && usize_from_i64(9223372036854775807) == 9223372036854775807usize;
    let remainders = rem32(-7i32, rem32(7i32, 4i32)) == -1i32 && rem8(255u8, 16u8) == 15u8 && rem8(0u8, 1u8) == 0u8 && rem32(2147483647i32, 1i32) == 0i32;
    failures + if signed && sizes && remainders { 0 } else { 1 }
}
@id("numeric.negative") fn negative() -> i64 { let invalid = usize_from_i64(-1); 1 }
@id("numeric.large") fn large() -> i64 { i64_from_usize(9223372036854775808usize) }
@id("numeric.rem32zero") fn rem32zero() -> i64 { let invalid = rem32(3i32, 0i32); 1 }
@id("numeric.rem32overflow") fn rem32overflow() -> i64 { let invalid = rem32(-2147483648i32, -1i32); 1 }
@id("numeric.rem8zero") fn rem8zero() -> i64 { let invalid = rem8(3u8, 0u8); 1 }
@id("app.main") fn main() -> i64 { all() }
"#;

const CASES: &[(&str, &str)] = &[
    ("numeric.all", "ok|0"),
    ("numeric.argument", "ok|255"),
    ("numeric.negative", "semaprax.convert.v1|1"),
    ("numeric.large", "semaprax.convert.v1|1"),
    ("numeric.rem32zero", "semaprax.arithmetic.v1|6"),
    ("numeric.rem32overflow", "semaprax.arithmetic.v1|7"),
    ("numeric.rem8zero", "semaprax.arithmetic.v1|6"),
];

const OWNED_FAILURE_SOURCE: &str = r#"module test.integer_owned_failure;
@id("numeric.owned_rem32") fn owned_rem32(a: i32, b: i32) -> i32 { a % b }
@id("numeric.live_bytes") fn live_bytes() -> i64 {
    let live = bytes_zeroed(4usize);
    let invalid = usize_from_i64(-1);
    if byte_len(bytes_as_slice(live)) == invalid { 1 } else { 0 }
}
@id("numeric.live_rem32overflow") fn live_rem32overflow() -> i64 {
    let live = bytes_zeroed(4usize);
    let left = -2147483648i32;
    let right = -1i32;
    let invalid = owned_rem32(left, right);
    if byte_len(bytes_as_slice(live)) == usize_from_i64(i64_from_i32(invalid)) { 1 } else { 0 }
}
@id("app.main") fn main() -> i64 { live_bytes() }
"#;

const OWNED_FAILURE_CASES: &[(&str, &str)] = &[
    ("numeric.live_bytes", "semaprax.convert.v1|1"),
    ("numeric.live_rem32overflow", "semaprax.arithmetic.v1|7"),
];

fn checked_program(source: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new("integer-profiles.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "{diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    hir::resolve(&program).unwrap();
    program
}

fn parity_cases(source: &str, cases: &[(&str, &str)], public_exports: bool) {
    let program = checked_program(source);
    let mut fixture = Fixture::new(source);
    let mut expected = String::new();
    for (id, observation) in cases {
        let result = interpreter::interpret(
            &fixture.source,
            id,
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let actual = if result.returned {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"]
            )
        };
        assert_eq!(actual, *observation, "{id}: {}", result.envelope);
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!("{}\n{generated}\nint main(void) {{ if(!fixture_binary_stdout()) return 90; struct spx_status_entry entries[32]; struct spx_context context={{0}}; if (!spx_context_init(&context,19,entries,32,NULL,NULL,NULL)) return 91;\n", include_str!("../support/native_fixture_stdio.c"));
    for (id, _) in cases {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(r#"{{ int64_t value=INT64_MAX; spx_status_token token=spx_decl_{symbol}(&context,&value);
if(token==0) printf("{id}|ok|%lld\n",(long long)value);
else {{ if(value!=INT64_MAX) return 92; const struct spx_normalized_status *status=spx_status_resolve(&context,token); if(!status)return 93; printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code); }} }}
"#));
    }
    probe.push_str("return 0; }\n");
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    if !public_exports {
        for (id, observation) in cases {
            let name = &program
                .functions
                .iter()
                .find(|function| function.stable_id == *id)
                .unwrap()
                .name;
            let prefix = source.rsplit_once("@id(\"app.main\")").unwrap().0;
            let selected = checked_program(&format!(
                "{prefix}@id(\"app.main\") fn main() -> i64 {{ {name}() }}"
            ));
            let web = fixture.root.join("web");
            wasm::build_web(&selected, &web).unwrap();
            std::fs::write(web.join("package.json"), "{\"type\":\"module\"}").unwrap();
            let runner = fixture.write("runner.mjs", &format!(r#"import {{ readFile }} from 'node:fs/promises';
import {{ instantiateBytes, semanticStatus }} from './web/semaprax.js';
const {{instance}}=await instantiateBytes(await readFile(new URL('./web/app.wasm',import.meta.url)),{{maxOwnedByteEntries:1}});
for(let repeat=0;repeat<2;repeat++) {{
  let result;
  try {{ result='ok|'+instance.exports.semaprax_main(); }}
  catch(error) {{ const status=semanticStatus(error); if(!status)throw error; result=status.domain_id+'|'+status.code; }}
  if(result!=={expected})throw Error('wrong result: '+result);
}}
"#, expected=serde_json::to_string(observation).unwrap()));
            let output = Command::new("node")
                .arg(runner)
                .output()
                .expect("Node required for integer profile");
            assert!(
                output.status.success(),
                "{id}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            std::fs::remove_dir_all(web).unwrap();
        }
    } else {
        let exports = cases
            .iter()
            .map(|(id, _)| (*id).to_owned())
            .collect::<Vec<_>>();
        let web = fixture.root.join("web");
        wasm::build_web_with_scalar_exports(&program, &web, &exports).unwrap();
        let ids = serde_json::to_string(&exports).unwrap();
        let runner = fixture.write("runner.mjs", &format!(r#"import {{ readFile }} from 'node:fs/promises';
import {{ instantiateBytes }} from './web/semaprax.bindings.js';
const runtime=await instantiateBytes(await readFile(new URL('./web/app.wasm',import.meta.url)));
for (const id of {ids}) {{
  for(let repeat=0;repeat<2;repeat++) {{
    const result=runtime.call(id);
    if(repeat===0) console.log(id+'|'+(result.ok?'ok|'+result.value:result.status.domain_id+'|'+result.status.code));
  }}
}}
"#));
        let output = Command::new("node")
            .arg(runner)
            .output()
            .expect("Node required for integer profile");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        std::fs::remove_dir_all(web).unwrap();
    }
    fixture.cleanup();
}

fn parity(source: &str, public_exports: bool) {
    parity_cases(source, CASES, public_exports);
}

#[test]
fn integer_profiles_agree_on_scalar_and_aggregate_backends() {
    parity(SOURCE, true);
    let aggregate = SOURCE.replace("module test.integer_profiles;", "module test.integer_profiles;\n@id(\"force.record\") record Force { @id(\"force.record.value\") value: i64, }");
    parity(&aggregate, false);
}

#[test]
fn checked_numeric_failures_finalize_an_unrelated_live_bytes_owner() {
    let resolved = hir::resolve(&checked_program(OWNED_FAILURE_SOURCE)).unwrap();
    for id in ["numeric.live_bytes", "numeric.live_rem32overflow"] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("numeric failure witness must remain a block")
        };
        let live = statements
            .iter()
            .find_map(|statement| match statement {
                hir::ResolvedStatement::Let { binding, .. } if binding.name == "live" => {
                    Some(&binding.id)
                }
                _ => None,
            })
            .expect("numeric failure witness keeps its live Bytes binding");
        assert!(function.cleanup_plan.exits.iter().any(|exit| {
            matches!(
                exit.continuation,
                semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            ) && exit.finalize_in_order.iter().any(|action| {
                matches!(
                    &action.source.storage,
                    semaprax::cleanup_plan::StorageId::Value(value) if value == live
                ) && action.source.projections.is_empty()
            })
        }));
    }
    parity_cases(OWNED_FAILURE_SOURCE, OWNED_FAILURE_CASES, false);
}

#[test]
fn integer_profile_types_and_forged_results_fail_closed() {
    let internal_usize = checked_program(
        r#"module test.internal_usize;
@id("numeric.publicsize") fn publicsize() -> usize { usize_from_u8(255u8) }
@id("app.main") fn main() -> i64 { i64_from_usize(publicsize()) }
"#,
    );
    let refused =
        wasm::emit_module_with_scalar_exports(&internal_usize, &["numeric.publicsize".to_owned()])
            .expect_err("integer profile preserves the public scalar ABI's usize refusal");
    assert_eq!(refused.code, "SPX-W115");
    for expression in [
        "i64_from_u8(1)",
        "i64_from_i32(1u8)",
        "usize_from_u8(1i32)",
        "1i32 % 1u8",
        "1.0 % 2.0",
    ] {
        let source = format!("module test.invalid; @id(\"app.main\") fn main() -> i64 {{ let value = {expression}; 0 }}");
        let program = parse(&source, "invalid.spx").unwrap();
        assert!(
            verify::verify(&program)
                .iter()
                .any(|d| d.severity.is_error()),
            "{expression}"
        );
    }
    let source = "module test.forged; @id(\"app.main\") fn main() -> i64 { i64_from_u8(255u8) }";
    let resolved = hir::resolve(&checked_program(source)).unwrap();
    let mut forged_result = resolved.clone();
    let function = forged_result
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("block expected")
    };
    tail.ty = hir::ResolvedType::Usize;
    assert!(hir::validate(&forged_result).is_err());

    let mut forged_operand = resolved;
    let function = forged_operand
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("block expected")
    };
    let hir::ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        panic!("conversion call expected")
    };
    args[0].ty = hir::ResolvedType::I64;
    assert!(hir::validate(&forged_operand).is_err());
}
