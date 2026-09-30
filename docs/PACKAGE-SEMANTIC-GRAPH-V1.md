# Package Semantic Graph v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, or separately provisioned observations below retain
their narrower scope; public promotion and broader product completion remain
separately gated.

Audience: package-tooling contributors, embedding hosts, and agent authors.

This derived graph makes authenticated package consumers queryable. It starts
from the [multi-package source capsule](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md),
not a directory scan, installed-package guess, or unrelated equal symbol IDs.
Canonical `.spx` remains the source of meaning.

## Authentication and identity

`PackageSemanticGraph::derive` receives the exact capsule, implementation
sources, resolution evidence, resolution input, resolution options and capsule
options. It uses the existing complete capsule verification path: replay the
Resolver-v1 selection and Subject-v2/Report-v2 inputs, rebuild canonical source,
compare source interfaces with selected package interfaces, and compare actual
source import dependencies with the complete selected dependency graph.

Package coordinates qualify identities. Source, source-set, interface, report,
subject, link and capsule digests preserve the evidence behind relationships.
Report source is interface evidence; it cannot replace the implementation
source authenticated by the capsule. A graph identity binds the derived facts
and the selected package subject. Stale graph expectations and a different
version of the same package do not select the retained facts.

An import declaration is not a call site: a program can import a symbol without
calling it. Authenticated cross-package calls record static source relationships.
They do not prove execution, test coverage, or inclusion in every deployed
artifact. Likewise, a consumer missing from this selected graph may still exist
outside it.

## Queries and host attachment

The summary schema is `semaprax.package-semantic-summary.v1`; the consumer
schema is `semaprax.package-semantic-consumers.v1`. Summary facts include the
exact selected root coordinate, per-package source/interface bindings, export
IDs, declared imports and counts. Consumer rows retain caller/target package
coordinates, declaration identities, both source revisions, contract/body site,
expression identity, AST path, alias and ordinal. Call sites cover authenticated
cross-package source calls, including callers outside the linked export closure;
local calls are outside this relationship family.

Use the library's compact, revision-bound summary to inspect the graph.
To query consumers, supply the exact provider coordinate and stable declaration
identity. Both queries read only the already derived graph; they do not fetch
dependencies, build artifacts, or execute them.

A v5 embedding host may attach one immutable verified package graph before
processing any frame or parallel-read invocation. Attachment is a typed host
API, not an RPC accepting paths, capsule bytes or arbitrary graph JSON. It does
not grant candidate preparation, diagnostics, tests, builds, or publication.
Without attachment, package queries are absent from discovery and dispatch.
Attachment cannot replace the selected graph after the session starts.

The attached surface provides `package/summary`, taking `image_revision`, to
discover the fixed attached subject and its `graph_revision`.
`package/consumers` additionally takes `package_revision`, `provider_package`,
`provider_version`, and `target`, binding the independent package graph and the
exact provider declaration. A caller does not have to guess the graph digest.
The image expectation and ordinary before/after source authentication protect
the session; they do not prove that its Project consumes those packages.
Responses explicitly declare `project_association: "none"`. Manual Project
refresh does not rewrite, rebind or rediscover the immutable package subject.

Selected methods, instructions, closed schemas, typed clients and MCP tools
describe the same host-attached subject. Parallel reads share its immutable
derived facts, without package acquisition, mutable registries or execution
authority. No public package query can attach a graph or widen host policy.

The separate [Candidate Package Consumer Replay
v1](PROJECT-CANDIDATE-PACKAGE-CONSUMER-REPLAY-V1.md) derives a fresh graph from
an explicit candidate-era provider report, source and capsule, then requires
that provider source to equal one exact final-candidate source. This adds a
bounded source projection for known consumers; it does not change this graph's
independent `project_association: "none"` contract or infer installed consumers.

## Declared session protocols (issue #297 follow-on)

`PackageSemanticGraph::derive` selects `semaprax.package-semantic-graph.v2`
only when at least one selected package declares a `.spx` `session protocol`;
a protocol-free package graph keeps `semaprax.package-semantic-graph.v1` and
byte-identical output, mirroring the per-source graph's own `semaprax.graph.v48`
gating and the Workspace Semantic Graph's own `.v2` gating
(see [Session/protocol types v1](SESSION-PROTOCOL-TYPES-V1.md)). A declaring
graph gains one trailing `session_protocols` object: `base_schema` (the `.v1`
schema it extends), `authority: "none"`, and one fact per declaration, each
the same canonical fact the per-source graph emits plus `package` and
`version` naming the exact selected coordinate that owns it. Every `via` is
bound against the checked HIR built from that package's own source before it
is retained, in the same per-module pass the ordinary Workspace Semantic Graph
build already runs (`retain_workspace_module`); `build_package_scalar_sources`
is that same pass reused for the package-source workspace profile, so no
protocol fact here is derived by a second, independent code path. Declaration
count is bounded to 256 (four selected packages times the parser's own
64-declarations-per-module cap).

## Scope and preservation

The existing capsule profile remains two through four selected packages,
effect-free scalar `i64`/`bool` interfaces, explicit root-owned exports, and an
empty capability allowlist for the `wasm32` selection. This is not general
package-profile admission, source migration across package versions, dynamic
dispatch, registry discovery, deployment inventory, or trusted publisher
provenance. All existing source, import, resolver and capsule bounds remain.
The graph adds bounded deterministic query reports; excess inventory fails
rather than silently omitting relationships.

The derived graph is bounded to 16 MiB, each query report to 1 MiB, and the
cross-package call inventory to 65,536 sites. Selected interface function
inventory is bounded to 4,096 and imports to the capsule's existing 256 limit.
The ordinary transport response-envelope bound still applies after report
construction. These bounds do not claim total compiler heap or CPU metering.

Graph query grammar uses `SPX-PS601`; stale graph or unknown exact provider/
target selection uses `SPX-PS602`; graph/report inventory bounds use
`SPX-PS603`. Nested source-capsule/resolver admission retains its owning
diagnostics. Attachment lifecycle rejection uses `SPX-G280`; no failed
attachment changes the selected graph or host grants.

The graph is derived, deletable and reconstructible from the explicit source
and package inputs. It is not a new canonical package store. Private retained
facts do not change old capsule, resolver, lock, build or Project image wire
bytes. No cleanup vectors, source evaluation order, target semantics, or
publication rules are modified.

## Evidence

Focused cases in [semantic_graph.rs](../tests/offline_package/semantic_graph.rs)
are implemented. They exercise source/selection/interface replay,
coordinate and revision selection, import/call distinctions, independent
Project association, startup-only attachment, method availability and retained
read behavior. The released regression corpus is HOSTED GREEN; the full graph-operational
programme remains incomplete.

## Additive scalar-source Agent metadata (v070 candidate)

An already admitted scalar package source can contain inert Agent metadata with embedded scalar operation bodies. Such checked facts select `semaprax.package-semantic-graph.v4` and append `agent_execution` containing the actual v1/v2/v3 `base_schema`, `authority: "none"`, and package/version-qualified Agent rows. The [source syntax contract](LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md#additive-embedded-execution-metadata-v070-candidate) owns the row shape. The inventory is bounded to 256 rows over the existing four-package limit and 64 Agents per source. Existing protocol/follows limits and legacy graph bytes remain unchanged. This metadata does not broaden the scalar package ABI: nominal owned-State lifecycle sources still receive the existing `SPX-PS504` refusal. Authentication remains mandatory and these facts grant no runtime authority.
