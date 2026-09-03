//! Focused evidence for the explicit String-settling Wasm profile.
//! Raw imports independently observe compiler drops; generated-runtime tests
//! separately exercise policy and the safe host boundary. Neither substitutes
//! for the other or claims ordinary Wasm settlement.

use std::process::Command;

use semaprax::interpreter::{internal_strings as interpreter, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringModule, InternalStringOptions};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

#[path = "../interpreter_internal_strings_v1/support.rs"]
mod support;
use support::Fixture;

#[path = "../wasm_internal_strings_v1/literal_bounds.rs"]
mod literal_bounds;

const BASE: &str = include_str!("../interpreter_internal_strings_v1/source.spx");
const EXTRA: &str = include_str!("../wasm_internal_strings_v1/source.spx");
const CASES: &[(&str, &str)] = &[
    ("case.content", "ok|42"),
    ("case.requires", "semaprax.contract.v1|1"),
    ("case.ensures", "semaprax.contract.v1|2"),
    ("case.late", "semaprax.arithmetic.v1|4"),
    ("case.first", "semaprax.contract.v1|1"),
    ("case.nested", "semaprax.contract.v1|2"),
    ("case.branch", "ok|42"),
    ("case.loop", "ok|42"),
    ("case.recover", "ok|42"),
    ("case.guarded", "ok|42"),
    ("case.lazy", "ok|42"),
    ("case.assign-failure", "semaprax.arithmetic.v1|4"),
    ("case.condition-failure", "semaprax.arithmetic.v1|4"),
];

fn source() -> String {
    format!("{BASE}\n{EXTRA}")
}

fn write_artifact(fixture: &mut Fixture, name: &str, artifact: &InternalStringModule) {
    fixture.write(&format!("{name}.wasm"), artifact.wasm_bytes());
    fixture.write(&format!("{name}.json"), artifact.descriptor());
    fixture.write(&format!("{name}.mjs"), artifact.runtime_source());
}

