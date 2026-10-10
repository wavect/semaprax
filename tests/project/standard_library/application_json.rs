//! #724: generated ordinary application code, not a host/schema-only decoder.

use std::process::Command;

use semaprax::{codegen, format, parse, project, wasm};
use sha2::{Digest, Sha256};

#[path = "application_json/collection_response.rs"]
mod collection_response;
#[path = "application_json/nested_response.rs"]
mod nested_response;
#[path = "application_json/nested_request.rs"]
mod nested_request;
#[path = "application_json/owned.rs"]
mod owned;
#[path = "application_json/stream.rs"]
mod stream;
#[path = "application_json/stream_native.rs"]
mod stream_native;
#[path = "application_json/stream_nested_request.rs"]
mod stream_nested_request;
#[path = "application_json/utf8.rs"]
mod utf8;
#[path = "application_json/views.rs"]
mod views;

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "application-json"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/schema.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.data.json.digits = "=0.1.0"
std.data.json.query = "=0.1.0"
std.data.json.scan = "=0.1.0"
std.data.json.token = "=0.1.0"
std.data.json.write = "=0.1.0"
"#;

fn command_manifest(profile: &str) -> String {
    MANIFEST
        .replace("owned-data-api.v1", profile)
        .replace("web = []", "web = [\"consumer.command\"]")
        .replace(
            "[dependencies]",
            "[command]\nfunction = \"consumer.command\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n\n[dependencies]",
        )
}

const SCHEMA: &str = r#"module consumer.schema;
@id("application.patient") record Patient {
    @id("application.patient.n") n: i64,
    @id("application.patient.count") count: usize,
    @id("application.patient.byte") byte: u8,
    @id("application.patient.ok") ok: bool,
}
@id("consumer.schema.anchor") fn schema_anchor()->i64 {0}
"#;

fn canonical(source: &str) -> String {
    format::canonical(&parse(source, "application-json.spx").unwrap())
}

// Stream profile admission checks the bootstrap before generated helpers exist.
// Retain a genuine reader rather than relying on the selected permit alone.
fn stream_bootstrap_command() -> &'static str {
    r#"@id("consumer.command") fn command()->i64 uses{process.stdin.read}{
let mut reader=stdin_stream_open();
while !stdin_stream_eof(reader){reader=stdin_stream_next(reader);0}
0
}
"#
}

fn fixture(label: &str, schema: &str) -> std::path::PathBuf {
    let root = super::temporary(label);
    std::fs::create_dir_all(root.join("src")).unwrap();
    assert_eq!(
        project::ProjectManifest::parse(MANIFEST)
            .unwrap()
            .to_canonical_toml(),
        MANIFEST
    );
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("src/schema.spx"), canonical(schema)).unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        canonical("module consumer.app; @id(\"consumer.main\") fn main()->i64 { 0 }"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        canonical("module consumer.tests; @id(\"consumer.tests.main\") fn main()->i64 { 0 }"),
    )
    .unwrap();
    root
}

