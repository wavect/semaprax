# Installed proof tools v1

Status: opt-in bounded source-proof tool profile.

Audience: Project authors and maintainers evaluating bounded law and proof support.

This opt-in source-proof profile runs an explicitly selected installed Lean or
Z3 executable. It is separate from complete law coverage and never executes
an application entry point, foreign library, or application build script.

## Host selection and process bounds

`proof_export::installed::InstalledProofTool::open` requires absolute executable
and working-directory paths, a tool kind, the exact expected version line,
an explicit host profile, limits, and a monotonic cancellation handle.
Acquisition holds executable/cwd handles through the existing registered
process provider. There is no PATH lookup, inherited environment, tool install,
network fetch or implicit proof-artifact provider.

`TrustedLocal` explicitly authorizes that installed executable and its runtime
libraries with the host user's rights. It is not a sandbox: filesystem/network
access, memory, and a deliberately escaped descendant are not confined.
`Confined` refuses before acquisition because this adapter cannot enforce that
profile. The host must trust the selected executable, toolchain libraries and
source translation; a version string does not authenticate a malicious tool.

Linux and macOS use the existing held executable process provider. Other hosts
refuse. Arguments are closed to `--version`, Lean `--stdin`, or Z3 `-in -smt2`.
The child environment is empty. Use the actual installed Lean executable, not
an automatic-installation shim. All scripts pass through bounded stdin;
the adapter creates no proof files and does not load arbitrary caller scripts
from a proof-reference string.

Default version/run limits are 2/10 seconds. Each may be selected within
1–30,000 ms. Combined argv wire and stdin are at most 65,536 bytes; the result
wire is at most 65,536 bytes including its 32-byte header. Each stream has a
maximum of 32,752 bytes. One tool capability permits at most 16 launches and
1 MiB of cumulative input plus reserved output. Version probes use the same
runner, bounds, cancellation and settlement as proof execution. No unbounded
`Command::output` is used by this product route.

The provider kills/settles owned process groups before a successful return.
Timeout, cancellation, output overflow, nonzero exit, malformed or partial
output never produces checked proof. Failed settlement follows the existing
quarantine/fail-stop contract in [Bounded Process I/O v1](BOUNDED-PROCESS-IO-V1.md),
including its absence of a hard OS reap deadline. Abrupt host termination and
descendants that deliberately escape the trusted group are not confinement
claims.

## Exact Project evidence

`installed_project::prove_postcondition` selects an exact source/declaration/
postcondition in a retained Project, resolves it, and uses the existing closed
source translation. Missing, unselected or unsupported subjects refuse before proof work. The
declaration must belong to an existing entry/public/test HIR view; proof
requests cannot silently expand Project assurance coverage.
Lean generates the existing certificate, kernel-checks the exact export, then
replays its ProgramRoot binding through the existing opaque attachment path.
The semantic Lean pin is `leanprover/lean4:v4.34.0`, separate from the host's
exact version-line pin.

Z3 receives the existing checked-arithmetic SMT formula with only the trailing
model-retrieval command omitted. A strict success requires exit 0 and exactly
`unsat` plus surrounding whitespace. `sat`, `unknown`, surplus output and
partial answers refuse. The opaque Project attachment binds script bytes,
source revision/digest, Project/ProgramRoot, postcondition and exact version.
It claims source-level checking under the trusted translator, not proved
lowering or independently checked SMT proof objects.

Strict Law Assurance's `pinned_smt_source` requirement additionally requires
the exact tool version and the accepted frozen SMT translation profile. The
unpinned `smt_source` requirement continues to refuse.

`law_set::native_proof::prove_scalar_law` is the explicit native relational-law
entry point. It authenticates the selected LawSet against the retained Project,
constructs a typed function whose postcondition is the exact law proposition,
and obtains actual Lean/Z3 confirmation through the same runner. The function
introduces no precondition and is never executed. All admitted scalar bounds
and checked arithmetic remain part of the source proof. The opaque token binds
the exact Project/ProgramRoot, inventory, law semantics and generated proof
bytes. It cannot be constructed from a wire success, callback or proof URL.
Named assumptions and prerequisite laws are settled independently by the law
inventory evaluator; obtaining one proposition proof does not discharge them.
Complete protected-route configuration remains open #379 work.

