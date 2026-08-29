# Offline Effect-Free Scalar Core-Wasm Package Build v1

Status: additive implementation and evidence authored, unrun and unpromoted.
Audience: compiler, package-tooling, and platform-authority contributors.

Offline Effect-Free Scalar Core-Wasm Package Build v1 consumes one exact Offline
Deterministic Package Resolver v1 evidence envelope and its original caller-owned
input. It independently replays resolution and Lock v2, recovers only the exact
selected subject from that authenticated catalog, replays its embedded
canonical source through the ordinary parser, verifier, and HIR resolver, and
emits one deterministic Core-Wasm module through the existing Public Scalar
Export Profile v1.

The effect-free builder is authority-free. A separate safe publisher independently
replays the complete build before it acquires destination authority, creates one
new staged directory with an exact three-file inventory, and publishes it with
no replacement. Evidence never serves as a commit token.

This version executes no external program and makes no hermetic-sandbox claim.

## Frozen public surface

The compiler crate exposes:

```text
OfflinePackageBuildOptions {
    root_package: String,
    exports: Vec<String>,
    max_artifact_bytes: usize,
    max_evidence_bytes: usize,
}

OfflinePackageBuild {
    module_wasm: Vec<u8>,
    manifest_json: String,
    evidence_json: String,
}

VerifiedOfflinePackageBuild {
    root_package: String,
    packages: Vec<Coordinate>,
    wasm_sha256: String,
    artifact_bytes: usize,
}

generate(
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    build_options: &OfflinePackageBuildOptions,
) -> Result<OfflinePackageBuild, Vec<Diagnostic>>

verify(
    build: &OfflinePackageBuild,
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    build_options: &OfflinePackageBuildOptions,
) -> Result<VerifiedOfflinePackageBuild, Diagnostic>
```

The publisher crate exposes one consuming, stateless `publish` function. It
accepts an output path plus owned build, resolver evidence, original resolver
input/options, and build options. It calls the compiler verifier before any
destination authority is acquired and again from retained invocation-local
inputs before the no-replace publication boundary. Its handle-owning authority
state is private.

## Admission

- `resolution_input.target` is exactly `wasm32`.
- `resolution_input.allowed_capabilities` and every selected subject capability
  inventory are empty.
- The resolver input has exactly one root requirement, its package equals
  `root_package`, and replay selects exactly one package coordinate.
- The selected Subject v2 declares no dependencies. Report v2 intentionally
  authenticates its source through the ordinary single-file resolver, so a
  future multi-package build requires a new source capsule designed for
  workspace resolution; dependency metadata may not stand in for linked code.
- `exports` contains one through 32 unique stable IDs in strict byte order.
- Stable IDs use the unchanged Public Scalar Export Profile v1 grammar.
- Every selected Subject v2 is associated with exactly its resolver-selected
  coordinate and is independently source-replayed through Report v2.
- The embedded source module name equals its package identity. Package names
  outside the existing module-name grammar, including names containing `-`,
  are intentionally outside this build profile even though Resolver v1 admits
  them.
- The source has no `use fn` or `use type` declarations.
- The source declares exactly one explicit `main` with the existing scalar
  project signature.
- The source has empty `permit`, interface, native-import, generic, resource,
  and authored aggregate inventories under the existing scalar profile.
- The resolved HIR has no function effects and passes the ordinary HIR validator.
- Every selected export is an explicit monomorphic function declared by the
  root module and is admitted by the unchanged Public Scalar Export Profile v1
  (`i64` and `bool` only).
- The source-replayed report has `wasm32: available`; this is
  compiler projection evidence, not execution or conformance evidence.

All input and cardinality bounds are checked before Wasm emission. Each
variable artifact builder is capped and metered, and the final cumulative
checked-add is enforced before return or staging. Build options admit
`max_artifact_bytes` from 4 KiB through 16 MiB and
`max_evidence_bytes` from 4 KiB through 16 MiB. The combined canonical source
input remains bounded by the existing Report and Resolver limits. The Wasm
module, manifest, and evidence each count toward the cumulative
artifact bound using checked addition.

## Canonical artifacts and evidence

The fixed published inventory is, in byte order:

```text
module.wasm
semaprax.package-build.evidence.json
semaprax.package-build.json
```

The manifest deliberately binds only `module.wasm`; evidence separately
authenticates the manifest and module to avoid a cyclic hash. The publisher
authenticates the complete three-file inventory.

`semaprax.package-build.json` uses schema
`semaprax.offline-effect-free-wasm-package-build.v1` and exact member order:
`schema,profile,root,packages,exports,runtime_imports,module,compiler,limits,nonclaims`.
The profile is `effect-free-core-wasm-scalar.v1`. Coordinates use
`package,version`; selected subject facts use
`package,version,subject_digest,source_revision`; export facts use
`stable_id,wasm_export,parameters,result`. `runtime_imports` binds the exact
compiler-required `env` function inventory `spx_add`, `spx_sub`, `spx_mul`,
`spx_div`, `spx_rem`, `spx_neg`, and `spx_contract_fail`; these imports are a
runtime semantic dependency, not declared SEMAPRAX capability authority.
`module` has exact order `path,sha256,bytes`. Compiler facts bind only
`package,version`. An optional environment-dependent Git commit is not bound:
it is build metadata, not provenance, and must not make identical compiler
source emit different package bytes. No host path, clock, nonce, locale,
environment, or publication destination is rendered. Manifest and evidence are
compact canonical UTF-8 JSON without a terminal LF.

