# Standalone internal String Web package v1

Status: authored implementation; all new executable gates are unrun.
No completion-matrix row or supported-host claim is promoted.

Audience: compiler contributors, generated-package consumers and reviewers.

## Purpose and selection

This package makes the existing [standalone internal String compiler and
runtime](WASM-INTERNAL-STRINGS-V1.md) usable from the source build command.
It introduces no new language semantics, Wasm ABI, public String parameter or
result, Project profile, npm carrier, or runtime implementation.

```sh
semaprax build app.spx --target web --profile internal-strings-v1 \
  --export app.main -o app-web
```

The `wasm` target alias is equivalent. The profile requires an explicit source
input and 1..=32 selected stable export identities. It is unavailable for
Project inputs, including the implicit default manifest, and for other targets.
Unknown/repeated profiles, missing exports and incompatible options fail with
CLI usage exit 2 before source loading or filesystem effects. Existing
`--export=<stable-id>` syntax remains available for leading-dash identities.
This tranche has no CLI quota override; existing `InternalStringOptions`
defaults apply unchanged.

The public library entry is
`wasm::internal_strings::build_web_from_source(&Path, &Path, &[String]) -> Result<(), Vec<Diagnostic>>`.
The existing `emit_module` remains compilation-only and byte-identical for the
same admitted input. The new build entry alone owns explicit source reads and
publication through the already existing Web publisher.

## Bounded source and admission before effects

The new CLI branch precedes the legacy unbounded `checked`/`load` route. It
uses the existing regular-file, canonical-identity source snapshot helper with
a 16-MiB byte limit, then parses, runs ordinary verification and invokes the
existing standalone compiler. Original parser/verifier/compiler diagnostics
are preserved. Initial source-size and package-size bounds use `SPX-W111`; source
identity/read failures retain the existing `SPX-I201`/`SPX-I207` diagnostics.
This does not acquire an A0 lock or source mutation authority.

Immediately before publication, the same bounded source helper rechecks source
identity, exact contents and Graph revision. Any final read failure, including
growth beyond the source limit, is drift (`SPX-I207`) and fails before creating
the destination. This is a checked snapshot, not a source lock or a promise against
future edits after that final observation.

All artifacts, their checked lengths, manifest digests and canonical manifest
consistency are prepared before invoking the publisher. The exact existing
compiler descriptor is limited to 1 MiB for this package; the complete package,
including its manifest, is limited to 32 MiB. Wasm retains its existing 16-MiB
limit. These are source/output bounds, not total compiler or JavaScript heap,
wall-time, fuel, cancellation or garbage-collection guarantees.

Selected closure, scalar signatures, canonical HIR ownership, fixed memory,
String owner/liveness policy, cleanup, arithmetic/contract outcomes and capacity
causes remain those of the compiler/runtime contract. Unselected valid source
does not widen or alter the selected runtime. No generics, recursive selected
closure, public String boundary or effects are admitted implicitly.

## Fixed package and binding

The exact ordered inventory is:

1. `app.wasm`
2. `semaprax.js`
3. `semaprax.d.ts`
4. `semaprax.internal-strings.json`
5. `semaprax.manifest.json`
6. `package.json`
7. `index.html`
8. `app.js`

Wasm, runtime JavaScript and compiler descriptor are the exact existing
`InternalStringModule` outputs, without wrappers or added newlines. No artifact
path is derived from a source name, module name or stable identity.

The separate canonical package manifest uses
`semaprax.web-internal-strings.v1`. In fixed field order it records `schema`,
`module`, `source_digest`, `graph_revision`, `compiler_schema`, `runtime_schema`,
`capabilities` (the empty array), and `artifacts`. Artifact rows follow the
inventory order with the manifest omitted, each recording `path`, `bytes`, and
`sha256`. Source and artifact digests use `sha256:<lowercase hex>`. There is no
self-digest, signature, provenance guarantee, new replay authority or claim of
target execution. A display-only rename may change source/Graph/package
manifest facts without changing stable-ID consumer APIs.

The private ES-module `package.json` exports `./semaprax.js` and names
`./semaprax.d.ts` as its types. Declarations mirror only the existing named
`instantiate(bytes: Uint8Array)` and frozen `{ call }` API. Each stable-ID
overload admits its exact primitive `bigint`/`boolean` arguments and result.
There is no arbitrary-string fallback, default export, `functions` map,
String argument/result, exposed raw instance or memory.