## CLI

```text
semaprax project-proof-check /absolute/semaprax.toml \
  --tool lean|z3 --executable /absolute/installed/tool \
  --version-line "exact version output" --host-profile trusted-local \
  --source src/app.spx --declaration app.function --ensures 0
```

For an explicitly selected native relational declaration, replace the last
three selection options with `--law <stable-law-id>`. This form derives the
inventory from the Project's retained `law_sources`; it refuses a missing or
non-relational law. The result schema is
`semaprax.installed-native-law-proof-check.v1`, binding the Project/ProgramRoot,
whole law inventory and exact semantic law ID. It reports one checked law;
serialized output cannot recreate the opaque proof token or select strict
policy. Source and native-law selection options cannot be mixed.

All common tool options and one complete subject selection are required;
duplicates and unknown options are syntax errors.
The result schema is `semaprax.installed-project-proof-check.v1`, including the
exact Project assurance report, host limits and explicit nonclaims. This route
checks one selected postcondition; it does not claim complete law coverage,
application execution, build admission or publication authority. A false or
unsupported proof exits unsuccessfully without mutating source or Git.

## Selected-law failure workflow

An installed tool can drive the additive host-selected LAW-12 diagnostic
workflow after the host installs a strict law policy:

```text
semaprax project-proof-check /absolute/semaprax.toml \
  --workflow detail --law app.required-law \
  --tool z3 --executable /absolute/z3 --version-line "exact version output" \
  --host-profile trusted-local \
  --source src/app.spx --declaration app.function --ensures 0
```

`--workflow summary` returns an `--offset`/`--limit` page; `detail` returns
one selected law and a repair target. Both replay the complete protected
inventory against the current authenticated Project and repeat its whole
acceptance verdict and counts. The JSON envelope binds the candidate revision,
law semantics, policy, dependency IDs, failed obligation IDs, evidence profile,
and source location where available. The v2 envelope adds `validity` copied
from the independently replayed strict verdict and full required counts, plus
`work` from the held tool's monotone process/query and reserved-byte ledger.
That work is attempted local process reservation, including version probes;
it is neither a completed-query count nor provider spend. Model tokens and
provider cost are explicitly unavailable here. A failure exits with status 1
after printing bounded JSON. `--max-bytes` refuses an oversized envelope
rather than truncating counts or the verdict.

For an admitted Z3 postcondition failure, a separate bounded model query is
checked with the independent source evaluator. Only a reproduced checked trap
or violated ensures is reported as `disproved_concrete`. Other results remain
`unknown`, `timeout`, `unsupported`, `incomplete`, or `solver_error`; none is a proof token.
Concrete values are redacted by default; `--show-witness-values` is available
only with `detail` for a trusted local caller. An implementation edit changes
the candidate revision and must be reproved. An edit to protected law intent
requires the separate host specification-review route and cannot be counted
as a successful repair by this command.

### Opt-in selected-law agent transport

`semapraxd --stdio --manifest-path /absolute/semaprax.toml
--allow-project-law-workflow --law-tool z3|lean
--law-executable /absolute/tool --law-version-line "exact output"` selects the
additive `semaprax.agent-transport.v7` profile. It uses the existing bounded
Project NDJSON/JSON-RPC codec. Startup pins the manifest and tool; requests
cannot choose another root, executable, version, host profile, source write,
or publication route. Existing v2-v6 profiles and methods are unchanged.

The four methods are `protocol`, `law/status`, `law/check`, and `shutdown`.
`law/status` returns the authenticated current candidate revision, selected
law inventory digest, and host policy digest. `law/check` requires the exact
`candidate_revision`, `law_id`, and `view` (`summary` or `detail`); a source
postcondition also supplies `source`, `declaration`, and `ensures_index`.
Optional `offset`, `limit`, and `max_bytes` use the owning workflow bounds.
Both CLI and daemon call the same library evaluator after independently
reloading the host-selected policy. A stale requested revision yields a
`stale` nonproof attempt with complete current counts and no solver invocation.
The daemon never retains a prior proof token across edits. Law intent drift
refuses through the existing protected review boundary.

