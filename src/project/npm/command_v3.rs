//! Project-v6 Language Command I/O v1 npm carrier.

use std::path::Path;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::project::{
    ProjectManifest, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_LANGUAGE_COMMAND_INPUT_V1,
    PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1, PROJECT_SCHEMA_V6,
};
use sha2::{Digest, Sha256};

use super::{
    artifact, package_error, payload_digest_artifacts_v5, render_carrier_artifacts,
    valid_package_name, valid_package_semver, valid_sha256_fact, NpmArtifact, NpmBuildIdentity,
    ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V5,
};

pub(super) const LANGUAGE_COMMAND_IO_PACKAGE_PATHS: [&str; 7] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.command.json",
    "semaprax.command.js",
    "package.json",
];

pub(super) fn prepare(
    manifest: &ProjectManifest,
    program: &ResolvedProgram,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    if !manifest.is_v6()
        || manifest.profile() != Some(PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1)
        || manifest.command_input() != Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1)
        || !manifest
            .capabilities()
            .iter()
            .map(String::as_str)
            .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
        || manifest.command() != manifest.web_exports().first().map(String::as_str)
        || manifest.web_exports().len() != 1
    {
        return Err(package_error(
            "Language Command I/O npm package requires the exact Project v6 profile",
        ));
    }
    let version = manifest
        .package_version()
        .ok_or_else(|| package_error("Language Command I/O npm package requires a version"))?;
    super::validate_carrier_limit(0, max_bytes)?;
    let command = manifest
        .command()
        .ok_or_else(|| package_error("Language Command I/O npm package requires a command"))?;
    let wasm = crate::wasm::emit_resolved_language_command_io_v1(program, command)?;
    let recipe = super::render_semantic_recipe(program)?;
    let artifacts = render_package(manifest.name(), version, command, &wasm);
    let artifact_bytes = artifacts.iter().try_fold(0usize, |total, item| {
        total
            .checked_add(item.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| {
                package_error("Language Command I/O npm artifacts exceed the trusted limit")
            })
    })?;
    let identity = NpmBuildIdentity {
        project_schema: manifest.schema(),
        package: manifest.name(),
        version,
        project_revision,
        workspace_revision,
        project_graph_digest,
        semantic_recipe: &recipe,
    };
    let payload_digest = payload_digest_artifacts_v5(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA_V5,
        identity,
        &artifacts,
        artifact_bytes,
        &payload_digest,
    );
    super::validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: super::trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

fn render_package(name: &str, version: &str, command: &str, wasm: &[u8]) -> [NpmArtifact; 7] {
    let wasm_sha256 = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(wasm)));
    let metadata = format!(
        "{{\"schema\":\"semaprax.language-command-io.v1\",\"package\":{},\"version\":{},\"command\":{},\"input\":\"argv-utf8+stdin-bytes.v1\",\"input_bounds\":{{\"max_args\":16,\"cumulative_bytes\":65536}},\"capabilities\":[\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"],\"transcripts\":{{\"policy\":\"success-only.v1\",\"combined_max_bytes\":65536,\"max_writes_per_channel_per_path\":1}},\"result\":\"bool\",\"exits\":{{\"true\":0,\"false\":1,\"adapter_failure\":2}},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}}}}\n",
        quote_json(name), quote_json(version), quote_json(command), quote_json(&wasm_sha256)
    );
    let runtime = render_runtime(&wasm_sha256, command);
    let bindings = "export { createInvocation, instantiate } from './semaprax.js';\n";
    let declarations = "export interface CommandResult { readonly result: boolean; readonly stdout: Uint8Array; readonly stderr: Uint8Array; }\nexport declare function createInvocation(argv: readonly string[], stdin: Uint8Array): object;\nexport declare function instantiate(wasm: Uint8Array, invocation: object): Promise<CommandResult>;\n";
    let adapter = render_adapter();
    let package = format!("{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"bin\":{{\"spxgrep\":\"./semaprax.command.js\"}},\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.command.json\"}},\"engines\":{{\"node\":\">=22\"}}}}\n", quote_json(name), quote_json(version));
    [
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.command.json", metadata.as_bytes()),
        artifact("semaprax.command.js", adapter.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ]
}

