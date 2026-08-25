# Project Manifest v3

Status: locally evidenced, Partial. Exact-head hosted promotion, safe Windows
v2 package publication, npm registry publication, and release promotion remain
open.

Project Manifest v3 is the additive public Project boundary for the bounded
[`useful-data.v1`](PORTABLE-INDEXED-BYTE-DATA-V1.md) profile. It does not
reinterpret or re-render Project Manifest v1 or v2.

## Canonical manifest

The canonical manifest has exactly eight assignments in this order:

```toml
schema = "semaprax.project.v3"
name = "binary-frame"
version = "1.0.0"
profile = "useful-data.v1"
entry = "binary_frame.app"
sources = ["src/app.spx", "src/core.spx", "src/frame.spx", "src/tests.spx"]
web_exports = ["binary-frame.checksum", "binary-frame.combine-length", "binary-frame.has-magic", "binary-frame.length"]
tests = ["binary_frame.tests"]
```

`ProjectProfile` is closed. Schema v3 admits exactly `useful-data.v1`; profile
and schema confusion rejects rather than falling back to a boolean or legacy
route. V1 and v2 canonical bytes and diagnostics remain independently frozen.

## Linking and semantic authority

One held Project snapshot authenticates the manifest and complete source set.
The profile links the exact entry closure, sole test closure, and every selected
stable-ID Web export root. The linker rebuilds and validates compiler-owned
byte-operation call-index facts, slice provenance/value facts, capacity facts,
and cleanup instead of trusting or flattening source-shaped data. The linked
test closure uses the same useful-data profile as the entry; it is not replaced
by a scalar surrogate.

Semantic Workspace/Project source preflight and replay admit source Graph
schemas v10 through v17. This does not widen Workspace Patch or Change evidence
admission: their existing schema gates remain separate and fail closed.

## Public Web and npm boundary

Selected public functions accept only borrowed `Slice<u8>` inputs and return
`i64`, `bool`, or `usize`. Internal fixed arrays and owned `Bytes` may occur in
the authenticated closure, but are not public JavaScript return carriers.

The Core-Wasm adapter uses fixed memory and checked offset/length inputs. The
generated JavaScript facade accepts an ordinary attached, fixed-length
`Uint8Array`; it rejects shared, resizable, detached, differently typed, and
coercible inputs. It snapshots every accepted argument before reusing public
scratch, enforces cumulative input bounds, and authenticates the exact Wasm and
data-export metadata before invocation. TypeScript exposes the corresponding
exact `Uint8Array` and scalar signatures.

The npm route emits exactly:

1. `app.wasm`
2. `semaprax.js`
3. `semaprax.bindings.js`
4. `semaprax.bindings.d.ts`
5. `semaprax.data-exports.json`
6. `package.json`

The context-bound `semaprax.project-npm-build.v2` carrier binds the retained
Project facts, canonical data-export plan, ordered artifacts, bytes, digests,
and payload digest. Independent inspection proves compiler consistency and
rejects tampering, but does not authenticate self-claimed Project authority or
mint a publishable build. Only an opaque build prepared from the retained
Project snapshot may authorize publication.

Unix publication resolves and authenticates the parent once and performs
handle-relative create-new effects without following substituted paths or
clobbering existing bytes. The v2 publication route deliberately fails closed
on Windows until the public crate has an equivalently strong handle-relative
primitive. This asymmetry is a safety boundary, not Windows support evidence.

## Local executable evidence

`examples/binary-frame-project` exercises a fixed magic array, `Slice<u8>`,
bounded `while`, total indexed reads, owned `Bytes` copy/move/drop behavior,
explicit stable-ID exports, and a useful-data test closure. Focused local tests
cover canonical v3 parsing/rendering and v1/v2 preservation, graph/linking
replay, interpreter test execution, Core-Wasm emission, strict JavaScript and
TypeScript generation, carrier replay/tamper rejection, Unix publication, and
offline pack/install followed by compiler-free installed consumption.

This evidence does not claim exact-head Linux/macOS/Windows/Rust-1.85 success,
safe Windows v2 publication, npm registry behavior, package signing,
provenance, compatibility resolution, Component Model support, or release
promotion. Completion-matrix statuses and totals therefore do not change.