fn node(fixture: &mut Fixture, script: &str, contents: &str) -> String {
    let script = fixture.write(script, contents);
    let output = Command::new(std::env::var_os("NODE").unwrap_or_else(|| "node".into()))
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .expect("Node is required for the selected standalone Wasm String evidence");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn compiler_settlement_matches_native_and_interpreter_and_reuses_every_failure_site() {
    let source = source();
    let ast = semaprax::check(&source, "wasm-internal-strings.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    assert_eq!(
        semaprax::graph::to_json(&ast).unwrap(),
        semaprax::graph::to_json(&reparsed).unwrap()
    );
    let mut fixture = Fixture::new(&canonical);
    let mut selected = CASES
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect::<Vec<_>>();
    selected.extend(["case.bool".to_owned(), "case.scalar".to_owned()]);
    let artifact = emit_module(&ast, &selected, InternalStringOptions::default()).unwrap();
    write_artifact(&mut fixture, "program", &artifact);
    selected.reverse();
    let reordered = emit_module(&ast, &selected, InternalStringOptions::default()).unwrap();
    assert_eq!(artifact.wasm_bytes(), reordered.wasm_bytes());
    assert_eq!(artifact.descriptor(), reordered.descriptor());
    assert_eq!(artifact.runtime_source(), reordered.runtime_source());
    let descriptor: Value = serde_json::from_str(artifact.descriptor()).unwrap();
    assert_eq!(descriptor["schema"], "semaprax.wasm-internal-strings.v1");
    assert_eq!(
        descriptor["wasm_sha256"],
        Sha256::digest(artifact.wasm_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(descriptor["wasm_bytes"], artifact.wasm_bytes().len());
    assert_eq!(descriptor["memory_pages"], 4);
    assert_eq!(descriptor["result_offset"], 65536);
    assert!(descriptor["stack_bytes"].as_u64().unwrap() <= 65536);

    let mut expected = String::new();
    for (id, observation) in CASES {
        let result =
            interpreter::interpret(&fixture.source, id, &[], &InterpreterOptions::default())
                .unwrap();
        interpreter::verify_envelope_against_source(&result.envelope, &fixture.source).unwrap();
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            assert_eq!(outcome["kind"], "failed");
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, observation);
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    fixture.write("cases.json", serde_json::to_vec(CASES).unwrap());

    let generated = semaprax::codegen::emit_c(&ast).unwrap();
    let mut probe = format!("{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n", include_str!("../support/native_fixture_stdio.c"), include_str!("../native_owned_utf8_settlement_v1/allocations.c"));
    for (id, _) in CASES {
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
    probe.push_str("return 0; }\n");
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    assert_eq!(
        node(
            &mut fixture,
            "raw-probe.mjs",
            include_str!("../wasm_internal_strings_v1/raw.mjs")
        ),
        expected
    );
    assert_eq!(
        node(
            &mut fixture,
            "host-probe.mjs",
            include_str!("../wasm_internal_strings_v1/host.mjs")
        ),
        expected
    );
    fixture.cleanup();
}

#[test]
fn quota_refusals_are_profile_outcomes_and_allow_settled_reuse() {
    let source = source();
    let ast = semaprax::check(&source, "quota.spx").unwrap();
    let mut fixture = Fixture::new(&source);
    let mut cases = Vec::new();
    for (name, id, options, cause) in [
        (
            "value",
            "quota.literal",
            InternalStringOptions {
                max_string_bytes: 3,
                ..Default::default()
            },
            "value_bytes",
        ),
        (
            "live",
            "quota.literal",
            InternalStringOptions {
                max_live_bytes: 3,
                ..Default::default()
            },
            "live_bytes",
        ),
        (
            "cumulative",
            "quota.literal",
            InternalStringOptions {
                max_cumulative_bytes: 3,
                ..Default::default()
            },
            "cumulative_bytes",
        ),
        (
            "owners",
            "quota.clone",
            InternalStringOptions {
                max_live_owners: Some(1),
                ..Default::default()
            },
            "owners",
        ),
        (
            "concat",
            "quota.concat",
            InternalStringOptions {
                max_live_bytes: 7,
                ..Default::default()
            },
            "live_bytes",
        ),
        (
            "char",
            "quota.char",
            InternalStringOptions {
                max_string_bytes: 2,
                ..Default::default()
            },
            "value_bytes",
        ),
    ] {
        let artifact =
            emit_module(&ast, &[id.to_owned(), "case.scalar".to_owned()], options).unwrap();
        write_artifact(&mut fixture, name, &artifact);
        cases.push(json!({"name": name, "id": id, "cause": cause}));
    }
    fixture.write("quotas.json", serde_json::to_vec(&cases).unwrap());
    assert_eq!(
        node(
            &mut fixture,
            "quota-probe.mjs",
            include_str!("../wasm_internal_strings_v1/quotas.mjs")
        ),
        "quotas settled\n"
    );
    fixture.cleanup();
}

#[test]
fn admission_is_additive_and_source_rejections_remain_authoritative() {
    let ast = semaprax::check(BASE, "base.spx").unwrap();
    let ordinary_before = semaprax::wasm::emit_module(&ast).unwrap();
    let empty = emit_module(&ast, &[], InternalStringOptions::default()).unwrap_err();
    assert_eq!(empty.code, "SPX-W111");
    for ids in [
        vec!["missing".to_owned()],
        vec!["case.scalar".to_owned(); 2],
        vec!["helper.identity".to_owned()],
    ] {
        assert_eq!(
            emit_module(&ast, &ids, InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    let _new = emit_module(
        &ast,
        &["case.content".to_owned()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(ordinary_before, semaprax::wasm::emit_module(&ast).unwrap());
    for source in [
        "module negative; @id(\"main\") fn main() -> i64 { let mut text = \"x\"; text = \"y\"; 0 }",
        "module negative; @id(\"main\") fn main() -> i64 { while false { let text = \"x\"; 0 } 0 }",
    ] {
        let errors = semaprax::check(source, "negative.spx").unwrap_err();
        let expected = if source.contains("let mut text") {
            "SPX-U105"
        } else {
            "SPX-T252"
        };
        assert!(errors.iter().any(|error| error.code == expected));
    }
    let recursive = semaprax::check(
        "module cycle; @id(\"cycle\") fn main() -> i64 { main() }",
        "cycle.spx",
    )
    .unwrap();
    assert_eq!(
        emit_module(
            &recursive,
            &["cycle".to_owned()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    for options in [
        InternalStringOptions {
            max_string_bytes: 65537,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_bytes: 16777217,
            ..Default::default()
        },
        InternalStringOptions {
            max_cumulative_bytes: 67108865,
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(0),
            ..Default::default()
        },
        InternalStringOptions {
            max_live_owners: Some(65537),
            ..Default::default()
        },
    ] {
        assert_eq!(
            emit_module(&ast, &["case.scalar".to_owned()], options)
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
}

#[test]
fn selected_function_and_export_count_limits_are_exact() {
    fn chain(count: usize) -> String {
        let mut source = "module bounded.chain;\n".to_owned();
        for index in 0..count {
            let name = if index == 0 {
                "main".to_owned()
            } else {
                format!("f_{index}")
            };
            let body = if index + 1 == count {
                "42".to_owned()
            } else {
                format!("f_{}()", index + 1)
            };
            source.push_str(&format!(
                "@id(\"f.{index}\") fn {name}() -> i64 {{ {body} }}\n"
            ));
        }
        source
    }
    let exact = semaprax::check(&chain(256), "exact-functions.spx").unwrap();
    assert!(emit_module(
        &exact,
        &["f.0".to_owned()],
        InternalStringOptions::default()
    )
    .is_ok());
    let excess = semaprax::check(&chain(257), "excess-functions.spx").unwrap();
    assert_eq!(
        emit_module(
            &excess,
            &["f.0".to_owned()],
            InternalStringOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-W111"
    );
    let ids = (0..33)
        .map(|index| format!("f.{index}"))
        .collect::<Vec<_>>();
    assert!(emit_module(&exact, &ids[..32], InternalStringOptions::default()).is_ok());
    assert_eq!(
        emit_module(&exact, &ids, InternalStringOptions::default())
            .unwrap_err()
            .code,
        "SPX-W111"
    );
}
