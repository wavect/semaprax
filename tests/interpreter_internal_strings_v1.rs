//! Additive internal-String interpreter evidence. Legacy profiles remain closed.
//! Native observations concern its C allocations; the interpreter observations
//! concern values/status/fuel, and the Wasm host deliberately claims no drops.

use std::fs;
use std::process::Command;

use semaprax::interpreter::{self, internal_strings, Interpretation, InterpreterOptions};
use sha2::{Digest as _, Sha256};

#[path = "interpreter_internal_strings_v1/support.rs"]
mod support;
use support::Fixture;

#[path = "interpreter_internal_strings_v1/protocol.rs"]
mod protocol;

const SOURCE: &str = include_str!("interpreter_internal_strings_v1/source.spx");
const SCHEMA: &str = "semaprax.interpret.internal-strings.v1";
const DOMAIN: &[u8] = b"semaprax.interpret.internal-strings.payload.v1\0";
const LEGACY_DOMAIN: &[u8] = b"semaprax.interpret.payload.v1\0";
const CAP: usize = 16 * 1024 * 1024;
const CASES: &[(&str, &str, &str)] = &[
    ("case.content", "content", "ok|42"),
    (
        "case.requires",
        "requires_failure",
        "semaprax.contract.v1|1",
    ),
    ("case.ensures", "ensures_failure", "semaprax.contract.v1|2"),
    ("case.late", "late_failure", "semaprax.arithmetic.v1|4"),
    ("case.first", "first_failure", "semaprax.contract.v1|1"),
    ("case.nested", "nested_failure", "semaprax.contract.v1|2"),
    ("case.branch", "branch", "ok|42"),
    ("case.loop", "looped", "ok|42"),
    ("case.recover", "recover", "ok|42"),
];

fn run(
    fixture: &Fixture,
    id: &str,
    arguments: &[String],
    options: &InterpreterOptions,
) -> Interpretation {
    internal_strings::interpret(&fixture.source, id, arguments, options).unwrap()
}

fn payload(envelope: &str) -> &str {
    envelope
        .split_once(",\"payload\":")
        .unwrap()
        .1
        .strip_suffix('}')
        .unwrap()
}

fn remint(payload: &str, domain: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload.as_bytes());
    let hex = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{{\"schema\":\"{SCHEMA}\",\"digest\":\"sha256:{hex}\",\"bytes\":{},\"payload\":{payload}}}", payload.len())
}

fn outcome(envelope: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(envelope).unwrap()["payload"]["outcome"].clone()
}

#[test]
fn internal_calls_match_native_o0_o2_and_raw_wasm_normalized_outcomes() {
    let ast = semaprax::check(SOURCE, "internal-strings.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&reparsed), canonical);
    assert_eq!(
        semaprax::graph::to_json(&ast).unwrap(),
        semaprax::graph::to_json(&reparsed).unwrap()
    );
    let mut fixture = Fixture::new(&canonical);
    let options = InterpreterOptions::default();
    let mut expected = String::new();
    for (id, _, observation) in CASES {
        let result = run(&fixture, id, &[], &options);
        assert_eq!(result, run(&fixture, id, &[], &options));
        internal_strings::verify_envelope_against_source(&result.envelope, &fixture.source)
            .unwrap();
        assert_eq!(remint(payload(&result.envelope), DOMAIN), result.envelope);
        let value = outcome(&result.envelope);
        let observed = if value["kind"] == "returned" {
            format!("ok|{}", value["value"].as_str().unwrap())
        } else {
            assert_eq!(value["kind"], "failed");
            assert_eq!(value["status"]["schema"], "semaprax.status.v1");
            assert_eq!(value["status"]["retryable"], false);
            format!(
                "{}|{}",
                value["status"]["domain_id"].as_str().unwrap(),
                value["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, observation);
        expected.push_str(&format!("{id}|{observed}\n"));
    }

    let generated = semaprax::codegen::emit_c(&ast).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&ast).unwrap());
    let mut probe = format!("{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nstruct spx_status_entry entries[8]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,8,NULL,NULL,NULL));\n", include_str!("native_owned_utf8_settlement_v1/allocations.c"));
    for (id, _, _) in CASES {
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
        REQUIRE(status!=NULL && status->retryability==SPX_RETRYABILITY_FALSE);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
    }
    probe.push_str("REQUIRE(context.status_arena.length==5); return 0; }\n");
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }

    let script = fixture.write(
        "probe.mjs",
        include_str!("interpreter_internal_strings_v1/probe.mjs"),
    );
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    let mut command = Command::new(node);
    command.current_dir(&fixture.root).arg(script);
    for (index, (id, name, _)) in CASES.iter().enumerate() {
        let source = SOURCE.replace(
            "fn main() -> i64 { content() }",
            &format!("fn main() -> i64 {{ {name}() }}"),
        );
        let ast = semaprax::check(&source, "case-main.spx").unwrap();
        let module = semaprax::wasm::emit_module(&ast).unwrap();
        assert_eq!(module, semaprax::wasm::emit_module(&ast).unwrap());
        let path = fixture.write(&format!("case-{index}.wasm"), module);
        command.arg(format!("{id}|{}", path.display()));
    }
    let output = command
        .output()
        .expect("Node is required for the selected internal String parity gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Exact contract codes are compared above for native and interpreter.
    // The legacy raw-Wasm import carries only the contract-failure domain.
    let wasm_expected = expected
        .replace(
            "|semaprax.contract.v1|1\n",
            "|semaprax.contract.v1|unspecified\n",
        )
        .replace(
            "|semaprax.contract.v1|2\n",
            "|semaprax.contract.v1|unspecified\n",
        );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), wasm_expected);
    assert!(output.stderr.is_empty());
    fixture.cleanup();
}

#[test]
fn old_profile_and_external_string_boundaries_remain_closed() {
    let fixture = Fixture::new(SOURCE);
    let options = InterpreterOptions::default();
    let rejected =
        interpreter::interpret(&fixture.source, "case.content", &[], &options).unwrap_err();
    assert!(rejected
        .iter()
        .any(|error| error.code == "SPX-F102" && error.message.contains("unsupported_callee")));
    for (id, reason) in [
        ("helper.identity", "unsupported_parameter_type"),
        ("helper.pick", "unsupported_result_type"),
    ] {
        let errors = internal_strings::interpret(&fixture.source, id, &[], &options).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.code == "SPX-F102" && error.message.contains(reason)));
    }
    let arguments = vec!["41".to_owned()];
    let old = interpreter::interpret(&fixture.source, "case.scalar", &arguments, &options).unwrap();
    let new = run(&fixture, "case.scalar", &arguments, &options);
    let old_value: serde_json::Value = serde_json::from_str(&old.envelope).unwrap();
    let new_value: serde_json::Value = serde_json::from_str(&new.envelope).unwrap();
    assert_eq!(old_value["payload"]["fuel"], new_value["payload"]["fuel"]);
    assert_eq!(
        old_value["payload"]["outcome"],
        new_value["payload"]["outcome"]
    );
    assert_eq!(
        old,
        interpreter::interpret(&fixture.source, "case.scalar", &arguments, &options).unwrap()
    );
    assert_eq!(
        internal_strings::verify_envelope(&old.envelope)
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    assert_eq!(
        interpreter::verify_envelope(&new.envelope)
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    fixture.cleanup();
}
