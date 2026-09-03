# Project Candidate Analysis Coverage Change v1

Status: **Partial, authored/unrun**. The authority-free library report and
regressions are authored. No test target, protocol route, generated client, MCP
surface, hosted run, or executed comparison is claimed.

Audience: agent authors, compiler contributors, and candidate reviewers.

## Contract

`ProjectCandidate::analysis_coverage_change` accepts the exact final candidate
revision and a closed `CandidateAnalysisCoverageChangeInput`. The input has
separate optional base and final `CandidateAnalysisCoverageBoundaryInput`
values. Each contains only exact canonical boundary-bundle bytes and the owning
domain digest; it contains no status, area, score, or report assertion.

The method constructs a new immutable `ProjectCandidate` over the exact
retained base revision. For each side without a bundle it independently
regenerates `semaprax.project-candidate-analysis-coverage.v1`. For each side
with a bundle it invokes that exact candidate's existing
`analysis_boundary_bundle` owner, which authenticates the candidate selector,
canonical bundle bytes and digest and independently replays the deployment,
generated-file, and external-API child declarations through their owning APIs.
Neither nested coverage report nor any evidence status is supplied or trusted
by the caller.

The closed report schema is
`semaprax.project-candidate-analysis-coverage-change.v1`, bounded to 5 MiB. It
binds the exact base and final candidate revisions, both Project/workspace/graph
revisions, both complete regenerated reports, their exact SHA-256 digests,
which evidence owner produced each report, and a domain-separated report
revision.

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

The comparison is about authenticated evidence status, never real-world coverage.
Exact equal rows are `unchanged`. A change from `not_inspected` to `partial` or
`known` is `advanced`; the reverse is `regressed`. Different rows at the same
status are `unknown`, because changed basis or limitations cannot be ordered.
No percentage, score, ranking, completeness, compatibility, or behavioral
equivalence is inferred.

This v1 composes only the existing three-declaration boundary bundle. Therefore
deployment, generated-file, and external-API rows can truthfully advance or
regress when one side has independently authenticated declarations and the
other does not. Runtime and external-consumer rows are regenerated from their
source-only coverage owners and remain unchanged in this version. The method
accepts no runtime result, test policy, package graph, consumer inventory,
filesystem path, or network handle. Their distinct evidence owners are not
silently imported.

`SPX-G492` owns closed report/binding failures and `SPX-G493` owns capacity.
Every result keeps source, publication, filesystem, network, execution, and
runtime-observation authority false. `unchanged` does not mean the environment
or consumers stayed unchanged, `advanced` does not mean complete, `regressed`
does not prove a runtime regression, and `unknown` is not absence evidence.

## Authored evidence

The candidate harness authors an exact base-to-final source change and pins all
five source-only rows as `unchanged`. It then builds candidate-bound canonical
declarations for each side and pins three real `advanced` and three real
`regressed` rows while runtime and external consumers stay unchanged. Supplying
the final candidate's bundle as base evidence fails in the existing owning
bundle API before any comparison. The cases also pin the 5 MiB cap, false
grants, stale-candidate rejection, and module-local categorical helper behavior.
They are authored and unrun.
