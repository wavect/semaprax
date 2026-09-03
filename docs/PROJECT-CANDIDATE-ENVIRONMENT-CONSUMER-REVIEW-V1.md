# Project Candidate Environment Consumer Review v1

Status: **Partial**. The library composition, Project Agent Transport v5 route,
closed schemas, generic-client discovery, MCP catalogue entry and regression
sources are authored. The regressions are unrun. This status does not claim a
current-head test, generated-client, MCP, runtime, provider or deployment pass.

Audience: agent clients and reviewers that need declared environment blind
spots and a bounded inventory of known package consumers beside one exact
candidate, without treating that inventory as installed-consumer discovery or
compatibility evidence.

## Inputs and trust boundary

`ProjectCandidate::environment_consumer_review` accepts:

- an exact candidate revision;
- the canonical 24,576-byte-bounded analysis-boundary bundle and its digest;
- an immutable `PackageSemanticGraph` already authenticated from its explicit
  source capsule, source set and resolution evidence;
- the exact provider package and version, candidate provider source path, and
  exported stable target identity.

The method does not accept serialized graph facts as authority. It independently
regenerates the complete candidate environment review, package semantic summary
and selected consumer report through their existing public APIs. It parses each
result as its exact canonical compiler schema and retains each complete report
in the result, together with the SHA-256 of its exact bytes.

The join requires the candidate manifest name and version to equal the provider
coordinate. The provider source path must select exactly one retained candidate
source. The package summary must contain exactly one matching provider row, and
its source revision, package-source digest, byte count and interface-source
revision must equal that candidate source. The selected consumer report must
bind the same package graph revision, provider coordinate, provider source
revision and digest, and target. The environment review must contain the same
candidate, base Project, candidate Project, Workspace, semantic graph and bundle
identities, and its retained source inventory must contain the same provider
source revision and digest.

The package graph's original `project_association: "none"` remains required.
This composition adds only an exact candidate-era provider-source association
inside this report. It does not mutate the graph, turn it into Project
authority, retain it, or infer that its inventory represents installed,
Workspace-wide, ambient or deployed consumers.

## Reports and coverage meaning

The combined schema is
`semaprax.project-candidate-environment-consumer-review.v1`. It contains the
complete environment review, package summary and package consumer report,
their exact digests, all candidate and package bindings, consumer counts, and a
domain-separated report revision. Its maximum canonical size is 23,265,280
bytes: the complete 18,939,904-byte environment review, one additional 2 MiB
operational projection, two 1 MiB package reports, and a 128 KiB wrapper
allowance. Construction fails rather than truncating any nested report.

The operational projection has its own closed schema,
`semaprax.project-candidate-environment-consumer-coverage.v1`. It begins with
the complete analysis-boundary report nested in the independently regenerated
environment review. Before changing it, the compiler requires the exact
baseline `external_consumers` row. It then changes only that area from
`not_inspected` to `partial` and adds the package-consumer-specific nonclaims.
Every other coverage area, blind-spot row, source and manifest fact, attachment,
authority field and existing nonclaim is retained. The original nested
environment review and its analysis-boundary report remain unchanged.

`partial` means that the compiler inspected the bounded, authenticated inventory
in this one attached graph for the selected candidate provider declaration and
reports its exact import and static-call counts. Either inventory may be empty.
An empty inventory does not prove that no consumer exists. The status does not
mean that the inventory is complete, that any import executes, or that an
affected consumer accepts the candidate.

## Transport v5

`candidate/environment-consumer-review` is discoverable only when the embedding
host selected `candidate_prepare` and attached one package graph before the
session accepted a frame. Neither condition can be requested over the wire.
Without either condition, the method is absent from capabilities, schemas,
clients, MCP and dispatch.

The closed request binds `image_revision`, `candidate_revision`,
`package_revision`, `bundle`, `bundle_digest`, `provider_package`,
`provider_version`, `provider_source_path` and `target`. `offset` and
`chunk_bytes` select a response chunk. Chunk sizes are 1,024 through 65,536
bytes, defaulting to 16,384. Offsets must be within the 23,265,280-byte bound
and on a UTF-8 boundary. Every chunk repeats the exact image, candidate,
package, bundle, provider, source and target selectors, total byte count and
stable report SHA-256. A client must keep all selectors and bundle bytes fixed,
follow `next_offset`, and verify the same digest while reassembling the report.

The chunk schema is
`semaprax.image-candidate-environment-consumer-review-chunk.v1`. It is closed,
sets `compatibility` to `not_assessed`, and exposes every observation,
authority, execution, completeness and retention flag as false. The
bundle-bearing method is deliberately absent from the parallel-read subset.

The generic TypeScript, Python and Rust generators derive request and decoder
methods from the same closed descriptor. The MCP catalogue derives the same
input schema and tool availability. MCP forwards the inner v5 JSON-RPC response
as opaque text; it does not add an output schema, interpret report contents,
attach a graph, or grant candidate preparation or other authority.

## Diagnostics and failure behavior

Library diagnostics are:

- `SPX-G472` for invalid or noncanonical nested report structure;
- `SPX-G473` for nested or combined report capacity failures;
- `SPX-G474` for stale, ambiguous or inconsistent candidate, environment,
  provider-source, package graph, target, authority or baseline coverage joins.

Transport diagnostics are `SPX-G475` for a stale package revision and
`SPX-G476` for report capacity or invalid UTF-8 chunk selection. The ordinary
candidate, bundle, graph and protocol diagnostics remain authoritative for
failures in their owning layers. A rejected request changes no candidate,
graph, source, session grant or publication state.

## Nonclaims and absent authority

`compatibility: "not_assessed"` is neither a compatibility nor an
incompatibility result. This report does not assess semantic, source, API, ABI,
behavioral, migration, build, link, runtime or deployment compatibility. It
does not automatically migrate consumers or prove that a static call succeeds
against the candidate.

The attached graph is not ambient, installed, registry, filesystem, Workspace
or deployment discovery. Absence from it is not absence of other external
consumers. Imports do not prove calls; authenticated static calls do not prove
runtime use, executed tests, linked-closure membership or deployed behavior.

The composition performs no filesystem or environment observation, package
acquisition, registry lookup, network access, provider observation, generator
execution, artifact materialization, test or target execution, runtime
observation, conformance check or deployment inspection. It grants no source,
approval, publication, filesystem, environment, package, network, provider,
generator, runtime, conformance, ambient or deployment authority. Neither the
candidate nor the package graph is retained by the report.

All nonclaims from the complete nested environment and package reports remain
present. The operational projection adds its narrower attached-graph limits;
it does not replace or weaken the nested reports' statements.

## Authored evidence

Library regression sources cover the sole owned coverage transition and
fail-closed behavior for baseline drift or duplicate external-consumer rows.
Transport regression sources cover dual host gating, the closed request and
chunk schema, generic TypeScript/Python/Rust client generation, MCP catalogue
selection, the 23,265,280-byte selector bound, false grants and exclusion from
parallel reads. These regressions are authored and unrun. No test target or
generated consumer was compiled or executed for this Partial stage.
