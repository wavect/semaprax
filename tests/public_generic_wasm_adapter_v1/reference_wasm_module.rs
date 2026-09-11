//! Assembles one genuine, minimal `.wasm` binary module by hand: a real
//! WebAssembly Core module (magic, version, and the type/function/memory/
//! export/code sections the binary format spec requires), never a WAT text
//! file handed to an external tool. No `wat2wasm`/`wasm-tools`/`wat` crate
//! is available in this environment (checked: none on `PATH`, none in
//! `Cargo.lock`), and this test harness may not add a new Cargo dependency
//! (`Cargo.toml` is coordinator-owned), so this module builds the bytes
//! itself, section by section, rather than compiling text.
//!
//! The module exports exactly two things, matching what
//! `typescript_calling`'s generated `wasm-provider.ts` requires and nothing
//! more ("Do not access private allocator exports or internal record
//! offsets" -- there are none to access):
//!
//! - `memory`: one growable linear memory (initial 1 page, max 256 pages --
//!   the same 16 MiB bound `boundary_profile.rs::MAX_TOTAL_PAYLOAD_BYTES`
//!   states).
//! - the function named by
//!   `public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME`
//!   (`"spx_pg_wasm_endpoint_reverse_bytes_v1"`), a real, compiled Wasm
//!   function of signature `(i32 ptr, i32 len) -> ()` that reverses `len`
//!   bytes in place starting at byte offset `ptr` of `memory` -- an
//!   in-bounds two-pointer swap loop, real Wasm bytecode, not a JS
//!   function pretending to be one.
//!
//! This is a TEST-ONLY fixture standing in for a genuinely compiled Wasm
//! artifact of issue #155's Core Wasm physical adapter, which does not
//! exist (see `typescript_calling.rs`'s own module documentation for the
//! full accounting). It implements only the one endpoint issue #155
//! already names and documents as a real Wasm export
//! (`FIXTURE_ENDPOINT_EXPORT_NAME`); it never reimplements
//! `WasmProvider`'s allocator, handle registry, or carrier/lifecycle state
//! machine -- those stay host-side, in the generated `wasm-provider.ts`
//! this harness exercises against this fixture.

