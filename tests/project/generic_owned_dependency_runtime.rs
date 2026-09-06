use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use semaprax::hir::{self, DeclarationId, ResolvedType};
use semaprax::project::{with_authenticated_project, ProjectExecutionOutcome};
use semaprax::{codegen, package_lock_v3, package_report_v2, wasm};
use serde_json::Value;

use super::owned_bounded_vec_dependencies::compile_and_run_c;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const DEPENDENCY: &str = r#"
module acme.generic;

@id("acme.generic.pair")
record Pair<T, U> {
    @id("acme.generic.pair.left")
    left: T,
    @id("acme.generic.pair.right")
    right: U,
}

@id("acme.generic.evaluate")
fn evaluate() -> i64 {
    let input = [1u8, 2u8, 3u8];
    let value = Pair<Bytes, bool> {
        left: bytes_copy(array_as_slice(input)),
        right: true,
    };
    match own value {
        Pair { left: payload, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("acme.generic.main")
fn main() -> i64 { evaluate() }
"#;

const APP: &str = r#"
module consumer.app;
use function @id("consumer.evaluate") from consumer.api as evaluate;

@id("consumer.main")
fn main() -> i64 { evaluate() }
"#;

const API: &str = r#"
module consumer.api;
use function @id("acme.generic.evaluate") from acme.generic as dependency_evaluate;

@id("consumer.evaluate")
fn evaluate() -> i64 { dependency_evaluate() }
"#;

const TESTS: &str = r#"
module consumer.tests;
use function @id("acme.generic.evaluate") from acme.generic as evaluate;

@id("consumer.tests.main")
fn main() -> i64 {
    if evaluate() == 1 { 0 } else { 1 }
}
"#;

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "consumer"
version = "0.1.0"

[modules]
entry = "consumer.app"
sources = ["src/api.spx", "src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = ["consumer.evaluate"]

[dependencies]
acme.generic = "^1.0.0"

[dependency-sources]
acme.generic = "vendor/acme-generic.subject.json"
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-dependency-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("vendor")).unwrap();
        let root = root.canonicalize().unwrap();
        let dependency = root.join("dependency.spx");
        std::fs::write(&dependency, canonical(DEPENDENCY, &dependency)).unwrap();
        let report = package_report_v2::generate(
            &dependency,
            &package_report_v2::PackageReportV2Options::default(),
        )
        .unwrap();
        let subject = package_lock_v3::create_subject(
            &package_lock_v3::Coordinate {
                package: "acme.generic".to_owned(),
                version: "1.0.0".to_owned(),
            },
            &report,
            &[],
            &[],
        )
        .unwrap();
        std::fs::write(root.join("vendor/acme-generic.subject.json"), subject).unwrap();
        for (relative, source) in [
            ("src/api.spx", API),
            ("src/app.spx", APP),
            ("src/tests.spx", TESTS),
        ] {
            let path = root.join(relative);
            std::fs::write(&path, canonical(source, &path)).unwrap();
        }
        std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
        Self(root)
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn canonical(source: &str, path: &Path) -> String {
    let parsed = semaprax::parse(source, path).unwrap();
    let formatted = semaprax::format::canonical(&parsed);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&formatted, path).unwrap()),
        formatted
    );
    formatted
}

fn pair_type() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("acme.generic.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
    }
}

fn assert_linked_identity(program: &hir::ResolvedProgram) {
    hir::validate(program).unwrap();
    let pair = pair_type();
    let evaluate = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "acme.generic.evaluate")
        .expect("retained dependency implementation");
    assert_eq!(evaluate.return_type, ResolvedType::I64);
    assert!(evaluate.params.is_empty());
    assert_eq!(evaluate.cleanup_plan.schema, "semaprax.cleanup-plan.v5");
    assert!(evaluate.cleanup.slots.iter().any(|slot| slot.ty == pair));
    let leaf = vec![DeclarationId::new("acme.generic.pair.left")];
    assert!(evaluate
        .cleanup
        .flags
        .iter()
        .any(|flag| flag.place.projections == leaf));
    let consumer_functions = program
        .functions
        .iter()
        .filter(|function| function.id.as_str().starts_with("consumer."))
        .collect::<Vec<_>>();
    assert!(!consumer_functions.is_empty());
    for function in consumer_functions {
        assert_ne!(function.return_type, pair);
        assert!(function.params.iter().all(|parameter| parameter.ty != pair));
    }
}

