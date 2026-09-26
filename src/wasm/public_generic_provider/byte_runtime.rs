//! Import-free owned-byte runtime for the compiled public-generic provider.
//!
//! Aggregate lowering reserves slots 7..=12 for the owned-byte operations
//! (`copy`, `get`, `drop`, `as_slice`, `zeroed`, `set`) that a legacy module
//! imports from its host. The provider has no host, so it implements them
//! here with the host runtime's carrier encoding: an owned value is
//! `((0x8000_0000 | token) << 32) | len`, a byte-range view is a
//! `0x4000_0000`-tagged descriptor, and any other value is a fixed view of at
//! most 64 KiB inside the first 128 KiB (the host's plain-root bounds). A violated
//! invariant traps (`unreachable`) where the host import would throw.
//!
//! Deliberate differences from the browser host, none reachable from a
//! checked Bytes-only endpoint: the host additionally admits a 256 KiB owned
//! String literal window (196608..262144) only in its String profile, and the
//! provider admits no String; the host caps 16 live owned entries and reuses
//! freed tokens, while the provider issues up to `HEAP_ENTRIES - 1` tokens per
//! invocation without reuse, so it never refuses where the host succeeds on a
//! checked program's bounded allocation sites.
//!
//! Storage is invocation-local: `spx_pg_v1_call` resets the heap, registers
//! the two input leaves as tokens 1 and 2, and resolves the result leaves
//! back to `(ptr, len)` rows before encoding. Capacity is bounded by the
//! checked language limits (`MAX_OWNED_BYTE_PAYLOAD_BYTES` cumulative owned
//! payload and a bounded number of allocation sites), so exhaustion of this
//! reservation is an invariant defect, as on the native reserved profile.

use super::{global_get, global_set, i32_const, i64_const_imm, local_get, local_set, u32_leb};

/// Unexported heap globals appended after the provider's twelve state cells.
pub(super) const GLOBAL_HEAP_CURSOR: u32 = 12;
pub(super) const GLOBAL_HEAP_NEXT: u32 = 13;
/// Token 0 is never issued; tokens 1 and 2 are the invocation's input leaves.
pub(super) const HEAP_ENTRIES: u32 = 1_024;
pub(super) const HEAP_TABLE_BYTES: u32 = HEAP_ENTRIES * 8;
/// Twice the checked cumulative owned-payload limit, so a checked program
/// can never reach it; reaching it anyway is an invariant defect.
pub(super) const HEAP_DATA_BYTES: u32 =
    2 * crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES as u32;
/// Host `FIXED_MEMORY_BYTES`: fixed views and range descriptors end below it.
const FIXED_MEMORY_BYTES: i32 = 131_072;
/// Host plain fixed-root length bound (the 64 KiB external-root limit).
const FIXED_ROOT_MAX_BYTES: i32 = 65_536;
/// Largest owned Bytes value (`MAX_OWNED_BYTE_VALUE_BYTES`).
const MAX_OWNED_BYTES: i64 = 131_072;
// Every checked invocation holds two input tokens plus at most one token per
// bounded allocation site, and at most the checked cumulative owned payload.
const _: () = assert!(2 + crate::byte_data_capacity::MAX_BYTES_COPY_SITES < HEAP_ENTRIES);
const _: () =
    assert!(crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES <= HEAP_DATA_BYTES as u64);
const OWNED_TAG: i32 = i32::MIN; // 0x8000_0000
const RANGE_MASK: i32 = -1_073_741_824; // 0xc000_0000
const RANGE_TAG: i32 = 0x4000_0000;

/// Function-type payloads of the two private helpers appended after the
/// carrier codecs: `resolve(i64) -> i32` and `alloc(i32) -> i64`.
pub(super) fn append_type_entries(types: &mut Vec<u8>) {
    types.extend([0x60, 1, 0x7e, 1, 0x7f]);
    types.extend([0x60, 1, 0x7f, 1, 0x7e]);
}

pub(super) const TYPE_COUNT: u32 = 2;
pub(super) const FUNCTION_COUNT: u32 = 2;

#[derive(Clone, Copy)]
pub(super) struct Heap {
    pub(super) table: u32,
    pub(super) data: u32,
    pub(super) end: u32,
    /// Absolute index of `resolve`; `alloc` follows it.
    pub(super) resolve: u32,
}

impl Heap {
    fn alloc(self) -> u32 {
        self.resolve + 1
    }
}

