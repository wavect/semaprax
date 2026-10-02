# Native Rust Rich Cargo Preparation v1

Status: additive private toolchain preparation profile. It records an already
resolved Cargo closure; it does not authorize acquisition, compilation,
build-script execution, publication, or a public CLI route.

Audience: toolchain hosts and contributors implementing rich Native Rust
interop.

## Purpose and authority

Rich Native Rust interop consumes two retained RI-01 documents: a rich binding
plan and a rich interop descriptor. Cargo preparation treats both as opaque
exact byte strings. It records their SHA-256 digests and does not parse,
reinterpret, or reuse either document's schema. RI-01 owns their admission and
the `SPX-B117` through `SPX-B120` diagnostic space.

Preparation belongs in the private `semaprax-toolchain` crate. The root
compiler may produce and authenticate a subject, but it never invokes Cargo,
consults a Cargo cache, reads Cargo configuration, acquires a package, or runs
a build script. Check, format, graph, context, LSP, and ordinary semantic
operations therefore remain pure with respect to Cargo.

The current implementation is `rich_cargo_preparation`. Its public function
`prepare_cargo_closure` accepts caller-collected bytes and returns canonical
preparation bytes. It has no path or subprocess API.

## Admitted closure

The host obtains Cargo metadata with Cargo's format-version 1 interface, then
passes its exact JSON bytes to preparation. Metadata is a selected package
graph; it does not authorize a function or type import. Preparation requires:

- a nonempty `packages` inventory and nonempty `resolve.nodes` inventory;
- each resolved node to identify exactly one declared package and carry an
  array of selected feature names;
- at most 512 selected packages;
- either a local package (`source` omitted or null) or the ordinary crates.io
  registry source `registry+https://github.com/rust-lang/crates.io-index`;
- one strictly byte-sorted source fact for every resolved package.

Registry facts carry a raw crate checksum. Local facts carry a digest of the
held local source tree. A path source therefore never claims a registry
checksum. Git sources, custom registries, and custom JSON target specifications
reject before any later stage. The preparation record retains the package IDs,
which distinguish renamed dependencies, different library target names,
features, and multiple versions selected by Cargo. The complete raw metadata
digest remains bound too, so later extensions do not silently discard Cargo
facts that this version does not interpret.

## Bound inputs and canonical record

The input holds nonempty exact bytes for:

1. RI-01 binding plan and rich descriptor;
2. Cargo metadata, `Cargo.lock`, and explicit Cargo configuration;
3. toolchain identity and target-spec identity.

It also holds a generator revision, target triple, panic strategy, profile,
sorted selected feature names, and sorted source facts. The combined byte input
is capped at 16 MiB. The target is a fixed Rust triple with lowercase
letters/digits/underscore/hyphen only; a `.json` custom target is refused.
Panic strategy is `unwind` or `abort`.

Successful output is compact JSON plus one LF with this ordered shape:

```text
{
  "schema":"semaprax.native-rust-rich-cargo-preparation.v1",
  "binding_plan_digest":"sha256:…",
  "descriptor_digest":"sha256:…",
  "cargo_metadata_digest":"sha256:…",
  "cargo_lock_digest":"sha256:…",
  "cargo_config_digest":"sha256:…",
  "toolchain_digest":"sha256:…",
  "target_spec_digest":"sha256:…",
  "generator_revision":"…",
  "target":"…",
  "panic_strategy":"…",
  "profile":"…",
  "features":["…"],
  "packages":["…"],
  "sources":[{"id":"…","kind":"registry","checksum":"sha256:…"}]
}
```

The preparation digest is SHA-256 over
`semaprax.native-rust-rich-cargo-preparation.digest.v1\0` followed by those
exact canonical bytes. Replay checks the final LF, UTF-8, CR rejection, byte
bound, and the domain-separated digest. The digest binds source, descriptor,
generator, lock, features, target, profile, Cargo config, toolchain, and
source-tree/checksum identities, so any change invalidates a prior prepared
closure.

## Later effectful stages

`semaprax-toolchain::rich_cargo_execution` provides the only current
effectful entry points. `collect_cargo_metadata` and
`prepare_with_cargo_metadata` require absolute regular-file paths for Cargo
and `rustc`, an absolute workspace containing the exact `Cargo.toml`, and an
existing absolute target directory. They clear the inherited environment and
set only the explicit `RUSTC`, `CARGO_TARGET_DIR`, and Cargo offline setting.
Metadata runs as `cargo metadata --format-version=1 --locked --offline`; the
caller then supplies the held source facts required by the pure record.

`build_locked_offline` runs `cargo build --locked --offline` only for
`TrustedHost`. It reports that build scripts and proc macros have ordinary host
authority. `StrictDenyExecution` refuses before Cargo is spawned. The current
toolchain has no verified sandbox runner, so `EnforcedSandbox` also refuses
before Cargo is spawned instead of labeling an unenforced process as confined.
The focused strict and sandbox negative controls use a marker-writing Cargo
stub and prove that neither policy enters it. A local no-dependency Cargo
fixture and a checked-in vendored registry fixture supply explicit offline
metadata/pure-record cases; the vendored fixture also proves a trusted-host
`--locked --offline` build without registry access.

Acquisition is separately authorized. A future acquisition host must retain
the exact executable, configuration, source facts, and output directory while
obtaining the admitted vendored registry closure or local source trees. It may
not use an unrecorded Cargo cache as proof of a closure.

After acquisition, an execution host must use explicit tool images and the
prepared inputs with `--locked --offline`. The `--offline` flag constrains
Cargo networking only; it does not confine a build script or proc macro. RI-11
owns authorization and physical enforcement for those processes. No current
preparation result claims such enforcement, hermetic arbitrary Cargo builds,
or successful compilation.

## Diagnostics and evidence

| Code | Meaning |
| --- | --- |
| `SPX-B121` | Preparation input, metadata shape, sorting, canonical digest, or retained byte invariant is malformed. |
| `SPX-B122` | A git/custom-registry source or custom target is outside the admitted profile. |
| `SPX-B123` | Resolved package IDs and held registry/local source facts disagree. |
| `SPX-B124` | A preparation byte or package-inventory bound is exceeded. |

Focused unit coverage proves deterministic closure recording and independent
replay, then uses custom-source and missing-source-fact negative controls. An
end-to-end closure, physical offline build, unauthorized build-script/proc-macro
refusal, and publication remain separate gates; absence of those observations
is not a support claim.
