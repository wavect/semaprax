//! Environment-only borrowed-text helpers over fixed or tagged byte carriers.

use super::super::{write_i64, write_u32, ByteOutput, I32};

fn get(output: &mut impl ByteOutput, value: u32, index: u32, destination: u32) {
    output.extend_bytes(&[0x20]);
    write_u32(output, value);
    output.extend_bytes(&[0x42]);
    write_i64(output, 32);
    output.push(0x89); // i64.rotl: helper view -> byte carrier
    output.extend_bytes(&[0x20]);
    write_u32(output, index);
    output.push(0xad); // i64.extend_i32_u
    output.push(0x10);
    write_u32(output, super::super::aggregate::BYTE_GET_IMPORT);
    output.push(0x22);
    write_u32(output, destination);
    output.extend_bytes(&[0x41]);
    write_i64(output, 255);
    output.extend_bytes(&[0x4b, 0x04, 0x40, 0x00, 0x0b]);
}
fn len(output: &mut impl ByteOutput, value: u32) {
    output.extend_bytes(&[0x20]);
    write_u32(output, value);
    output.extend_bytes(&[0x42]);
    write_i64(output, 32);
    output.extend_bytes(&[0x88, 0xa7]);
}
fn bounded(output: &mut impl ByteOutput, value: u32) {
    len(output, value);
    output.extend_bytes(&[0x41]);
    write_i64(output, 65_536);
    output.extend_bytes(&[0x4b, 0x04, 0x40, 0x41, 0x00, 0x0f, 0x0b]);
}

/// `(i64 value, i64 prefix) -> i32`.
pub(in crate::wasm) fn emit_starts_with_body(body: &mut impl ByteOutput) {
    // i, value byte, prefix byte
    write_u32(body, 1);
    write_u32(body, 3);
    body.push(I32);
    bounded(body, 0);
    bounded(body, 1);
    len(body, 1);
    len(body, 0);
    body.extend_bytes(&[0x4b, 0x04, 0x40, 0x41, 0x00, 0x0f, 0x0b]);
    body.extend_bytes(&[0x41, 0x00, 0x21]);
    write_u32(body, 2);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 2);
    len(body, 1);
    body.extend_bytes(&[0x4f, 0x0d, 0x01]);
    get(body, 0, 2, 3);
    get(body, 1, 2, 4);
    body.extend_bytes(&[0x20]);
    write_u32(body, 3);
    body.extend_bytes(&[0x20]);
    write_u32(body, 4);
    body.extend_bytes(&[0x47, 0x04, 0x40, 0x41, 0x00, 0x0f, 0x0b]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 2);
    body.extend_bytes(&[0x41, 0x01, 0x6a, 0x21]);
    write_u32(body, 2);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b, 0x41, 0x01, 0x0b]);
}

/// Bounded deterministic substring scan. It is allocation-free; every byte
/// read remains authenticated by the existing byte provider.
pub(in crate::wasm) fn emit_contains_body(body: &mut impl ByteOutput) {
    // start, offset, value index, value byte, needle byte
    write_u32(body, 1);
    write_u32(body, 5);
    body.push(I32);
    bounded(body, 0);
    bounded(body, 1);
    len(body, 1);
    body.push(0x45);
    body.extend_bytes(&[0x04, 0x40, 0x41, 0x01, 0x0f, 0x0b]);
    len(body, 1);
    len(body, 0);
    body.extend_bytes(&[0x4b, 0x04, 0x40, 0x41, 0x00, 0x0f, 0x0b]);
    body.extend_bytes(&[0x41, 0x00, 0x21]);
    write_u32(body, 2);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 2);
    len(body, 0);
    len(body, 1);
    body.extend_bytes(&[0x6b, 0x4b, 0x0d, 0x01]);
    body.extend_bytes(&[0x41, 0x00, 0x21]);
    write_u32(body, 3);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 3);
    len(body, 1);
    body.extend_bytes(&[0x4f, 0x04, 0x40, 0x41, 0x01, 0x0f, 0x0b]);
    // value index = start + offset
    body.extend_bytes(&[0x20]);
    write_u32(body, 2);
    body.extend_bytes(&[0x20]);
    write_u32(body, 3);
    body.push(0x6a);
    body.push(0x21);
    write_u32(body, 4);
    get(body, 0, 4, 5);
    get(body, 1, 3, 6);
    body.extend_bytes(&[0x20]);
    write_u32(body, 5);
    body.extend_bytes(&[0x20]);
    write_u32(body, 6);
    body.extend_bytes(&[0x47, 0x0d, 0x01]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 3);
    body.extend_bytes(&[0x41, 0x01, 0x6a, 0x21]);
    write_u32(body, 3);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
    body.extend_bytes(&[0x20]);
    write_u32(body, 2);
    body.extend_bytes(&[0x41, 0x01, 0x6a, 0x21]);
    write_u32(body, 2);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b, 0x41, 0x00, 0x0b]);
}
