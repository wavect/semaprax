use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup_plan::CleanupTransition;
use semaprax::hir::{
    self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType,
};
use semaprax::project::{with_authenticated_project, ProjectExecutionOutcome};
use semaprax::{codegen, package_lock_v3, package_report_v2, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const DEPENDENCY: &str = r#"
module acme.vec;

@id("acme.vec.sum")
fn sum() -> i64 {
    let mut values = vec_with_capacity<i64>(3usize);
    values = vec_push<i64>(values, 6);
    values = vec_push<i64>(values, 7);
    values = vec_push<i64>(values, 8);
    vec_get<i64>(values, 0usize) + vec_get<i64>(values, 1usize) + vec_get<i64>(values, 2usize)
}

@id("acme.vec.main")
fn main() -> i64 { sum() }
"#;

const LOCAL: &str = r#"
module consumer.local;

@id("consumer.local.sum")
fn sum() -> i64 {
    let mut values = vec_with_capacity<i64>(3usize);
    values = vec_push<i64>(values, 6);
    values = vec_push<i64>(values, 7);
    values = vec_push<i64>(values, 8);
    vec_get<i64>(values, 0usize) + vec_get<i64>(values, 1usize) + vec_get<i64>(values, 2usize)
}
"#;

const APP: &str = r#"
module consumer.app;
use function @id("acme.vec.sum") from acme.vec as dependency_sum;
use function @id("consumer.local.sum") from consumer.local as local_sum;

@id("consumer.public")
fn published_scalar() -> i64 { 7 }

@id("consumer.main")
fn main() -> i64 { dependency_sum() + local_sum() }
"#;

const TESTS: &str = r#"
module consumer.tests;
use function @id("acme.vec.sum") from acme.vec as dependency_sum;
use function @id("consumer.local.sum") from consumer.local as local_sum;

@id("consumer.tests.main")
fn main() -> i64 { dependency_sum() + local_sum() }
"#;

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "consumer"
version = "0.1.0"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/local.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = ["consumer.public"]

[dependencies]
acme.vec = "^1.0.0"

[dependency-sources]
acme.vec = "vendor/acme-vec.subject.json"
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        fn canonical(source: &str, path: &Path) -> String {
            let parsed = semaprax::parse(source, path).unwrap();
            semaprax::format::canonical(&parsed)
        }
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-owned-vec-{}-{}",
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
                package: "acme.vec".to_owned(),
                version: "1.0.0".to_owned(),
            },
            &report,
            &[],
            &[],
        )
        .unwrap();
        std::fs::write(root.join("vendor/acme-vec.subject.json"), subject).unwrap();
        let app = root.join("src/app.spx");
        let local = root.join("src/local.spx");
        let tests = root.join("src/tests.spx");
        std::fs::write(&app, canonical(APP, &app)).unwrap();
        std::fs::write(&local, canonical(LOCAL, &local)).unwrap();
        std::fs::write(&tests, canonical(TESTS, &tests)).unwrap();
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

fn vec_type(element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("core.vec"),
        arguments: vec![element],
    }
}

fn assert_linked_vec(program: &hir::ResolvedProgram, function_id: &str, element: ResolvedType) {
    hir::validate(program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .expect("retained Vec provider");
    let expected = vec_type(element.clone());
    assert!(function
        .cleanup
        .slots
        .iter()
        .any(|slot| slot.ty == expected));
    assert!(function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(transition, CleanupTransition::Transfer { .. })));
    fn collect_calls(
        expression: &ResolvedExpr,
        calls: &mut Vec<(String, Option<hir::FunctionInstanceId>, Vec<ResolvedType>)>,
    ) {
        match &expression.kind {
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                if callee.as_str().starts_with("core.vec.") {
                    calls.push((
                        callee.as_str().to_owned(),
                        instance.clone(),
                        type_arguments.clone(),
                    ));
                }
                for argument in args {
                    collect_calls(argument, calls);
                }
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                collect_calls(left, calls);
                collect_calls(right, calls);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { value, .. }
                        | ResolvedStatement::Assign { value, .. } => collect_calls(value, calls),
                        other => panic!("unexpected Vec fixture statement {other:?}"),
                    }
                }
                collect_calls(tail, calls);
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Place(_) => {}
            other => panic!("unexpected Vec fixture expression {other:?}"),
        }
    }
    let mut calls = Vec::new();
    collect_calls(&function.body, &mut calls);
    assert!(calls.iter().any(|(callee, instance, arguments)| {
        callee == "core.vec.with-capacity" && instance.is_none() && arguments == &[element.clone()]
    }));
    assert!(
        calls
            .iter()
            .filter(|(callee, _, _)| callee == "core.vec.push")
            .count()
            == 3
    );
    assert!(
        calls
            .iter()
            .filter(|(callee, _, _)| callee == "core.vec.get")
            .count()
            == 3
    );
}

