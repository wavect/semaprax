//! Additive raw Wasm boundary for Bounded Language Command I/O v1.

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, IdentityOrigin, ResolvedProgram, ResolvedType};

use super::{write_u32, I32};

pub(super) const IMPORT_COUNT: u32 = 4;
pub(super) const ARGS_LEN_IMPORT: u32 = super::SCALAR_IMPORT_COUNT + 4;
pub(super) const ARG_UTF8_IMPORT: u32 = ARGS_LEN_IMPORT + 1;
pub(super) const STDIN_READ_IMPORT: u32 = ARGS_LEN_IMPORT + 2;
pub(super) const OWNED_BYTES_VALIDATE_IMPORT: u32 = ARGS_LEN_IMPORT + 3;
pub(super) const INPUT_STATUS_GLOBAL: u32 = 14;
pub(super) const INPUT_STATUS_EXPORT: &str = "__spx_command_input_status_v1";

#[derive(Clone, Debug)]
pub(super) struct CommandPlan {
    pub(super) function_id: DeclarationId,
    pub(super) wasm_export: String,
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<CommandPlan, Diagnostic> {
    crate::hir::validate(program)?;
    if program.permits
        != [
            crate::command_io_ops::ARGS_READ_EFFECT,
            crate::command_io_ops::STDERR_WRITE_EFFECT,
            crate::command_io_ops::STDIN_READ_EFFECT,
            crate::host_io_ops::STDOUT_WRITE_EFFECT,
        ]
    {
        return Err(admission(
            "Language Command I/O v1 requires its exact four permits",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| admission("selected Language Command I/O function is absent"))?;
    if program
        .declarations
        .declaration(&function.id)
        .map(|fact| fact.identity_origin)
        != Some(IdentityOrigin::Explicit)
        || !function.params.is_empty()
        || function.return_type != ResolvedType::Bool
    {
        return Err(admission(
            "selected Language Command I/O function must be explicit fn() -> bool",
        ));
    }
    Ok(CommandPlan {
        function_id: function.id.clone(),
        wasm_export: super::data_exports::raw_symbol(command_id),
    })
}

pub(super) fn emit_wrapper_body(target_index: u32) -> Vec<u8> {
    const OLD_STACK: u32 = 0;
    const RESULT_OUT: u32 = 1;
    const STATUS: u32 = 2;
    const RESULT: u32 = 3;
    let mut body = Vec::new();
    write_u32(&mut body, 1);
    write_u32(&mut body, 4);
    body.push(I32);
    super::host_output::emit_reset(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_reset(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x41, 0x00, 0x24, 0x01]); // public status = success
    body.extend([0x41, 0x00, 0x24]);
    write_u32(&mut body, INPUT_STATUS_GLOBAL);
    body.extend([0x23, 0x00, 0x22]);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x41, 0x08, 0x49, 0x04, 0x40, 0x00, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x41, 0x08, 0x6b, 0x22]);
    write_u32(&mut body, RESULT_OUT);
    body.extend([0x24, 0x00, 0x20]);
    write_u32(&mut body, RESULT_OUT);
    body.push(0x10);
    write_u32(&mut body, target_index);
    body.extend([0x21]);
    write_u32(&mut body, STATUS);
    body.push(0x20);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x24, 0x00, 0x20]);
    write_u32(&mut body, STATUS);
    body.extend([0x24, 0x01, 0x20]);
    write_u32(&mut body, STATUS);
    body.extend([0x04, 0x40]);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x41, 0x00, 0x0f, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, RESULT_OUT);
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, RESULT);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40]);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x00, 0x0b]);
    super::host_output::emit_publish_immediate(
        &mut body,
        super::host_output::COMMAND_STDOUT_GLOBALS,
    );
    super::host_output::emit_publish_immediate(
        &mut body,
        super::host_output::COMMAND_STDERR_GLOBALS,
    );
    body.push(0x20);
    write_u32(&mut body, RESULT);
    body.push(0x0b);
    body
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W114", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn node_owned_stdin_is_staged_before_drop_and_provider_failure_discards() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let source = r#"
module test.command;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view) == byte_len(view)
}
@id("main")
fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, Path::new("command-io-wasm.spx")).unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let wasm = super::super::emit_resolved_language_command_io_v1(&program, "test.command.run")
            .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "semaprax-command-io-node-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let wasm_path = directory.join("app.wasm");
        let script_path = directory.join("test.mjs");
        std::fs::write(&wasm_path, wasm).unwrap();
        let symbol = super::super::data_exports::raw_symbol("test.command.run");
        let script = format!(
            r#"import fs from 'node:fs';
const bytes=fs.readFileSync(process.argv[2]);
async function run(fail,hostile=false,fixed=false,oversized=false){{let instance=null,next=1;const entries=new Map();
const decode=c=>{{const w=BigInt.asUintN(64,c),n=Number(w&0xffffffffn),r=Number((w>>32n)&0xffffffffn);return{{n,r,t:(r&0x80000000)!==0,k:r&0x7fffffff}}}};
const read=c=>{{const d=decode(c);if(d.t){{const b=entries.get(d.k);if(!b||b.length!==d.n)throw Error('token');return b}}return new Uint8Array(instance.exports.memory.buffer).slice(d.r,d.r+d.n)}};
const alloc=b=>{{const k=next++;entries.set(k,new Uint8Array(b));return BigInt.asIntN(64,((0x80000000n|BigInt(k))<<32n)|BigInt(b.length))}};
const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},
spx_bytes_copy:c=>alloc(read(c)),spx_bytes_get:(c,i)=>{{if(hostile)return -1;const b=read(c),n=Number(i);return n<b.length?b[n]:-1}},spx_bytes_drop:c=>{{const d=decode(c);if(!d.t||!entries.delete(d.k))throw Error('drop')}},spx_bytes_as_slice:c=>{{read(c);return c}},
spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:p=>{{if(fail)return 3;const c=fixed?11n:oversized?BigInt.asIntN(64,(0x80000001n<<32n)|65537n):alloc(new TextEncoder().encode('owned-stdin'));new DataView(instance.exports.memory.buffer).setBigInt64(p,c,true);return 0}}}};
const result=await WebAssembly.instantiate(bytes,{{env}});instance=result.instance;const value=instance.exports[{symbol}]();const out=new Uint8Array(instance.exports.memory.buffer);const len=Number(instance.exports.__spx_stdout_length_v1.value);return{{value,len,text:new TextDecoder().decode(out.slice(131072,131072+len)),err:Number(instance.exports.__spx_stderr_length_v1.value),entries:entries.size,status:Number(instance.exports.__spx_data_status_v1.value),inputStatus:Number(instance.exports.__spx_command_input_status_v1.value),dirty:out.slice(131072,393216).some(value=>value!==0)}}}}
const ok=await run(false);if(ok.value!==1||ok.text!=='owned-stdin'||ok.err!==0||ok.entries!==0||ok.status!==0)throw Error(JSON.stringify(ok));
const bad=await run(true);if(bad.len!==0||bad.err!==0||bad.entries!==0||bad.status!==3||bad.inputStatus!==3)throw Error(JSON.stringify(bad));for(const forged of [await run(false,false,true),await run(false,false,false,true)])if(forged.len!==0||forged.err!==0||forged.entries!==0||forged.status!==-1||forged.inputStatus!==0)throw Error(JSON.stringify(forged));const hostile=await run(false,true);if(hostile.value!==0||hostile.len!==0||hostile.err!==0||hostile.dirty||hostile.entries!==0||hostile.status!==-1||hostile.inputStatus!==0)throw Error(JSON.stringify(hostile));console.log('command-io-node-ok');
"#,
            symbol = crate::diagnostic::quote_json(&symbol),
        );
        let script = script
            .replace(
                "spx_command_stdin_read_v1:p=>{if(fail)return 3;const c=fixed?11n:oversized?BigInt.asIntN(64,(0x80000001n<<32n)|65537n):alloc(new TextEncoder().encode('owned-stdin'));new DataView(instance.exports.memory.buffer).setBigInt64(p,c,true);return 0}",
                "spx_command_stdin_read_v1:p=>{if(fail)return 3;let c;if(fixed)c=11n;else if(oversized)c=BigInt.asIntN(64,(0x80000001n<<32n)|65537n);else if(forge==='missing')c=BigInt.asIntN(64,(0x80000063n<<32n)|1n);else if(forge==='zero')c=alloc(new Uint8Array());else{c=alloc(new TextEncoder().encode('owned-stdin'));if(forge==='wrong')c-=1n}new DataView(instance.exports.memory.buffer).setBigInt64(p,c,true);return 0},spx_command_owned_bytes_validate_v1:c=>{try{const d=decode(c);if(!d.t||d.k===0)return 1;const b=entries.get(d.k);if(!b)return 1;if(b.length!==d.n){entries.delete(d.k);return 1}return 0}catch{return 1}}",
            )
            .replace(
                "async function run(fail,hostile=false,fixed=false,oversized=false){",
                "async function run(fail,hostile=false,fixed=false,oversized=false,forge=''){",
            )
            .replace(
                "console.log('command-io-node-ok');",
                "for(const kind of ['missing','wrong']){const forged=await run(false,false,false,false,kind);if(forged.len!==0||forged.err!==0||forged.entries!==0||forged.status!==-1)throw Error(kind+JSON.stringify(forged))}const empty=await run(false,false,false,false,'zero');if(empty.value!==1||empty.len!==0||empty.err!==0||empty.entries!==0||empty.status!==0||empty.dirty)throw Error('zero'+JSON.stringify(empty));console.log('command-io-node-ok');",
            )
            .replace(
                "dirty:out.slice(131072,393216).some(value=>value!==0)}",
                "dirty:out.slice(131072,393216).some(value=>value!==0),privateDirty:out.slice(262144,393216).some(value=>value!==0)}",
            )
            .replace(
                "if(ok.value!==1||ok.text!=='owned-stdin'||ok.err!==0||ok.entries!==0||ok.status!==0)",
                "if(ok.value!==1||ok.text!=='owned-stdin'||ok.err!==0||ok.entries!==0||ok.status!==0||ok.privateDirty)",
            );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_file(script_path).unwrap();
        std::fs::remove_file(wasm_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn node_arg_utf8_accepts_only_the_closed_zero_one_two_status_domain() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let source = r#"
module test.command_arg_status;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_arg_status.run")
fn run() -> bool uses { process.args.read } {
    let argument = arg_utf8(0usize);
    byte_len(str_as_bytes(argument)) == 0usize
}
@id("main")
fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, Path::new("command-arg-status-wasm.spx")).unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let wasm = super::super::emit_resolved_language_command_io_v1(
            &program,
            "test.command_arg_status.run",
        )
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "semaprax-command-arg-status-node-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let wasm_path = directory.join("app.wasm");
        let script_path = directory.join("test.mjs");
        std::fs::write(&wasm_path, wasm).unwrap();
        let symbol = super::super::data_exports::raw_symbol("test.command_arg_status.run");
        let script = format!(
            r#"import fs from 'node:fs';
const bytes=fs.readFileSync(process.argv[2]);
async function run(providerStatus){{let instance=null;const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},spx_bytes_copy:c=>c,spx_bytes_get:()=>-1,spx_bytes_drop:()=>{{}},spx_bytes_as_slice:c=>c,spx_command_args_len_v1:()=>1n,spx_command_arg_utf8_v1:(i,p)=>{{if(providerStatus===0)new DataView(instance.exports.memory.buffer).setBigInt64(p,0n,true);return providerStatus}},spx_command_stdin_read_v1:()=>3,spx_command_owned_bytes_validate_v1:()=>1}};const result=await WebAssembly.instantiate(bytes,{{env}});instance=result.instance;const value=instance.exports[{symbol}]();return{{value,status:Number(instance.exports.__spx_data_status_v1.value),inputStatus:Number(instance.exports.__spx_command_input_status_v1.value)}}}}
for(const [providerStatus,expected,marker] of [[0,0,0],[1,1,1],[2,2,2],[3,-1,0],[-1,-1,0]]){{const actual=await run(providerStatus);if(actual.status!==expected||actual.inputStatus!==marker||(providerStatus===0&&actual.value!==1))throw Error(JSON.stringify({{providerStatus,actual}}))}}
console.log('command-arg-status-ok');
"#,
            symbol = crate::diagnostic::quote_json(&symbol),
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_file(script_path).unwrap();
        std::fs::remove_file(wasm_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
