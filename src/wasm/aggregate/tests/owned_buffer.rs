use super::*;

use sha2::{Digest, Sha256};

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SUCCESS: &str = r#"
module test.wasm_owned_buffer;

@id("buffer.run")
fn run() -> i64 {
    let buffer = bytes_set(bytes_set(bytes_set(bytes_zeroed(3usize), 0usize, 65u8), 1usize, 66u8), 2usize, 67u8);
    let view = bytes_as_slice(buffer);
    let first = match byte_get(view, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let middle = match byte_get(view, 1usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let last = match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    if byte_len(view) == 3usize && first == 65u8 && middle == 66u8 && last == 67u8 { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { run() }
"#;

fn owned_buffer_host_probe_module() -> Vec<u8> {
    let signatures = [
        Signature {
            params: vec![0x7e],
            results: vec![0x7e],
        },
        Signature {
            params: vec![0x7e, 0x7e, 0x7f],
            results: vec![0x7e],
        },
        Signature {
            params: vec![0x7e],
            results: Vec::new(),
        },
        Signature {
            params: vec![0x7e, 0x7e],
            results: vec![0x7f],
        },
    ];
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut types = Vec::new();
    write_u32(&mut types, signatures.len() as u32);
    for signature in &signatures {
        types.push(0x60);
        write_u32(&mut types, signature.params.len() as u32);
        types.extend_from_slice(&signature.params);
        write_u32(&mut types, signature.results.len() as u32);
        types.extend_from_slice(&signature.results);
    }
    section(&mut module, 1, types);

    let mut imports = Vec::new();
    write_u32(&mut imports, 4);
    function_import(&mut imports, "env", "spx_bytes_zeroed", 0);
    function_import(&mut imports, "env", "spx_bytes_set", 1);
    function_import(&mut imports, "env", "spx_bytes_drop", 2);
    function_import(&mut imports, "env", "spx_bytes_get", 3);
    section(&mut module, 2, imports);

    let mut memories = Vec::new();
    write_u32(&mut memories, 1);
    memories.push(0x01);
    write_u32(&mut memories, 2);
    write_u32(&mut memories, 2);
    section(&mut module, 5, memories);

    let mut exports = Vec::new();
    write_u32(&mut exports, 5);
    for (name, index) in [
        ("probe_zeroed", 0),
        ("probe_set", 1),
        ("probe_drop", 2),
        ("probe_get", 3),
    ] {
        write_name(&mut exports, name);
        exports.push(0x00);
        write_u32(&mut exports, index);
    }
    write_name(&mut exports, "__spx_byte_memory");
    exports.push(0x02);
    write_u32(&mut exports, 0);
    section(&mut module, 7, exports);
    module
}

#[test]
fn owned_buffer_host_boundary_rejects_forged_inputs_without_poisoning_reentry() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let bytes = owned_buffer_host_probe_module();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "semaprax-owned-buffer-hostile-{}-{serial}",
        std::process::id()
    )));
    std::fs::create_dir(&fixture.0).unwrap();
    let digest = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes)));
    let runtime = crate::wasm::browser_runtime()
        .replace("__SEMAPRAX_OWNED_EXPORTS__", "Object.freeze({})")
        .replace("__SEMAPRAX_WASM_SHA256__", &digest);
    std::fs::write(fixture.0.join("runtime.mjs"), runtime).unwrap();
    std::fs::write(fixture.0.join("probe.wasm"), bytes).unwrap();
    std::fs::write(
        fixture.0.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './runtime.mjs';
const {instance}=await instantiateBytes(await readFile('./probe.wasm'),{maxOwnedByteEntries:1});
const {probe_zeroed:zeroed,probe_set:set,probe_drop:drop,probe_get:get}=instance.exports;
const reject=(action,message)=>{let failed=false;try{action()}catch(error){if(error.message!==message)throw error;failed=true}if(!failed)throw Error(`accepted hostile input: ${message}`)};
reject(()=>zeroed(-1n),'SEMAPRAX owned byte buffer capacity invariant');
reject(()=>zeroed(65537n),'SEMAPRAX owned byte buffer capacity invariant');
let wrongType=false;try{zeroed(1)}catch(error){if(!(error instanceof TypeError))throw error;wrongType=true}if(!wrongType)throw Error('accepted non-i64 capacity');
let carrier=zeroed(1n);
if(get(carrier,0n)!==0)throw Error('zeroed byte mismatch');
reject(()=>set(1n,0n,7),'SEMAPRAX owned Bytes token invariant');
const malformed=BigInt.asIntN(64,(BigInt.asUintN(64,carrier)&~0xffffffffn)|2n);
reject(()=>set(malformed,0n,7),'SEMAPRAX stale or malformed owned Bytes carrier');
reject(()=>set(carrier,-1n,7),'SEMAPRAX owned byte buffer element invariant');
reject(()=>set(carrier,1n,7),'SEMAPRAX owned byte buffer element invariant');
reject(()=>set(carrier,0n,-1),'SEMAPRAX owned byte buffer element invariant');
reject(()=>set(carrier,0n,256),'SEMAPRAX owned byte buffer element invariant');
if(get(carrier,0n)!==0)throw Error('failed store mutated the buffer');
if(set(carrier,0n,9)!==carrier||get(carrier,0n)!==9)throw Error('valid store changed owner or value');
drop(carrier);
reject(()=>set(carrier,0n,1),'SEMAPRAX stale or malformed owned Bytes carrier');
for(let round=0;round<4;++round){carrier=zeroed(1n);if(set(carrier,0n,round)!==carrier||get(carrier,0n)!==round)throw Error('reentry');drop(carrier)}
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A computed element index. The offsets come from a call the compiler cannot
/// fold, so the bound is a run-time check in generated code rather than a
/// compile-time fact.
const COMPUTED: &str = r#"
module test.wasm_owned_buffer_computed;

@id("buffer.offset")
fn offset(base: usize) -> usize { base + 1usize }

@id("buffer.run")
fn run() -> i64 {
    let buffer = bytes_set(bytes_set(bytes_zeroed(3usize), offset(0usize), 66u8), offset(1usize), 67u8);
    let view = bytes_as_slice(buffer);
    let first = match byte_get(view, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 1u8,
    };
    let second = match byte_get(view, 1usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let third = match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    if byte_len(view) == 3usize && first == 0u8 && second == 66u8 && third == 67u8 { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { run() }
"#;

/// The same chain with one computed index one past the capacity. Generated
/// code selects the failure before the owner transfer commits, so the arena
/// entry is released by the canonical cleanup and a one-entry arena still
/// balances across repeated invocations.
const COMPUTED_OUT_OF_RANGE: &str = r#"
module test.wasm_owned_buffer_past_end;

@id("buffer.offset")
fn offset(base: usize) -> usize { base + 1usize }

@id("buffer.run")
fn run() -> i64 {
    let buffer = bytes_set(bytes_set(bytes_zeroed(3usize), offset(0usize), 66u8), offset(2usize), 67u8);
    let view = bytes_as_slice(buffer);
    if byte_len(view) == 3usize { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { run() }
"#;

/// The loop-carried fill: the buffer is allocated once outside one bounded
/// `while` and the body republishes that same binding from
/// `bytes_set(binding, index, value)`. Every iteration mutates the one arena
/// entry in place and hands back the same carrier, so a one-entry arena stays
/// balanced across repeated invocations.
const LOOP_FILL: &str = r#"
module test.wasm_owned_buffer_loop;

@id("buffer.run")
fn run() -> i64 {
    let mut buffer = bytes_zeroed(3usize);
    let mut index = 0usize;
    let mut value = 65u8;
    while index < 3usize {
        buffer = bytes_set(buffer, index, value);
        index = index + 1usize;
        value = value + 1u8;
        0
    }
    let view = bytes_as_slice(buffer);
    let first = match byte_get(view, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let last = match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    if byte_len(view) == 3usize && first == 65u8 && last == 67u8 { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { run() }
"#;

/// The same loop run one iteration past the capacity. The store that leaves the
/// buffer selects the element-bound failure before the owner transfer commits,
/// so the arena entry is released exactly once by the canonical cleanup and a
/// one-entry arena still balances on every repeat.
const LOOP_PAST_END: &str = r#"
module test.wasm_owned_buffer_loop_past_end;

@id("buffer.run")
fn run() -> i64 {
    let mut buffer = bytes_zeroed(3usize);
    let mut index = 0usize;
    while index < 4usize {
        buffer = bytes_set(buffer, index, 65u8);
        index = index + 1usize;
        0
    }
    let view = bytes_as_slice(buffer);
    if byte_len(view) == 3usize { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { run() }
"#;

/// Issue #63's decoded-string output buffer. The write cursor advances
/// independently of the read cursor, so the element index is a computed `usize`
/// that is not the loop counter, and the one arena entry is mutated in place on
/// every iteration. The escaped input `a\nb` is produced by a pure function of
/// the read offset rather than an array literal, so this case keeps asserting
/// that the *buffer* lowering neither copies nor grows linear memory.
const DECODED_STRING_BUFFER: &str = r#"
module test.wasm_owned_buffer_decode;

@id("buffer.source")
fn source(offset: usize) -> u8 {
    if offset == 0usize { 97u8 }
    else { if offset == 1usize { 92u8 }
    else { if offset == 2usize { 110u8 } else { 98u8 } } }
}

@id("buffer.decode")
fn decode() -> i64 {
    let mut out = bytes_zeroed(8usize);
    let mut read = 0usize;
    let mut write = 0usize;
    while read < 4usize {
        let raw = source(read);
        if raw == 92u8 {
            let next = source(read + 1usize);
            let decoded = if next == 110u8 { 10u8 } else { next };
            out = bytes_set(out, write, decoded);
            read = read + 2usize;
            write = write + 1usize;
            0
        } else {
            out = bytes_set(out, write, raw);
            read = read + 1usize;
            write = write + 1usize;
            0
        }
    }
    let view = bytes_as_slice(out);
    let first = match byte_get(view, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let second = match byte_get(view, 1usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let third = match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    if byte_len(view) == 8usize && write == 3usize && first == 97u8 && second == 10u8 && third == 98u8 { 7 } else { 1 }
}

@id("app.main")
fn main() -> i64 { decode() }
"#;

const FAILURE: &str = r#"
module test.wasm_owned_buffer_failure;

@id("buffer.fail")
fn fail(value: i64) -> i64
requires value > 0
{ value }

@id("app.main")
fn main() -> i64 {
    let buffer = bytes_set(bytes_zeroed(1usize), 0usize, 9u8);
    let view = bytes_as_slice(buffer);
    let failed = fail(0);
    if byte_len(view) == 1usize { failed } else { 0 }
}
"#;

#[test]
fn owned_bounded_byte_buffer_executes_and_reenters_without_memory_copy() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }

    const CONTRACT_FAILURE: &str = "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.contract.v1'||status.code!==1||error.message!=='SEMAPRAX contract failure')throw error;failed=true;}if(!failed)throw Error('missing owned buffer failure');";
    // Generated code, not the host import, selects the element-bound failure.
    // Repeating it against a one-entry arena proves the failed store released
    // the buffer exactly once through the canonical cleanup plan.
    const BOUND_FAILURE: &str = "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.byte-buffer.v1'||status.code!==1)throw error;failed=true;}if(!failed)throw Error('missing owned buffer element-bound failure');";
    const RETURNS_SEVEN: &str =
        "if(instance.exports.semaprax_main()!==7n)throw Error('wrong owned buffer value');";

    for (label, source, expectation) in [
        ("success", SUCCESS, RETURNS_SEVEN),
        ("computed", COMPUTED, RETURNS_SEVEN),
        ("computed-past-end", COMPUTED_OUT_OF_RANGE, BOUND_FAILURE),
        ("loop-fill", LOOP_FILL, RETURNS_SEVEN),
        ("loop-past-end", LOOP_PAST_END, BOUND_FAILURE),
        ("decoded-string", DECODED_STRING_BUFFER, RETURNS_SEVEN),
        ("failure", FAILURE, CONTRACT_FAILURE),
    ] {
        let parsed = parse(source, Path::new("wasm-owned-buffer-v1.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        hir::validate(&resolved).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
        wasmparser::Validator::new().validate_all(&bytes).unwrap();

        let mut zeroed_imports = 0;
        let mut set_imports = 0;
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                wasmparser::Payload::ImportSection(section) => {
                    for import in section.into_imports() {
                        let import = import.unwrap();
                        zeroed_imports += usize::from(
                            import.module == "env" && import.name == "spx_bytes_zeroed",
                        );
                        set_imports +=
                            usize::from(import.module == "env" && import.name == "spx_bytes_set");
                    }
                }
                wasmparser::Payload::CodeSectionEntry(body) => {
                    let mut operators = body.get_operators_reader().unwrap();
                    while !operators.eof() {
                        assert!(
                            !matches!(
                                operators.read().unwrap(),
                                wasmparser::Operator::MemoryCopy { .. }
                                    | wasmparser::Operator::MemoryGrow { .. }
                            ),
                            "owned buffer lowering must neither copy nor grow linear memory"
                        );
                    }
                }
                _ => {}
            }
        }
        assert_eq!(zeroed_imports, 1);
        assert_eq!(set_imports, 1);

        let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "semaprax-owned-buffer-wasm-{}-{serial}-{label}",
            std::process::id()
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        let digest = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes)));
        let runtime = crate::wasm::browser_runtime()
            .replace("__SEMAPRAX_OWNED_EXPORTS__", "Object.freeze({})")
            .replace("__SEMAPRAX_WASM_SHA256__", &digest);
        std::fs::write(fixture.0.join("runtime.mjs"), runtime).unwrap();
        std::fs::write(fixture.0.join("app.wasm"), bytes).unwrap();
        std::fs::write(
            fixture.0.join("probe.mjs"),
            format!(
                r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './runtime.mjs';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:1}});
for(let round=0;round<4;++round){{{expectation}}}
"#
            ),
        )
        .unwrap();
        let output = Command::new("node")
            .arg("probe.mjs")
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{label}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
