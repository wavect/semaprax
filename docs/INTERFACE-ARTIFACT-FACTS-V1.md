# Interface and artifact facts v1

Status: additive SEG-02 input with locally passing focused evidence. This is not SEG-02
completion or a ProgramRoot v2 implementation.

Audience: compiler contributors, semantic-service implementers, and reviewers
of source-owned ProgramRoot facts.

`InterfaceArtifactFacts` is an authority-free, canonical bundle over one exact
admitted `ProjectRevision`. It is designed for private retention by a future
enriched canonical workspace and for a new, versioned ProgramRoot segment
inventory. It is deliberately absent from Canonical Semantic Workspace
Revision v1 and ProgramRoot v1, so their bytes, identities, nine nodes, and
fixed segment order remain unchanged.

## Inputs and derivation

Derivation requires the exact expected lowercase Project revision, one to four
unique `ImageArtifactKind` values in the closed order `web`, `npm`, `openapi`,
`c`, and one build limit in the existing 1 KiB through 16 MiB range.

The source-interface fact is derived only where an existing Project profile
owns a canonical descriptor:

| Project profile | existing descriptor |
| --- | --- |
| v1 scalar | Scalar WIT interface v1 |
| v8 owned data | Public owned-data API v1 |
| v9 flat owned record | Public flat-owned-record API v1 |
| v10 owned UTF-8 | Public owned-UTF8 API v1 |
| v11 nested owned record | Public nested-owned-record API v1 |

Other profiles produce `source_interface: null`; the bundle does not invent a
general source interface. For each selected artifact kind it derives a fresh
`ProjectSemanticImage`, invokes the existing pathless artifact projection, and
invokes its independent verifier. Unsupported profile/kind combinations retain
their existing compiler diagnostics.

The canonical bundle embeds the exact UTF-8 descriptor bytes, its authoritative
descriptor digest and a separately domain-separated byte digest. Each generated
fact embeds the exact canonical projection report and a domain-separated report
digest. Those reports bind the exact carrier envelope, payload and emitted-file
digests through the existing Image target contract; this bundle does not claim
to contain or materialize the carrier files themselves.

Inputs are limited to four reports, 5 MiB aggregate retained descriptor/report
bytes, and an 8 MiB canonical bundle. Existing 1 MiB per-report and compiler
carrier bounds apply first. Caller array order is semantic and is never sorted
or repaired.

## Identity and replay

The schema is `semaprax.interface-artifact-facts.v1`. Canonical JSON is compact,
recursively key-sorted UTF-8 with one terminal LF. Its identity uses
`semaprax.interface-artifact-facts.digest.v1\0` over the exact length-framed
canonical bytes.

Replay validates bounds, UTF-8, JSON, canonical encoding, the closed record
shape, and lowercase digest grammar. It then freshly rederives the interface
and every selected artifact projection from the supplied immutable Project
revision and exact-compares both identity and complete bytes. Submitted bytes
never become AST, HIR, graph, artifact, cache, or execution state.

`SPX-G552` reports malformed, noncanonical, unordered, duplicate, or over-bound
facts. `SPX-G553` reports a stale Project expectation, bundle identity, or exact
replay mismatch. Existing interface, image and artifact diagnostics propagate
where those owning derivations reject an input.

## Authority and compatibility boundary

The bundle records `source_authority: false`, `artifact_materialization: false`,
and `target_execution: false`. It grants no filesystem, process, network,
publication, deployment, external-consumer, Agent, approval, or signing
authority. It does not introduce `.spx` Agent syntax or populate an
AgentDefinitions node.

Integration must add a private optional fact-bundle field and explicit derive/
replay route without changing legacy default derivation. ProgramRoot must use a
new versioned segment inventory before this fact bundle becomes part of root
identity; silently overloading the existing TargetProfiles node or v1's fixed
nine segments is forbidden.

The focused workspace-harness module
`tests/workspace/interface_artifact_facts.rs` owns exact success/replay,
descriptor and report equality, canonical workspace/ProgramRoot v1 byte
preservation, selector rejection, stale expectation, malformed canonical bytes,
self-consistent hostile mutation, lowercase digest rejection, and explicit
no-authority assertions. Both focused cases pass locally.