Outcomes stay discriminated as `success` with a scalar value; `failure` with
the exact arithmetic or contract domain/code; or `capacity` with one of
`owners`, `value_bytes`, `live_bytes`, `cumulative_bytes`, `tokens`. Exceptions
remain terminal runtime errors or ordinary preflight errors as specified by
the existing runtime, never fabricated language outcomes.

## Browser consumer

The browser files are fixed generator-owned templates, not generated inline
source/identity markup. Only the descriptor's compiler-derived hexadecimal
digest and decimal byte length are substituted into external `app.js`; no
source text, module name or stable identity enters its executable template.
External `app.js` loads only the fixed local descriptor
and Wasm URLs, bounds descriptor response bytes to 1 MiB and Wasm response bytes
to 16 MiB while reading, authenticates exact descriptor length/digest before
parsing its closed signature facts, and instantiates the existing authenticated
runtime. Descriptor facts construct controls, not runtime authority; the
runtime independently enforces its embedded signatures. Digest authentication
assumes the generated JavaScript is already trusted, not signed provenance.
It does not invoke a selected function until a user action. Controls reflect
the actual scalar signature; results distinguish success, checked failure and
capacity. User-controlled text is rendered through DOM `textContent`, not
`innerHTML`, executable strings or interpolated inline script.

The page is a small keyboard-accessible function console, not a new UI framework.
It uses local system fonts and no downloaded dependencies. It requires a local
HTTP server/appropriate Web Crypto context; direct file opening is not promised.
The runtime remains synchronous and trusted-realm: an admitted loop can occupy
the browser thread. This package adds no sandbox, deadline, Worker isolation,
cross-realm guarantee or broad browser-support claim.

## Publication authority and failure

The route reuses the existing scalar Web package publisher unchanged. Output
must not exist; its real non-reparse parent must already exist and be under
the caller's exclusive control throughout publication. It creates no parent
directories and does not use Project publication or parent-preparation code.
All names are fixed compiler-owned leaves. Writes use create-new, retained
parent/destination identities and exact inventory/byte authentication.

This path-based protocol is not atomic directory publication or protection
against a hostile concurrent same-authority writer. Cleanup stops on identity
uncertainty and removes only expected regular files matching the complete
rendered bytes. Partial, changed or foreign files can remain as inert residue;
failure does not universally mean an absent destination. No recursive cleanup
or rollback guarantee is added. Existing `SPX-I301`, `SPX-I302` and `SPX-I307`
publication diagnostics, including inherited wording, remain unchanged.

## Legacy correction and compatibility

Legacy `build_web` currently emits String imports that its browser runtime
does not supply. It must reject String-bearing ordinary or materialized generic
functions before directory creation, using `SPX-W116`, instead of reporting
success for an unlinkable package. Guidance points to this explicit profile
for admitted scalar exports; it does not promise that generics/resources or
every rejected legacy program fit the new route.

This is an explicit admission correction, not silent migration. Successful
String-free legacy Web v3 bytes, scalar Web v4 admission/output, ordinary raw
Wasm emission and every Project v1-v10 route remain unchanged. The old raw
emitter's separate String settlement/coverage gaps are not declared solved.

## Required authored evidence

Before promotion, require actual CLI packages, exact reopened inventory and
canonical manifest/digest checks, equality with direct compiler outputs,
deterministic repeated builds, stable-ID rename witnesses, hostile source-valid
identities, and preserved earlier Web/Project known answers. Node consumers
must use the real generated runtime for scalar/Boolean success, internal
NUL/Unicode, checked failures and reuse, capacity and reuse, malformed bytes,
and exact opaque facade shape.

Author source/descriptor/package exact-bound and plus-one cases, source drift,
invalid profiles and closure diagnostics before effects, existing destination
and foreign-byte preservation, and supported-host symlink/reparse rejection.
Legacy rejection includes String literals in materialized generic bodies.
Strict TypeScript and real-browser consumers must use explicit provisioned
tools without downloading or silently skipping missing prerequisites. Existing
raw owner-accounting evidence remains separately required.

All new executable gates are unrun. Formatting, literal checks and static
review do not establish release, supported browsers, npm publication, ordinary
Wasm settlement or overall production readiness.
