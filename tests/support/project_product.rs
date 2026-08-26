use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

#[path = "native_rust_cargo.rs"]
mod native_rust_cargo;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

pub const BUILD_MAX_BYTES: usize = 512 * 1024;
const EXPECTED_42_LINE: &[u8] = b"42\n";

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = temporary(label);
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct ProjectFixture(TempDir);

impl ProjectFixture {
    pub fn calculator(label: &str) -> Self {
        let root = TempDir::new(&format!("fixture-{label}"));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in PROJECT_FILES {
            std::fs::copy(example.join(relative), root.join(relative)).unwrap();
        }
        Self(root)
    }

    pub fn manifest(&self) -> PathBuf {
        self.0.path().join("semaprax.toml")
    }

    pub fn core_source(&self) -> String {
        std::fs::read_to_string(self.0.path().join("src/core.spx")).unwrap()
    }
}

pub struct Daemon {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Daemon {
    pub fn workflow(fixture: &ProjectFixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest())
            .arg("--allow-project-workflow")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    pub fn call(&mut self, id: u64, method: &str, params: Option<Value>) -> Value {
        let mut request = json!({"jsonrpc":"2.0", "id":id, "method":method});
        if let Some(params) = params {
            request
                .as_object_mut()
                .unwrap()
                .insert("params".to_owned(), params);
        }
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();

        let mut response = String::new();
        if self.output.read_line(&mut response).unwrap() == 0 {
            let status = self.child.wait().unwrap();
            let mut stderr = String::new();
            self.child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!("daemon closed before response ({status}): {stderr}");
        }
        serde_json::from_str(response.trim_end()).unwrap()
    }

