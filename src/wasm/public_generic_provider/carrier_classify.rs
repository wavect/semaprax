//! Wasm adapter ABI v2 input-carrier admission classifier.
//!
//! A line-for-line port of the native authenticated profile's
//! `spx_pg_auth_frame` (`native/authenticated_prepare.c`) against the same
//! trusted canonical empty frame, so both targets refuse a hostile carrier
//! with the same raw status: 6 for bounds, 5 for malformed structure or an
//! unknown schema/direction literal, and 14 (`SPX-PG803`) only after the
//! complete structural decode and self-digest check found a semantic binding
//! mismatch (descriptor/endpoint/instance identity, leaf count or leaf path)
//! or a digest mismatch. It runs in static memory before any private
//! reservation and writes only its bounded path table and SHA-256 workspace.

use super::{i32_const, i64_const_imm, local_get, local_set, u32_leb};
use crate::public_generic_abi::boundary_profile::MAX_TOTAL_PAYLOAD_BYTES;

const FRAME_DOMAIN: &[u8] = b"semaprax.public-generic-carrier.v1.frame\0";
const MALFORMED: i64 = 5;
const CAPACITY: i64 = 6;
const REPLAY: i64 =
    crate::public_generic_abi::wasm::binding::WASM_ADAPTER_V2_STATUS_CARRIER_REPLAY_MISMATCH as i64;
/// Native rejects a frame over 20 MiB as capacity before parsing it.
const MAX_FRAME_BYTES: i32 = 20 * 1024 * 1024;
const MAX_LEAVES: i64 = 256;
const LEAF_COUNT: i64 = 2;

/// Static data: domain, direction literals, digest prefix, trusted empty
/// frame; then the path table (256 rows of `(ptr, len)`).
pub(super) const DATA_OFFSET: u32 = 100_352;
pub(super) const PATH_TABLE: u32 = 126_976;
const _: () = assert!(PATH_TABLE + 256 * 8 <= super::BINDING_OFFSET);
const _: () = assert!(super::STATIC_RESULT_SHA256_WORKSPACE + 288 <= DATA_OFFSET);

pub(super) const TYPE_COUNT: u32 = 3;
pub(super) const FUNCTION_COUNT: u32 = 3;

pub(super) struct Classifier {
    data: Vec<u8>,
    empty_len: u32,
}

impl Classifier {
    /// `empty_frame` is the descriptor-bound canonical input frame with empty
    /// payloads, exactly the native profile's `SPX_PG_AUTH_EMPTY_FRAME`.
    pub(super) fn new(empty_frame: &[u8]) -> Result<Self, String> {
        let mut data = FRAME_DOMAIN.to_vec();
        data.extend_from_slice(b"inputresultsha256:");
        data.extend_from_slice(empty_frame);
        if DATA_OFFSET as usize + data.len() > PATH_TABLE as usize {
            return Err("carrier classifier trusted frame exceeds its static window".into());
        }
        Ok(Self {
            data,
            empty_len: empty_frame.len() as u32,
        })
    }

    pub(super) fn data(&self) -> &[u8] {
        &self.data
    }
}

const DOMAIN_AT: u32 = DATA_OFFSET;
const INPUT_AT: u32 = DOMAIN_AT + FRAME_DOMAIN.len() as u32;
const RESULT_AT: u32 = INPUT_AT + 5;
const PREFIX_AT: u32 = RESULT_AT + 6;
const EMPTY_AT: u32 = PREFIX_AT + 7;

pub(super) fn append_type_entries(types: &mut Vec<u8>) {
    types.extend([0x60, 2, 0x7f, 0x7f, 1, 0x7f]); // utf8(ptr, len) -> ok
    types.extend([0x60, 4, 0x7f, 0x7f, 0x7f, 0x7f, 1, 0x7f]); // eq(a, al, b, bl) -> ok
    types.extend([0x60, 2, 0x7f, 0x7f, 1, 0x7e]); // classify(ptr, len) -> lane
}

#[derive(Clone, Copy)]
pub(super) struct Indexes {
    /// Absolute index of `utf8`; `eq` and `classify` follow it.
    pub(super) utf8: u32,
    pub(super) sha256: u32,
    pub(super) workspace: u32,
}

