use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, ProjectNpmBuild, PublicApiSubject,
    PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};

const FACT: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const SOURCE: &str = "module utf8.npm;\n@id(\"bytes.raw\") fn raw(value: borrow Slice<u8>) -> Bytes { bytes_copy(value) }\n@id(\"utf8.greeting\") fn greeting() -> string { \"hello\\0世界\" }\n@id(\"utf8.length\") fn length(value: borrow str) -> i64 { str_len_bytes(value) }\n@id(\"app.main\") fn main() -> i64 { 0 }\n";

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    }
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

#[test]
fn v10_npm_maps_only_string_results_to_fatal_decoded_js_strings() {
    let checked = semaprax::check(SOURCE, "owned-utf8-npm.spx").unwrap();
    let program = semaprax::hir::resolve(&checked).unwrap();
    let selected = vec![
        "bytes.raw".to_owned(),
        "utf8.greeting".to_owned(),
        "utf8.length".to_owned(),
    ];
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "owned-utf8",
        "1.0.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    build.verify().unwrap();

    let package = artifacts(&build);
    let runtime = String::from_utf8(
        package
            .iter()
            .find(|row| row.0 == "semaprax.js")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(runtime.contains("TextDecoder(\"utf-8\",{fatal:true,ignoreBOM:true})"));
    assert!(runtime.contains("case \"owned-utf8\""));
    let declarations = String::from_utf8(
        package
            .iter()
            .find(|row| row.0 == "semaprax.bindings.d.ts")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(declarations.contains("readonly \"utf8.greeting\": () => string;"));
    assert!(declarations.contains("readonly \"bytes.raw\": (arg0: Uint8Array) => Uint8Array;"));

    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "semaprax-owned-utf8-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for (path, bytes) in package {
        fs::write(directory.join(path), bytes).unwrap();
    }
    fs::write(
        directory.join("contract.mjs"),
        "import fs from 'node:fs';import instantiate from './semaprax.bindings.js';const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));const api=await instantiate(wasm);const text=api.functions['utf8.greeting']();if(typeof text!=='string'||text!=='hello\\0世界')throw Error('string mapping');const input=new Uint8Array([0xff,0xc3,0x28]);const raw=api.functions['bytes.raw'](input);if(!(raw instanceof Uint8Array)||raw.length!==3||raw[0]!==0xff)throw Error('Bytes decoded');let rejected=false;try{api.functions['utf8.length']('x\\ud800y')}catch(error){rejected=error instanceof TypeError}if(!rejected)throw Error('unpaired UTF-16 admitted');console.log('owned-utf8-ok');\n",
    )
    .unwrap();
    let output = Command::new("node")
        .arg("contract.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