The evidence wrapper schema is
`semaprax.offline-effect-free-wasm-package-build-evidence.v1`, with exact
`schema,digest,bytes,payload` order and a domain-separated SHA-256 digest over
the exact payload length and bytes. Payload member order is
`schema,resolution_digest,resolution_bytes,lock_digest,lock_bytes,subjects,
root,exports,package_source_set_digest,package_link_digest,manifest_digest,
manifest_bytes,wasm_digest,wasm_bytes,limits,budget,nonclaims`.

Verification parses only bounded structural fields, independently regenerates
the resolution, selected-subject association, lock, canonical source, empty
dependency/import association, HIR, Wasm bytes and their exact runtime
import/export inventories, manifest bytes, and evidence bytes, then
exact-compares all three submitted artifacts. Submitted digests and semantic
facts are never reconstruction inputs.

Diagnostics use the closed family:

- `SPX-PB501`: invalid build options or selection grammar;
- `SPX-PB502`: resolver, subject, report, or lock authentication failure;
- `SPX-PB503`: package/source/dependency association confusion;
- `SPX-PB504`: effect-free scalar profile rejection;
- `SPX-PB505`: byte, cardinality, work, or render bound;
- `SPX-PB506`: malformed/non-canonical manifest or evidence wire;
- `SPX-PB507`: exact replay mismatch.

## Publication authority

The separate publisher is safe Rust and forbids unsafe code. It uses only the
safe platform facade. The invocation:

1. validates the destination leaf grammar without opening it;
2. independently verifies all build inputs in memory;
3. acquires one held destination parent and confirms the leaf is absent;
4. prepares every fixed child name, exact scan, discard inventory, post-publish
   scan, and publish carrier before stage creation;
5. creates the stage no-clobber, writes the three fixed files create-new, and
   authenticates exact inventory and file identities;
6. independently re-verifies the retained immutable build inputs and
   exact-compares every held staged file immediately before settle;
7. sets a fail-stop `publication_attempted` state and publishes the prepared
   directory no-clobber;
8. immediately marks a successful rename return as `published` before any
   post-publication check; neither state may discard;
9. on a failure after a stage was successfully held but before publication was
   attempted, attempts only exact authenticated discard; cleanup failure cannot
   replace the sticky primary failure and is reported separately.

Open, information, write, close, inventory, identity, rename, or post-rename
uncertainty is fail-stop. Publisher diagnostics are `SPX-PP501` invalid or
unsupported destination, `PP502` compiler replay rejection, `PP503` existing
output, `PP504` exhausted stage allocation, `PP505` previsibility authority
drift, `PP506` sticky primary with incomplete cleanup, and `PP507` visible
post-publication authentication failure. Errors separately expose
`NotPublished|Published` visibility and `NotNeeded|Settled|Incomplete` cleanup.
The publisher never overwrites, resumes, recovers, garbage-collects, or deletes
foreign bytes. Unix creates mode-0700 stages and mode-0600 files. Windows uses
inherited ACLs and makes no Unix-equivalent confidentiality claim.

## Nonclaims

The exact nonclaim inventory states:

```text
offline_caller_owned_catalog_and_source_replay_only
effect_free_source_with_fixed_runtime_imports_not_target_execution_or_conformance
no_registry_network_fetch_cache_or_dependency_discovery
no_build_scripts_external_compiler_linker_or_tool_execution
no_native_artifact_or_cross_platform_hermetic_sandbox_claim
capabilities_and_effects_must_be_empty_not_runtime_enforced
no_signature_publisher_identity_provenance_license_or_sbom
no_multi_package_source_linking_component_model_wasi_dynamic_linking_or_runtime_instantiation
evidence_is_not_publication_authority
```

## Compatibility boundary

Resolver v1, Report v1/v2, Subject/Lock v2, Lock v1, Compatibility v1,
Project v1-v10, Transport v2-v5, Graph, CleanupPlan, existing scalar-Wasm
artifacts, and all accepted CLI output bytes remain unchanged. The only
resolver widening is a crate-private authenticated selected-subject snapshot;
the public Resolver v1 API and evidence bytes are frozen.

## Authored evidence

The focused integration evidence is split by failure class:

- canonical manifest/evidence field order, compact-wire rejection,
  duplicate-key rejection, domain re-mint resistance, and independent receipt
  replay;
- exact singleton root/selected-subject association, dependency-metadata
  rejection, empty capability/target policy, stable-ID selection, and authored
  aggregate exclusion;
- `wasmparser` validation plus exact fixed runtime-import and selected-export
  inventories, including cross-paired module/manifest/evidence rejection; and
- option endpoints, cumulative three-artifact checked accounting, and exact
  evidence/artifact fixed-point boundaries with `limit - 1` rejection.

These tests are authored but were not executed by this implementation batch.
They do not promote the profile, publisher, Wasm runtime behavior, or any
completion-matrix row.