impl Indexes {
    fn eq(self) -> u32 {
        self.utf8 + 1
    }
    pub(super) fn classify(self) -> u32 {
        self.utf8 + 2
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

fn tee(body: &mut Vec<u8>, local: u32) {
    body.push(0x22);
    u32_leb(body, local);
}

/// `if (cond) return status_lane; end` for an i64-returning function.
fn refuse_if(body: &mut Vec<u8>, status: i64) {
    body.extend([0x04, 0x40]);
    body.extend(i64_const_imm(status));
    body.extend([0x0f, 0x0b]);
}

/// `if (cond) return 0; end` for an i32-returning predicate.
fn false_if(body: &mut Vec<u8>) {
    body.extend([0x04, 0x40]);
    body.extend(i32_const(0));
    body.extend([0x0f, 0x0b]);
}

fn add_to(body: &mut Vec<u8>, local: u32, amount: i32) {
    body.extend(local_get(local));
    body.extend(i32_const(amount));
    body.push(0x6a);
    body.extend(local_set(local));
}

#[derive(Clone, Copy)]
enum Source {
    Frame,
    Trusted(u32),
}

impl Source {
    fn base(self, body: &mut Vec<u8>) {
        match self {
            Self::Frame => body.extend(local_get(0)),
            Self::Trusted(_) => body.extend(i32_const(EMPTY_AT as i32)),
        }
    }
    fn len(self, body: &mut Vec<u8>) {
        match self {
            Self::Frame => body.extend(local_get(1)),
            Self::Trusted(len) => body.extend(i32_const(len as i32)),
        }
    }
}

// classify locals
const AT: u32 = 2;
const TRUSTED: u32 = 3;
const VALUE: u32 = 4;
const LEN: u32 = 5;
const TVALUE: u32 = 6;
const TLEN: u32 = 7;
const MISMATCH: u32 = 8;
const LEAF: u32 = 9;
const PREV: u32 = 10;
const PREIMAGE: u32 = 11;
const INDEX: u32 = 12;
const BYTE: u32 = 13;
const SIZE: u32 = 14;
const COUNT: u32 = 15;
const TOTAL: u32 = 16;
const SUM: u32 = 17;

/// `spx_pg_auth_field`: an 8-byte length then that many bytes. A length over
/// 64 KiB is `oversize` (capacity for payloads, malformed for metadata).
fn field(body: &mut Vec<u8>, source: Source, at: u32, value: u32, len: u32, oversize: i64) {
    body.extend(local_get(at));
    source.len(body);
    body.push(0x4b); // at > n
    source.len(body);
    body.extend(local_get(at));
    body.push(0x6b);
    body.extend(i32_const(8));
    body.extend([0x49, 0x72]); // n - at < 8
    refuse_if(body, MALFORMED);
    source.base(body);
    body.extend(local_get(at));
    body.push(0x6a);
    body.extend([0x29, 0, 0]);
    body.extend(local_set(SIZE));
    add_to(body, at, 8);
    body.extend(local_get(SIZE));
    body.extend(i64_const_imm(65_536));
    body.push(0x56);
    refuse_if(body, oversize);
    body.extend(local_get(SIZE));
    source.len(body);
    body.extend(local_get(at));
    body.extend([0x6b, 0xad, 0x56]);
    refuse_if(body, MALFORMED);
    source.base(body);
    body.extend(local_get(at));
    body.push(0x6a);
    body.extend(local_set(value));
    body.extend(local_get(SIZE));
    body.push(0xa7);
    tee(body, len);
    body.extend(local_get(at));
    body.push(0x6a);
    body.extend(local_set(at));
}

fn require_utf8(body: &mut Vec<u8>, ix: Indexes) {
    body.extend(local_get(VALUE));
    body.extend(local_get(LEN));
    call(body, ix.utf8);
    body.push(0x45);
    refuse_if(body, MALFORMED);
}

fn equal(body: &mut Vec<u8>, ix: Indexes, a: (u32, u32), b: (Vec<u8>, Vec<u8>)) {
    body.extend(local_get(a.0));
    body.extend(local_get(a.1));
    body.extend(b.0);
    body.extend(b.1);
    call(body, ix.eq());
}

pub(super) fn bodies(code: &mut Vec<u8>, classifier: &Classifier, ix: Indexes) {
    utf8_body(code);
    eq_body(code);
    classify_body(code, classifier, ix);
}

fn utf8_body(code: &mut Vec<u8>) {
    // Rust `str::from_utf8` acceptance, exactly as `spx_pg_auth_utf8`.
    // Locals: at, first, remaining, scalar, minimum, next.
    let (at, first, rem, scalar, minimum, next) = (2, 3, 4, 5, 6, 7);
    let mut body = vec![1, 6, 0x7f];
    body.extend([0x02, 0x40, 0x03, 0x40]); // block, loop
    body.extend(local_get(at));
    body.extend(local_get(1));
    body.extend([0x4f, 0x0d, 1]); // at >= n: done
    body.extend(local_get(0));
    body.extend(local_get(at));
    body.extend([0x6a, 0x2d, 0, 0]);
    body.extend(local_set(first));
    add_to(&mut body, at, 1);
    body.extend(local_get(first));
    body.extend(i32_const(0x80));
    body.extend([0x49, 0x0d, 0]); // ASCII: continue
    for (low, high, remaining, mask, min) in [
        (0xc2, 0xdf, 1, 0x1f, 0x80),
        (0xe0, 0xef, 2, 0x0f, 0x800),
        (0xf0, 0xf4, 3, 0x07, 0x1_0000),
    ] {
        body.extend(local_get(first));
        body.extend(i32_const(low));
        body.push(0x4f);
        body.extend(local_get(first));
        body.extend(i32_const(high));
        body.extend([0x4d, 0x71, 0x04, 0x40]);
        body.extend(i32_const(remaining));
        body.extend(local_set(rem));
        body.extend(local_get(first));
        body.extend(i32_const(mask));
        body.push(0x71);
        body.extend(local_set(scalar));
        body.extend(i32_const(min));
        body.extend(local_set(minimum));
        body.push(0x05);
    }
    body.extend(i32_const(0));
    body.push(0x0f);
    body.extend([0x0b, 0x0b, 0x0b]);
    body.extend(local_get(rem));
    body.extend(local_get(1));
    body.extend(local_get(at));
    body.extend([0x6b, 0x4b]);
    false_if(&mut body);
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(rem));
    body.extend([0x45, 0x0d, 1]);
    body.extend(local_get(0));
    body.extend(local_get(at));
    body.extend([0x6a, 0x2d, 0, 0]);
    tee(&mut body, next);
    body.extend(i32_const(0xc0));
    body.push(0x71);
    body.extend(i32_const(0x80));
    body.push(0x47);
    false_if(&mut body);
    add_to(&mut body, at, 1);
    body.extend(local_get(scalar));
    body.extend(i32_const(6));
    body.push(0x74);
    body.extend(local_get(next));
    body.extend(i32_const(0x3f));
    body.extend([0x71, 0x72]);
    body.extend(local_set(scalar));
    add_to(&mut body, rem, -1);
    body.extend([0x0c, 0, 0x0b, 0x0b]);
    body.extend(local_get(scalar));
    body.extend(local_get(minimum));
    body.push(0x49);
    body.extend(local_get(scalar));
    body.extend(i32_const(0x10_ffff));
    body.extend([0x4b, 0x72]);
    body.extend(local_get(scalar));
    body.extend(i32_const(0xd800));
    body.push(0x4f);
    body.extend(local_get(scalar));
    body.extend(i32_const(0xdfff));
    body.extend([0x4d, 0x71, 0x72]);
    false_if(&mut body);
    body.extend([0x0c, 0, 0x0b, 0x0b]); // continue; end loop; end block
    body.extend(i32_const(1));
    finish(code, body);
}

fn eq_body(code: &mut Vec<u8>) {
    // eq(a, a_len, b, b_len): equal lengths and bytes. Local: index.
    let mut body = vec![1, 1, 0x7f];
    body.extend(local_get(1));
    body.extend(local_get(3));
    body.push(0x47);
    false_if(&mut body);
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(4));
    body.extend(local_get(1));
    body.extend([0x4f, 0x0d, 1]);
    body.extend(local_get(0));
    body.extend(local_get(4));
    body.extend([0x6a, 0x2d, 0, 0]);
    body.extend(local_get(2));
    body.extend(local_get(4));
    body.extend([0x6a, 0x2d, 0, 0, 0x47]);
    false_if(&mut body);
    add_to(&mut body, 4, 1);
    body.extend([0x0c, 0, 0x0b, 0x0b]);
    body.extend(i32_const(1));
    finish(code, body);
}

