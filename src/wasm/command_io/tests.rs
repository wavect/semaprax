use std::path::Path;
use std::process::Command;

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn line_command_output_marker_is_additive_to_frozen_v6_modules() {
    let legacy = r#"
module test.command_legacy;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_legacy.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view) == byte_len(view)
}
@id("main") fn main() -> i64 { 0 }
"#;
    let line = legacy
            .replace("test.command_legacy", "test.command_line")
            .replace(
                "stdout_write(view) == byte_len(view)",
                "let full = byte_range(view, 0usize, byte_len(view));\n    stdout_append(full) == byte_len(full)",
            );
    let legacy_program =
        crate::hir::resolve(&crate::parse(legacy, Path::new("command-legacy-wasm.spx")).unwrap())
            .unwrap();
    let legacy_wasm = super::super::emit_resolved_language_command_io_v1(
        &legacy_program,
        "test.command_legacy.run",
    )
    .unwrap();
    assert!(!contains_bytes(
        &legacy_wasm,
        super::super::line_command_io::OUTPUT_STATUS_EXPORT.as_bytes()
    ));

    let line_program =
        crate::hir::resolve(&crate::parse(&line, Path::new("command-line-wasm.spx")).unwrap())
            .unwrap();
    let v6_error =
        super::super::emit_resolved_language_command_io_v1(&line_program, "test.command_line.run")
            .expect_err("the frozen v6 Wasm boundary must reject v7 operations");
    assert!(
        v6_error.message.contains("cannot reach byte_range"),
        "{v6_error:?}"
    );
    let line_wasm =
        super::super::emit_resolved_line_command_io_v1(&line_program, "test.command_line.run")
            .unwrap();
    assert!(contains_bytes(
        &line_wasm,
        super::super::line_command_io::OUTPUT_STATUS_EXPORT.as_bytes()
    ));
    assert!(contains_bytes(
        &line_wasm,
        super::INPUT_STATUS_EXPORT.as_bytes()
    ));
}

#[test]
fn node_nested_byte_ranges_flatten_and_append_exact_bytes() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.command_range;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_range.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let root = bytes_as_slice(input);
    let first = byte_range(root, 1usize, 5usize);
    let second = byte_range(first, 1usize, 3usize);
    stdout_append(second) == 2usize
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program =
        crate::hir::resolve(&crate::parse(source, Path::new("command-range-wasm.spx")).unwrap())
            .unwrap();
    let wasm =
        super::super::emit_resolved_line_command_io_v1(&program, "test.command_range.run").unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-range-node-{}-{}",
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
    let symbol = super::super::data_exports::raw_symbol("test.command_range.run");
    let script = format!(
        r#"import fs from 'node:fs';
const bytes=fs.readFileSync(process.argv[2]);let instance=null,next=1;const entries=new Map();
const word=c=>BigInt.asUintN(64,c);const raw=c=>{{const w=word(c);return{{w,n:Number(w&0xffffffffn),h:Number((w>>32n)&0xffffffffn)}}}};
const read=c=>{{const d=raw(c);if((d.h&0xc0000000)===0x40000000){{const p=(d.h&0xffff)*8,k=(d.h>>>16)&0x1fff,v=new DataView(instance.exports.memory.buffer);if(p+32>v.byteLength||v.getUint32(p,true)!==k||v.getUint32(p+4,true)!==p||Number(v.getBigUint64(p+24,true))!==d.n)throw Error('descriptor');const root=v.getBigInt64(p+8,true),off=Number(v.getBigUint64(p+16,true)),all=read(root);if(off>all.length||d.n>all.length-off)throw Error('range');return all.slice(off,off+d.n)}}if((d.h&0x80000000)!==0){{const b=entries.get(d.h&0x7fffffff);if(!b||b.length!==d.n)throw Error('token');return b}}const memory=new Uint8Array(instance.exports.memory.buffer);if(d.h>memory.length-d.n)throw Error('fixed');return memory.slice(d.h,d.h+d.n)}};
const alloc=b=>{{const k=next++;entries.set(k,new Uint8Array(b));return BigInt.asIntN(64,((0x80000000n|BigInt(k))<<32n)|BigInt(b.length))}};
const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},spx_bytes_copy:c=>alloc(read(c)),spx_bytes_get:(c,i)=>{{const b=read(c),n=Number(i);return n<b.length?b[n]:-1}},spx_bytes_drop:c=>{{const d=raw(c);if((d.h&0x80000000)===0||!entries.delete(d.h&0x7fffffff))throw Error('drop')}},spx_bytes_as_slice:c=>{{read(c);return c}},spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:p=>{{const c=alloc(new TextEncoder().encode('abcdef'));new DataView(instance.exports.memory.buffer).setBigInt64(p,c,true);return 0}},spx_command_owned_bytes_validate_v1:c=>{{try{{const d=raw(c),b=entries.get(d.h&0x7fffffff);return (d.h&0x80000000)!==0&&b&&b.length===d.n?0:1}}catch{{return 1}}}}}};
const result=await WebAssembly.instantiate(bytes,{{env}});instance=result.instance;const value=instance.exports[{symbol}](),memory=new Uint8Array(instance.exports.memory.buffer),len=Number(instance.exports.__spx_stdout_length_v1.value),text=new TextDecoder().decode(memory.slice(131072,131072+len));if(value!==1||text!=='cd'||len!==2||Number(instance.exports.__spx_data_status_v1.value)!==0||Number(instance.exports.__spx_command_input_status_v1.value)!==0||Number(instance.exports.__spx_command_output_status_v1.value)!==0||entries.size!==0)throw Error(JSON.stringify({{value,text,len,entries:entries.size}}));console.log('command-range-ok');"#,
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

