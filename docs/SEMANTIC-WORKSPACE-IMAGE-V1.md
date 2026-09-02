# Semantic Workspace Image v1

Status: first bounded foundation authored, unrun; the full graph-operational
programme remains Partial.

Audience: agent builders, compiler contributors, and reviewers.

## Source authority and image lifetime

Canonical human-readable `.spx` source remains the Git and review authority.
The compiler derives an operational semantic image from an already admitted
`Arc<ProjectRevision>`. The image retains the same validated entry/test HIR,
complete declared-project graph, and typed analysis indexes in memory. It does
not make graph JSON the canonical source of program meaning.

An image is immutable, deterministic, revision-bound descriptive data. Its
optional persisted bytes are a rebuildable projection: consumers must supply
a freshly authenticated or otherwise valid retained Project revision and replay
the exact image against it. No serialized graph or HIR is deserialized into
trusted compiler state. Replay derives a new image from the supplied revision
and compares every byte, including canonical ordering and the terminal LF.
A matching caller-computed digest alone cannot admit an altered image.

The image does not retain a filesystem root, held file handles, a lock, or
publication authority. An old image may continue to describe its immutable old
revision after files change; that does not authenticate current source paths.
Live CLI requests use the ordinary Project held-input recheck around the
operation and fail if their input snapshot drifts.

## Canonical payload

The image schema is `semaprax.semantic-workspace-image.v1`. Its payload binds:

- the compiler package version and explicit image compatibility schema;
- the exact canonical Project manifest and its schema;
- Project and Workspace Phase-A revision identities;
- every declared source's relative path, source graph schema, source revision,
  and source digest in canonical order;
- the complete existing Project semantic graph and graph digest; and
- stable-ID declaration lookup plus forward and reverse typed adjacency
  indexes over the existing admitted graph edge families.

The compiler version is a package compatibility fact, **not** a compiler binary
fingerprint, reproducible-build attestation, or trusted provenance claim.
Source digests bind exact admitted canonical input; source text and HIR are not
restored from the image. No mtime, absolute host path, or process identity is
part of the deterministic image.

Stable-ID index entries identify declarations in the graph. Adjacency entries
identify typed nodes and canonical graph edge ordinals. The compiler derives
all entries from its retained typed index rather than rebuilding semantic
meaning by parsing graph JSON. Existing Project graph, Context, Impact, and
transport schemas remain unchanged.

Canonical JSON recursively sorts object keys lexically, preserves array
order, uses compact separators, and includes exactly one terminal LF. The
image digest is `sha256:` plus lowercase hexadecimal SHA-256 of the domain
`semaprax.semantic-workspace-image.digest.v1` followed by a zero byte, the
eight-byte little-endian image byte length, and the complete image bytes. The
digest is returned separately rather than embedded into its own payload.

The writer bounds the complete image, including its terminal LF, to
`MAX_SEMANTIC_IMAGE_BYTES` (32 MiB). Replay rejects an oversized input before
fresh derivation. Bounds do not authorize truncation or partial-image success.
This is an output byte bound, not a total heap-memory bound. Symbol output has
a separate 64 KiB bound.

## Read-only API and CLI

`project::ProjectSemanticImage::derive(revision, expected_project_revision)`
creates an image; `replay(revision, expected_project_revision, image_bytes)`
requires exact fresh reconstruction. `to_json()` returns canonical JSON with
one terminal LF and `image_digest()` returns its revision-binding digest.

`symbol(expected_image_digest, stable_id)` provides a typed declaration lookup.
It exposes declaration identity, kind, origin, owner, source path/module, and
incoming/outgoing and `direct_cross_file_callers` counts; function declarations
additionally have their retained display name. These counts cover only the six
existing cross-file families, explicitly identified by
`edge_scope: "six_cross_file_families"`; they do not include local function calls.
It does not promise a complete typed signature, contract body, expression tree,
or a new source-edit operation.
`context` and `impact` require the same expected image digest and delegate to
the existing bounded Project typed analysis queries. Their ordinary target
kinds, options, diagnostics, and byte schemas remain intact; their responses
are not image-bound proof capsules. Symbol JSON uses
`semaprax.semantic-workspace-image-symbol.v1`; query strings omit the terminal
LF and CLI rendering adds it.

```text
semaprax project-image examples/calculator-project/semaprax.toml
semaprax project-image-verify examples/calculator-project/semaprax.toml image.json
semaprax project-symbol examples/calculator-project/semaprax.toml calculator.add
```

These commands have no image-output directory or publication option.
Persistence is explicit caller policy, for example stdout redirection. Nested
`.semaprax-images/` directories are ignored by Git for callers choosing that
convention; the compiler never discovers, creates, loads, or writes them
implicitly. The existing opt-in Project Revision Store remains a distinct
source-inventory replay mechanism and is not replaced or silently enabled.

| Diagnostic | Meaning |
| --- | --- |
| `SPX-G219` | Invalid image query grammar or missing stable-ID declaration. |
| `SPX-G220` | Image input or canonical output exceeds its fixed capacity. |
| `SPX-G221` | Stale expected revision/digest or image bytes disagree with fresh derivation. |

Project source admission and held-input drift retain their existing diagnostic
codes. Images grant no source write, A0 commit, managed `ACTIVE` pivot,
execution, network, dependency resolution, or target build authority.

## Evidence and remaining work

[Integration evidence](../tests/workspace/semantic_image.rs) is authored but
was not run in this change, as explicitly requested. It covers deterministic
repeated/cross-root derivation, exact graph/source binding, stable-ID lookup,
bounded context/impact delegation, exact replay, altered/reminted and
noncanonical inputs, oversize and stale rejection, immutable revision lifetime,
held-source drift, and absence of incidental filesystem writes. CLI evidence
is maintained separately with the command implementation. No local or hosted
quality-gate success is claimed.

This is the first reusable image foundation, not completion of the
graph-operational roadmap. It does not add incremental invalidation or
rechecking, serialized trusted HIR, a persistent daemon image cache, typed
holes, function-body replacement, signature changes, semantic deltas,
benchmark results, or broad new graph edge families. The remaining programme
must establish those surfaces independently while preserving canonical source,
revision binding, exact replay, deterministic projections, and existing commit
authority. The completion-matrix rows remain Partial.