fn classify_body(code: &mut Vec<u8>, classifier: &Classifier, ix: Indexes) {
    let trusted = Source::Trusted(classifier.empty_len);
    let mut body = vec![2, 12, 0x7f, 4, 0x7e];
    body.extend(local_get(1));
    body.extend(i32_const(MAX_FRAME_BYTES));
    body.push(0x4b);
    refuse_if(&mut body, CAPACITY);
    for index in 0..6 {
        field(&mut body, Source::Frame, AT, VALUE, LEN, MALFORMED);
        require_utf8(&mut body, ix);
        if index == 1 {
            equal(
                &mut body,
                ix,
                (VALUE, LEN),
                (i32_const(INPUT_AT as i32), i32_const(5)),
            );
            equal(
                &mut body,
                ix,
                (VALUE, LEN),
                (i32_const(RESULT_AT as i32), i32_const(6)),
            );
            body.extend([0x72, 0x45]);
            refuse_if(&mut body, MALFORMED);
        }
        field(&mut body, trusted, TRUSTED, TVALUE, TLEN, MALFORMED);
        equal(
            &mut body,
            ix,
            (VALUE, LEN),
            (local_get(TVALUE), local_get(TLEN)),
        );
        body.push(0x45);
        if index == 0 {
            refuse_if(&mut body, MALFORMED);
        } else {
            body.extend([0x04, 0x40]);
            body.extend(i32_const(1));
            body.extend(local_set(MISMATCH));
            body.push(0x0b);
        }
    }
    for (local, bound) in [(COUNT, MAX_LEAVES), (TOTAL, MAX_TOTAL_PAYLOAD_BYTES as i64)] {
        body.extend(local_get(1));
        body.extend(local_get(AT));
        body.push(0x6b);
        body.extend(i32_const(8));
        body.push(0x49);
        refuse_if(&mut body, MALFORMED);
        body.extend(local_get(0));
        body.extend(local_get(AT));
        body.extend([0x6a, 0x29, 0, 0]);
        tee(&mut body, local);
        add_to(&mut body, AT, 8);
        body.extend(i64_const_imm(bound));
        body.push(0x56);
        refuse_if(&mut body, CAPACITY);
    }
    add_to(&mut body, TRUSTED, 16);
    body.extend(local_get(COUNT));
    body.extend(i64_const_imm(LEAF_COUNT));
    body.extend([0x52, 0x04, 0x40]);
    body.extend(i32_const(1));
    body.extend(local_set(MISMATCH));
    body.push(0x0b);
    // Leaves.
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(LEAF));
    body.push(0xad);
    body.extend(local_get(COUNT));
    body.extend([0x5a, 0x0d, 1]);
    field(&mut body, Source::Frame, AT, VALUE, LEN, MALFORMED);
    require_utf8(&mut body, ix);
    body.extend(i32_const(0));
    body.extend(local_set(PREV));
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(PREV));
    body.extend(local_get(LEAF));
    body.extend([0x4f, 0x0d, 1]);
    let row = |body: &mut Vec<u8>, local: u32| {
        body.extend(local_get(local));
        body.extend(i32_const(8));
        body.push(0x6c);
        body.extend(i32_const(PATH_TABLE as i32));
        body.push(0x6a);
    };
    let mut prev_ptr = Vec::new();
    row(&mut prev_ptr, PREV);
    prev_ptr.extend([0x28, 2, 0]);
    let mut prev_len = Vec::new();
    row(&mut prev_len, PREV);
    prev_len.extend([0x28, 2, 4]);
    equal(&mut body, ix, (VALUE, LEN), (prev_ptr, prev_len));
    refuse_if(&mut body, MALFORMED); // duplicate leaf path
    add_to(&mut body, PREV, 1);
    body.extend([0x0c, 0, 0x0b, 0x0b]);
    row(&mut body, LEAF);
    body.extend(local_get(VALUE));
    body.extend([0x36, 2, 0]);
    row(&mut body, LEAF);
    body.extend(local_get(LEN));
    body.extend([0x36, 2, 4]);
    body.extend(local_get(LEAF));
    body.push(0xad);
    body.extend(i64_const_imm(LEAF_COUNT));
    body.extend([0x54, 0x04, 0x40]);
    field(&mut body, trusted, TRUSTED, TVALUE, TLEN, MALFORMED);
    equal(
        &mut body,
        ix,
        (VALUE, LEN),
        (local_get(TVALUE), local_get(TLEN)),
    );
    body.extend([0x45, 0x04, 0x40]);
    body.extend(i32_const(1));
    body.extend(local_set(MISMATCH));
    body.push(0x0b);
    add_to(&mut body, TRUSTED, 9); // trusted Bytes tag and empty payload length
    body.push(0x0b);
    // `if (at == n || p[at++] != 0)`: the kind tag byte.
    body.extend(local_get(AT));
    body.extend(local_get(1));
    body.push(0x46);
    refuse_if(&mut body, MALFORMED);
    body.extend(local_get(0));
    body.extend(local_get(AT));
    body.extend([0x6a, 0x2d, 0, 0]);
    add_to(&mut body, AT, 1);
    refuse_if(&mut body, MALFORMED);
    field(&mut body, Source::Frame, AT, VALUE, LEN, CAPACITY);
    body.extend(local_get(SUM));
    body.extend(local_get(SIZE));
    body.push(0x7c);
    body.extend(local_set(SUM));
    add_to(&mut body, LEAF, 1);
    body.extend([0x0c, 0, 0x0b, 0x0b]);
    body.extend(local_get(SUM));
    body.extend(local_get(TOTAL));
    body.push(0x52);
    refuse_if(&mut body, MALFORMED);
    body.extend(local_get(AT));
    body.extend(local_set(PREIMAGE));
    field(&mut body, Source::Frame, AT, VALUE, LEN, MALFORMED);
    require_utf8(&mut body, ix);
    body.extend(local_get(AT));
    body.extend(local_get(1));
    body.push(0x47);
    refuse_if(&mut body, MALFORMED);
    // Self-digest: `sha256:` plus 64 lowercase hex digits of the preimage.
    body.extend(i32_const(DOMAIN_AT as i32));
    body.extend(i32_const(FRAME_DOMAIN.len() as i32));
    body.extend(local_get(0));
    body.extend(local_get(PREIMAGE));
    body.extend(i32_const(ix.workspace as i32));
    call(&mut body, ix.sha256);
    body.extend(local_get(LEN));
    body.extend(i32_const(71));
    body.push(0x47);
    refuse_if(&mut body, REPLAY);
    // The length is exactly 71, so compare the 7-byte prefix then the hex.
    body.extend(local_get(VALUE));
    body.extend(i32_const(7));
    body.extend(i32_const(PREFIX_AT as i32));
    body.extend(i32_const(7));
    call(&mut body, ix.eq());
    body.push(0x45);
    refuse_if(&mut body, REPLAY);
    body.extend(i32_const(0));
    body.extend(local_set(INDEX));
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(INDEX));
    body.extend(i32_const(32));
    body.extend([0x4f, 0x0d, 1]);
    body.extend(i32_const(ix.workspace as i32 + 256));
    body.extend(local_get(INDEX));
    body.extend([0x6a, 0x2d, 0, 0]);
    body.extend(local_set(BYTE));
    for (shift, offset) in [(4, 0), (0, 1)] {
        // nibble -> lowercase hex digit, compared with the submitted byte.
        body.extend(local_get(VALUE));
        body.extend(local_get(INDEX));
        body.extend(i32_const(2));
        body.push(0x6c);
        body.push(0x6a);
        body.extend([0x2d, 0]);
        u32_leb(&mut body, 7 + offset);
        body.extend(local_get(BYTE));
        body.extend(i32_const(shift));
        body.push(0x76);
        body.extend(i32_const(15));
        body.push(0x71);
        tee(&mut body, PREV);
        body.extend(i32_const(i32::from(b'0')));
        body.push(0x6a);
        body.extend(local_get(PREV));
        body.extend(i32_const(i32::from(b'a') - 10));
        body.push(0x6a);
        body.extend(local_get(PREV));
        body.extend(i32_const(10));
        body.extend([0x49, 0x1b]); // select(digit, letter, nibble < 10)
        body.push(0x47);
        refuse_if(&mut body, REPLAY);
    }
    add_to(&mut body, INDEX, 1);
    body.extend([0x0c, 0, 0x0b, 0x0b]);
    body.extend(local_get(MISMATCH));
    refuse_if(&mut body, REPLAY);
    // Success lane: status 0, admitted payload total in the high half.
    body.extend(local_get(SUM));
    body.extend(i64_const_imm(32));
    body.push(0x86);
    finish(code, body);
}
