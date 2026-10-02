# Kernel-0 Accepted-Revision Validation v1

Audience: the reviewer resolving issue #328.

Status: a closed acceptance-record format, read-only validation gate, and
receipt inventory. The reviewed component decision is to retain rung 1.
The six focused receipts at the exact candidate are complete; no
rung promotion, hosted result or full-profile pass is claimed.

## Purpose and boundary

Issue #328 requires one bounded decision over the existing Kernel-0 formatter
candidate. The evidence in the renderer, authority, bootstrap, target-recovery,
owned-handoff, differential, and Lean records was produced at different times.
This document provides the single record that prevents a reviewer from joining
those observations into an accepted-head result without either rerunning a gate
or proving its complete subject unchanged.

The record concerns the five scalar formatter lanes and their ordinary checked
`own Bytes -> Bytes` handoff. Rust remains formatter authority. A matching
candidate still copies into a Rust caller-owned token; it is neither an owned
Kernel-0 renderer nor an ownership theorem. The record does not authorize a
target run, a publication, a support-policy change, or any work above rung 1.

## Candidate and receipt rule

The reviewer first chooses one immutable 40-lower-hex Git commit as
`candidate_revision`. A receipt may count for that candidate only when it says
which command or gate ran, whether it passed, its execution revision, and the
relevant tool versions where the gate executes external tools. An old result is
eligible only when its `execution_revision` equals `candidate_revision`, or the
review record identifies the gate's complete committed subject--its selector,
harness, profile sources, fixtures, generated-input definitions, and
`Cargo.lock`--and demonstrates that set is byte-identical between the two
revisions.

Comparison must be made from the exact commits, rather than an ancestor,
branch name, working tree, test cache, or digest reminted from modified inputs.
A result whose subject inventory is unknown, incomplete, or changed is pending
and must be rerun. The reconciliation records the exact compared path list and
both full commit IDs. A source-equivalence comparison validates only the named
unchanged subject; it does not turn a historical execution into a new execution
of a broader profile.

## Read-only record gate

`scripts/kernel0-accepted-revision-gate.py` validates one canonical JSON record
without running a receipt command, writing any input, selecting a candidate
revision, or changing the current outcome. It requires full lowercase 40-hex
commits and resolves each as a commit object. Its fixed ordered inventory is the
seven rows below. The baseline row alone also permits the explicit user waiver
described below; the other six rows must still execute or reconcile.

An `executed` receipt must have passed at `candidate_revision`. A `reconciled`
receipt records its earlier execution commit plus a complete declared inventory
grouped as selector, harness, profile sources, fixtures, generated inputs, and
the exact tracked `Cargo.lock`. The gate requires all declared paths to be
regular tracked file blobs (mode `100644` or `100755`, never symlinks) at both
commits and uses Git's exact commit comparison to
refuse byte drift. It validates the declared inventory; it does not infer that
a reviewer declared every relevant file. The owning receipt specification
remains responsible for that completeness judgment.

The gate accepts only `validation-incomplete` and `rung-1-retained` outcomes.
It deliberately cannot validate or produce `rung-2-promoted`: component-boundary
acceptance remains the separate explicit reviewer decision required below.

The record's top-level keys are exactly `schema`, `candidate_revision`,
`receipts`, `outcome`, and `reason`; its bytes are canonical sorted-key JSON
with one trailing LF. Receipts occur once in this fixed order: `lean-proof`,
`renderer-authority`, `bootstrap-artifact`, `scalar-targets-recovery`,
`differential-corpus`, `owned-handoff`, `baseline-preservation`. A pending row
is exactly `{"id":"...","state":"pending"}`. An executed row additionally
names a nonempty `command`, true `passed`, `execution_revision` equal to the
candidate, and a string-valued `tool_versions` object. A reconciled row instead
includes `inventory`: six nonempty, sorted path lists named `selector`,
`harness`, `profile_sources`, `fixtures`, `generated_inputs`, and `cargo_lock`.
The final list is exactly `["Cargo.lock"]`, and paths may not overlap. Both
completed forms retain the same `command`, `passed`, `execution_revision`, and
`tool_versions` fields. Unknown or omitted keys refuse.

A waived row is exactly `id`, `state`, `authority`, `scope`, and `reason`.
It is admitted only for `id: baseline-preservation`, `state: waived`,
`authority: user`, and `scope: local-full-profile-delegated-to-hosted-ci`,
with a nonempty reason. It has no `passed`, command or execution revision: a
waiver is not execution evidence. The gate checks this closed declaration;
the accompanying review must identify the actual user instruction. No waiver
can satisfy any of the six focused receipt rows or hide a pending row.

Focused regression selector:

```sh
python3 scripts/kernel0-accepted-revision-gate.py --self-test
```

For a committed record, invoke the same script with `--record PATH` and an
explicit checkout path via `--repository`. The script emits a small canonical
summary on success and a stable `SPX-K328-*` diagnostic on refusal.

## Required local receipt inventory

The accepted-profile review must contain one passing receipt or an exact-subject
reconciliation for each focused row. On 2 October 2026 the user instructed:
“skip the full profile as acceptance criteria, the hosted ci runs will handle
that later.” Accordingly, `baseline-preservation` is explicitly waived for
local #328 closure; hosted CI remains responsible for the full profile.
This instruction changes no focused receipt or implementation requirement.
The named owner remains the source of truth for
the focused selector and hostile cases; this table intentionally does not
duplicate `quality.sh`'s full-route command sequence.

