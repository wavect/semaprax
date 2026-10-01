#!/usr/bin/env python3
"""Test-only assembly from the production TypeScript caller templates.

Like public_generic_consumer_fixture.py, this is NOT a compiler/descriptor
producer. The Cargo bridge compares all files against actual generator output.
The tiny Wasm binary is the existing, explicitly hand-assembled endpoint fixture,
with an explicit memory-page parameter. It is not a compiled provider ABI.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from public_generic_consumer_fixture import bindings as native_bindings, frame, identities
from public_generic_settlement_evidence import digest

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'src/public_generic_consumer/typescript_calling/render'
ENDPOINT = 'spx_pg_wasm_endpoint_reverse_bytes_v1'
DOMAIN = 'semaprax.public-generic-typescript-wasm-consumer.v1.module-artifact'


def leb(value: int) -> bytes:
    if type(value) is not int or not 0 <= value < 2**32:
        raise ValueError('fixture-integer')
    result = bytearray()
    while True:
        byte = value & 127
        value >>= 7
        result.append(byte | (128 if value else 0))
        if not value:
            return bytes(result)


def wasm_bytes(max_pages: int = 257) -> bytes:
    if type(max_pages) is not int or not 1 <= max_pages <= 257:
        raise ValueError('fixture-memory-bound')
    def section(tag: int, data: bytes) -> bytes:
        return bytes([tag]) + leb(len(data)) + data
    def name(value: str) -> bytes:
        b = value.encode('ascii')
        return leb(len(b)) + b
    # Exact instruction stream of reference_wasm_module::build: i/j/tmp
    # locals, byte-swap loop, no imports, no host-side endpoint implementation.
    code = bytes.fromhex(
        '20002102200020016a41016b210302400340200220034e0d01'
        '20022d00002104200220032d00003a0000200320043a0000'
        '200241016a2102200341016b21030c000b0b0b')
    body = bytes.fromhex('01037f') + code
    return (b'\0asm\x01\0\0\0' + section(1, bytes.fromhex('0160027f7f00'))
        + section(3, bytes.fromhex('0100'))
        + section(5, b'\x01\x01\x01' + leb(max_pages))
        + section(7, b'\x02' + name('memory') + b'\x02\x00' + name(ENDPOINT) + b'\x00\x00')
        + section(10, b'\x01' + leb(len(body)) + body))


def bindings(module: bytes) -> tuple[bytes, bytes]:
    descriptor, _ = native_bindings()
    carrier = b''.join(frame(x.encode('ascii')) for x in [
        'semaprax.public-generic-carrier.v1', 'sha256:' + '9' * 64,
        'core-wasm', 'runtime:core-wasm-fixture-issue-157'])
    binding = b''.join(map(frame, [b'semaprax.public-generic-wasm-adapter.v1', carrier, b'v1',
        digest(DOMAIN, module).encode('ascii'), ENDPOINT.encode('ascii'),
        b'semaprax-0.4.1', b'unsupported-unpublished']))
    return descriptor, binding


def asset(name: str) -> str:
    return (ASSETS / name).read_text(encoding='utf-8').replace('\r\n', '\n')


def literal(data: bytes) -> str:
    return 'Uint8Array.from([\n' + ''.join('  ' + ''.join(f'0x{x:02x}, ' for x in data[n:n+12]) + '\n'
        for n in range(0, len(data), 12)) + '])'


def render(count: int, module: bytes) -> dict[str, str]:
    ids = identities(count)
    fields = ['field_' + identity.encode().hex() for identity in ids]
    descriptor, binding = bindings(module)
    desc = asset('descriptor_header.ts.txt') + '\n'
    desc += ('const MODULE_ARTIFACT_DIGEST_DOMAIN = new TextEncoder().encode(\n'
        '  "semaprax.public-generic-typescript-wasm-consumer.v1.module-artifact\\0",\n);\n\n')
    desc += 'export const TRUSTED_DESCRIPTOR_BYTES: Uint8Array = ' + literal(descriptor) + ';\n\n'
    desc += 'export const TRUSTED_BINDING_BYTES: Uint8Array = ' + literal(binding) + ';\n\n'
    desc += 'export const TRUSTED_PROVIDER_ARTIFACT_DIGEST: string = ' + json.dumps(digest(DOMAIN, module)) + ';\n'
    desc += 'export const TRUSTED_ENDPOINT_EXPORT_NAME: string = ' + json.dumps(ENDPOINT) + ';\n'
    desc += 'export const TRUSTED_WASM_ADAPTER_ABI_VERSION: string = "v1";\n'
    desc += 'export const TRUSTED_COMPILED_PROVIDER: boolean = false;\n'
    desc += asset('descriptor_verify.ts.txt')
    def interface(name: str) -> str:
        return f'export interface {name} {{\n' + ''.join(
            f'  /** Field identity: {json.dumps(identity)} */\n  readonly {field}: Uint8Array;\n'
            for identity, field in zip(ids, fields)) + '}\n'
    types = asset('types_header.ts.txt') + '\n' + interface('Input') + '\n' + interface('Output')
    carrier = asset('carrier_header.ts.txt') + '\n' + f'export const FIELD_COUNT = {count};\n\n'
    # Dynamic fragments mirror render.rs and are byte-compared by Cargo.
    carrier += 'function inputLeaves(value: Input): readonly Uint8Array[] {\n  return readInputFields(value, [\n'
    carrier += ''.join(f'    {json.dumps(field)},\n' for field in fields) + '  ]);\n}\n\n'
    carrier += 'function outputFromLeaves(leaves: readonly Uint8Array[]): Output {\n  return {\n'
    carrier += ''.join(f'    {field}: leaves[{i}] as Uint8Array,\n' for i, field in enumerate(fields)) + '  };\n}\n\n'
    carrier += ('export function encodeInput(value: Input): Uint8Array {\n  return encodeLeaves(inputLeaves(value));\n}\n\n'
        'export function decodeOutput(bytes: Uint8Array): Output {\n  return outputFromLeaves(decodeLeaves(bytes, FIELD_COUNT));\n}\n')
    descriptor_identity = digest('semaprax.public-generic-descriptor.v1.identity',
        descriptor[:-len(frame(b'transform'))])
    carrier += '\nexport const CARRIER_DESCRIPTOR_DIGEST = ' + json.dumps(descriptor_identity)
    carrier += ';\nexport const CARRIER_EXPORT_ID = "sample.transform";\n'
    carrier += 'export const INPUT_INSTANCE_DIGEST = "sha256:' + '4' * 64 + '";\n'
    carrier += 'export const OUTPUT_INSTANCE_DIGEST = "sha256:' + '5' * 64 + '";\n'
    carrier += 'export const INPUT_LEAF_PATHS = Object.freeze([\n'
    carrier += ''.join('  ' + json.dumps(identity) + ',\n' for identity in ids)
    carrier += ']);\nexport const OUTPUT_LEAF_PATHS = Object.freeze([\n'
    carrier += ''.join('  ' + json.dumps(identity) + ',\n' for identity in ids)
    carrier += (']);\n\nexport function encodeCanonicalInput(value: Input): Uint8Array {\n'
        '  return encodeCanonicalFrame("input", CARRIER_DESCRIPTOR_DIGEST, CARRIER_EXPORT_ID, INPUT_INSTANCE_DIGEST, INPUT_LEAF_PATHS, inputLeaves(value));\n}\n\n'
        'export function decodeCanonicalOutput(bytes: Uint8Array): Output {\n'
        '  return outputFromLeaves(decodeCanonicalFrame("result", CARRIER_DESCRIPTOR_DIGEST, CARRIER_EXPORT_ID, OUTPUT_INSTANCE_DIGEST, OUTPUT_LEAF_PATHS, bytes));\n}\n')
    round_trip = asset('round_trip_header.mjs.txt') + '\n' + 'function sampleInput() {\n  return {\n'
    round_trip += ''.join(f'    {field}: new TextEncoder().encode("sample-{i}"),\n' for i, field in enumerate(fields)) + '  };\n}\n\n'
    round_trip += 'function inputWithFirstField(bytes) {\n  return {\n'
    round_trip += ''.join(f'    {field}: ' + ('bytes' if i == 0 else 'new Uint8Array(0)') + ',\n'
        for i, field in enumerate(fields)) + '  };\n}\n\n'
    # Filled from the exact fixed template plus per-shape assertions below.
    round_trip += 'function assertReversed(output, original) {\n'
    round_trip += ''.join(f'  assert.deepEqual(output.{field}, reversed(original.{field}));\n' for field in fields) + '}\n\n'
    round_trip += f'function FIRST_OUTPUT_FIELD(output) {{\n  return output.{fields[0]};\n}}\n'
    round_trip += asset('round_trip_body.mjs.txt')
    files = {name: asset(name + '.txt') for name in ['package.json', 'package-lock.json', 'tsconfig.json']}
    files.update({'src/errors.ts': asset('errors.ts.txt'), 'src/descriptor.ts': desc,
        'src/types.ts': types, 'src/carrier.ts': carrier,
        'src/wasm-provider.ts': asset('wasm-provider.ts.txt'), 'src/index.ts': asset('index.ts.txt'),
        'test/round-trip.mjs': round_trip})
    return files


def write(path: Path, count: int, module: bytes) -> None:
    for name, text in render(count, module).items():
        file = path / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text, encoding='utf-8', newline='\n')
    (path / 'reference.wasm').write_bytes(module)




def module_variants() -> dict[str, bytes]:
    """Authenticated test-only subjects for the admission/refusal branches.

    These retain explicit fixture provenance; none claims a compiler export.
    """
    base = wasm_bytes()
    def read_leb(data: bytes, cursor: int) -> tuple[int, int]:
        value, shift = 0, 0
        while True:
            byte = data[cursor]
            cursor += 1
            value |= (byte & 127) << shift
            if byte < 128:
                return value, cursor
            shift += 7
    sections = {}
    cursor = 8
    while cursor < len(base):
        tag = base[cursor]
        length, start = read_leb(base, cursor + 1)
        sections[tag] = base[start:start + length]
        cursor = start + length
    def emit(parts: dict[int, bytes]) -> bytes:
        return base[:8] + b''.join(bytes([tag]) + leb(len(data)) + data for tag, data in sorted(parts.items()))
    def name(value: str) -> bytes:
        data = value.encode('ascii')
        return leb(len(data)) + data
    def replace(tag: int, content: bytes) -> bytes:
        return emit({**sections, tag: content})
    exports = sections[7]
    variants = {
        'grow-limit': wasm_bytes(1),
        'malformed-module': base[:-1],
        'unexpected-import': emit({**sections, 2: b'\x01' + name('env') + name('callback') + b'\x00\x00'}),
        'extra-export': replace(7, b'\x03' + exports[1:] + name('extra') + b'\x00\x00'),
        'missing-endpoint': replace(7, b'\x01' + name('memory') + b'\x02\x00'),
        'wrong-export-kind': replace(7, b'\x02' + name('memory') + b'\x02\x00' + name(ENDPOINT) + b'\x02\x00'),
        'shared-memory': replace(5, b'\x01\x03\x01' + leb(257)),
        'zero-memory': replace(5, b'\x01\x01\x00' + leb(257)),
        'oversized-memory': replace(5, b'\x01\x01' + leb(258) + leb(258)),
        'endpoint-trap': replace(10, b'\x01\x03\x00\x00\x0b'),
    }
    # Two functions: existing endpoint, then ()->() start function which traps.
    _, position = read_leb(sections[10], 0)
    endpoint_body = sections[10][position:]
    variants['start-trap'] = emit({**sections,
        1: bytes.fromhex('0260027f7f00600000'), 3: bytes.fromhex('020001'),
        8: b'\x01', 10: b'\x02' + endpoint_body + bytes.fromhex('0300000b')})
    body_len, body_start = read_leb(sections[10], 1)
    body = sections[10][body_start:body_start + body_len]
    # memory.grow(0) detaches the old view without increasing retained pages.
    grow_body = body[:3] + bytes.fromhex('410040001a') + body[3:]
    variants['endpoint-grow'] = replace(10, b'\x01' + leb(len(grow_body)) + grow_body)
    target = 16777216
    payload_size = target - len(base) - 5  # section id + four-byte LEB length
    variants['module-exact-max'] = base + b'\x00' + leb(payload_size) + b'\x01x' + bytes(payload_size - 2)
    assert len(variants['module-exact-max']) == target
    return variants


def subjects() -> dict[str, tuple[int, bytes]]:
    result = {f'fields-{n}': (n, wasm_bytes()) for n in [1, 2, 256]}
    result.update({name: (2, module) for name, module in module_variants().items()})
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--fields', type=int, default=2)
    parser.add_argument('--pages', type=int, default=257)
    args = parser.parse_args()
    write(args.output, args.fields, wasm_bytes(args.pages))
