# Kernel-0 Accepted-Revision Validation v1

Audience: the reviewer resolving issue #328.

Status: a closed acceptance-record format and pending gate inventory. It records
no accepted revision, completed gate, reviewer decision, hosted result, or rung
promotion.

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

## Required local receipt inventory

The accepted-profile review must contain one passing receipt or an exact-subject
reconciliation for every row. The named owner remains the source of truth for
the focused selector and hostile cases; this table intentionally does not
duplicate `quality.sh`'s full-route command sequence.

| Receipt | Owner and required subject | Acceptance state |
|---|---|---|
| Lean proof | [Kernel-0 proof mechanization](KERNEL-PROOF-MECHANIZATION-V1.md) and the `kernel0-lean-proof-gate`, including source pins, hostile controls, build, and axiom audit when the pinned toolchain is available | Pending |
| Renderer and formatter authority | [Formatter Authority v1](KERNEL-ZERO-RUNG-TWO-AUTHORITY-V1.md), the five embedded renderer sources, `src/format/kernel_zero_tokens.rs`, and their broad byte-oracle/shadow and production traversal cases | Pending |
| Bootstrap artifact | [Bootstrap v2](KERNEL-ZERO-RUNG-TWO-BOOTSTRAP-V2.md), including exact regeneration, v1 refusal, decoding, and hostile wire cases | Pending |
| Scalar targets and recovery | [Target and Recovery Evidence v1](KERNEL-ZERO-RUNG-TWO-TARGET-RECOVERY-V1.md), including C11 `-O0`/`-O2`, Node/Core-Wasm, candidate corruption, Rust-byte recovery, and re-entry | Pending |
| Differential corpus | `kernel_zero::differential`, including reference/interpreter agreement and native C11 `-O0`/`-O2` plus Core-Wasm agreement with required target tools | Pending |
| Owned handoff | [Owned Handoff v1](KERNEL-ZERO-RUNG-TWO-OWNED-HANDOFF-V1.md), including binding authentication, graph replay, zero-owner mutation refusal, settlement, panic recovery, and the 13-row native/Wasm wrapper evidence | Pending |
| Baseline preservation | [Quality gates](QUALITY-GATES.md)'s `full` profile, including its Kernel-0 Lean gate | Pending |

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
| `rung-1-retained` | The required local validation completed, but the rung-2 criterion remains unmet or is intentionally not promoted. Name the narrow technical reason. |
| `rung-2-promoted` | The reviewer accepts that the stated rung-2 criterion is met. This requires a separate explicit decision that the five scalar lanes meet the component boundary; wrapper evidence alone cannot supply it. |
| `validation-incomplete` | At least one row is pending, failed, or lacks an exact-subject reconciliation. This is the current outcome. |

The decision must state whether the remaining reason is a technical boundary,
an intentionally deferred promotion, or a failed/missing local receipt. It must
not describe missing hosting as the reason by itself. The decision also leaves
the finite differential corpus, Lean proof boundary, Rust formatter authority,
and whole-compiler verification claims at the limits stated by their owning
specifications.

## Current record

`candidate_revision`: unselected.

All seven receipt rows are pending. The current outcome is
`validation-incomplete`; rung 1 remains the only accepted rung. This is a
planning and provenance increment for #328, not evidence that any required
gate has run at a later revision.
