# Installed Agent Guidance v1

Status: additive authority-free installed projection; focused local integration
evidence passes 5/5.

Audience: compiler contributors, coding agents, CLI users, and reviewers of
version-matched installed guidance.

Installed Agent Guidance v1 packages a closed set of descriptive resources
inside the `semaprax` binary. It lets an agent obtain compiler-version-matched
language, graph, standard-library, package, effect, and workflow guidance
without locating a checkout or consulting a network service. It also exposes
the exact five operations installed by Universal Semantic Query v1.

These documents are data. They are not compiler input, executable
instructions, binary attestation, live service discovery, or a capability
grant.

The separate [Installed Diagnostics v1](INSTALLED-DIAGNOSTICS-V1.md) projection
catalogues version-matched static diagnostic identifiers and explains one
installed code. Guidance does not duplicate or imply that catalogue.

## Public API and selectors

`src/installed_guidance.rs` owns the library projection and exports:

```rust
pub enum InstalledSkill {
    Agent,
    Language,
    Graph,
    Stdlib,
    Packages,
    Effects,
}

pub const INSTALLED_SKILL_SCHEMA: &str = "semaprax.installed-skill.v1";
pub const INSTALLED_QUERY_CAPABILITIES_SCHEMA: &str =
    "semaprax.installed-query-capabilities.v1";
pub const MAX_INSTALLED_GUIDANCE_BYTES: usize = 1_048_576;

pub fn installed_skill(skill: InstalledSkill) -> Result<InstalledGuidance, _>;
pub fn installed_query_capabilities() -> Result<InstalledGuidance, _>;
```

`InstalledSkill::ALL`, `as_str`, and `parse` define the exact lowercase closed
selector inventory: `agent`, `language`, `graph`, `stdlib`, `packages`, and
`effects`. An `InstalledGuidance` exposes only its schema, digest, and exact
canonical JSON.

The CLI adapters are:

```text
semaprax skills get <agent|language|graph|stdlib|packages|effects>
semaprax query --capabilities
```

Each successful CLI command writes the exact bytes returned by the
corresponding library constructor. The grammar accepts no skill aliases,
wildcards, host-selected paths, response files, or extra operands. Invalid or
unknown grammar exits with status 2 before constructing an artifact.

## Canonical envelope and identity

Both artifact kinds use the exact compact, recursively key-sorted outer
envelope below, terminated by one LF:

```json
{"digest":"sha256:...","payload":{},"schema":"..."}
```

`digest` binds the exact canonical payload bytes, including their terminal LF,
as lowercase SHA-256 over:

```text
domain || u64le(payload_byte_length) || payload_bytes
```

The domains are:

```text
semaprax.installed-skill.payload.digest.v1\0
semaprax.installed-query-capabilities.payload.digest.v1\0
```

Every complete envelope is at most 1,048,576 bytes. `SPX-G534` reports invalid
embedded material or build identity; `SPX-G535` reports a document that exceeds
the capacity limit.

Every payload carries `authority: false` and a compiler binding containing the
Cargo package name and version, an optional build commit, and
`binary_identity_claimed: false`. When present, the build commit is exactly 40
lowercase hexadecimal characters. This metadata identifies the installed
package build inputs available to the compiler; it is not a reproducible-build
claim, executable hash, signature, or provenance attestation.

## Six installed skills

All skill payloads contain `authority`, `compiler`, `content`, `limits`,
`nonclaims`, `skill`, and `sources`.

- `agent` embeds the agent quick reference and names the installed semantic
  query and transaction schemas.
- `language` embeds the agent quick reference and declaration-shapes catalog.
  The example-derived shapes are not a complete formal grammar.
- `graph` embeds the declaration-shapes catalog and lists the current Project
  semantic projection schemas and the `graph`, `symbol`, `context`, and
  `impact` read operations. It does not promise a complete or stable module
  graph schema or node/edge catalog.
- `stdlib` embeds the generated standard-library guide and parses the installed
  standard-library catalog into canonical JSON. Catalog status and target rows
  remain descriptive rather than release promotion.
- `packages` parses the installed standard-library package catalog and states
  that ordinary dependency material is an explicit caller-supplied,
  authenticated subject closure. It supplies no registry, network fetch,
  installed ordinary-package inventory, or project admission claim.
- `effects` derives the sorted unique compiler-owned host-operation effects and
  standard-library-declared effect tokens from installed compiler constants and
  the embedded catalog. It is not a complete stable vocabulary for
  user-defined effects or capabilities.

Each `sources` row binds the exact embedded source bytes with byte length,
format, stable resource ID, optional source schema, and a digest using:

```text
semaprax.installed-guidance.source.digest.v1\0
    || u64le(source_byte_length) || source_bytes
```

This source identity makes the provenance of copied content reviewable without
granting a caller access to repository paths or trusting separately installed
documentation at runtime.

## Installed query capabilities

`semaprax.installed-query-capabilities.v1` reports exactly these Universal
Semantic Query v1 operations, in order:

```text
declarations
symbol
context
impact
available_operations
```

Each row names the owning result payload schema. The document also carries the
query request/result schemas and installed paging/request/result limits.
`host_grants` is the empty array and `authority` is false.

This is installed-support metadata, not live workspace, service, transport,
plugin, or host discovery. It cannot enable an operation or change request
capabilities. `available_operations` still requires an exact workspace
revision and declaration target, and an available rename does not prove that
an arbitrary proposed name will validate.

## Authority and compatibility

Constructing either artifact reads only resources already embedded in the
binary. It does not inspect the current directory, home directory, repository,
environment-selected resource paths, installed packages, service state, or the
network. It performs no filesystem write, process execution, source mutation,
cache update, transaction, build, test, commit, signing, deployment, payment,
or publication action.

The feature is additive. It does not change `.spx` parsing or formatting,
legacy declaration query behavior, Project or managed Workspace revisions,
Semantic Workspace Image, canonical workspace revision, query, transaction,
service, or frozen Project Agent Transport v5 bytes. It adds no MCP, LSP,
editor, daemon, wire discovery, generated SDK, installation/update mechanism,
or general skill execution protocol.

## Focused evidence

The integration evidence lives in
`tests/projections/installed_guidance.rs` as a module of the existing
projections harness. It covers exact core/CLI parity for all six selectors;
deterministic canonical LF-terminated envelopes, payload/source digests,
compiler version binding, and the one-MiB bound; inert unknown and malformed
grammar from an empty working directory; the exact authority-free five-operation
capability inventory; and byte-exact preservation of the legacy source-query
route.

The focused gate is:

```sh
CARGO_TARGET_DIR=target/installed-agent-guidance-v1 \
  cargo test --locked -p semaprax --test projections \
  installed_guidance --no-fail-fast
```

That command passes 5/5 in this checkout. The focused library constructors pass
4/4 unit cases, and the two new CLI parsers each pass their focused unit case.