fn artifacts(envelope: &str) -> BTreeMap<String, Vec<u8>> {
    let value: Value = serde_json::from_str(envelope).unwrap();
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["content_hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

fn run_internal_wasm(bytes: &[u8], root: &Path) {
    let wasm = root.join("internal.wasm");
    std::fs::write(&wasm, bytes).unwrap();
    let script = r#"const fs=require('fs');const bytes=fs.readFileSync(process.argv[1]);let instance=null,next=1;const entries=new Map();const decode=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn),token=root&0x7fffffff;if((root&0x80000000)===0||token===0||length>65536)throw Error('carrier');return{length,root,token}};const resolve=v=>{const b=entries.get(v.token);if(!(b instanceof Uint8Array)||b.length!==v.length)throw Error('stale');return b};const read=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn);if((root&0x80000000)!==0)return resolve(decode(c));const memory=instance?.exports.__spx_byte_memory;if(!memory||root>memory.buffer.byteLength-length)throw Error('range');return new Uint8Array(memory.buffer,root,length)};const allocate=b=>{const token=next++,copy=new Uint8Array(b);entries.set(token,copy);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(copy.length))};const fail=()=>{throw Error('semantic')};const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>b===0n?fail():a/b,spx_rem:(a,b)=>b===0n?fail():a%b,spx_neg:a=>-a,spx_contract_fail:fail,spx_bytes_copy:c=>allocate(read(c)),spx_bytes_get:(c,i)=>{const b=read(c);return i<0n||i>=BigInt(b.length)?-1:b[Number(i)]},spx_bytes_drop:c=>{const v=decode(c);resolve(v);entries.delete(v.token)},spx_bytes_as_slice:c=>{read(c);return c}};WebAssembly.instantiate(bytes,{env}).then(linked=>{instance=linked.instance;for(let i=0;i<4;i++){const value=instance.exports.semaprax_main();if(value!==1n||entries.size!==0)throw Error('result-or-leak:'+value+':'+entries.size)}}).catch(error=>{console.error(error);process.exit(2)});"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dependency_internal_generic_owned_record_executes_without_public_generic_abi() {
    assert!(Command::new("clang")
        .arg("--version")
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new("node")
        .arg("--version")
        .output()
        .unwrap()
        .status
        .success());
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.check()?;
        assert_linked_identity(snapshot.entry_program());
        assert_linked_identity(snapshot.test_program());
        for _ in 0..4 {
            assert_eq!(
                snapshot.execute_entry(&Default::default())?.outcome(),
                &ProjectExecutionOutcome::Returned(1)
            );
            assert_eq!(
                snapshot.execute_test(&Default::default())?.outcome(),
                &ProjectExecutionOutcome::Returned(0)
            );
        }

        let public = snapshot.public_api_program();
        assert_eq!(
            public
                .functions
                .iter()
                .map(|function| function.id.as_str())
                .collect::<Vec<_>>(),
            vec!["acme.generic.evaluate", "consumer.evaluate"]
        );
        assert!(public.functions.iter().all(|function| {
            function.params.is_empty() && function.return_type == ResolvedType::I64
        }));
        assert_eq!(
            public
                .types
                .iter()
                .map(|declaration| declaration.id.as_str())
                .collect::<Vec<_>>(),
            vec!["acme.generic.pair"]
        );

        let native = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(
            native,
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?
        );
        let mut ownership_surface = native.clone();
        for admitted in [
            "memcpy(payload, value.ptr, (size_t)value.len);",
            "memcpy(entry->domain_storage, status.domain_id, domain_size);",
        ] {
            assert_eq!(ownership_surface.matches(admitted).count(), 1);
            ownership_surface = ownership_surface.replacen(admitted, "", 1);
        }
        assert!(!ownership_surface.contains("memcpy("));
        for optimization in ["-O0", "-O2"] {
            compile_and_run_c(&native, &fixture.0, optimization, "1");
        }

        let core = wasm::emit_resolved_module(snapshot.entry_program())
            .map_err(|error| vec![error])?;
        assert_eq!(
            core,
            wasm::emit_resolved_module(snapshot.entry_program())
                .map_err(|error| vec![error])?
        );
        run_internal_wasm(&core, &fixture.0);

        let first = snapshot.build_web_inline(wasm::MAX_PROJECT_WEB_BUILD_BYTES)?;
        let second = snapshot.build_web_inline(wasm::MAX_PROJECT_WEB_BUILD_BYTES)?;
        first.verify().map_err(|error| vec![error])?;
        second.verify().map_err(|error| vec![error])?;
        assert_eq!(first.envelope(), second.envelope());
        assert_eq!(first.payload_digest(), second.payload_digest());
        let package = artifacts(first.envelope());
        for (path, bytes) in &package {
            std::fs::write(fixture.0.join(path), bytes).unwrap();
        }
        let scalar_manifest: Value =
            serde_json::from_slice(&package["semaprax.scalar-exports.json"]).unwrap();
        let public_functions = scalar_manifest["scalar_abi"]["functions"]
            .as_array()
            .unwrap();
        assert_eq!(public_functions.len(), 1);
        assert_eq!(public_functions[0]["stable_id"], "consumer.evaluate");
        assert_eq!(public_functions[0]["parameters"], serde_json::json!([]));
        assert_eq!(public_functions[0]["result"], "i64");
        for path in ["semaprax.scalar-exports.json", "semaprax.bindings.d.ts"] {
            assert!(!String::from_utf8(package[path].clone())
                .unwrap()
                .contains("acme.generic.pair"));
        }
        std::fs::write(
            fixture.0.join("consumer.mjs"),
            r#"import fs from 'node:fs';import {instantiateBytes} from './semaprax.bindings.js';const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));const runtime=await instantiateBytes(wasm);for(let i=0;i<4;i++){const result=runtime.call('consumer.evaluate');if(!result.ok||result.value!==1n)throw Error('wrong scalar result')}console.log('generic-owned-dependency-ok');"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg("consumer.mjs")
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "generic-owned-dependency-ok"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn generic_owned_dependency_subject_tamper_is_rejected_before_callback() {
    let fixture = Fixture::new();
    let subject = fixture.0.join("vendor/acme-generic.subject.json");
    let tampered = std::fs::read_to_string(&subject)
        .unwrap()
        .replacen("1.0.0", "1.0.1", 1);
    std::fs::write(subject, tampered).unwrap();
    let invoked = AtomicBool::new(false);
    let errors = with_authenticated_project(&fixture.manifest(), |_| {
        invoked.store(true, Ordering::Relaxed);
        Ok(())
    })
    .unwrap_err();
    assert!(!invoked.load(Ordering::Relaxed));
    assert!(errors.iter().any(|error| error.code == "SPX-J123"));
}