#[test]
fn node_range_binding_rejects_same_length_root_and_offset_substitution() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.command_range_binding;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_range_binding.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let root = bytes_as_slice(input);
    let selected = byte_range(root, 0usize, 2usize);
    let first = stdout_append(selected);
    stdout_append(selected) == first
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program = crate::hir::resolve(
        &crate::parse(source, Path::new("command-range-binding-wasm.spx")).unwrap(),
    )
    .unwrap();
    let wasm =
        super::super::emit_resolved_line_command_io_v1(&program, "test.command_range_binding.run")
            .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-range-binding-node-{}-{}",
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
    let symbol = super::super::data_exports::raw_symbol("test.command_range_binding.run");
    let script = format!(
        r#"import fs from 'node:fs';const bytes=fs.readFileSync(process.argv[2]);
async function run(attack){{let instance=null,mutated=false;const a=new TextEncoder().encode('abcd'),b=new TextEncoder().encode('wxyz'),entries=new Map([[1,a],[2,b]]),one=BigInt.asIntN(64,(0x80000001n<<32n)|4n),two=BigInt.asIntN(64,(0x80000002n<<32n)|4n);const raw=c=>{{const w=BigInt.asUintN(64,c);return{{n:Number(w&0xffffffffn),h:Number((w>>32n)&0xffffffffn)}}}};const read=c=>{{const d=raw(c);if(((d.h&0xc0000000)>>>0)===0x40000000){{const p=(d.h&0xffff)*8,v=new DataView(instance.exports.memory.buffer),root=v.getBigInt64(p+8,true),off=Number(v.getBigUint64(p+16,true)),all=read(root);return all.subarray(off,off+d.n)}}const value=entries.get(d.h&0x7fffffff);if(!value||value.length!==d.n)throw Error('root');return value}};const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},spx_bytes_copy:c=>c,spx_bytes_get:(c,i)=>{{const d=raw(c),n=Number(i),value=read(c);if(attack&&!mutated&&n===d.n-1){{const p=(d.h&0xffff)*8,v=new DataView(instance.exports.memory.buffer);if(attack==='root')v.setBigInt64(p+8,two,true);else v.setBigUint64(p+16,v.getBigUint64(p+16,true)+1n,true);mutated=true}}return n<value.length?value[n]:-1}},spx_bytes_drop:c=>{{const d=raw(c);if(!entries.delete(d.h&0x7fffffff))throw Error('drop')}},spx_bytes_as_slice:c=>c,spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:p=>{{new DataView(instance.exports.memory.buffer).setBigInt64(p,one,true);return 0}},spx_command_owned_bytes_validate_v1:()=>0}};const result=await WebAssembly.instantiate(bytes,{{env}});instance=result.instance;return instance.exports[{symbol}]()}}
if(await run('')!==1)throw Error('control');for(const attack of ['root','offset']){{let rejected=false;try{{await run(attack)}}catch{{rejected=true}}if(!rejected)throw Error(attack+' substitution')}}console.log('range-binding-ok');"#,
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

