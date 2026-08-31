# Project Candidate Package Consumer Replay v1

Status: additive implementation and regression sources authored, **unrun**.
No installed-consumer discovery, compatibility, execution or publication claim
is made.

Audience: compiler contributors, package-tooling hosts and agents reviewing one
exact Project candidate against an explicit package corpus.

The existing [Package Semantic Graph v1](PACKAGE-SEMANTIC-GRAPH-V1.md)
authenticates coordinate-qualified imports and cross-package source call sites
from a complete offline source capsule. Candidate Package Consumer Replay
rebuilds that graph from caller-supplied candidate-era evidence and accepts it
only when the selected provider source is byte-for-byte the exact canonical
source in the final candidate. It does not scan a registry, installation,
workspace, deployment or filesystem consumer tree.

## Library API and authentication

```rust
pub struct CandidatePackageConsumerReplayInput<'a> {
    pub provider: &'a Coordinate,
    pub provider_source_path: &'a str,
    pub target: &'a str,
    pub capsule: &'a str,
    pub sources: &'a [PackageSource],
    pub resolution_evidence: &'a str,
    pub resolution_input: &'a ResolutionInput,
    pub resolution_options: &'a ResolutionOptions,
    pub capsule_options: &'a SourceCapsuleOptions,
}

pub fn ProjectCandidate::package_consumer_replay(
    &self,
    expected_candidate: &str,
    input: &CandidatePackageConsumerReplayInput<'_>,
) -> Result<String, Vec<Diagnostic>>;
```

The candidate selector is authenticated first. The manifest name and version
must equal the provider coordinate. `provider_source_path` must select exactly
one source in both the original base and final candidate. Exactly one explicit
package source must have that provider package, and its complete canonical
source must equal the final candidate source.

The method then independently derives `PackageSemanticGraph` through the
ordinary resolver, selected subjects, reports, implementation sources and
source-capsule replay. The provider report and capsule are therefore
candidate-era evidence. A baseline report paired with changed candidate source,
or a source carrying the same stable ID under another coordinate, cannot create
an association.

The graph's provider source and interface-source revisions must equal the
candidate Project source revision. Its package-domain source digest is
independently recomputed over the exact candidate source and compared with the
graph's provider and consumer facts; it is kept distinct from the Project source
digest because the domains differ. The selected coordinate and target must be
an actual verified interface export. Package reports must retain false source,
execution and publication authority. These checks associate only that provider
source projection; they do not associate the whole Project or candidate with a
package.

## Report

The compact no-LF report schema is
`semaprax.project-candidate-package-consumer-replay.v1`, capped at 2,097,152
bytes. Its 26 fields bind:

- candidate, base Project, final Project, Workspace and Project graph revisions;
- exact provider coordinate, logical source path, provider interface revision,
  package-domain source digest, base/final Project source revisions and digests,
  final byte length and whether that source changed from the base;
- exact target, package graph revision, source-capsule, source-set and link
  digests;
- the existing verified import and cross-package call rows plus package/import/
  call counts;
- fixed association, validation and unrun-test classifications; and
- false source, execution, publication, candidate-retention and graph-retention
  flags with fixed nonclaims.

Imports and calls remain separate. A declared import may have zero call sites.
Calls retain their authenticated caller/provider coordinates, source revisions,
contract/body site, expression and AST provenance, alias and ordinal. A call is
a static source relationship, not runtime execution or coverage. Callers outside
the linked export closure remain source facts when the verified capsule owns
them.

The replay uses the existing graph limits: two through four packages, at most
256 imports, 65,536 call sites and 4,096 selected interface functions. Package
source and capsule bounds remain unchanged. Provider package/version/target
bounds remain 255/128/4,096 bytes, and the Project logical path retains its
240-byte bound. Excess final report bytes fail instead of dropping facts.

Malformed or stale candidate selectors retain `SPX-G222` and `SPX-G224`.
Resolver/report/capsule failures retain their owning `SPX-PS5xx` diagnostics.
`SPX-G336` covers candidate/provider/source/report association failures;
`SPX-G337` covers final replay-report capacity.

## Authority and nonclaims

The eight fixed nonclaims state:

```text
no_ambient_consumer_discovery_or_completeness
candidate_association_covers_only_the_selected_provider_source
not_api_abi_or_behavioral_compatibility
imports_do_not_prove_calls
calls_are_static_authenticated_source_sites_not_runtime_execution
no_test_build_artifact_or_deployment_evidence
no_filesystem_network_registry_or_dependency_acquisition_authority
no_source_mutation_candidate_or_graph_retention_or_publication_authority
```

The method performs no package acquisition, version negotiation, compatibility
classification, consumer migration, target build, test, interpreter/native/Wasm
execution, artifact materialization, source update or publication. Empty results
mean only that the explicit verified capsule contains no matching fact; they do
not establish absence of other consumers.

## Evidence

Authored, unrun regressions in
`tests/project_candidate_package_consumer_replay_v1.rs` construct a real
candidate-era provider report and complete two-package source capsule. They
cover a called export and a separately imported-only export, a private caller
outside the linked export closure, exact source/candidate/package bindings,
baseline and sibling source mismatch, foreign coordinates and paths, tampered
package source, deterministic sibling isolation, no retained state and unchanged
authoritative source. No tests, compiler, package build, target, application or
quality gate were run while authoring this tranche.