fn finish(code: &mut Vec<u8>, mut body: Vec<u8>) {
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn call(body: &mut Vec<u8>, index: u32) {
    body.push(0x10);
    u32_leb(body, index);
}

fn trap_if(body: &mut Vec<u8>) {
    body.extend([0x04, 0x40, 0x00, 0x0b]);
}

fn high_word(body: &mut Vec<u8>, carrier: u32) {
    body.extend(local_get(carrier));
    body.extend(i64_const_imm(32));
    body.extend([0x88, 0xa7]); // i64.shr_u; i32.wrap_i64
}

fn entry(body: &mut Vec<u8>, heap: Heap, token: u32) {
    body.extend(local_get(token));
    body.extend(i32_const(8));
    body.push(0x6c);
    body.extend(i32_const(heap.table as i32));
    body.push(0x6a);
}

/// Owned carrier from `token` and `len` locals.
fn owned_carrier(body: &mut Vec<u8>, token: u32, len: u32) {
    body.extend(local_get(token));
    body.extend(i32_const(OWNED_TAG));
    body.extend([0x72, 0xad]);
    body.extend(i64_const_imm(32));
    body.push(0x86);
    body.extend(local_get(len));
    body.extend([0xad, 0x84]);
}

fn require_owned(body: &mut Vec<u8>, carrier: u32) {
    high_word(body, carrier);
    body.extend(i32_const(OWNED_TAG));
    body.extend([0x71, 0x45]);
    trap_if(body);
}

/// Private bodies appended after the carrier codecs, in index order.
pub(super) fn helper_bodies(code: &mut Vec<u8>, heap: Heap) {
    // resolve(carrier) -> ptr. Locals: high, len, token, dp, base, ulen (i32);
    // ult, off (i64).
    let mut body = vec![2, 6, 0x7f, 2, 0x7e];
    let (high, len, token, dp, base, ulen, ult, off) = (1, 2, 3, 4, 5, 6, 7, 8);
    high_word(&mut body, 0);
    body.extend(local_set(high));
    body.extend(local_get(0));
    body.push(0xa7);
    body.extend(local_set(len));
    // Owned: a live token below the next issue whose entry length matches.
    body.extend(local_get(high));
    body.extend(i32_const(OWNED_TAG));
    body.extend([0x71, 0x04, 0x40]);
    body.extend(local_get(high));
    body.extend(i32_const(i32::MAX));
    body.push(0x71);
    body.extend([0x22]);
    u32_leb(&mut body, token);
    body.push(0x45);
    body.extend(local_get(token));
    body.extend(global_get(GLOBAL_HEAP_NEXT));
    body.extend([0x4f, 0x72]);
    trap_if(&mut body);
    entry(&mut body, heap, token);
    body.push(0x22);
    u32_leb(&mut body, dp);
    body.extend([0x28, 2, 0, 0x22]);
    u32_leb(&mut body, base);
    body.push(0x45);
    trap_if(&mut body);
    body.extend(local_get(dp));
    body.extend([0x28, 2, 4]);
    body.extend(local_get(len));
    body.push(0x47);
    trap_if(&mut body);
    body.extend(local_get(base));
    body.extend([0x0f, 0x0b]);
    // Range view: validate the descriptor as the host adapter replays it.
    body.extend(local_get(high));
    body.extend(i32_const(RANGE_MASK));
    body.push(0x71);
    body.extend(i32_const(RANGE_TAG));
    body.extend([0x46, 0x04, 0x40]);
    body.extend(local_get(high));
    body.extend(i32_const(0xffff));
    body.push(0x71);
    body.extend(i32_const(8));
    body.push(0x6c);
    body.push(0x22);
    u32_leb(&mut body, dp);
    body.extend(i32_const(FIXED_MEMORY_BYTES - 32));
    body.push(0x4b);
    trap_if(&mut body);
    body.extend(local_get(dp));
    body.extend([0x28, 2, 0, 0x22]);
    u32_leb(&mut body, token);
    body.push(0x45);
    body.extend(local_get(token));
    body.extend(local_get(high));
    body.extend(i32_const(16));
    body.push(0x76);
    body.extend(i32_const(0x1fff));
    body.extend([0x71, 0x47, 0x72]);
    body.extend(local_get(dp));
    body.extend([0x28, 2, 4]);
    body.extend(local_get(dp));
    body.extend([0x47, 0x72]);
    trap_if(&mut body);
    body.extend(local_get(dp));
    body.extend([0x29, 3, 8, 0x22]);
    u32_leb(&mut body, ult);
    body.extend(i64_const_imm(32));
    body.extend([0x88, 0xa7]);
    body.extend(i32_const(RANGE_MASK));
    body.push(0x71);
    body.extend(i32_const(RANGE_TAG));
    body.push(0x46);
    trap_if(&mut body); // nested range descriptor invariant
    body.extend(local_get(dp));
    body.extend([0x29, 3, 24]);
    body.extend(local_get(len));
    body.extend([0xad, 0x52]);
    trap_if(&mut body);
    body.extend(local_get(ult));
    call(&mut body, heap.resolve);
    body.extend(local_set(base));
    body.extend(local_get(ult));
    body.push(0xa7);
    body.extend(local_set(ulen));
    body.extend(local_get(dp));
    body.extend([0x29, 3, 16, 0x22]);
    u32_leb(&mut body, off);
    body.extend(local_get(ulen));
    body.extend([0xad, 0x56]);
    body.extend(local_get(len));
    body.push(0xad);
    body.extend(local_get(ulen));
    body.push(0xad);
    body.extend(local_get(off));
    body.extend([0x7d, 0x56, 0x72]);
    trap_if(&mut body);
    body.extend(local_get(base));
    body.extend(local_get(off));
    body.extend([0xa7, 0x6a, 0x0f, 0x0b]);
    // Fixed view: at most 64 KiB, wholly inside the first 128 KiB.
    body.extend(local_get(len));
    body.extend(i32_const(FIXED_ROOT_MAX_BYTES));
    body.push(0x4b);
    body.extend(local_get(high));
    body.extend(i32_const(FIXED_MEMORY_BYTES));
    body.extend(local_get(len));
    body.extend([0x6b, 0x4b, 0x72]);
    trap_if(&mut body);
    body.extend(local_get(high));
    finish(code, body);

    // alloc(len) -> owned carrier. Locals: token, ptr.
    let mut body = vec![1, 2, 0x7f];
    let (token, ptr) = (1, 2);
    body.extend(global_get(GLOBAL_HEAP_NEXT));
    body.push(0x22);
    u32_leb(&mut body, token);
    body.extend(i32_const(HEAP_ENTRIES as i32));
    body.push(0x4f);
    trap_if(&mut body);
    body.extend(local_get(0));
    body.extend(i32_const(heap.end as i32));
    body.extend(global_get(GLOBAL_HEAP_CURSOR));
    body.push(0x22);
    u32_leb(&mut body, ptr);
    body.extend([0x6b, 0x4b]);
    trap_if(&mut body);
    body.extend(local_get(ptr));
    body.extend(local_get(0));
    body.push(0x6a);
    body.extend(global_set(GLOBAL_HEAP_CURSOR));
    body.extend(local_get(token));
    body.extend(i32_const(1));
    body.push(0x6a);
    body.extend(global_set(GLOBAL_HEAP_NEXT));
    entry(&mut body, heap, token);
    body.extend(local_get(ptr));
    body.extend([0x36, 2, 0]);
    entry(&mut body, heap, token);
    body.extend(local_get(0));
    body.extend([0x36, 2, 4]);
    owned_carrier(&mut body, token, 0);
    finish(code, body);
}

/// Slots 7..=12 in aggregate-lowering order.
pub(super) fn runtime_bodies(code: &mut Vec<u8>, heap: Heap) {
    // 7 copy(carrier) -> owned carrier. Locals: src, len, dst (i32).
    let mut body = vec![1, 3, 0x7f];
    body.extend(local_get(0));
    call(&mut body, heap.resolve);
    body.extend(local_set(1));
    body.extend(local_get(0));
    body.push(0xa7);
    body.extend(local_set(2));
    body.extend(global_get(GLOBAL_HEAP_CURSOR));
    body.extend(local_set(3));
    body.extend(local_get(2));
    call(&mut body, heap.alloc());
    body.extend(local_get(3));
    body.extend(local_get(1));
    body.extend(local_get(2));
    body.extend([0xfc, 0x0a, 0, 0]);
    finish(code, body);

    // 8 get(carrier, index) -> byte or -1. Local: ptr.
    let mut body = vec![1, 1, 0x7f];
    body.extend(local_get(0));
    call(&mut body, heap.resolve);
    body.extend(local_set(2));
    body.extend(local_get(1));
    body.extend(local_get(0));
    body.extend([0xa7, 0xad, 0x5a, 0x04, 0x40]);
    body.extend(i32_const(-1));
    body.extend([0x0f, 0x0b]);
    body.extend(local_get(2));
    body.extend(local_get(1));
    body.extend([0xa7, 0x6a, 0x2d, 0, 0]);
    finish(code, body);

    // 9 drop(owned carrier): the entry is retired once; later use traps.
    let mut body = vec![0];
    require_owned(&mut body, 0);
    body.extend(local_get(0));
    call(&mut body, heap.resolve);
    body.push(0x1a);
    high_word(&mut body, 0);
    body.extend(i32_const(i32::MAX));
    body.extend([0x71]);
    body.extend(i32_const(8));
    body.push(0x6c);
    body.extend(i32_const(heap.table as i32));
    body.push(0x6a);
    body.extend(i32_const(0));
    body.extend([0x36, 2, 0]);
    finish(code, body);

    // 10 as_slice(carrier) -> the same validated carrier.
    let mut body = vec![0];
    body.extend(local_get(0));
    call(&mut body, heap.resolve);
    body.push(0x1a);
    body.extend(local_get(0));
    finish(code, body);

    // 11 zeroed(count) -> owned carrier of `count` zero bytes. Local: dst.
    let mut body = vec![1, 1, 0x7f];
    body.extend(local_get(0));
    body.extend(i64_const_imm(MAX_OWNED_BYTES));
    body.push(0x56);
    trap_if(&mut body);
    body.extend(global_get(GLOBAL_HEAP_CURSOR));
    body.extend(local_set(1));
    body.extend(local_get(0));
    body.push(0xa7);
    call(&mut body, heap.alloc());
    body.extend(local_get(1));
    body.extend(i32_const(0));
    body.extend(local_get(0));
    body.extend([0xa7, 0xfc, 0x0b, 0]);
    finish(code, body);

    // 12 set(owned carrier, index, value) -> the same carrier. Local: ptr.
    let mut body = vec![1, 1, 0x7f];
    require_owned(&mut body, 0);
    body.extend(local_get(0));
    call(&mut body, heap.resolve);
    body.extend(local_set(3));
    body.extend(local_get(1));
    body.extend(local_get(0));
    body.extend([0xa7, 0xad, 0x5a]);
    body.extend(local_get(2));
    body.extend(i32_const(255));
    body.extend([0x4b, 0x72]);
    trap_if(&mut body);
    body.extend(local_get(3));
    body.extend(local_get(1));
    body.extend([0xa7, 0x6a]);
    body.extend(local_get(2));
    body.extend([0x3a, 0, 0]);
    body.extend(local_get(0));
    finish(code, body);
}

/// At invocation start: reset the heap and present the two prepared input
/// leaves (`(ptr, len)` rows) to the checked body as owned tokens 1 and 2.
pub(super) fn emit_register_inputs(body: &mut Vec<u8>, heap: Heap, table: u32, aggregate: u32) {
    body.extend(i32_const(heap.data as i32));
    body.extend(global_set(GLOBAL_HEAP_CURSOR));
    for leaf in 0..2_u32 {
        let token = leaf + 1;
        let row = heap.table + token * 8;
        body.extend(i32_const(row as i32));
        body.extend(i32_const((table + leaf * 8) as i32));
        body.extend([0x28, 2, 0, 0x36, 2, 0]);
        body.extend(i32_const((row + 4) as i32));
        body.extend(i32_const((table + leaf * 8 + 4) as i32));
        body.extend([0x28, 2, 0, 0x36, 2, 0]);
        body.extend(i32_const((aggregate + leaf * 8) as i32));
        body.extend(i64_const_imm(
            ((u64::from(OWNED_TAG as u32 | token)) << 32) as i64,
        ));
        body.extend(i32_const((table + leaf * 8 + 4) as i32));
        body.extend([0x28, 2, 0, 0xad, 0x84, 0x37, 3, 0]);
    }
    body.extend(i32_const(3));
    body.extend(global_set(GLOBAL_HEAP_NEXT));
}

/// After a successful checked call: resolve each result leaf carrier to the
/// `(ptr, len)` row the result codec encodes. Invalid carriers trap.
pub(super) fn emit_resolve_results(body: &mut Vec<u8>, heap: Heap, aggregate: u32, table: u32) {
    for leaf in 0..2_u32 {
        body.extend(i32_const((table + leaf * 8) as i32));
        body.extend(i32_const((aggregate + leaf * 8) as i32));
        body.extend([0x29, 3, 0]);
        call(body, heap.resolve);
        body.extend([0x36, 2, 0]);
        body.extend(i32_const((table + leaf * 8 + 4) as i32));
        body.extend(i32_const((aggregate + leaf * 8) as i32));
        body.extend([0x29, 3, 0, 0xa7, 0x36, 2, 0]);
    }
}