#[test]
fn line_command_rejects_recursive_range_binding_reentry() {
    let source = r#"
module test.command_range_recursion;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_range_recursion.run")
fn run() -> bool uses { process.args.read, process.stdin.read, process.stdout.write } {
    if args_len() == 0usize {
        let input = stdin_read();
        let view = bytes_as_slice(input);
        stdout_append(byte_range(view, 0usize, byte_len(view))) == byte_len(view)
    } else {
        run()
    }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let errors = crate::hir::resolve(
        &crate::parse(source, Path::new("command-range-recursion-wasm.spx")).unwrap(),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-T267" && error.message.contains("cyclic")),
        "{errors:?}"
    );
}

#[test]
fn node_append_capacity_is_cumulative_atomic_and_domain_marked() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.command_capacity;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.command_capacity.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write, process.stderr.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    let selected = byte_range(view, 0usize, byte_len(view));
    let first = stdout_append(view);
    let second = stderr_append(view);
    first == second
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program =
        crate::hir::resolve(&crate::parse(source, Path::new("command-capacity-wasm.spx")).unwrap())
            .unwrap();
    let wasm =
        super::super::emit_resolved_line_command_io_v1(&program, "test.command_capacity.run")
            .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-command-capacity-node-{}-{}",
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
    let symbol = super::super::data_exports::raw_symbol("test.command_capacity.run");
    let script = format!(
        r#"import fs from 'node:fs';const bytes=fs.readFileSync(process.argv[2]);let instance=null;const data=new Uint8Array(40000).fill(97),entries=new Map([[1,data]]);const carrier=BigInt.asIntN(64,(0x80000001n<<32n)|40000n);const decode=c=>{{const w=BigInt.asUintN(64,c);return{{n:Number(w&0xffffffffn),h:Number((w>>32n)&0xffffffffn)}}}};const read=c=>{{const d=decode(c),b=entries.get(d.h&0x7fffffff);if(!b||b.length!==d.n)throw Error('token');return b}};const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error('contract')}},spx_bytes_copy:c=>c,spx_bytes_get:(c,i)=>{{const b=read(c),n=Number(i);return n<b.length?b[n]:-1}},spx_bytes_drop:c=>{{const d=decode(c);if(!entries.delete(d.h&0x7fffffff))throw Error('drop')}},spx_bytes_as_slice:c=>{{read(c);return c}},spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:p=>{{new DataView(instance.exports.memory.buffer).setBigInt64(p,carrier,true);return 0}},spx_command_owned_bytes_validate_v1:()=>0}};const result=await WebAssembly.instantiate(bytes,{{env}});instance=result.instance;const value=instance.exports[{symbol}](),memory=new Uint8Array(instance.exports.memory.buffer),actual={{value,status:Number(instance.exports.__spx_data_status_v1.value),input:Number(instance.exports.__spx_command_input_status_v1.value),output:Number(instance.exports.__spx_command_output_status_v1.value),stdout:Number(instance.exports.__spx_stdout_length_v1.value),stderr:Number(instance.exports.__spx_stderr_length_v1.value),entries:entries.size,dirty:memory.slice(131072,393216).some(x=>x!==0)}};if(actual.value!==0||actual.status!==1||actual.input!==0||actual.output!==1||actual.stdout!==0||actual.stderr!==0||actual.entries!==0||actual.dirty)throw Error(JSON.stringify(actual));console.log('command-capacity-ok');"#,
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
    let wasm =
        super::super::emit_resolved_language_command_io_v1(&program, "test.command.run").unwrap();
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
    let wasm =
        super::super::emit_resolved_language_command_io_v1(&program, "test.command_arg_status.run")
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