`--mcp` is an additional exclusive startup switch for this selected-law
profile. It retains the same manifest/tool pins and bounded stdio framing,
then exposes a fixed MCP 2025-11-25 catalog with `law__status` and
`law__check`. After `initialize` and `notifications/initialized`, each
`tools/call` returns one text item containing the complete v7 JSON-RPC
response with inner ID zero and `isError` matching its error envelope. The
same authenticated selected-law dispatch checks current source/policy on
every call. Unknown tool names and request-supplied tool/root fields refuse;
MCP does not add edit or publication authority. The direct v7 JSON-RPC mode
remains the default without `--mcp`.

This is a local stdio MCP tool surface, not a hosted service or editor plugin.

The exact LAW-12 status selector
`workspace selected_law_status_test::selected_law_unknown_timeout_unsupported_and_stale_preserve_summary_and_detail_counts`
uses explicitly compiled local process fixtures to exercise diagnostic
classification in summary and detail. Those fixtures do not establish a proof;
the installed-Z3 selectors above own the real counterexample and repair gates.
The existing agent-workflow `connectMcpWorkflowTransport` adapter carries
`law/status` and `law/check` through the two MCP tool names. Its
`ToolPayloadObserver` records the one delivered tool text in the existing
`semaprax.token-observation.v1` envelope. Observer `success` denotes payload
delivery only; the strict `view.accepted` and complete required count remain
the independent law-validity result. No model billing or provider cost is
inferred from tool-payload bytes.

## Focused physical gate

The ignored Workspace selector `project_assurance_manifest::law_set::installed_law`
requires explicit `SEMAPRAX_LAW_LEAN`, `SEMAPRAX_LAW_LEAN_VERSION`,
`SEMAPRAX_LAW_Z3`, `SEMAPRAX_LAW_Z3_VERSION` and `CLANG` provisioning, then
runs with `--ignored`. Missing provisioning fails; it never silently skips.
The gate covers both real kernels and CLI calls against a newly authored
postcondition plus false/stale/trust changes, hanging/overflow/malformed probes,
cooperative cancellation and owned descendant settlement. Recorded callbacks
cannot substitute for these physical invocations.

Local gate result (2026-10-03): 3 passed, 0 failed, 211 filtered; 10.30s
using Lean 4.34.0 (commit `293d5d0c0c3f3dded4688b3ccd6a33939ac5102b`) and
Z3 4.12.5 on arm64 macOS. The exact invocation was the Workspace selector
above with `--offline --locked`, `--ignored --test-threads=1`, one Cargo job,
debug info disabled and explicit installed paths/version pins. This is local
source-proof/process evidence, not cross-platform confinement or complete
LAW-04 admission.

Native relational local gate (2026-10-03): `workspace installed_native_law`
with the same explicit tool pins and ignored-test provisioning passed both
library cases (2 passed, 214 filtered, 5.93s). Actual kernels accepted the new
typed identity and refused false and overflowing propositions. Exact candidate
replay, missing/duplicate/stale/forged evidence, proved-lowering refusal and
open assumptions/prerequisites were exercised.

The separate exact `workspace
installed_native_law_cli_checks_new_law_and_refuses_false_or_mixed_selection`
selector passed 1 case (216 filtered, 7.10s), using both actual kernels through
`project-proof-check --law`. False laws and mixed subject options refused;
all manifest/source bytes remained unchanged and no `ACTIVE` or Git directory
was created. Both native selector invocations used `cargo test --offline
--locked -p semaprax --test workspace <selector> -- --ignored --test-threads=1`
with the same explicit tool pins, private target, one job and disabled debug
info as the installed source-proof gate.

The checked `scripts/law14-strict-gate.sh` selector requires all four exact
Lean/Z3 provisioning variables before invoking Cargo, so absent tools fail
setup instead of skipping. It runs the fast mutation corpus, the forged
nonempty-reference regression, the deterministic strict final-boundary race,
and the ignored `installed_native_law_law14_adversarial_gate` selector serially. The real corpus rejects
a wrong law body, proof reuse for another body, Lean `sorry` and a non-policy
axiom, missing executable, timeout and false proposition through actual Lean
and Z3 invocations. Its snapshots require authored source, `ACTIVE` and Git to
remain absent or byte-identical after each refused route.
