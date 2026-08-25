# Public Wasm Scalar Exports v1

Status: implemented as a bounded public Core-Wasm and generated JavaScript/
TypeScript package profile. Local executable evidence covers admission,
deterministic artifacts, Node consumption, status normalization, and stable-ID
rename preservation. Exact TypeScript 5.8.3 independently compiles the real
generated-declaration consumers for both the direct and Project packages. The
locked dual-package Chromium loopback job is exact-head hosted green on Ubuntu
at `27dbfafe0f6a3c7e68e0434a0a082020104f2241` in
[job 97930658621](https://github.com/wavect/semaprax/actions/runs/32887305666/job/97930658621).

## Purpose and command

The profile makes selected SEMAPRAX scalar functions callable from an ordinary
JavaScript or TypeScript shell without adding source syntax or exposing every
compiled function:

```sh
semaprax build calculator.spx --target web \
  --export calculator.add \
  --export calculator.divide \
  -o calculator-web
```

Selection is by persistent declaration identity, never by display name. A
source-level function rename that preserves `@id("calculator.add")` therefore
preserves the generated API key and raw Wasm adapter symbol.
Stable IDs beginning with `-` use the unambiguous `--export=<stable-id>` CLI
spelling; the ordinary separated spelling remains valid for all other IDs.

This is a build profile, not a package manager, Component Model interface, or
general JavaScript/TypeScript interoperability layer.

## Admission

The complete emitted program must satisfy all of these conditions:

- 1–32 distinct selected functions, each with an explicit persistent ID;
- export IDs contain 1–128 bytes from lowercase ASCII `a-z`, `0-9`, `.`, `_`,
  and `-`;
- every function is monomorphic and effect-free;
- the program contains at most 256 monomorphic executable functions, and every
  function has an explicit persistent ID;
- parameters are 0–8 by-value `i64` or `bool` values;
- results are `i64` or `bool`;
- no module permits, authored interfaces/imports, resources, records, variants,
  generic templates or instances, borrowed/shared values, callbacks, or async;
- no implicit ABI fallback for an excluded declaration or expression.

Selection is canonicalized into bytewise stable-ID order. Duplicate, missing,
automatic, malformed, over-limit, aggregate, generic, resource, imported, or
effectful selections fail with the profile diagnostics `SPX-W115` or
`SPX-W116` before output creation.

The whole-program restriction is deliberate. The existing aggregate Wasm lane
uses an out-pointer/status ABI and shadow-stack memory, while this profile uses
direct scalar adapters. Supporting selected scalar declarations inside an
aggregate/resource program would be a different, separately evidenced ABI.

## Wasm and binding ABI

SDK-mode modules export only the explicitly selected adapters. They do not
export the legacy `semaprax_main`, memory, unselected functions, or owned
resource adapters. Builds without `--export` retain the unchanged legacy
`semaprax.web.v3` package and `semaprax_main` behavior.

Each raw adapter name is `spx_scalar_` followed by lowercase hexadecimal for
the exact stable-ID bytes. This injective spelling is independent of source
names, declaration order, and the other selected exports. Adapters reject
non-canonical Wasm booleans; generated JavaScript accepts only `boolean` and
signed-range `bigint` arguments, and returns `boolean` or `bigint` values.

The generated facade exposes a frozen, null-prototype `functions` map plus
`call(stableId, ...arguments)`. Calls return a closed discriminated result:

```ts
type ScalarResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; status: ScalarStatus }>;
```

Checked failures use `semaprax.status.v1`. Arithmetic cases preserve the
repository-wide codes: addition overflow 1, subtraction overflow 2,
multiplication overflow 3, division by zero 4, signed division overflow 5,
remainder by zero 6, signed remainder overflow 7, and negation overflow 8.
Contract precondition and postcondition failures use
`semaprax.contract.v1` codes 1 and 2. Only the runtime's private branded
semantic failure is normalized; an unknown JavaScript exception, missing raw
adapter, or Wasm trap remains an out-of-band failure rather than being
misreported as a language status.

## Package and integrity binding

The destination must not exist and its parent directory must already exist.
The caller must exclusively control that parent and the new output tree for the
whole publication. Concurrent same-authority rename, replacement, insertion,
deletion, symlink/reparse creation, or byte mutation is outside this v1 threat
model. Admission and rendering finish before the directory is created.
Publication rejects symlink/reparse parents and children, retains parent and
destination identities, performs every fixed-name write with create-new,
rebinds both identities, and immediately replays the exact inventory and bytes
before success. Failure cleanup first reauthenticates both identities, removes
only expected-name regular files whose bytes still match the rendered bytes,
and never recursively deletes a directory. This path-based trusted-parent
protocol is not a lock and is not hostile-concurrent-writer evidence. The
profile writes this exact inventory:

- `app.wasm`
- `semaprax.js`
- `semaprax.bindings.js`
- `semaprax.bindings.d.ts`
- `semaprax.scalar-exports.json`
- `package.json`
- `index.html`

`semaprax.scalar-exports.json` uses `semaprax.web.v4` and contains the module,
Graph revision, an empty capability set, ordered scalar ABI facts, and exact
SHA-256 digests for all six non-manifest artifacts. The generated runtime embeds the exact Wasm digest,
copies caller-provided bytes, authenticates them with Web Crypto before
instantiation, and rejects substitution.

These digests bind artifacts when the generated runtime or manifest is already
trusted. They are not signatures, provenance, reproducible-toolchain proof,
sandboxing, or authority.

## Rename and compatibility contract

For a semantic rename that preserves the declaration's explicit stable ID and
behavior:

- the JavaScript API key and raw Wasm adapter name remain unchanged;
- the TypeScript call signature remains unchanged;
- the function remains callable through the same stable-ID key;
- source display names are not part of the public scalar ABI;
- the Graph revision and its containing manifest are expected to change.

The v1 manifest and bindings are pre-1.0 public formats. Any incompatible
change requires a schema/profile version bump, migration note, and preservation
test for legacy web v3 output.

## Evidence gate and nonclaims

Promotion requires:

- exact positive multi-export `i64`/`bool` calls and boundary values;
- all eight arithmetic cases plus precondition and postcondition failures;
- wrong type/count, unknown ID, duplicate selection, and every excluded shape;
- exact Wasm export/type inventory with no legacy or unselected export;
- deterministic double build, canonical manifest replay, artifact-digest
  mutation rejection, and fresh/no-clobber output behavior;
- an actual stable-ID semantic rename followed by byte/API/behavior checks;
- native/Core-Wasm scalar outcome and status equivalence;
- generated JavaScript execution under Node;
- strict compilation of a generated-declaration consumer with a pinned
  TypeScript compiler;
- the locked Ubuntu Chromium loopback calculator interaction in
  `platform-tests/wasm-scalar-browser-v1` (one worker, no retries), plus its
  exact-head hosted-green evidence;
- formatting, strict Clippy, Rust 1.85, package/source locks, the full hosted
  Ubuntu/macOS/Windows matrix, and independent security review.

The exact-head hosted Chromium/TypeScript job proves the generated direct and
Project calculator packages under one pinned browser on Ubuntu loopback. It
does not establish general browser-SDK compatibility, multi-engine conformance,
external-network behavior, or production-browser compatibility. It also claims
no Components, WIT, npm
publication, dependency resolution, imports/capabilities, resources,
aggregates, strings, typed arrays, promises, callbacks, async, workers,
cross-realm identity, CSP generation, SSR/hydration, UI dialect, provenance,
signing, or production readiness.