pub(super) fn compile_and_run_c(source: &str, root: &Path, optimization: &str, expected: &str) {
    let source_path = root.join(format!("entry-{optimization}.c"));
    let binary = root.join(format!("entry-{optimization}"));
    std::fs::write(&source_path, source).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
}

pub(super) fn run_internal_wasm(bytes: &[u8], root: &Path, expected: i64) {
    let wasm = root.join("entry.wasm");
    std::fs::write(&wasm, bytes).unwrap();
    let script = r#"const fs=require('fs');const bytes=fs.readFileSync(process.argv[1]);const expected=BigInt(process.argv[2]);let next=1n;const entries=new Map();const key=v=>{if(typeof v!=='bigint'||v===0n)throw Error('carrier');return v.toString()};const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('stale-or-type');return e};const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:c=>{throw Error('status:'+c)},spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?alloc(tag,n):0n},spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;entries.delete(key(source));return alloc(tag,old.capacity,old.values.concat([bits]))},spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),spx_vec_get:(source,tag,index)=>{const e=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=e.values.length)throw Error('oob');return e.values[n]},spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')}};WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i++){const value=instance.exports.semaprax_main();if(value!==expected||entries.size!==0)throw Error('result-or-leak:'+value+':'+entries.size)}}).catch(error=>{console.error(error);process.exit(2)});"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm)
        .arg(expected.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn internal_vec_crosses_project_and_authenticated_dependency_boundaries() {
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
        assert_linked_vec(snapshot.entry_program(), "acme.vec.sum", ResolvedType::I64);
        assert_linked_vec(
            snapshot.entry_program(),
            "consumer.local.sum",
            ResolvedType::I64,
        );
        assert_linked_vec(snapshot.test_program(), "acme.vec.sum", ResolvedType::I64);
        for _ in 0..3 {
            assert_eq!(
                snapshot.execute_entry(&Default::default())?.outcome(),
                &ProjectExecutionOutcome::Returned(42)
            );
            assert_eq!(
                snapshot.execute_test(&Default::default())?.outcome(),
                &ProjectExecutionOutcome::Returned(42)
            );
        }
        let generated =
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert!(!generated.contains("memcpy(result, source"));
        for optimization in ["-O0", "-O2"] {
            compile_and_run_c(&generated, &fixture.0, optimization, "42");
        }
        let core =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        run_internal_wasm(&core, &fixture.0, 42);
        assert_eq!(snapshot.public_api_program().functions.len(), 1);
        assert_eq!(
            snapshot.public_api_program().functions[0].id.as_str(),
            "consumer.public"
        );
        snapshot.build_web_inline(wasm::MAX_PROJECT_WEB_BUILD_BYTES)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn vec_reaching_public_adapter_and_dependency_tamper_fail_before_execution() {
    let fixture = Fixture::new();
    let manifest = std::fs::read_to_string(fixture.manifest()).unwrap();
    std::fs::write(
        fixture.manifest(),
        manifest.replace("web = [\"consumer.public\"]", "web = [\"consumer.main\"]"),
    )
    .unwrap();
    let errors = with_authenticated_project(&fixture.manifest(), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-W115"));

    std::fs::write(fixture.manifest(), MANIFEST).unwrap();
    let subject = fixture.0.join("vendor/acme-vec.subject.json");
    let tampered = std::fs::read_to_string(&subject)
        .unwrap()
        .replacen("1.0.0", "1.0.1", 1);
    std::fs::write(subject, tampered).unwrap();
    let errors = with_authenticated_project(&fixture.manifest(), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-J123"));
}
