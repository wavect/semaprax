# Public Generic Candidate Reconciliation — v080

Status: bounded reconciliation record for issue #337; **not a release
candidate acceptance packet**.

Audience: maintainers and reviewers selecting the next integrated
public-generic candidate.

This record selects one exact revision as the subject for the remaining
acceptance work. It does not report a gate as run, retain a newly-built
artifact, or make a compatibility, security, support, publication, signing, or
promotion decision.

## Selected subject

| Field | Value |
| --- | --- |
| Candidate revision | `c378cb1a303d97ac2cc8bec6c3688212d978e454` |
| Named branch at selection | `wavect/v080` |
| Commit | `Merge pull request #314 from wavect/wavect/v070` |
| Selection check | An isolated managed worktree was created directly at this revision and was clean before this record was written. |
| Candidate meaning | The sole revision against which the remaining local and hosted acceptance evidence must bind. |

The revision is a selected *acceptance subject*, not a frozen release
candidate. The release-candidate evidence record still requires its formal
freeze protocol, including a fresh fast-forward and clean-worktree check when
the final packet is prepared. See [Public Generic Release-Candidate Evidence
v1](PUBLIC-GENERIC-RELEASE-CANDIDATE-EVIDENCE-V1.md#3-what-164-still-requires-explicitly-open).

## Exact profile and boundaries

The selected profile is the existing `semaprax.public-generic-ownership.v1`
milestone: PG-1 through PG-8 are evidence prerequisites and PG-9 is a
maintainer decision. Its standing PG-9 outcome remains `unsupported`,
`unpublished`; this record neither revisits nor broadens it. The milestone
charter owns the exact gate meanings and nonclaims in [Public Generic Ownership
Milestone v1](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md#prerequisite-gates-and-decision-gate).

The admission and compatibility boundary remains the versioned type grammar,
candidate surface comparison, descriptor, logical carrier, settlement plan,
and generated-consumer specifications already catalogued by the release
evidence record. A comparison result has
`semantic_version_decision: not_inferred`, `support: not_assessed`, and
`publication: not_assessed`; it cannot decide release support. [Public Generic
Compatibility v1](PUBLIC-GENERIC-COMPATIBILITY-V1.md) owns that rule.

The candidate retains the current fixture and scope limits. In particular, it
does not establish a general public-generic source admission, a compiled public
generic provider ABI, a distributable package, or a cross-platform support
claim. The complete limits and standing decision are owned by the milestone,
not by this reconciliation record.

## Inventory reconciliation

| Inventory class | Candidate-specific status | Evidence handling |
| --- | --- | --- |
| Versioned contracts and profile definitions | Source present at `c378cb1a` | Listed by the existing release-evidence contract inventory; re-read at final review. |
| Acceptance toolchains | Required: Rust/Cargo, C11/C++ compiler, Node/TypeScript/Wasm; Linux sanitizer tooling for its scoped evidence | Availability and versions have not been measured for this candidate. |
| Descriptor, carrier, native/Wasm provider, component, and generated-consumer artifacts | No `c378cb1a` build outputs, hashes, sizes, or retained packages were produced | Build, hash, and retain a bounded immutable inventory during the candidate gate run. |
| R08 settlement-v2 inventory | Retained subject is `888ac18e416303c3a78544194cf3937d9ff4c5ff`, with its own 210-entry inventory | It is historical scoped evidence only. The subject is an ancestor of `c378cb1a`, but the intervening range changes the CI workflow, settlement-matrix engine/evidence code, and component-runtime lock/toolchain inputs; it cannot be relabelled as exact-candidate evidence. |

This classification distinguishes an inventory of expected artifact classes
from retained distributable or replayable assets. No asset in this table is a
release artifact.

## Remaining acceptance and decision packet

All rows below are open for `c378cb1a`; no command in this table was run while
preparing this record.

| Required evidence | Exact owner | Candidate status |
| --- | --- | --- |
| Local integrated milestone selectors, generated callers, descriptor/carrier replay, and `public_generic_abi` library coverage | `public-generic-ownership-milestone` job definition in [CI required checks v1](CI-REQUIRED-CHECKS-V1.md) | Not run at this SHA. |
| Linux sanitizer, settlement replay, mutation, thread-admission, TypeScript settlement, and compiled-reference-Wasm evidence | The Linux-only steps in `.github/workflows/ci.yml` | Not run at this SHA. |
| Immutable artifact/corpus digest inventory | Section E of [Public Generic Release-Candidate Evidence v1](PUBLIC-GENERIC-RELEASE-CANDIDATE-EVIDENCE-V1.md#3-what-164-still-requires-explicitly-open) | Not produced at this SHA. |
| Compatibility/delta review and security/trust-boundary review | Sections F and G of the release-evidence record | Not performed at this SHA. |
| Cross-platform exact-SHA hosted reconciliation | PG-8’s Linux, macOS, and Windows job | No exact-SHA hosted run is recorded here. It remains evidence for a release-support decision, but is not required solely to close issue #337. |
| Final support/release decision | Responsible maintainer under PG-9 and the release-candidate decision packet | No new decision. Existing standing outcome remains `unsupported`, `unpublished`. |

A reviewer may accept this record only as a clean candidate-selection and
evidence-boundary increment. For issue #337 closure, the reviewer must
reconcile the local acceptance results and record the bounded
release/support outcome for `c378cb1a`. Missing exact-SHA hosted evidence by
itself does not keep the issue open. A proposed supported, published, or
release-ready outcome still requires the distinct PG-8 evidence and the
responsible explicit decision.

## Nonclaims

No Cargo, compiler, sanitizer, generated-consumer, Node, or test command was
run to create this record. No hosted query, artifact build, artifact retention,
signature, publication, deployment, or issue-state change occurred. Historical
local or hosted evidence keeps its original revision and host binding.