fn array(bytes: &[u8]) -> String {
    format!(
        "[{}]",
        bytes
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn imports() -> &'static str {
    r#"module consumer.app;
use type @id("application.patient") from consumer.schema as Patient;
use type @id("application.patient.json.decode-result") from consumer.schema as PatientJsonDecode;
use type @id("application.patient.json.encode-result") from consumer.schema as PatientJsonEncode;
use function @id("application.patient.json.decode") from consumer.schema as decode;
use function @id("application.patient.json.encode") from consumer.schema as encode;
use function @id("application.patient.json.encoded-len") from consumer.schema as encoded_len;
"#
}

#[test]
fn checked_application_json_derivation_replays_and_refuses_mutated_or_missing_contracts() {
    let root = fixture("json-derive", SCHEMA);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let source =
            project::derive_json_codec_source(&revision, "src/schema.spx", "application.patient")?;
        assert_eq!(
            source,
            project::derive_json_codec_source(&revision, "src/schema.spx", "application.patient")?
        );
        project::verify_json_codec_source(
            &revision,
            "src/schema.spx",
            "application.patient",
            &source,
        )?;
        let forged = source.replace("number > 255", "number > 256");
        assert_ne!(forged, source);
        assert_eq!(
            project::verify_json_codec_source(
                &revision,
                "src/schema.spx",
                "application.patient",
                &forged
            )
            .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(
            project::derive_json_codec_source(&revision, "src/schema.spx", "forged.patient")
                .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(
            project::derive_json_codec_source(
                &revision,
                "../src/schema.spx",
                "application.patient"
            )
            .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(canonical(&source), source);
        assert_eq!(
            std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
            canonical(SCHEMA)
        );
        Ok(())
    })
    .unwrap();
    // A second, unrelated configuration shape derives the same general layer.
    let config = SCHEMA
        .replace("Patient", "Config")
        .replace("application.patient", "application.config")
        .replace("n: i64", "n: bool");
    std::fs::write(root.join("src/schema.spx"), canonical(&config)).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let source = project::derive_json_codec_source(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.config",
        )?;
        assert!(source.contains("fn json_Config_decode"));
        assert!(source.contains("fn json_Config_encode"));
        Ok(())
    })
    .unwrap();
    // The generator does not secretly add dependencies or broaden a profile.
    std::fs::write(
        root.join("semaprax.toml"),
        MANIFEST.replace("std.data.json.token = \"=0.1.0\"\n", ""),
    )
    .unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(project::derive_json_codec_source(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.config"
        )
        .is_err());
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_application_json_success_errors_and_exact_capacity_agree_on_three_backends() {
    let root = fixture("json-codec-runtime", SCHEMA);
    let derived = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.patient",
        )
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), derived).unwrap();
    let mut app = imports().to_owned();
    app.push_str("@id(\"consumer.check-error\") fn check_error(input: borrow Slice<u8>, limit: usize, expected: i64, at: usize, target: i64) -> i64 { match decode(input, limit) { PatientJsonDecode::Decoded { value } => 1, PatientJsonDecode::Error { code, offset, field } => if code == expected && offset == at && field == target { 0 } else { 1 }, } }\n");
    app.push_str("@id(\"consumer.main\") fn main()->i64 {\nlet mut failures = 0;\n");
    let success = br#" { "ok": true, "byte": 255, "count": 18446744073709551615, "\u006e": -9223372036854775808 } "#;
    app.push_str(&format!("let good = {};\nlet actual = decode(array_as_slice(good), {}usize);\nfailures = failures + match actual {{ PatientJsonDecode::Decoded {{ value }} => if value.n == -9223372036854775808 && value.count == 18446744073709551615usize && value.byte == 255u8 && value.ok {{ 0 }} else {{ 1 }}, PatientJsonDecode::Error {{ code, offset, field }} => 1, }};\n", array(success), success.len()));
    let errors: Vec<(&[u8], i64, usize, i64)> = vec![
        (b"{\"n\":", 1, 5, 0),
        (b"{}", 3, 2, 1),
        (br#"{"n":1,"\u006e":2}"#, 2, 7, 1),
        (br#"{"x":1}"#, 4, 1, 0),
        (br#"{"n":true}"#, 5, 5, 1),
        (br#"{"n":null}"#, 5, 5, 1),
        (br#"{"n":1.0}"#, 6, 5, 1),
        (br#"{"n":1e0}"#, 6, 5, 1),
        (br#"{"n":9223372036854775808}"#, 6, 5, 1),
        (br#"{"n":-9223372036854775809}"#, 6, 5, 1),
        (br#"{"n":0,"count":-1}"#, 6, 15, 2),
        (br#"{"n":0,"count":18446744073709551616}"#, 6, 15, 2),
        (br#"{"n":0,"count":0,"byte":256}"#, 6, 24, 3),
        (br#"{"n":"\u0000"}"#, 5, 5, 1),
        (br#"{"n":"\q"}"#, 1, 6, 0),
        (br#"{"n":"\uD800"}"#, 1, 6, 0),
        (b"{\"n\":\"\xc0\xaf\"}", 1, 6, 0),
        (b"{\"n\":\"\0\"}", 1, 6, 0),
        (b"{\"n\":\"\xc3\xa9\"}", 5, 5, 1),
        (b"{} x", 1, 3, 0),
        (b"   ", 1, 3, 0),
        (b"[]", 5, 0, 0),
    ];
    for (index, (bytes, code, offset, field)) in errors.iter().enumerate() {
        app.push_str(&format!("let bad_{index} = {}; failures = failures + check_error(array_as_slice(bad_{index}), 4096usize, {code}, {offset}usize, {field});\n", array(bytes)));
    }
    app.push_str(&format!(
        "failures = failures + check_error(array_as_slice(good), {}usize, 7, 0usize, 0);\n",
        success.len() - 1
    ));
    let expected =
        br#"{"n":-9223372036854775808,"count":18446744073709551615,"byte":255,"ok":true}"#;
    app.push_str(&format!("let value = Patient {{ n: -9223372036854775808, count: 18446744073709551615usize, byte: 255u8, ok: true }};\nlet required = encoded_len(value);\nfailures = failures + if required == {}usize {{ 0 }} else {{ 1 }};\nlet refused = encode(value, required - 1usize);\nfailures = failures + match own refused {{ PatientJsonEncode::Refused {{ required: count }} => if count == required {{ 0 }} else {{ 1 }}, PatientJsonEncode::Encoded {{ text: short_text }} => 1, }};\nlet rendered = encode(value, required);\nfailures = failures + match own rendered {{ PatientJsonEncode::Refused {{ required: refused_count }} => 1, PatientJsonEncode::Encoded {{ text }} => {{ let view = string_as_str(text); let expected = {}; let expected_view = array_as_slice(expected); let mut index = 0usize; let mut mismatch = usize_from_i64(str_len_bytes(view)) != byte_len(expected_view); while index < byte_len(expected_view) {{ let a = str_byte_at(view, index); let b = byte_get(expected_view, index); let equal = match a {{ Option::Some {{ value: actual_byte }} => match b {{ Option::Some {{ value: wanted }} => actual_byte == wanted, Option::None {{}} => false, }}, Option::None {{}} => false, }}; mismatch = mismatch || !equal; index = index + 1usize; index < byte_len(expected_view) }} if mismatch {{ 1 }} else {{ 0 }} }}, }};\nif failures == 0 {{ 439 }} else {{ 0 - failures }}\n}}", expected.len(), array(expected)));
    std::fs::write(root.join("src/app.spx"), canonical(&app)).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let graph = snapshot.retain_revision();
        assert!(graph.semantic_graph().contains("application.patient.json.decode"));
        // Workspace edges index authored/imported declarations, not prelude
        // intrinsics. Assert the actual checked encoder calls instead.
        assert!(graph.semantic_graph().contains("application.patient.json.encode"));
        use semaprax::hir::{OwnershipMode, ResolvedExprKind, ResolvedType};
        let encoder = snapshot.entry_program().functions.iter()
            .find(|function| function.id.as_str()=="application.patient.json.encode").unwrap();
        let ResolvedExprKind::Block {tail,..} = &encoder.body.kind else {panic!("encoder block")};
        let ResolvedExprKind::If {else_branch,..} = &tail.kind else {panic!("output limit preflight")};
        let ResolvedExprKind::Block {statements,tail} = &else_branch.kind else {panic!("render block")};
        let concat = |expression: &semaprax::hir::ResolvedExpr| {
            matches!(&expression.kind, ResolvedExprKind::Call {callee,instance:None,type_arguments,args}
                if callee.as_str()=="core.string.concat" && type_arguments.is_empty() && args.len()==2)
                && expression.ty==ResolvedType::String && expression.ownership==OwnershipMode::Own
        };
        // Four fields require exactly a key append and value append each.
        assert_eq!(statements.iter().filter(|statement| concat(statement.value())).count(),8);
        let ResolvedExprKind::ConstructVariant {case,fields,..} = &tail.kind else {panic!("encoded result")};
        assert_eq!(case.as_str(),"application.patient.json.encoded");
        assert_eq!(fields.len(),1);
        assert!(concat(&fields[0].value));
        let outcome = snapshot.execute_entry(&project::ProjectExecutionOptions::default())?;
        assert_eq!(outcome.outcome(), &project::ProjectExecutionOutcome::Returned(439));
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] { super::compile_and_run_c(&c, &root, optimization, "439"); }
        let bytes = wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let digest = format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes)));
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        let runtime = include_str!("../../../src/wasm/browser_runtime.js")
            .replace("__SEMAPRAX_OWNED_EXPORTS__", "{}")
            .replace("__SEMAPRAX_WASM_SHA256__", &digest);
        std::fs::write(root.join("runtime.mjs"), runtime).unwrap();
        std::fs::write(root.join("run.mjs"), "import {readFile} from 'node:fs/promises'; import {instantiateBytes} from './runtime.mjs'; const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:8}); for(let i=0;i<32;i++){const value=instance.exports.semaprax_main();if(value!==439n)throw Error('actual JSON codec result:'+value);} ").unwrap();
        let output = Command::new("node").arg("run.mjs").current_dir(&root).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        Ok(())
    }).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
