# Project Candidate Analysis Coverage Change v1

Status: **Partial, authored/unrun**. The authority-free library report and
regressions are authored. No test target, protocol route, generated client, MCP
surface, hosted run, or executed comparison is claimed.

## Contract

`ProjectCandidate::analysis_coverage_change` accepts the exact final candidate
revision. It independently derives a temporary semantic image from the
candidate's retained base `ProjectRevision`, regenerates
`semaprax.image-analysis-coverage.v1`, and separately regenerates the existing
`semaprax.project-candidate-analysis-coverage.v1` report for the fully admitted
final candidate. Neither nested report is supplied or trusted by the caller.

The closed report schema is
`semaprax.project-candidate-analysis-coverage-change.v1`, bounded to 3 MiB. It
binds the exact candidate, base and final Project/workspace/graph revisions,
the derived base image revision, both complete nested reports, their exact
SHA-256 digests, and a domain-separated report revision.

Five rows are compared in fixed order:

1. `deployment_configuration`;
2. `generated_file_provenance`;
3. `external_api_behavior`;
4. `runtime_environment`; and
5. `external_consumers`.

Each row retains the complete base and final area objects and the applicable
existing blind-spot rows. External API and runtime rows deliberately share the
existing `external_api_and_deployed_runtime_contracts` blind-spot declaration;
external consumers has no separate blind-spot object, so its closed area row is
the only comparison input.

## Categorical result

The comparison is about attached evidence status, never real-world coverage.
Exact equal rows are `unchanged`. A change from `not_inspected` to `partial` or
`known` is `advanced`; the reverse is `regressed`. Different rows at the same
status are `unknown`, because changed basis or limitations cannot be ordered.
No percentage, score, ranking, completeness, compatibility, or behavioral
equivalence is inferred.

This v1 report replays retained source and graph coverage only. It accepts no
deployment declaration, generator declaration, external-provider declaration,
runtime result, package graph, consumer inventory, filesystem path, or network
handle. Existing attachment reports remain separate invocations; their facts
are not silently imported into this comparison.

`SPX-G492` owns closed report/binding failures and `SPX-G493` owns capacity.
Every result keeps source, publication, filesystem, network, execution, and
runtime-observation authority false. `unchanged` does not mean the environment
or consumers stayed unchanged, `advanced` does not mean complete, `regressed`
does not prove a runtime regression, and `unknown` is not absence evidence.

## Authored evidence

The candidate harness authors an exact base-to-final source change and pins all
five current retained-source rows as `unchanged`, both nested bindings, the
3 MiB cap, false grants, and stale-candidate rejection. Module-local hostile
cases pin all four categorical outcomes without executing external systems.
They are authored and unrun.
