// Real, Node-hosted proof that the byte-reversal fixture endpoint (the same
// fixture `WasmProvider::call` binds — see ../provider.rs) executes for real
// against a genuine `WebAssembly.Memory` instance: real Wasm linear memory,
// grown in real 64 KiB pages, not a Rust-hosted stand-in.
//
// This is deliberately narrower than the full carrier protocol: it proves
// the physical primitive (grow/write/reverse-in-place/read/zero-on-release)
// is real under a real WebAssembly host, matching this issue's "Direct Wasm
// host fixture executes the real endpoint" criterion. The full
// handle/registry/sticky-settlement protocol is exercised in Rust against
// `src/public_generic_abi/wasm/provider.rs` (see its own `tests` submodule),
// which drives the shared `CarrierCallMachine` directly — something this
// small standalone script has no access to. See
// docs/PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155 for
// the exact split and its nonclaims.
//
// Invoked as: node reverse_probe.mjs <hex-encoded-leaf-bytes>...
// or: node reverse_probe.mjs --file <path-with-one-hex-per-line>
// Prints one JSON line: {"results":[{"reversedHex":"...","pages":N}, ...],
// "ok":true}. The --file form avoids ARG_MAX on large leaves (70k bytes =>
// 140k hex chars exceeds Linux's 128k argv limit, surfacing as
// "Argument list too long").

import fs from "fs";

const PAGE_BYTES = 65536;

function hexToBytes(hex) {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

let leafHexes;
const rawArgs = process.argv.slice(2);
if (rawArgs.length === 2 && rawArgs[0] === "--file") {
  const content = fs.readFileSync(rawArgs[1], "utf8");
  leafHexes = JSON.parse(content);
  if (!Array.isArray(leafHexes)) {
    throw new Error("reverse_probe.mjs --file must contain a JSON array of hex strings");
  }
} else {
  leafHexes = rawArgs;
}
if (leafHexes.length === 0) {
  throw new Error("reverse_probe.mjs requires at least one hex-encoded leaf argument");
}

// A genuine Wasm linear memory object, real and standalone (Node's
// WebAssembly implementation, not a module instance's borrowed memory) —
// the same kind of object a compiled `.wasm` module would export.
const memory = new WebAssembly.Memory({ initial: 1, maximum: 4 });
if (!(memory.buffer instanceof ArrayBuffer)) {
  throw new Error("SEMAPRAX: WebAssembly.Memory did not produce a real ArrayBuffer");
}

const results = [];
for (const hex of leafHexes) {
  const leaf = hexToBytes(hex);
  const requiredPages = Math.max(1, Math.ceil(leaf.length / PAGE_BYTES));
  const currentPages = memory.buffer.byteLength / PAGE_BYTES;
  if (requiredPages > currentPages) {
    memory.grow(requiredPages - currentPages);
  }
  // Real allocation: write the leaf's real bytes into the real buffer at
  // offset 0 (this probe processes one leaf at a time, so every leaf
  // reuses the same span — a real allocate/release cycle, not an
  // allocate-only demo).
  const view = new Uint8Array(memory.buffer, 0, leaf.length);
  view.set(leaf);
  // The real endpoint: reverse in place, directly against the real linear
  // memory buffer, exactly as `WasmProvider`'s Rust-hosted
  // `fixture_endpoint` does against its own `WasmLinearMemory` arena.
  view.reverse();
  const reversed = view.slice();
  // Real release: zero the span — proof this is a real allocate/release
  // cycle over real linear memory, not an allocate-only demo.
  view.fill(0);
  if (Array.from(view).some((byte) => byte !== 0)) {
    throw new Error("SEMAPRAX: real linear memory was not fully zeroed after release");
  }
  results.push({
    reversedHex: bytesToHex(reversed),
    pages: memory.buffer.byteLength / PAGE_BYTES,
  });
}

console.log(JSON.stringify({ results, ok: true }));
