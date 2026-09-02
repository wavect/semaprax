# Graph-operational execution evidence v1

Status: focused exact-commit runner executed locally for subject
`474c481bf3c3561c144e077f0000460f61af55f2`; the selected three-test gate passed
3/3. The full graph-operational programme remains Partial.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract turns the existing authored canonical-Git workflow regressions
into one inspectable **local** evidence bundle. It does not broaden the workflow,
promote an operation, or reinterpret one focused test process as a product-wide
quality gate.

## Exact subject and runner

The sole runner is:

```sh
python3 scripts/graph-operational-evidence.py
```

By default it writes a fresh bundle beneath
`.semaprax/evidence/graph-operational/<exact-sha>/<bundle-id>/`.
`--output <new-bundle-directory>` selects the exact new bundle directory
explicitly; the runner will not adopt an existing directory. The output is
private derived evidence and is not canonical source.

The runner must bind the bundle to the exact checked-out commit, reject a dirty
or detached subject that it cannot identify exactly, and record the current
source manifest and `Cargo.lock` state before invoking Cargo. A later commit,
even one containing documentation-only changes, is a different subject and
cannot inherit the result.

The reviewed local invocation for exact subject
`474c481bf3c3561c144e077f0000460f61af55f2` produced bundle
`5269b6acba08a197e6a8411ba95ccdec6e6a4ff724d35681344b5260087cb2e8`.
Its [archived envelope](evidence/graph-operational/474c481bf3c3561c144e077f0000460f61af55f2/5269b6acba08a197e6a8411ba95ccdec6e6a4ff724d35681344b5260087cb2e8/evidence.json)
and three authenticated artifacts are checked in as evidence of that subject,
not of this later documentation commit. The recorded host is Darwin arm64 with
Rust/Cargo 1.98.0, Git 2.47.1 and Python 3.14.2; this is local evidence only.

The focused command is exactly:

```sh
cargo test --locked --offline -p semaprax \
  --test project_graph_operational_git_workflow_v1 -- \
  --test-threads=1 --nocapture
```

The runner sets `CARGO_NET_OFFLINE=true` and `CARGO_INCREMENTAL=0`, records the
non-incremental selection in its envelope, and supplies an absolute
`SEMAPRAX_GRAPH_WORKFLOW_EVIDENCE_DIR` only to the test process. The two
successful provider scenarios export their already
validated compact economics observations as:

- `agent-task-economics-sha1.json`
- `agent-task-economics-sha256.json`

The bundle contains exactly `evidence.json`, `cargo.log` and the two reports.
The envelope uses schema
`semaprax.graph-operational-execution-evidence.v1`; `cargo.log` retains the
focused command transcript. Its top-level fields are `schema`, `bundle_id`,
`repository`, `runner`, `gates`, `artifacts`, and `claims`. `repository` records
the commit, clean pre/post state, unchanged head and its relation at capture;
the clean exact commit binds the checked-in source manifest and `Cargo.lock`.
Each gate independently records selection, prerequisite, provisioning and
outcome. The envelope authenticates its artifacts by path,
byte length and SHA-256 digest rather than embedding mutable filesystem paths
as programme authority. The economics reports retain their existing
`semaprax.agent-task-economics.v1` schemas and host-route-bound digest warning.

## Selected test inventory

The selected integration binary must report exactly these three nonignored
tests and all three must pass:

1. `twelve_step_v5_review_to_real_sha1_git_commit`
2. `twelve_step_v5_review_to_real_sha256_git_commit`
3. `competing_real_git_ref_consumes_approval_without_overwriting_the_other_commit`

The first two produce the format-specific economics artifacts. The third uses
the real restricted Git subprocess adapter to advance the fixed ref before
publication and proves the stale expected-base rejection; it does not produce
a third economics artifact.

The separately authored managed-generation test
`signature_evolution_merge_reports_tests_and_separate_managed_publication` is
ignored at source and is outside this command. Its evidence dimension must say
`not_selected`, never `passed`, `failed`, or implicitly covered by the Git
provider scenarios.

The successful envelope contains exactly two gate rows:

| Gate ID | Selection | Prerequisite | Outcome after a qualifying bundle |
| --- | --- | --- | --- |
| `graph_operational_git_workflow_v1` | `default` | `local_unix_git` | `passed` with the exact command, exit zero, 3/3/0 counts and the three named test rows |
| `graph_operational_managed_workflow_v1` | `explicit_ignored_required` | `known_fixture_correction` | `not_selected`, with the source ignore reason `SPX-G150 wrong ACTIVE schema, needs workspace init fix` |