| Receipt | Owner and required subject | Acceptance state |
|---|---|---|
| Lean proof | [Kernel-0 proof mechanization](KERNEL-PROOF-MECHANIZATION-V1.md) and the `kernel0-lean-proof-gate`, including source pins, hostile controls, build, and axiom audit when the pinned toolchain is available | Executed at `f99c76dc2`; see retained receipt inventory below |
| Renderer and formatter authority | [Formatter Authority v1](KERNEL-ZERO-RUNG-TWO-AUTHORITY-V1.md), the five embedded renderer sources, `src/format/kernel_zero_tokens.rs`, and their broad byte-oracle/shadow and production traversal cases | Executed at `f99c76dc2`; see retained receipt inventory below |
| Bootstrap artifact | [Bootstrap v2](KERNEL-ZERO-RUNG-TWO-BOOTSTRAP-V2.md), including exact regeneration, v1 refusal, decoding, and hostile wire cases | Executed at `f99c76dc2`; see retained receipt inventory below |
| Scalar targets and recovery | [Target and Recovery Evidence v1](KERNEL-ZERO-RUNG-TWO-TARGET-RECOVERY-V1.md), including C11 `-O0`/`-O2`, Node/Core-Wasm, candidate corruption, Rust-byte recovery, and re-entry | Executed at `f99c76dc2`; see retained receipt inventory below |
| Differential corpus | `kernel_zero::differential`, including reference/interpreter agreement and native C11 `-O0`/`-O2` plus Core-Wasm agreement with required target tools | Executed at `f99c76dc2`; see retained receipt inventory below |
| Owned handoff | [Owned Handoff v1](KERNEL-ZERO-RUNG-TWO-OWNED-HANDOFF-V1.md), including binding authentication, graph replay, zero-owner mutation refusal, settlement, panic recovery, and the 13-row native/Wasm wrapper evidence | Executed at `f99c76dc2`; see retained receipt inventory below |
| Baseline preservation | [Quality gates](QUALITY-GATES.md)'s `full` profile; the focused Lean gate remains required independently | Waived locally by the explicit 2 October user instruction; delegated to hosted CI, not passed |

Historical counts and partial local receipts can remain cited as background,
but cannot fill a pending cell without the preceding candidate/reconciliation
rule. A missing hosted receipt does not invalidate a completed substantive
local receipt for this issue. Hosted evidence is recorded separately if later
claimed.

## Review outcome

After the inventory is complete, the responsible reviewer records exactly one
outcome with the candidate revision and links to every receipt:

| Outcome | Meaning |
|---|---|
| `rung-1-retained` | The six focused local receipts are complete and the baseline is either complete or explicitly waived as above, but the rung-2 criterion remains unmet or intentionally unpromoted. Name the narrow technical reason. |
| `rung-2-promoted` | The reviewer accepts that the stated rung-2 criterion is met. This requires a separate explicit decision that the five scalar lanes meet the component boundary; wrapper evidence alone cannot supply it. |
| `validation-incomplete` | At least one required focused row is pending, failed, or lacks an exact-subject reconciliation. The explicit baseline waiver is not a pending focused gate. |

The decision must state whether the remaining reason is a technical boundary,
an intentionally deferred promotion, or a failed/missing local receipt. It must
not describe missing hosting as the reason by itself. The decision also leaves
the finite differential corpus, Lean proof boundary, Rust formatter authority,
and whole-compiler verification claims at the limits stated by their owning
specifications.

## Reviewed component decision and completed record

The canonical [accepted-profile record](evidence/kernel-zero-accepted-revision-f99c76dc2.json)
selects `f99c76dc2d26dd57c81f4fdd5f26fe91d50118e4`. All six focused rows
are `executed` at that exact revision, the baseline row is explicitly `waived`,
and the reviewed outcome is **`rung-1-retained`**.

Reviewed on 2 October 2026: **retain rung 1; do not promote rung 2**. The five
scalar lanes still return individual bytes/lengths; Rust assembles the
candidate and retains the authoritative output. The ordinary checked
`own Bytes -> Bytes` wrapper transfers that Rust-assembled token. It does not
make the Kernel-0 renderer own its output, transfer formatter component
authority, or extend the Kernel-0 theorem to ownership. This is the concrete
unmet component boundary, not a missing hosted badge. No further implementation
or proof development is required to resolve #328 with this explicit
non-promotion outcome.

The [retained receipt inventory](evidence/kernel-zero-f99c76dc2/README.md)
links raw logs, exact runner scripts, execution-context/tool metadata, byte
hashes and a requirement-by-requirement coverage map. The Kernel-0 selector
passed **124 tests, 0 failed, 0 ignored**, with required native O0/O2 and
Node/Core-Wasm tools. The separate `owned_handoff` selector passed **10 tests,
0 failed, 0 ignored**, including the four retained-call lifetime/alias/panic
cases omitted by the first filter. Six tests overlap the two selectors;
these counts are execution totals, not 134 distinct cases.

The Lean gate used `--require-kernel`, passed its source pins and hostile
controls, built the pinned Lean 4.34.0 project, audited all 53 selected
headline theorem axiom sets, executed the real transaction fixture, and
rejected each negative control. Its log ends in full `RESULT: PASS`.
The Linux runtime corpus used Rust/Cargo 1.97.1, Clang 14.0.6 and Node 22.23.3;
the macOS Lean fixture used host Rust/Cargo 1.98.0. The receipt inventory
identifies contemporaneous versus retrospective version observations.

All evidence remains bound to the stated execution revision. The commit that
stores this review does not gain a fresh runtime result. Hashes identify the
retained bytes and do not authenticate their execution. No whole-compiler
verification, general backend-equivalence theorem, new ownership theorem,
owned Kernel-0 renderer, formatter-authority transfer, full-profile pass,
hosted result or support promotion follows from this acceptance.

The full profile is excluded from local #328 acceptance by the explicit user
instruction above and delegated to hosted CI. The existing full quality gate
remains intact. The six focused receipts and reviewed non-promotion outcome
complete this issue's bounded validation and decision requirements.