fn render_runtime(wasm_sha256: &str, command: &str) -> String {
    let runtime = format!(
        r#"const HASH={hash},COMMAND={command},MAX=65536,records=new WeakMap();
const encoder=new TextEncoder();
const copyBytes=(value,label)=>{{if(Object.getPrototypeOf(value)!==Uint8Array.prototype)throw new TypeError(label+' must be Uint8Array');return new Uint8Array(value)}};
export function createInvocation(argv,stdin){{if(!Array.isArray(argv)||argv.length>16)throw new RangeError('argument count');const args=[];let used=0;for(const value of argv){{if(typeof value!=='string'||value.includes('\0'))throw new TypeError('argument');for(let i=0;i<value.length;i++){{const c=value.charCodeAt(i);if(c>=0xd800&&c<=0xdbff){{if(++i>=value.length||value.charCodeAt(i)<0xdc00||value.charCodeAt(i)>0xdfff)throw new TypeError('argument utf8')}}else if(c>=0xdc00&&c<=0xdfff)throw new TypeError('argument utf8')}}const bytes=encoder.encode(value);used+=bytes.length;if(used>MAX)throw new RangeError('input capacity');args.push(bytes)}}const input=copyBytes(stdin,'stdin');used+=input.length;if(used>MAX)throw new RangeError('input capacity');const token=Object.freeze({{}});records.set(token,Object.freeze({{args:Object.freeze(args),stdin:input}}));return token}}
export async function instantiate(wasm,invocation){{const input=records.get(invocation);if(!input||!records.delete(invocation))throw new TypeError('invocation provider');const bytes=copyBytes(wasm,'wasm');if(!globalThis.crypto?.subtle)throw new Error('Web Crypto required');const digest=new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256',bytes)),actual=Array.from(digest,b=>b.toString(16).padStart(2,'0')).join('');if(actual!==HASH)throw new Error('Wasm authentication');let instance=null,next=1,stdinRead=false;const entries=new Map();const decode=c=>{{if(typeof c!=='bigint')throw Error('carrier');const w=BigInt.asUintN(64,c),n=Number(w&0xffffffffn),r=Number((w>>32n)&0xffffffffn);if(n>MAX)throw Error('carrier length');return{{w,n,r,t:(r&0x80000000)!==0,k:r&0x7fffffff}}}};const memory=()=>new Uint8Array(instance.exports.memory.buffer);const read=c=>{{const d=decode(c);if(d.t){{const b=entries.get(d.k);if(!b||b.length!==d.n)throw Error('owned token');return b}}if(d.r>131072-d.n)throw Error('fixed root');return memory().slice(d.r,d.r+d.n)}};const alloc=b=>{{if(entries.size>=16||next>0x7fffffff)throw Error('arena');const k=next++;entries.set(k,new Uint8Array(b));return BigInt.asIntN(64,((0x80000000n|BigInt(k))<<32n)|BigInt(b.length))}};const write=(p,c)=>new DataView(instance.exports.memory.buffer).setBigInt64(p,c,true);let argOffsets=[];let cursor=0;for(const arg of input.args){{argOffsets.push(cursor);cursor+=arg.length}}const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},spx_bytes_copy:c=>alloc(read(c)),spx_bytes_get:(c,i)=>{{const b=read(c),n=Number(BigInt.asUintN(64,i));return n<b.length?b[n]:-1}},spx_bytes_drop:c=>{{const d=decode(c);if(!d.t||!entries.delete(d.k))throw Error('drop')}},spx_bytes_as_slice:c=>{{read(c);return c}},spx_command_args_len_v1:()=>BigInt(input.args.length),spx_command_arg_utf8_v1:(i,p)=>{{const n=Number(BigInt.asUintN(64,i));if(n>=input.args.length)return 1;memory().set(input.args[n],argOffsets[n]);write(p,BigInt.asIntN(64,(BigInt(argOffsets[n])<<32n)|BigInt(input.args[n].length)));return 0}},spx_command_stdin_read_v1:p=>{{if(stdinRead)return 3;stdinRead=true;write(p,alloc(input.stdin));return 0}}}};try{{const result=await WebAssembly.instantiate(bytes,Object.freeze({{env:Object.freeze(env)}}));instance=result.instance;if(instance.exports.memory.buffer.byteLength!==393216)throw Error('memory');const raw=instance.exports[COMMAND](),status=Number(instance.exports.__spx_data_status_v1.value),inputStatus=Number(instance.exports.__spx_command_input_status_v1.value);if(status===0&&inputStatus!==0)throw Error('command input status marker');if(status!==0){{const commandInput=inputStatus!==0;if(commandInput&&(![1,2,3,4].includes(inputStatus)||inputStatus!==status))throw Error('command input status marker');const error=Object.assign(new Error(commandInput?'command input failure':'language command failure'),{{code:status}});if(commandInput)error.domain='semaprax.command-input.v1';throw error}}if(raw!==0&&raw!==1)throw Error('bool');if(entries.size!==0)throw Error('arena unsettled');const sl=Number(instance.exports.__spx_stdout_length_v1.value),el=Number(instance.exports.__spx_stderr_length_v1.value);if(sl<0||el<0||sl+el>MAX)throw Error('transcript');const mem=memory(),stdout=mem.slice(131072,131072+sl),stderr=mem.slice(196608,196608+el);mem.fill(0,131072,393216);return Object.freeze({{result:raw===1,stdout,stderr}})}}catch(error){{if(instance)memory().fill(0,131072,393216);entries.clear();throw error}}}}
"#,
        hash = quote_json(wasm_sha256),
        command = quote_json(&raw_symbol(command))
    );
    runtime.replace(
        "spx_command_stdin_read_v1:p=>{if(stdinRead)return 3;stdinRead=true;write(p,alloc(input.stdin));return 0}}",
        "spx_command_stdin_read_v1:p=>{if(stdinRead)return 3;stdinRead=true;write(p,alloc(input.stdin));return 0},spx_command_owned_bytes_validate_v1:c=>{try{const d=decode(c);if(!d.t||d.k===0)return 1;const b=entries.get(d.k);if(!b)return 1;if(b.length!==d.n){entries.delete(d.k);return 1}return 0}catch{return 1}}}",
    )
}

fn raw_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_data_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("String writes are infallible");
    }
    symbol
}

