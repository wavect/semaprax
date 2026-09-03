# Project Candidate Analysis Evidence v1

Status: additive implementation and regression sources authored, **unrun**.
No completeness, compatibility, installed-consumer discovery, execution or
publication claim is made.

Audience: agent authors, package-tooling hosts and compiler contributors
reviewing the explicit analysis boundary of one immutable candidate.

[Project Candidate Analysis Coverage v1](PROJECT-CANDIDATE-ANALYSIS-COVERAGE-V1.md)
keeps external consumers `not_inspected` because retained Project source cannot
identify them. [Candidate Package Consumer Replay
v1](PROJECT-CANDIDATE-PACKAGE-CONSUMER-REPLAY-V1.md) independently authenticates
one caller-supplied candidate-era provider report, source and complete offline
package capsule. Candidate Analysis Evidence composes those two reports and
changes only the `external_consumers` area to `partial` for that explicit
corpus.

A separate additive declaration attachment addresses the deployment blind spot
without pretending to observe a deployment. The closed schema
`semaprax.project-candidate-deployment-contract-declaration.v1` binds exact
canonical bytes and a length-delimited SHA-256 digest to the candidate, its
complete ordered manifest export inventory, and 1 through 64 sorted
`{key,type,required}` rows. Every export must join one retained explicit
function identity. The grammar has no value, secret, path, URL or provider
locator field.

`analysis_deployment_contract_evidence` changes only
`deployment_configuration` and its blind-spot row to `partial`, embeds the
authenticated declaration bytes/digest, and retains false source, environment,
deployment and publication authority. Caller-declared key shapes are not
environment observation, freshness, drift, artifact/runtime/API/consumer
verification or proof that the declared configuration is present or used.

Candidate-enabled v5 sessions expose the same derivation through
`candidate/analysis-deployment-contract-evidence`. Requests bind the live image,
candidate, exact canonical declaration string and digest; the compiler
regenerates the complete report on every chunk request. Responses use the
closed `semaprax.image-candidate-deployment-contract-evidence-chunk.v1` schema,
1 through 64 KiB UTF-8 chunks, a 2 MiB report cap and one invariant report
SHA-256. Discovery, generated TypeScript/Python/Rust clients and MCP expose the
method only with the existing candidate read grant. It is deliberately absent
from parallel-read batches and adds no filesystem, environment, network,
secret, provider or deployment capability.

## API and exact composition

```rust
pub fn ProjectCandidate::analysis_evidence(
    &self,
    expected_candidate: &str,
    input: &CandidatePackageConsumerReplayInput<'_>,
) -> Result<String, Vec<Diagnostic>>;
```

The method authenticates the candidate before independently recomputing both
owner reports. Their candidate, base Project, final Project, Workspace and
Project graph bindings must agree exactly. Both reports must retain false
source, execution, publication and candidate-retention flags; package replay
must also retain `graph_retained: false`.

The no-LF output schema is
`semaprax.project-candidate-analysis-evidence.v1`, capped at 3,145,728 bytes.
It starts from all 19 candidate-analysis-coverage fields, replacing the schema
value and changing `evidence_class` to
`retained_source_and_explicit_package_consumer_evidence`, and adds the exact
complete 26-field `package_consumer_replay` object. The root therefore has 20 fields. Package graph/capsule/source digests,
coordinate-qualified imports and authenticated static call sites remain owned
by the nested replay rather than being copied or reinterpreted.

Seven of the eight coverage-area rows remain byte-for-byte equal to ordinary
candidate coverage. Only `external_consumers` becomes:

```text
status: partial
basis: explicit_authenticated_candidate_provider_package_consumer_source_replay
limitations:
  absence_from_this_replay_is_not_absence_of_other_external_consumers
  not_api_abi_or_behavioral_compatibility
  imports_and_static_calls_are_not_runtime_execution
required_evidence:
  authorized_installed_consumer_inventory
  consumer_compatibility_and_runtime_conformance_evidence
```

`partial` applies when the selected export has a declared import but zero call
rows and when the explicit corpus has neither a matching import nor call. Such
a result says only that this explicit verified corpus has no matching fact. It
never changes to `known`, never proves absence of other consumers, and does not
upgrade deployment, generated provenance, artifact, external API or runtime
evidence.

## Diagnostics, bounds and authority

Candidate selector diagnostics remain `SPX-G222`/`SPX-G224`. Package report,
resolver and capsule diagnostics propagate unchanged, as do package-replay
`SPX-G336` association and `SPX-G337` capacity failures. `SPX-G338` covers an
invalid composite owner shape or disagreeing bindings; `SPX-G339` covers the
final 3 MiB report bound. Owner bounds remain active before composition; facts
are never dropped to make the wrapper fit.

The inherited candidate-coverage nonclaims remain unchanged and the nested
package replay retains its eight strict nonclaims. In particular, this report
does not scan installed packages, registries, deployments, filesystems or
networks; discover dynamic callers; classify API/ABI/behavioral or semantic
version compatibility; migrate consumer source; build, test or execute a target;
retain a graph/image/candidate; mutate source; or grant publication authority.

## Evidence

Authored, unrun regressions in `tests/project_candidate/analysis_evidence.rs`
construct an exact candidate-era provider report and two-package source capsule,
compare the nested replay with its independent owner result, preserve seven
coverage rows exactly, establish `partial` for explicit called, import-only and
zero-match consumers, retain exact revisions/digests and import/call rows,
reject stale, baseline, sibling and tampered evidence, preserve sibling
determinism and leave candidate/source state unchanged. No tests, compiler,
package build, target, application or quality gate were run while authoring this
tranche.