/// LEB128-encode an unsigned integer.
fn uleb128(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

/// LEB128-encode a signed integer (used for small non-negative `i32.const`
/// operands in this fixture; correct in general, not merely for small
/// values).
fn sleb128(mut value: i64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        let sign_bit_set = byte & 0x40 != 0;
        let done = (value == 0 && !sign_bit_set) || (value == -1 && sign_bit_set);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
    out
}

fn section(id: u8, contents: Vec<u8>) -> Vec<u8> {
    let mut out = vec![id];
    out.extend(uleb128(contents.len() as u64));
    out.extend(contents);
    out
}

fn vector(items: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = uleb128(items.len() as u64);
    for item in items {
        out.extend(item);
    }
    out
}

fn name_bytes(name: &str) -> Vec<u8> {
    let mut out = uleb128(name.len() as u64);
    out.extend_from_slice(name.as_bytes());
    out
}

// Core instruction opcodes (WebAssembly 1.0 / MVP).
const OP_END: u8 = 0x0b;
const OP_BLOCK: u8 = 0x02;
const OP_LOOP: u8 = 0x03;
const OP_BR: u8 = 0x0c;
const OP_BR_IF: u8 = 0x0d;
const OP_LOCAL_GET: u8 = 0x20;
const OP_LOCAL_SET: u8 = 0x21;
const OP_I32_LOAD8_U: u8 = 0x2d;
const OP_I32_STORE8: u8 = 0x3a;
const OP_I32_CONST: u8 = 0x41;
const OP_I32_GE_S: u8 = 0x4e;
const OP_I32_ADD: u8 = 0x6a;
const OP_I32_SUB: u8 = 0x6b;
const BLOCKTYPE_EMPTY: u8 = 0x40;
const VALTYPE_I32: u8 = 0x7f;

fn local_get(index: u32) -> Vec<u8> {
    let mut out = vec![OP_LOCAL_GET];
    out.extend(uleb128(index as u64));
    out
}

fn local_set(index: u32) -> Vec<u8> {
    let mut out = vec![OP_LOCAL_SET];
    out.extend(uleb128(index as u64));
    out
}

fn i32_const(value: i32) -> Vec<u8> {
    let mut out = vec![OP_I32_CONST];
    out.extend(sleb128(value as i64));
    out
}

fn i32_load8_u() -> Vec<u8> {
    // memarg: align=0, offset=0.
    vec![OP_I32_LOAD8_U, 0x00, 0x00]
}

fn i32_store8() -> Vec<u8> {
    vec![OP_I32_STORE8, 0x00, 0x00]
}

/// Build the function body for
/// `spx_pg_wasm_endpoint_reverse_bytes_v1(ptr: i32, len: i32)`: a real
/// in-place two-pointer byte-reversal loop over linear memory. Locals:
/// 0=ptr (param), 1=len (param), 2=i, 3=j, 4=tmp.
fn reverse_bytes_function_body() -> Vec<u8> {
    let mut code = Vec::new();

    // i = ptr
    code.extend(local_get(0));
    code.extend(local_set(2));
    // j = ptr + len - 1
    code.extend(local_get(0));
    code.extend(local_get(1));
    code.push(OP_I32_ADD);
    code.extend(i32_const(1));
    code.push(OP_I32_SUB);
    code.extend(local_set(3));

    // block
    code.push(OP_BLOCK);
    code.push(BLOCKTYPE_EMPTY);
    //   loop
    code.push(OP_LOOP);
    code.push(BLOCKTYPE_EMPTY);
    //     if i >= j, branch out of the block (label 1)
    code.extend(local_get(2));
    code.extend(local_get(3));
    code.push(OP_I32_GE_S);
    code.push(OP_BR_IF);
    code.extend(uleb128(1));
    //     tmp = mem[i]
    code.extend(local_get(2));
    code.extend(i32_load8_u());
    code.extend(local_set(4));
    //     mem[i] = mem[j]  (push address=i, then value=mem[j], then store)
    code.extend(local_get(2));
    code.extend(local_get(3));
    code.extend(i32_load8_u());
    code.extend(i32_store8());
    //     mem[j] = tmp
    code.extend(local_get(3));
    code.extend(local_get(4));
    code.extend(i32_store8());
    //     i = i + 1
    code.extend(local_get(2));
    code.extend(i32_const(1));
    code.push(OP_I32_ADD);
    code.extend(local_set(2));
    //     j = j - 1
    code.extend(local_get(3));
    code.extend(i32_const(1));
    code.push(OP_I32_SUB);
    code.extend(local_set(3));
    //     continue loop (label 0)
    code.push(OP_BR);
    code.extend(uleb128(0));
    code.push(OP_END); // end loop
    code.push(OP_END); // end block
    code.push(OP_END); // end function

    // Locals declaration: one group of 3 i32 locals (i, j, tmp), then the
    // body, matching the binary format's `func` production.
    let mut body = vector(vec![{
        let mut group = uleb128(3);
        group.push(VALTYPE_I32);
        group
    }]);
    body.extend(code);

    let mut entry = uleb128(body.len() as u64);
    entry.extend(body);
    entry
}

/// The one endpoint export name issue #155 already documents as real
/// (`FIXTURE_ENDPOINT_EXPORT_NAME`), restated as a plain string literal
/// here so this test-only harness module does not need to pull in the
/// `semaprax` lib crate's constant merely to spell it (both are asserted
/// equal by `typescript_calling_consumer.rs`, which does import the real
/// constant).
pub const ENDPOINT_EXPORT_NAME: &str = "spx_pg_wasm_endpoint_reverse_bytes_v1";

/// Assemble the complete `.wasm` binary. Deterministic: always the same
/// bytes.
pub fn build() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(b"\0asm");
    module.extend_from_slice(&1u32.to_le_bytes());

    // Type section: one function type, (i32, i32) -> ().
    let functype = {
        let mut out = vec![0x60u8];
        out.extend(vector(vec![vec![VALTYPE_I32], vec![VALTYPE_I32]]));
        out.extend(uleb128(0)); // zero results
        out
    };
    module.extend(section(1, vector(vec![functype])));

    // Function section: one function using type index 0.
    module.extend(section(3, vector(vec![uleb128(0)])));

    // Memory section: one memory, min=1 page, max=256 pages (16 MiB).
    let memory_limits = {
        let mut out = vec![0x01u8]; // flags: has-max
        out.extend(uleb128(1));
        out.extend(uleb128(256));
        out
    };
    module.extend(section(5, vector(vec![memory_limits])));

    // Export section: "memory" (memory index 0), then the endpoint
    // function (function index 0).
    let export_memory = {
        let mut out = name_bytes("memory");
        out.push(0x02); // memory export kind
        out.extend(uleb128(0));
        out
    };
    let export_function = {
        let mut out = name_bytes(ENDPOINT_EXPORT_NAME);
        out.push(0x00); // function export kind
        out.extend(uleb128(0));
        out
    };
    module.extend(section(7, vector(vec![export_memory, export_function])));

    // Code section: the one function body.
    module.extend(section(10, vector(vec![reverse_bytes_function_body()])));

    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_well_formed_module_header() {
        let bytes = build();
        assert_eq!(&bytes[0..4], b"\0asm");
        assert_eq!(&bytes[4..8], &1u32.to_le_bytes());
    }

    #[test]
    fn build_is_deterministic() {
        assert_eq!(build(), build());
    }
}