    pub fn finish(mut self) {
        let response = self.call(999, "shutdown", None);
        assert_eq!(response["result"]["ok"], true);
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(status.success(), "daemon failed: {stderr}");
        assert!(stderr.is_empty(), "daemon wrote stderr: {stderr}");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn subject(project_revision: &str, workspace_revision: &str) -> Value {
    json!({
        "project_revision": project_revision,
        "workspace_revision": workspace_revision
    })
}

pub fn run_web_carrier(build: &Value, label: &str) -> BTreeMap<String, Vec<u8>> {
    let files = validate_web_carrier(build).unwrap();
    let root = TempDir::new(&format!("web-{label}"));
    for (path, bytes) in &files {
        std::fs::write(root.path().join(path), bytes).unwrap();
    }
    compile_typescript_consumer(root.path());
    run_javascript_consumer(root.path());
    files
}

pub fn validate_web_carrier(build: &Value) -> Result<BTreeMap<String, Vec<u8>>, String> {
    const EXPECTED: [&str; 7] = [
        "app.wasm",
        "index.html",
        "package.json",
        "semaprax.bindings.d.ts",
        "semaprax.bindings.js",
        "semaprax.js",
        "semaprax.scalar-exports.json",
    ];
    let artifacts = build["artifacts"]
        .as_array()
        .ok_or_else(|| "Web carrier artifacts must be an array".to_owned())?;
    let mut files = BTreeMap::new();
    for artifact in artifacts {
        let path = artifact["path"]
            .as_str()
            .ok_or_else(|| "Web carrier artifact path must be a string".to_owned())?;
        let mut components = Path::new(path).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || Path::new(path).is_absolute()
        {
            return Err(format!(
                "Web carrier artifact path must be one relative filename: {path}"
            ));
        }
        let encoded = artifact["content_hex"]
            .as_str()
            .ok_or_else(|| format!("Web carrier artifact {path} content must be hex"))?;
        let bytes = decode_hex(encoded)?;
        if files.insert(path.to_owned(), bytes).is_some() {
            return Err(format!("duplicate Web carrier artifact path: {path}"));
        }
    }
    if files.keys().map(String::as_str).ne(EXPECTED) {
        return Err(format!(
            "Web carrier artifact inventory is not the exact closed package: {:?}",
            files.keys().collect::<Vec<_>>()
        ));
    }
    Ok(files)
}

fn compile_typescript_consumer(root: &Path) {
    if std::env::var_os("SEMAPRAX_REQUIRE_PROJECT_TYPESCRIPT").as_deref()
        != Some(std::ffi::OsStr::new("1"))
    {
        return;
    }
    std::fs::write(
        root.join("acceptance.ts"),
        r#"import { instantiateBytes, type ScalarResult } from "./semaprax.bindings.js";
const runtime = await instantiateBytes(new Uint8Array());
const added: ScalarResult<bigint> = runtime.functions["calculator.add"](19n, 23n);
const subtracted: ScalarResult<bigint> = runtime.functions["calculator.subtract"](84n, 42n);
const multiplied: ScalarResult<bigint> = runtime.functions["calculator.multiply"](6n, 7n);
const divided: ScalarResult<bigint> = runtime.functions["calculator.divide"](84n, 2n);
const negative: ScalarResult<boolean> = runtime.functions["calculator.is-negative"](-1n);
const negated: ScalarResult<boolean> = runtime.functions["calculator.not"](true);
for (const outcome of [added, subtracted, multiplied, divided]) {
  if (outcome.ok) {
    const value: bigint = outcome.value;
    if (value !== 42n) throw new Error("unexpected calculator result");
  } else {
    const domain: "semaprax.arithmetic.v1" | "semaprax.contract.v1" = outcome.status.domain_id;
    if (domain.length === 0) throw new Error("unreachable status domain");
  }
}
if (!negative.ok) throw new Error(`unexpected predicate status ${negative.status.code}`);
const negativeValue: boolean = negative.value;
if (!negativeValue) throw new Error("unexpected is-negative result");
if (!negated.ok) throw new Error(`unexpected predicate status ${negated.status.code}`);
const negatedValue: boolean = negated.value;
if (negatedValue) throw new Error("unexpected not result");
"#,
    )
    .unwrap();
    let tsc = std::env::var_os("TSC").unwrap_or_else(|| "tsc".into());
    let version = Command::new(&tsc).arg("--version").output().unwrap();
    assert!(
        version.status.success(),
        "required TypeScript compiler failed"
    );
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "Version 5.8.3",
        "Project acceptance requires the repository-pinned TypeScript compiler"
    );
    let checked = Command::new(&tsc)
        .args([
            "--strict",
            "--noEmit",
            "--target",
            "ES2022",
            "--module",
            "NodeNext",
            "--moduleResolution",
            "NodeNext",
            "acceptance.ts",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "generated Project TypeScript consumer failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
}

fn run_javascript_consumer(root: &Path) {
    std::fs::write(
        root.join("acceptance.mjs"),
        r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instantiateBytes } from "./semaprax.bindings.js";
const runtime = await instantiateBytes(await readFile("./app.wasm"));
assert.deepEqual(runtime.call("calculator.add", 19n, 23n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.subtract", 84n, 42n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.multiply", 6n, 7n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.divide", 84n, 2n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.is-negative", -1n), {ok:true,value:true});
assert.deepEqual(runtime.call("calculator.not", true), {ok:true,value:false});
assert.deepEqual(runtime.call("calculator.add", (1n << 63n) - 1n, 1n), {ok:false,status:{schema:"semaprax.status.v1",domain_id:"semaprax.arithmetic.v1",code:1}});
assert.deepEqual(runtime.call("calculator.divide", 1n, 0n), {ok:false,status:{schema:"semaprax.status.v1",domain_id:"semaprax.contract.v1",code:1}});
"#,
    )
    .unwrap();
    let node = Command::new("node")
        .arg("acceptance.mjs")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        node.status.success(),
        "returned Web carrier failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
}

pub fn run_native_c(source: &str, label: &str, expected: &str, optimizations: &[&str]) {
    let root = TempDir::new(&format!("native-{label}"));
    let source_path = root.join("program.c");
    std::fs::write(&source_path, source).unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in optimizations {
        let executable = root.join(format!("program-{}", optimization.trim_start_matches('-')));
        let compile = Command::new(&clang)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "native {label} {optimization} compilation failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert!(
            run.status.success(),
            "native {label} {optimization} execution failed: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), expected);
        assert!(run.stderr.is_empty());
    }
}

pub fn run_core_wasm(bytes: &[u8], label: &str, expected: &str) {
    let root = TempDir::new(&format!("core-wasm-{label}"));
    std::fs::write(root.join("program.wasm"), bytes).unwrap();
    std::fs::write(
        root.join("execute.mjs"),
        format!(
            r#"import assert from "node:assert/strict";
import {{ readFile }} from "node:fs/promises";
const checked = (operation) => (a, b) => {{
  const value = operation(a, b);
  if (value < -(1n << 63n) || value > (1n << 63n) - 1n) throw new RangeError();
  return value;
}};
const imports = {{env:{{
  spx_add:checked((a,b)=>a+b), spx_sub:checked((a,b)=>a-b),
  spx_mul:checked((a,b)=>a*b), spx_div:(a,b)=>a/b, spx_rem:(a,b)=>a%b,
  spx_neg:(a)=>-a, spx_contract_fail:()=>{{throw new Error("contract")}}
}}}};
const linked = await WebAssembly.instantiate(await readFile("./program.wasm"), imports);
assert.equal(linked.instance.exports.semaprax_main(), {expected}n);
"#
        ),
    )
    .unwrap();
    let node = Command::new("node")
        .arg("execute.mjs")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        node.status.success(),
        "Core Wasm {label} execution failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
}

pub struct RustSdkFacts {
    pub project_revision: String,
    pub workspace_revision: String,
    pub subject_digest: String,
    pub source_revisions: Vec<String>,
    pub manifest_exports: Vec<Value>,
}

pub fn native_rust_sdk_required() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_PROJECT_NATIVE_RUST_SDK").as_deref()
        == Some(std::ffi::OsStr::new("1"))
}

pub fn run_project_rust_sdk(fixture: &ProjectFixture, label: &str) -> RustSdkFacts {
    let root = TempDir::new(&format!("rust-{label}"));
    let generated = root.join("generated-project-sdk");
    let consumer = root.join("project-consumer");
    let cargo_target = root.join("target");
    #[cfg(windows)]
    assert_windows_cargo_target_budget(&cargo_target);
    std::fs::create_dir_all(consumer.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-rust");
    for relative in ["Cargo.toml", "src/main.rs"] {
        std::fs::copy(
            example.join("project-consumer").join(relative),
            consumer.join(relative),
        )
        .unwrap();
    }

    let setup = native_rust_cargo::cargo_command()
        .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
        .arg(example.join("Cargo.toml"))
        .arg("--")
        .arg("project")
        .arg(fixture.manifest())
        .arg(&generated)
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "generate {label} Project Rust SDK: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let stdout = String::from_utf8(setup.stdout).unwrap();
    let fields = stdout.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 6, "unexpected Project SDK setup output");

    let lock = native_rust_cargo::cargo_command()
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &cargo_target)
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "lock {label} Project Rust consumer: {}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let run = native_rust_cargo::cargo_command()
        .args([
            "run",
            "--verbose",
            "--locked",
            "--offline",
            "--manifest-path",
        ])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &cargo_target)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run {label} Project Rust consumer: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, EXPECTED_42_LINE);

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(generated.join("semaprax.native-rust-sdk.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["schema"], "semaprax.project-native-rust-sdk.v1");
    assert_eq!(manifest["project_subject"]["project_revision"], fields[3]);
    assert_eq!(manifest["project_subject"]["workspace_revision"], fields[4]);
    let sources = manifest["project_subject"]["sources"]
        .as_array()
        .expect("Project Rust SDK manifest must carry source inventory");
    assert_eq!(
        sources
            .iter()
            .map(|source| source["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["src/app.spx", "src/core.spx", "src/tests.spx"]
    );
    let source_revisions = sources
        .iter()
        .map(|source| source["source_revision"].as_str().unwrap().to_owned())
        .collect();
    let manifest_exports = manifest["project_subject"]["exports"]
        .as_array()
        .expect("Project Rust SDK manifest must carry exact export inventory")
        .clone();
    let descriptor: Value =
        serde_json::from_slice(&std::fs::read(generated.join("native/descriptor.json")).unwrap())
            .unwrap();
    assert_eq!(
        descriptor["schema"],
        "semaprax.project-native-rust-interop-descriptor.v1"
    );
    assert_eq!(descriptor["project_subject_digest"], fields[5]);

    RustSdkFacts {
        project_revision: fields[3].to_owned(),
        workspace_revision: fields[4].to_owned(),
        subject_digest: fields[5].to_owned(),
        source_revisions,
        manifest_exports,
    }
}

#[cfg(windows)]
fn assert_windows_cargo_target_budget(target: &Path) {
    use std::os::windows::ffi::OsStrExt as _;

    // link.exe still applies its legacy object-input path boundary even when
    // rustc passes an absolute verbatim path. Model the longest build-script
    // object name emitted for the generated SDK, including Cargo's three
    // fixed-width disambiguators, and leave room for the terminating NUL.
    const MAX_PATH_UTF16_UNITS: usize = 260;
    const GENERATED_SDK_BUILD_SCRIPT_OBJECT_SUFFIX: &str = concat!(
        r"\debug\build\semaprax-generated-native-rust-sdk-0000000000000000",
        r"\build_script_build-0000000000000000.build_script_build.",
        "0000000000000000-cgu.0.rcgu.o",
    );
    let target_units = target.as_os_str().encode_wide().count();
    let object_units = target_units
        .checked_add(
            GENERATED_SDK_BUILD_SCRIPT_OBJECT_SUFFIX
                .encode_utf16()
                .count(),
        )
        .expect("nested Cargo object path length overflow");
    eprintln!(
        "nested Product Cargo target path uses {target_units} UTF-16 units; longest modeled object uses {object_units}"
    );
    assert!(
        object_units < MAX_PATH_UTF16_UNITS,
        "nested Product Cargo object path exceeds the legacy link.exe boundary: {object_units} >= {MAX_PATH_UTF16_UNITS}",
    );
}

fn temporary(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-product-acceptance-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        return Err("Web carrier artifact hex has odd length".to_owned());
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in pairs {
        let high = char::from(pair[0])
            .to_digit(16)
            .ok_or_else(|| "Web carrier artifact content is not hex".to_owned())?;
        let low = char::from(pair[1])
            .to_digit(16)
            .ok_or_else(|| "Web carrier artifact content is not hex".to_owned())?;
        decoded.push(u8::try_from((high << 4) | low).unwrap());
    }
    Ok(decoded)
}
