# Installed Diagnostics v1

Status: additive authority-free installed projection; focused integration
evidence passes locally.

Audience: compiler contributors, coding agents, CLI users, and reviewers of
installed diagnostic identity.

Installed Diagnostics v1 exposes a deterministic, compiler-version-matched
catalogue of diagnostic code tokens present in the Rust sources used to build
SEMAPRAX, plus an exact explanation of one installed code. It answers whether
a canonical identifier occurs in the installed build's static source
inventory. It does not infer runtime reachability, reconstruct site-specific
messages, or claim a stable cross-version registry.

The separate [Installed Fix Plan v1](INSTALLED-FIX-PLAN-V1.md) catalog states
which installed diagnostic has a compiler-owned plan and can derive one exact
current-source plan. Catalogue presence here alone never implies repair
availability.

## Source inventory

`build.rs` recursively scans `.rs` files below exactly `src` and `crates`, with
paths and results in lexical order. A static token is admitted when it:

- begins at a non-identifier boundary with `SPX-`;
- has a 4-to-16-byte body;
- has one or more ASCII uppercase namespace bytes followed by exactly three
  ASCII digits; and
- ends at a non-identifier boundary.

Occurrences are unique `(path, one-based line)` pairs. Paths are package-root
relative and `/`-normalized. Rows below `src/` have scope
`compiler_package`; rows below `crates/` have scope
`workspace_member_source`.

The scan also records constructor sites containing `Diagnostic::error(`,
`Diagnostic::warning(`, or `Diagnostic::io(` whose first argument does not
start with a literal `"SPX-`. These are reported as unresolved dynamic
constructor sites. Their presence is explicit evidence that complete static
token coverage is not proof that every runtime-selected identifier has been
recovered at its constructor.

The generated inventory is embedded at build time. Runtime constructors read
no source tree, current directory, home directory, environment-selected path,
registry, service, or network.

## Public API

`src/installed_diagnostics.rs` owns:

```rust
pub const INSTALLED_DIAGNOSTIC_CATALOG_SCHEMA: &str =
    "semaprax.installed-diagnostic-catalog.v1";
pub const INSTALLED_DIAGNOSTIC_EXPLANATION_SCHEMA: &str =
    "semaprax.installed-diagnostic-explanation.v1";
pub const MAX_INSTALLED_DIAGNOSTIC_CATALOG_BYTES: usize = 8_388_608;
pub const MAX_INSTALLED_DIAGNOSTIC_EXPLANATION_BYTES: usize = 1_048_576;

pub fn installed_diagnostic_catalog() -> Result<InstalledDiagnosticCatalog, _>;
pub fn explain_installed_diagnostic(
    code: &str,
) -> Result<InstalledDiagnosticExplanation, _>;
```

The catalogue exposes its exact JSON, digest, and static code count. An
explanation exposes its installed code, exact concise text, exact JSON, digest,
and exact replay.

`src/cli/explain.rs` is a print-only adapter:

```text
semaprax explain <SPX-CODE>
semaprax explain <SPX-CODE> --json
```

The default output is the exact LF-terminated `to_text()` projection:

```text
{CODE}: installed {NAMESPACE} diagnostic ({N} static source occurrence[s]); emitted message and help are site-specific.
```

`--json` prints `to_json()` byte for byte. Missing or extra operands and
unknown options are CLI grammar errors with status 2. A well-formed but absent
code reaches the core and reports `SPX-G542`. The catalogue remains a library
API; this version adds no catalogue CLI route.

## Canonical documents and digests

Both documents are compact recursively key-sorted JSON terminated by one LF:

```json
{"digest":"sha256:...","payload":{},"schema":"..."}
```

The lowercase SHA-256 digest binds the canonical payload including its LF:

```text
domain || u64le(payload_byte_length) || payload_bytes
```

The domains are:

```text
semaprax.installed-diagnostic-catalog.payload.digest.v1\0
semaprax.installed-diagnostic-explanation.payload.digest.v1\0
```

Every payload has `authority: false`; bounded `limits`; explicit `nonclaims`;
and a compiler binding containing Cargo package and version, an optional
40-lowercase-hex build commit, and `binary_identity_claimed: false`. This is
version binding, not binary attestation, reproducible-build evidence, signing,
or executable provenance.

The catalogue payload contains the sorted diagnostic rows and coverage facts:
the exact source roots, static code count, unresolved dynamic-site count and
rows, and classification
`complete_static_code_token_inventory_with_unresolved_dynamic_constructor_sites`.
An explanation binds one catalogue row's namespace and occurrences, its exact
concise text, and the explicit contract that emitted message, help, severity,
and location remain site-specific.

`InstalledDiagnosticExplanation::replay` rejects over-limit input before
parsing, then requires canonical code and digest grammar, valid canonical
schema bytes, the matching payload digest, and a byte-identical fresh
derivation for the requested code.

## Diagnostics and limits

- `SPX-G540`: invalid code, digest, JSON, schema, canonical bytes, embedded
  build identity, or document construction.
- `SPX-G541`: catalogue or explanation capacity exceeded.
- `SPX-G542`: canonical code absent from the installed static catalogue.
- `SPX-G543`: digest or exact replay mismatch.

The complete catalogue is at most 8 MiB; one explanation is at most 1 MiB.
There is no truncation or partial success.

## Authority, compatibility, and nonclaims

The runtime API and CLI adapter are read-only and authority-free. They perform
no filesystem write, source mutation, process execution, network access,
cache update, transaction, compilation, test, repair, commit, signing,
deployment, or publication.

Static source-token presence is neither runtime reachability nor backend
support. Dynamic code selection may choose a separately catalogued static
identifier. Source lines are build-source provenance, not a promise that source
files are installed. Explanations do not promise a repair, successful build,
complete operational guidance, or cross-version code stability.

This additive feature does not change existing `Diagnostic` text or JSON,
diagnostic emission, `.spx` syntax or formatting, Project/workspace/image
artifacts, query/transaction/service artifacts, or frozen transport bytes. It
adds no MCP, LSP, editor, daemon, hosted catalogue, update mechanism, or host
grant.

## Focused evidence

`tests/projections/installed_diagnostics.rs`, registered only in the existing
projections harness, independently rescans all owned Rust sources and compares
every static code occurrence and unresolved dynamic constructor site with the
catalogue. It also covers canonical deterministic digest and version binding,
explanation replay, exact default/JSON CLI parity from an empty working
directory, malformed/unknown/noncanonical/tampered/oversized rejection, no
writes, and byte-exact legacy diagnostic text/JSON.

The focused gate is:

```sh
CARGO_TARGET_DIR=target/installed-diagnostics-v1 \
  cargo test --locked -p semaprax --test projections \
  installed_diagnostics --no-fail-fast
```

The five focused cases pass on the current local checkout.
