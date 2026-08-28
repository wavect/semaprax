# Quality gates

Status: living internal contributor documentation.

Audience: contributors, maintainers, and release reviewers.

This document defines repository-wide verification policy and routes changes to
their owning evidence. Exact protocol mutation matrices, known-answer digests,
platform fixtures, and focused command lists belong in the relevant versioned
specification and tests; they are not repeated here.

## The rule

A change is ready only when:

1. its baseline quality profile passes;
2. every affected versioned contract passes its focused evidence;
3. preservation tests for older schemas and unaffected behavior pass;
4. any public or hosted claim has evidence from the exact commit being claimed.

A local green test can support a local claim. It cannot be promoted to hosted,
public, cross-platform, or production evidence without the corresponding gate.

## Standard entry point

Use the routed script on Unix:

```sh
scripts/quality.sh full
```

It accepts `quick`, `changed`, or `full`. The script first emits and validates a
deterministic `semaprax.quality-route.v2` plan, then dispatches only the exact
listed gates. `changed` may widen to `full` when the path classification is not
safe enough for a narrower run.

| Profile | Intended use | Gates |
| --- | --- | --- |
| `quick` | Early local feedback | diff check, Rust formatting, workspace check, advisory documentation/examples/context tests |
| `changed` | Bounded reviewed changes | `quick` plus package Clippy, agent-context integration, and package rustdoc |
| `full` | Semantic changes and release candidates | workspace Clippy/tests/doctests/rustdoc, release build, package check, and canonical example checks |

The script is the executable source of truth for the precise command sequence.
Do not copy that sequence into feature documents.

## Manual baseline

On a host that cannot run the POSIX script, reproduce the `full` profile:

```sh
git diff --check
cargo fmt --all --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo build --locked --workspace --release
cargo package --locked --allow-dirty -p semaprax
```

Also run the example check and canonical-format loops from
`scripts/quality.sh`; keeping the list there prevents drift.

## Documentation changes

Documentation-only changes must pass at least:

```sh
git diff --check
cargo test --locked -p semaprax --test documentation --test examples
```

`tests/documentation.rs` checks local Markdown links recursively. The docs
workflow builds the mdBook using the pinned version in
`.github/workflows/docs.yml`.

If documentation changes a technical claim, run the evidence that owns that
claim. Editing prose does not substitute for implementation evidence.

## Change-specific evidence

Select every row touched by the change; these categories are cumulative.

| Change | Minimum additional evidence |
| --- | --- |
| Lexer, parser, or formatter | Success and diagnostic cases, canonical round-trip, unchanged legacy formatting |
| Verifier or HIR | Focused verifier tests, hostile-HIR rejection where applicable, deterministic identity checks |
| Runtime semantics | Interpreter/native O0/native O2/Wasm agreement for success, failure, evaluation order, and re-entry |
| Ownership or cleanup | Structural inventory, canonical plan build, independent replay, hostile mutation, success/failure settlement |
| Graph schema | Exact new projection, legacy byte preservation, context projection, invalid/tampered rejection |
| Semantic patch or repair | Preview, stale/drift rejection, no-write failures, independent replay, atomic A0 application |
| Workspace transaction | Held-input rechecks, replay before candidate/staging, one publication pivot, old-or-new process termination evidence |
| Project manifest or carrier | Exact source-set authentication, Phase-A reuse, closure/admission checks, carrier replay, post-publication drift behavior |
| Native backend or ABI | C11 compilation at required optimization levels, descriptor/header agreement, runtime status and cleanup conformance |
| Wasm or JavaScript boundary | Structural Wasm validation, generated binding checks, Node execution, and browser/multi-engine evidence when claimed |
| Report or schema projection | Closed admission/exclusion vocabulary, deterministic envelope, independent replay, tamper and budget rejection, cross-report consistency |
| Private host integration | Authority inventory, fail-stop uncertainty, process/loader settlement, platform-specific hosted jobs |
| Public API or generated SDK | External consumer with no source/workspace dependency, locked offline build, inventory and compatibility checks |

The owning specification lists exact focused tests. If it does not, add the
missing evidence section there instead of growing this document into a second
copy of the spec.

## Required semantic cases

When runtime meaning changes, cover all applicable cases:

- minimum and maximum admitted values and capacities;
- exact-capacity success and capacity-plus-one rejection;
- left-to-right evaluation and lazy boolean behavior;
- first-failure stickiness;
- success, contract failure, runtime failure, and cleanup failure;
- repeated entry and deterministic output;
- stale source, source drift, tampered evidence, and forged re-digested input;
- unsupported profile rejection before target or filesystem effects;
- unchanged bytes for older schema versions and unaffected examples.

Never weaken a diagnostic or golden merely to make the gate green. A deliberate
wire change needs a migration, an updated versioned contract, and explicit
compatibility evidence.

## Public Native Rust SDK promotion

The generated Rust SDK is a useful example of evidence layers. Local promotion
requires the focused `public_native_rust_sdk_v1` and
`public_native_rust_sdk_ci_contract` suites plus the standalone
`examples/calculator-rust` consumer. The consumer must use the generated
package with no repository source or workspace dependency and build in locked
offline mode.

Public promotion additionally requires the blocking Ubuntu, macOS, and Windows
jobs at the exact claimed commit, including deterministic inventory,
tool-authority, failure-settlement, and compiler-free consumer evidence. The
builder remains unpublished until that boundary is intentionally promoted.

## Hosted evidence

Hosted claims require the exact workflow jobs named by the owning
specification. A prior-head run is historical evidence only. A diagnostic or
allowed-failure job is not a passing promotion gate.

For platform claims:

- compilation or object inspection is not runtime execution;
- simulator evidence is not physical-device evidence;
- Node execution is not browser or multi-engine evidence;
- one operating system is not a cross-platform matrix;
- a private fixture is not a supported public SDK or application surface.

Record exact commit and run links in the owning specification's status/evidence
section or the changelog. The completion matrix should link to the owner rather
than duplicate those run IDs.

## Evidence strength

From weakest to strongest:

1. design text;
2. compilation or structural inspection;
3. deterministic local unit/integration evidence;
4. independent replay and hostile-input evidence;
5. exact-head hosted execution on the required target matrix;
6. external consumer or representative application evidence;
7. maintained release and compatibility evidence.

Higher evidence does not erase scope limits. A perfectly replayed scalar report
is still a scalar report; it does not prove general aggregates, resources, or
production interoperability.
