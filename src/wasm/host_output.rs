//! Fixed-memory staging for Bounded Stdout Transcript v1.
//!
//! The profile adds no import. Bytes remain in module-owned linear memory and
//! the public length stays zero until the root wrapper succeeds.

use super::{write_i64, write_u32, I32};

pub(super) const MEMORY_PAGES: u8 = 3;
pub(super) const TRANSCRIPT_BASE: u32 = 131_072;
pub(super) const TRANSCRIPT_CAPACITY: u32 = 65_536;
#[derive(Clone, Copy)]
pub(super) struct Globals {
    /// Present only for the public Useful Data command profile. That profile
    /// retains an authenticated scratch-memory source until wrapper commit;
    /// it never places staged bytes in the exported transcript range.
    pub(super) staged_source: Option<u32>,
    pub(super) staged_length: u32,
    pub(super) published_length: u32,
    pub(super) base: u32,
    pub(super) capacity: u32,
}

pub(super) const ROOT_GLOBALS: Globals = Globals {
    staged_source: None,
    staged_length: 1,
    published_length: 2,
    base: 3,
    capacity: 4,
};

pub(super) const DATA_GLOBALS: Globals = Globals {
    staged_source: Some(4),
    staged_length: 5,
    published_length: 6,
    base: 7,
    capacity: 8,
};

pub(super) const MEMORY_EXPORT: &str = "memory";
pub(super) const LENGTH_EXPORT: &str = "__spx_stdout_length_v1";
pub(super) const BASE_EXPORT: &str = "__spx_stdout_base_v1";
pub(super) const CAPACITY_EXPORT: &str = "__spx_stdout_capacity_v1";

pub(super) fn append_globals(globals: &mut Vec<u8>) {
    // Mutable staged and published lengths.
    for _ in 0..2 {
        globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    }
    // Immutable base and capacity metadata.
    for value in [TRANSCRIPT_BASE, TRANSCRIPT_CAPACITY] {
        globals.extend([I32, 0x00, 0x41]);
        write_i64(globals, i64::from(value));
        globals.push(0x0b);
    }
}

pub(super) fn append_data_globals(globals: &mut Vec<u8>) {
    // Private mutable staged source pointer.
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    append_globals(globals);
}

pub(super) fn append_exports(exports: &mut Vec<u8>, globals: Globals, export_memory: bool) {
    if export_memory {
        super::write_name(exports, MEMORY_EXPORT);
        exports.push(0x02);
        write_u32(exports, 0);
    }
    for (name, index) in [
        (LENGTH_EXPORT, globals.published_length),
        (BASE_EXPORT, globals.base),
        (CAPACITY_EXPORT, globals.capacity),
    ] {
        super::write_name(exports, name);
        exports.push(0x03);
        write_u32(exports, index);
    }
}

/// Clear all previously published/staged bytes and both lengths.
pub(super) fn emit_reset(body: &mut Vec<u8>, globals: Globals) {
    body.extend([0x41]);
    write_i64(body, i64::from(TRANSCRIPT_BASE));
    body.extend([0x41, 0x00, 0x41]);
    write_i64(body, i64::from(TRANSCRIPT_CAPACITY));
    body.extend([0xfc, 0x0b, 0x00]); // memory.fill 0
    for global in globals
        .staged_source
        .into_iter()
        .chain([globals.staged_length, globals.published_length])
    {
        body.extend([0x41, 0x00, 0x24]);
        write_u32(body, global);
    }
}

/// Stage one packed Slice<u8> carrier.
///
/// Standalone target evidence retains its original eager-copy behavior. The
/// public Useful Data command profile records only the authenticated external
/// scratch pointer and length; its wrapper performs the sole transcript copy
/// after the target and result carrier have both succeeded.
pub(super) fn emit_write(
    body: &mut Vec<u8>,
    slice_local: u32,
    result_local: u32,
    globals: Globals,
) {
    if let Some(staged_source) = globals.staged_source {
        body.push(0x20);
        write_u32(body, slice_local);
        body.extend([0x42, 0x20, 0x88, 0xa7, 0x24]); // root -> i32 -> global
        write_u32(body, staged_source);

        body.push(0x20);
        write_u32(body, slice_local);
        body.extend([0xa7, 0x24]);
        write_u32(body, globals.staged_length);
        body.push(0x20);
        write_u32(body, slice_local);
        body.extend([0xa7, 0xad, 0x21]);
        write_u32(body, result_local);
        return;
    }

    body.push(0x41);
    write_i64(body, i64::from(TRANSCRIPT_BASE));
    body.push(0x20);
    write_u32(body, slice_local);
    body.extend([0x42, 0x20, 0x88, 0xa7]); // i64.shr_u 32; i32.wrap_i64
    body.push(0x20);
    write_u32(body, slice_local);
    body.push(0xa7); // i32.wrap_i64 length
    body.extend([0xfc, 0x0a, 0x00, 0x00]); // memory.copy 0 0

    body.push(0x20);
    write_u32(body, slice_local);
    body.push(0xa7);
    body.push(0x24);
    write_u32(body, globals.staged_length);
    body.push(0x20);
    write_u32(body, slice_local);
    body.extend([0xa7, 0xad, 0x21]); // len -> i64 -> local
    write_u32(body, result_local);
}

pub(super) fn emit_publish(body: &mut Vec<u8>, globals: Globals) {
    if let Some(staged_source) = globals.staged_source {
        body.push(0x41);
        write_i64(body, i64::from(TRANSCRIPT_BASE));
        body.push(0x23);
        write_u32(body, staged_source);
        body.push(0x23);
        write_u32(body, globals.staged_length);
        body.extend([0xfc, 0x0a, 0x00, 0x00]); // memory.copy 0 0
    }
    body.push(0x23);
    write_u32(body, globals.staged_length);
    body.push(0x24);
    write_u32(body, globals.published_length);
    for global in globals
        .staged_source
        .into_iter()
        .chain(std::iter::once(globals.staged_length))
    {
        body.extend([0x41, 0x00, 0x24]);
        write_u32(body, global);
    }
}

pub(super) fn emit_discard(body: &mut Vec<u8>, globals: Globals) {
    body.push(0x41);
    write_i64(body, i64::from(TRANSCRIPT_BASE));
    body.extend([0x41, 0x00, 0x41]);
    write_i64(body, i64::from(TRANSCRIPT_CAPACITY));
    body.extend([0xfc, 0x0b, 0x00]);
    for global in globals
        .staged_source
        .into_iter()
        .chain([globals.staged_length, globals.published_length])
    {
        body.extend([0x41, 0x00, 0x24]);
        write_u32(body, global);
    }
}
