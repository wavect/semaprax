# Project Dependencies v1

Status: additive implementation with local executable gates; unpromoted.

Audience: application authors, package authors, Rust host-adapter authors, and
compiler contributors.

Project Dependencies v1 gives scalar `semaprax.manifest.v1` projects two
deliberately separate dependency boundaries. SEMAPRAX packages are replayed,
resolved, and linked into the checked semantic workspace. Rust crates are
exact inputs to the generated Native Rust SDK package and are available only
to Rust host code. Neither route grants the compiler ambient network access.

## SEMAPRAX package closure

`[dependencies]` remains the version-range root set. A build consumes ordinary
packages only when `[dependency-sources]` names project-relative Subject-v3
JSON files:

```text
[dependencies]
acme.math = "^1.2.0"

[dependency-sources]
acme.math = "vendor/acme-math.subject.json"
```

The table has at most four rows, is strictly byte-sorted by package identity,
and uses canonical relative `.json` paths. It contains the entire finite
closure, including transitive packages, not merely the roots. Each held file
must independently replay as a Semantic Package Subject v3 and its embedded
Semantic Package Report v2. The table key must equal the subject coordinate's
package name.

For every declared target, or both `native64` and `wasm32` when `[targets]` is
absent, the compiler runs Offline Resolver v2 over exactly those subjects. All
root ranges, transitive ranges, target availability, and declared capability
limits must pass. The selected coordinates must equal the complete supplied
set, so unused alternatives and incomplete closures fail closed. Ordinary
subjects cannot replace compiler-bundled `std.*` packages.

Only the authenticated canonical source embedded in each selected subject is
lent to the ordinary workspace linker. A package report's required `main`
function is a report anchor and is omitted from the linked application; all
other stable declarations remain available through normal `use` resolution.
The synthetic source path includes the exact subject digest, so Project and
workspace revisions change when any selected envelope changes.
The scalar Project profile is the admitted package-consumer profile in v1.

The manifest, project sources, dependency subjects, and ancestor directories
remain held through the operation and are rechecked after publication. Subject
bytes share the resolver's bounded catalog budget; embedded source bytes share
the Project's 16 MiB source budget, and declared plus dependency sources share
the 16-file workspace limit. Check, test, run, image, lock, Web, npm, and native
routes therefore consume one authenticated dependency closure without a cache,
registry, subprocess, acquisition step, or source rewrite.

## Rust crate inputs

`[rust-dependencies]` has at most 32 strictly byte-sorted Cargo package names.
Each value is a one-line array whose first item is an exact `=x.y.z` version;
remaining items are strictly byte-sorted Cargo feature names:

```text
[rust-dependencies]
same-file = ["=1.0.6"]
serde = ["=1.0.228", "derive", "std"]
```

The table is bound into the authenticated canonical Project manifest. On
`semaprax-full build ... --target rust`, the generated SDK `Cargo.toml` maps
each package to a private deterministic dependency alias and preserves the
exact version and feature set. `src/lib.rs` re-exports it as
`rust_dependency_<package_name_with_hyphens_as_underscores>`. The private
aliases avoid depending on a package's library-target name; colliding public
aliases are rejected by manifest admission.

Rust crates do not become SEMAPRAX modules and cannot add effects or authority
to checked SEMAPRAX code. They are conveniences for the Rust implementation of
`NativeRustSdkImports` and for Rust consumers of the generated package. Cargo,
run later and explicitly by the consumer, owns registry configuration,
acquisition, transitive resolution, checksums, and `Cargo.lock`. An offline
consumer can generate and commit a lock from a pre-populated Cargo cache and
then build with `--locked --offline`.

### Calling any declared crate from SEMAPRAX

There is no crate allowlist. Any Cargo package admitted by the canonical
`[rust-dependencies]` grammar can implement a typed `import rust fn` boundary.
The SEMAPRAX side declares only the stable ID, scalar signature, effects, and
failure policy it can verify:

```text
permit { host.filesystem }

@id("app.host")
interface Host permits { host.filesystem } {
    @id("app.host.same-file")
    import rust fn same_file(value: i64) -> i64
        effects { host.filesystem }
        failure status "app.host.v1";
}

@id("app.checked")
fn checked(value: i64) -> i64 uses { host.filesystem } { same_file(value) }
```

The generated SDK turns that import into a method on `NativeRustSdkImports`.
Host code implements the method with the dependency re-export; for example:

```rust
use semaprax_generated_native_rust_sdk::{
    rust_dependency_same_file, NativeRustSdkImportResult, NativeRustSdkImports,
};

struct Host;

impl NativeRustSdkImports for Host {
    fn spx_app_dot_host_dot_same_hyphen_file(
        &mut self,
        value: i64,
    ) -> NativeRustSdkImportResult<i64> {
        match rust_dependency_same_file::is_same_file(".", ".") {
            Ok(true) => NativeRustSdkImportResult::Success(value + 1),
            Ok(false) => NativeRustSdkImportResult::Success(value),
            Err(_) => NativeRustSdkImportResult::HostFailure,
        }
    }
}
```

This pattern works for any crate because the adapter owns conversion between
the crate's Rust API and the closed SEMAPRAX import ABI. Crate-specific Rust
types, traits, generics, lifetimes, async runtimes, macros, and unsafe contracts
remain on the Rust side; they are never guessed or smuggled into checked
SEMAPRAX types. Declared effects remain mandatory even when the chosen crate
can perform more operations than one adapter method exposes.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-J100` | A dependency table, exact Rust version, feature, name, order, or path violates the canonical manifest grammar. |
| `SPX-J101` | A dependency inventory or byte budget exceeds its bound. |
| `SPX-J121` | A dependency without an ordinary subject is not in the bundled standard-library inventory, or its range excludes the bundled version. |
| `SPX-J123` | The held ordinary subject inventory, replay, target resolution, selected closure, or package identity fails closed. |

## Evidence and nonclaims

`tests/project.rs::package_manifest_v1` pins canonical parsing and rejection,
an exact Subject-v3 package linked through check, test, run, project image,
project lock, and Web build, Rust and SEMAPRAX tables together, and tamper
failure. The Native Rust builder's unit and effectful integration tests pin the
generated Cargo requirements, deterministic re-exports, offline lock
generation, and `cargo check --locked --offline`. Repository full quality is
the local preservation gate.

This version does not acquire packages, contact a registry, establish trusted
publisher provenance, solve license policy, vendor a Cargo closure, or promote
a stable ecosystem ABI. It permits arbitrary declared crates behind explicit
typed adapters; it does not automatically project arbitrary raw Rust APIs or
Rust-only types into SEMAPRAX. Those require separate authority, provenance,
ABI, and hosted conformance contracts.