Both rows record `provisioning: not_required`. This means neither row is a
separately provisioned-host gate; it does not assert that the managed fixture's
required correction is present. This is not a hosted or provisioned-platform
result.

## Orthogonal status dimensions

The envelope reports distinct dimensions. A passed value in one dimension must
not fill another:

| Dimension | What this focused runner may establish | Initial state before a reviewed run |
| --- | --- | --- |
| Exact local subject | Clean commit, manifest and lock binding for this invocation | `not_observed` |
| Canonical-Git integration tests | Three selected, nonignored tests and their process exit | `not_observed` |
| SHA-1 provider workflow | One exact real-bare-provider workflow and exported economics report | `not_observed` |
| SHA-256 provider workflow | One exact real-bare-provider workflow and exported economics report | `not_observed` |
| Stale-ref hostile case | One real preflight ref displacement without overwrite | `not_observed` |
| Managed `ACTIVE` workflow | Ignored test excluded from the command | `not_selected` |
| Hosted CI | No hosted job is launched or inspected by this runner | `not_observed` |
| Native target runtime | Only native-C11 projection facts occur inside the workflow | `not_selected` |
| Wasm target runtime | Only Core-Wasm structural validation occurs inside the workflow | `not_selected` |
| Generated clients | The workflow uses direct v5 frames, not generated client processes | `not_selected` |
| MCP | The workflow does not enter the MCP adapter | `not_selected` |
| VS Code/editor host | No editor process participates | `not_selected` |
| Programme completion | Owned only by the completion matrix and programme audit | `not_selected` |

`not_selected` means the runner deliberately did not request that gate.
`not_observed` means no qualifying result is present. Neither is success. The
runner emits no passing bundle for a failed selected command; a missing artifact
cannot be repaired to success from the Cargo exit code alone.

For a qualifying bundle, `claims` is still deliberately narrower than the
passed gate:

```json
{
  "bounded_twelve_step_git_workflow": "executed",
  "managed_active_workflow": "not_executed",
  "native_target_execution": "not_claimed",
  "wasm_target_execution": "not_claimed",
  "hosted_or_cross_platform": "not_claimed",
  "full_quality_profile": "not_claimed",
  "programme_completion": "not_claimed"
}
```

No consumer may infer a wider claim from the artifact names, test count, Cargo
exit code, or `bounded_twelve_step_git_workflow` value.

## Bundle acceptance

A qualifying local bundle requires all of the following:

- an exact clean subject binding and unchanged post-run subject state;
- the one locked focused Cargo invocation above;
- exactly three selected and three passed nonignored Git-workflow tests;
- zero failed selected tests;
- canonical compact JSON for both required economics artifacts;
- exact SHA-1 and SHA-256 format labels in their respective reports;
- twelve passed scripted criteria in each report;
- artifact byte counts and digests that replay against the files in the bundle;
- explicit `not_selected` for the ignored managed workflow and every other
  unselected integration dimension.

The bundle is invocation evidence. It does not become source, candidate,
approval, Git publication, or release authority. Copying or renaming a bundle
does not rebind it to another commit.

## Strict nonclaims

Even after a passing local bundle, it proves only the selected exact-commit
workflow on the recorded local host. It does **not** prove:

- hosted execution, cross-platform behavior, or the final repository head after
  subsequent integration;
- native-C11 or Wasm runtime execution, runtime equivalence, deployment, or an
  external consumer;
- generated TypeScript, Python, or Rust client execution;
- MCP conformance, the real MCP stdio workflow, VS Code host behavior, HTTP,
  cancellation, or asynchronous scheduling;
- the ignored managed-generation scenario;
- a physical mid-CAS race, crash, power-loss, durability, remote-repository, or
  checkout publication guarantee;
- model-token, tool-call, latency, validation-cost, human-review, correctness,
  or comparative productivity improvement;
- completion of the twelve-step requirement in its general ownership-sensitive
  form, any completion-matrix row, or the graph-operational programme.

Any later evidence claim must cite the exact subject SHA, bundle path, runner
schema, selected command, host facts, and artifact digests. “Current head” is
valid only while that exact subject remains the repository head being discussed.
