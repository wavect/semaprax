# Semantic Image Analysis Coverage v1

Status: implementation and regression sources authored, unrun. No completeness,
runtime, deployment, or completion-matrix promotion.

Audience: agent authors, embedding hosts, and compiler contributors.

Finding the right symbol does not establish its complete runtime contract.
`ProjectSemanticImage::analysis_coverage(expected_image)` exposes both retained
facts and analysis blind spots as a bounded, read-only graph-facing report.
It uses schema `semaprax.image-analysis-coverage.v1`; canonical `.spx` remains
authoritative. The query creates no graph-only meaning and grants no authority.

## Selection and report

The v5 `image/analysis-coverage` method takes only `image_revision`. It belongs
to the default semantic read surface, including the host's parallel read API;
candidate, diagnostic, build, test, and publication grants are unnecessary.
Normal session source authentication still surrounds dispatch. The standalone
library query describes its retained immutable image, not the current checkout.

The closed report binds image, Project, workspace, and Project graph identities.
It includes the retained manifest's schema, optional profile, entry/test module,
ordered source paths, exports and capabilities; exact per-source module,
revision, digest and graph-schema bindings; and counts of functions, templates,
instances, nominal types, interfaces and interface imports. Interface-import
rows expose actual checked owner/import identities, name, import key,
`native_rust`, effects and required authority. These are declarations, not
independent provider verification or granted host capabilities.

Graph v25 and Project image admission now retain Native Rust import
declarations. The coverage report therefore exposes the actual declaration
with `native_rust:true`, effects and required authority. This remains partial
declared-contract evidence: it does not authenticate a Rust provider, deployed
implementation, runtime version or observed call. The separate non-native
resource-import case carries the same boundary with `native_rust:false`.

Each `areas` row has an area name, `status`, `basis`, `limitations`, and
`required_evidence`. The evidence descriptions explain what is missing; they
are not executable requests, available host grants, or permission to fetch it.

| Area | Status and scope |
| --- | --- |
| `declared_source_inputs` | `known`: exact retained manifest/source/graph bindings, within the admitted profile. No current-disk claim. |
| `declared_external_contracts` | `partial` when interface imports exist; otherwise `not_inspected`. Neither case proves absence of undeclared dependencies. |
| `deployment_configuration` | `not_inspected`: environment, secrets, routing and infrastructure are not discovered. |
| `generated_file_provenance` | `not_inspected`: listed generated `.spx` is checked as source, but generator identity, inputs and freshness remain unknown. |
| `generated_artifacts` | `not_inspected`: this query does not emit/replay target artifacts or bind them to deployment. Existing projection APIs remain separate. |
| `external_api_behavior` | `not_inspected`: provider implementation, remote versions, availability, authentication and side effects are unknown. |
| `runtime_environment` | `not_inspected`: no execution, path coverage, liveness or environment-drift observation. |
| `external_consumers` | `not_inspected`: exports do not enumerate installed clients or external callers. |

These statuses describe the evidence available to this query. They are not
coverage percentages, security verdicts, proof of contract conformance, or an
inventory of all files and services. In particular, an absent import or graph
edge is not evidence that no external API or consumer exists. Filenames do not establish
generated provenance. `source_authority`, `external_io`, and `execution` are
always false, and the report carries explicit nonclaims.

The additive `blind_spots` ledger makes the three most consequential absences
machine-readable in fixed order: `deployment_configuration`,
`generated_file_provenance`, and
`external_api_and_deployed_runtime_contracts`. Every row has
`evidence_status:"absent"`, names the evidence not supplied, binds the exact
retained Project revision and its manifest source inventory, and states that
missing evidence is not proof that the corresponding contract is absent. The
candidate projection re-derives the same ledger from the candidate revision,
so its binding changes when canonical candidate source changes. These rows do
not scan, fetch, execute, or verify any external system.

## Bounds and compatibility

The exact expected image is checked before inventory construction. At most
16 source modules and 65,536 combined inventory facts are admitted. A
conservative construction budget accounts for string escaping, and final JSON
is limited to 1 MiB. Oversized reports fail rather than silently truncating or
omitting unknown areas. The transport additionally enforces its ordinary 1 MiB
response-envelope limit. These are report bounds, not a global heap bound.
Sources and interface imports are sorted only for deterministic presentation;
no runtime or cleanup-plan vectors are reordered.

Stale image expectations retain the image API's diagnostic. Invalid retained
joins use `SPX-G219`; inventory/construction bounds use `SPX-G220`. The query
does not scan directories, read unlisted files, contact APIs, generate files,
execute programs, or change image serialization/digest or earlier protocols.
The v5 discovery bundle owns a fully closed response schema and derives typed
client helpers from the selected method registry.

The additive [candidate projection](PROJECT-CANDIDATE-ANALYSIS-COVERAGE-V1.md)
applies this same collector to an exact fully admitted candidate revision. It
preserves these facts and blind spots, adds candidate/base bindings and no
authority, and does not reinterpret an unchanged status as verified evidence.

Focused library and transport regressions in
[image_protocol/analysis_coverage_v1.rs](../tests/image_protocol/analysis_coverage_v1.rs) are
authored but unrun. Broader deployment ingestion, generator provenance, provider
conformance, and external-consumer analysis require separate explicit inputs
and independently designed authority boundaries.
