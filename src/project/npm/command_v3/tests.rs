use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn generated_javascript_is_syntax_valid() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let package = super::render_package("spxgrep", "0.1.0", "spxgrep.run", b"\0asm");
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v3-js-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    for name in ["semaprax.js", "semaprax.command.js"] {
        let artifact = package.iter().find(|item| item.path == name).unwrap();
        let path = directory.join(name);
        std::fs::write(&path, artifact.bytes()).unwrap();
        let output = Command::new("node")
            .arg("--check")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn generated_runtime_executes_owned_stdin_and_consumes_provider_once() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.npm.command;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.npm.command.run")
fn run() -> bool uses { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write } {
let input = stdin_read();
let view = bytes_as_slice(input);
if args_len() == 0usize {
    stdout_write(view) == byte_len(view)
} else {
    stderr_write(view) == byte_len(view) && false
}
}
@id("main")
fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("npm-command-v3-runtime.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let manifest = super::ProjectManifest::parse(
        "schema = \"semaprax.project.v6\"\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"language-command-io.v1\"\nentry = \"test.npm.command\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"test.npm.command.run\"]\ncommand = \"test.npm.command.run\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"test.npm.tests\"]\n",
    )
    .unwrap();
    let fact = format!("sha256:{}", "0".repeat(64));
    let carrier = super::prepare(
        &manifest,
        &program,
        &fact,
        &fact,
        &fact,
        super::super::MAX_PROJECT_NPM_BUILD_BYTES,
    )
    .unwrap();
    carrier.verify().unwrap();
    let wasm = crate::wasm::emit_resolved_language_command_io_v1(&program, "test.npm.command.run")
        .unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.command.run", &wasm);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v3-run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    for name in ["app.wasm", "semaprax.js", "semaprax.command.js"] {
        let artifact = package.iter().find(|item| item.path == name).unwrap();
        std::fs::write(directory.join(name), artifact.bytes()).unwrap();
    }
    let runner = directory.join("run.mjs");
    std::fs::write(
        &runner,
        r#"import fs from 'node:fs';import {createInvocation,instantiate} from './semaprax.js';
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url))),text=new TextEncoder().encode('npm-owned');const invocation=createInvocation([],text);const yes=await instantiate(wasm,invocation);if(!yes.result||new TextDecoder().decode(yes.stdout)!=='npm-owned'||yes.stderr.length!==0)throw Error('true envelope');const no=await instantiate(wasm,createInvocation(['miss'],text));if(no.result||no.stdout.length!==0||new TextDecoder().decode(no.stderr)!=='npm-owned')throw Error('false envelope');let rejected=false;try{await instantiate(wasm,invocation)}catch{rejected=true}if(!rejected)throw Error('provider reuse');console.log('command-v3-runtime-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node").arg(&runner).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let adapter = directory.join("semaprax.command.js");
    let run_adapter = |args: &[&str], stdin: &[u8]| {
        let mut child = Command::new("node")
            .arg(&adapter)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        child.wait_with_output().unwrap()
    };
    let yes = run_adapter(&[], b"adapter-yes");
    assert_eq!(yes.status.code(), Some(0));
    assert_eq!(yes.stdout, b"adapter-yes");
    assert!(yes.stderr.is_empty());
    let no = run_adapter(&["miss"], b"adapter-no");
    assert_eq!(no.status.code(), Some(1));
    assert!(no.stdout.is_empty());
    assert_eq!(no.stderr, b"adapter-no");
    let many = vec!["x"; 17];
    let failed = run_adapter(&many, b"");
    assert_eq!(failed.status.code(), Some(2));
    assert_eq!(failed.stderr, b"spxgrep: command failed\n");
    for name in ["run.mjs", "semaprax.command.js", "semaprax.js", "app.wasm"] {
        std::fs::remove_file(directory.join(name)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn generated_runtime_attributes_only_authenticated_input_failures() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.npm.status_domain;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.npm.status_domain.run")
fn run() -> bool uses { process.args.read } {
let count = args_len();
if count == 0usize {
    let zero = count - count;
    count / zero == 0usize
} else {
    let missing = arg_utf8(count);
    byte_len(str_as_bytes(missing)) == 0usize
}
}
@id("main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("npm-command-status-domain.spx")).unwrap();
    let diagnostics = crate::verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = crate::hir::resolve(&ast).unwrap();
    let wasm =
        crate::wasm::emit_resolved_language_command_io_v1(&program, "test.npm.status_domain.run")
            .unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.status_domain.run", &wasm);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v3-status-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    for name in ["app.wasm", "semaprax.js"] {
        let artifact = package.iter().find(|item| item.path == name).unwrap();
        std::fs::write(directory.join(name), artifact.bytes()).unwrap();
    }
    let runner = directory.join("run.mjs");
    std::fs::write(
        &runner,
        r#"import fs from 'node:fs';import {createInvocation,instantiate} from './semaprax.js';
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));
let generic;try{await instantiate(wasm,createInvocation([],new Uint8Array()))}catch(error){generic=error}if(!generic||generic.domain!==undefined||generic.message==='command input failure')throw Error('generic attribution');
let input;try{await instantiate(wasm,createInvocation(['x'],new Uint8Array()))}catch(error){input=error}if(!input||input.domain!=='semaprax.command-input.v1'||input.code!==1)throw Error('input attribution');
console.log('command-v3-status-domain-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node").arg(&runner).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["run.mjs", "semaprax.js", "app.wasm"] {
        std::fs::remove_file(directory.join(name)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}