fn render_adapter() -> String {
    r#"#!/usr/bin/env node
import {readFile} from 'node:fs/promises';import {stdin,stdout,stderr,argv} from 'node:process';import {fileURLToPath} from 'node:url';import {createInvocation,instantiate} from './semaprax.js';
const flush=(stream,bytes)=>new Promise((resolve,reject)=>stream.write(bytes,error=>error?reject(error):resolve()));
try{let used=0,chunks=[];for await(const chunk of stdin){const bytes=new Uint8Array(chunk);used+=bytes.length;if(used>65536)throw Error('input capacity');chunks.push(bytes)}const input=new Uint8Array(used);let at=0;for(const chunk of chunks){input.set(chunk,at);at+=chunk.length}const invocation=createInvocation(argv.slice(2),input);const wasm=new Uint8Array(await readFile(fileURLToPath(new URL('./app.wasm',import.meta.url))));const result=await instantiate(wasm,invocation);await flush(stdout,result.stdout);await flush(stderr,result.stderr);process.exitCode=result.result?0:1}catch{try{await flush(stderr,new TextEncoder().encode('spxgrep: command failed\n'))}catch{}process.exitCode=2}
"#.to_owned()
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 7],
) -> Result<(), Diagnostic> {
    if identity.project_schema != PROJECT_SCHEMA_V6
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
        || artifacts
            .iter()
            .map(|item| item.path)
            .ne(LANGUAGE_COMMAND_IO_PACKAGE_PATHS)
    {
        return Err(package_error(
            "Language Command I/O npm replay identity is invalid",
        ));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(artifact_bytes(artifacts, "semaprax.command.json")?)
            .map_err(|_| package_error("Language Command I/O metadata is invalid"))?;
    let command = metadata
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("Language Command I/O command identity is absent"))?;
    let ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-language-command-recipe.spx"),
    )
    .map_err(|_| package_error("Language Command I/O semantic recipe does not parse"))?;
    let program = crate::hir::resolve(&ast)
        .map_err(|_| package_error("Language Command I/O semantic recipe does not resolve"))?;
    let wasm = crate::wasm::emit_resolved_language_command_io_v1(&program, command)?;
    let expected = render_package(identity.package, identity.version, command, &wasm);
    if artifacts != &expected {
        return Err(package_error(
            "Language Command I/O artifacts disagree with semantic replay",
        ));
    }
    Ok(())
}

fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error(format!("Language Command I/O artifact `{path}` is absent")))
}

#[cfg(test)]
mod tests {
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
        let wasm =
            crate::wasm::emit_resolved_language_command_io_v1(&program, "test.npm.command.run")
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
        let wasm = crate::wasm::emit_resolved_language_command_io_v1(
            &program,
            "test.npm.status_domain.run",
        )
        .unwrap();
        let package =
            super::render_package("spxgrep", "0.1.0", "test.npm.status_domain.run", &wasm);
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
}
