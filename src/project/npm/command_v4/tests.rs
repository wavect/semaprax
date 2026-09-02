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
        "semaprax-command-v4-js-{}-{}",
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
    let selected = byte_range(view, 0usize, byte_len(view));
    if args_len() == 0usize {
        let first = stdout_append(selected);
        stdout_append(selected) == first
    } else {
        let first = stderr_append(selected);
        stderr_append(selected) == first && false
    }
}
@id("main")
fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("npm-command-v4-runtime.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let manifest = super::ProjectManifest::parse(
            "schema = \"semaprax.project.v7\"\nname = \"spxgrep\"\nversion = \"0.1.0\"\nprofile = \"line-command-io.v1\"\nentry = \"test.npm.command\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"test.npm.command.run\"]\ncommand = \"test.npm.command.run\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"test.npm.tests\"]\n",
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
    let envelope: serde_json::Value = serde_json::from_str(carrier.envelope()).unwrap();
    assert_eq!(envelope["schema"], super::PROJECT_NPM_BUILD_SCHEMA_V6);
    assert_eq!(envelope["project_schema"], super::PROJECT_SCHEMA_V7);
    let wasm =
        crate::wasm::emit_resolved_line_command_io_v1(&program, "test.npm.command.run").unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.command.run", &wasm);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v4-run-{}-{}",
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
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url))),text=new TextEncoder().encode('npm-owned');const invocation=createInvocation([],text);const yes=await instantiate(wasm,invocation);if(!yes.result||new TextDecoder().decode(yes.stdout)!=='npm-ownednpm-owned'||yes.stderr.length!==0)throw Error('true envelope');const no=await instantiate(wasm,createInvocation(['miss'],text));if(no.result||no.stdout.length!==0||new TextDecoder().decode(no.stderr)!=='npm-ownednpm-owned')throw Error('false envelope');let rejected=false;try{await instantiate(wasm,invocation)}catch{rejected=true}if(!rejected)throw Error('provider reuse');console.log('command-v4-runtime-ok');
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
    assert_eq!(yes.stdout, b"adapter-yesadapter-yes");
    assert!(yes.stderr.is_empty());
    let no = run_adapter(&["miss"], b"adapter-no");
    assert_eq!(no.status.code(), Some(1));
    assert!(no.stdout.is_empty());
    assert_eq!(no.stderr, b"adapter-noadapter-no");
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
fn generated_runtime_enforces_exact_cumulative_boundary_without_partial_publication() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.npm.append_boundary;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.npm.append_boundary.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    let selected = byte_range(view, 0usize, byte_len(view));
    let first = stdout_append(selected);
    stdout_append(selected) == first
}
@id("main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("npm-command-v4-boundary.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let wasm =
        crate::wasm::emit_resolved_line_command_io_v1(&program, "test.npm.append_boundary.run")
            .unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.append_boundary.run", &wasm);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v4-boundary-{}-{}",
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
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));const exact=new Uint8Array(32768);exact.fill(97);const accepted=await instantiate(wasm,createInvocation([],exact));if(!accepted.result||accepted.stdout.length!==65536||accepted.stderr.length!==0||accepted.stdout.some(value=>value!==97))throw Error('exact boundary');const over=new Uint8Array(32769);over.fill(98);let failure;try{await instantiate(wasm,createInvocation([],over))}catch(error){failure=error}if(!failure||failure.domain!=='semaprax.command-output.v1'||failure.code!==1)throw Error('output status');console.log('command-v4-boundary-ok');
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

    let mut child = Command::new("node")
        .arg(directory.join("semaprax.command.js"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![98; 32_769])
        .unwrap();
    let failed = child.wait_with_output().unwrap();
    assert_eq!(failed.status.code(), Some(2));
    assert!(failed.stdout.is_empty(), "staged stdout leaked on failure");
    assert_eq!(failed.stderr, b"spxgrep: command failed\n");

    for name in ["run.mjs", "semaprax.command.js", "semaprax.js", "app.wasm"] {
        std::fs::remove_file(directory.join(name)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn generated_runtime_rejects_same_length_range_root_and_offset_substitution() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.npm.range_binding;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.npm.range_binding.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    let selected = byte_range(view, 0usize, 2usize);
    let first = stdout_append(selected);
    stdout_append(selected) == first
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program = crate::hir::resolve(
        &crate::parse(source, Path::new("npm-command-v4-range-binding.spx")).unwrap(),
    )
    .unwrap();
    let wasm =
        crate::wasm::emit_resolved_line_command_io_v1(&program, "test.npm.range_binding.run")
            .unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.range_binding.run", &wasm);
    let original = std::str::from_utf8(
        package
            .iter()
            .find(|item| item.path == "semaprax.js")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    let needle = "spx_bytes_get:(c,i)=>{const n=Number(BigInt.asUintN(64,i)),b=read(c,n===0);return n<b.length?b[n]:-1}";
    assert!(original.contains(needle));
    for attack in ["root", "offset"] {
        let mutation = if attack == "root" {
            "spx_bytes_get:(c,i)=>{const n=Number(BigInt.asUintN(64,i)),b=read(c,n===0),value=n<b.length?b[n]:-1;if(n===b.length-1&&!globalThis.__spx_mutated){const d=decode(c),p=(d.r&0xffff)*8,replacement=alloc(new Uint8Array(b.length));new DataView(instance.exports.memory.buffer).setBigInt64(p+8,replacement,true);globalThis.__spx_mutated=true}return value}"
        } else {
            "spx_bytes_get:(c,i)=>{const n=Number(BigInt.asUintN(64,i)),b=read(c,n===0),value=n<b.length?b[n]:-1;if(n===b.length-1&&!globalThis.__spx_mutated){const d=decode(c),p=(d.r&0xffff)*8,view=new DataView(instance.exports.memory.buffer);view.setBigUint64(p+16,view.getBigUint64(p+16,true)+1n,true);globalThis.__spx_mutated=true}return value}"
        };
        let runtime = original.replacen(needle, mutation, 1);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-command-v4-range-binding-{attack}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("app.wasm"), &wasm).unwrap();
        std::fs::write(directory.join("semaprax.js"), runtime).unwrap();
        std::fs::write(
                directory.join("run.mjs"),
                "import fs from 'node:fs';import {createInvocation,instantiate} from './semaprax.js';const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));let rejected=false;try{await instantiate(wasm,createInvocation([],new TextEncoder().encode('abcd')))}catch{rejected=true}if(!rejected)throw Error('descriptor substitution');\n",
            )
            .unwrap();
        let output = Command::new("node")
            .arg(directory.join("run.mjs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{attack}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for name in ["run.mjs", "semaprax.js", "app.wasm"] {
            std::fs::remove_file(directory.join(name)).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
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
fn run() -> bool uses { process.args.read, process.stdin.read, process.stdout.write } {
    let count = args_len();
    if count == 0usize {
        let zero = count - count;
        count / zero == 0usize
    } else {
        let missing = arg_utf8(count);
        let input = stdin_read();
        let view = bytes_as_slice(input);
        let selected = byte_range(view, 0usize, byte_len(view));
        stdout_append(selected) == byte_len(str_as_bytes(missing))
    }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("npm-command-status-domain.spx")).unwrap();
    let diagnostics = crate::verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = crate::hir::resolve(&ast).unwrap();
    let wasm =
        crate::wasm::emit_resolved_line_command_io_v1(&program, "test.npm.status_domain.run")
            .unwrap();
    let package = super::render_package("spxgrep", "0.1.0", "test.npm.status_domain.run", &wasm);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-v4-status-{}-{}",
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
console.log('command-v4-status-domain-ok');
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
