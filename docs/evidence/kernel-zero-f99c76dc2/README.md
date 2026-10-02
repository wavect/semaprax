# Kernel-0 accepted-profile receipts, 2 October 2026

All execution receipts below concern exact source revision
`f99c76dc2d26dd57c81f4fdd5f26fe91d50118e4`. They support the
[canonical acceptance record](../kernel-zero-accepted-revision-f99c76dc2.json)
and its **rung-1-retained** decision. They do not describe a new execution at
the commit that stores this evidence, a full-profile pass, or rung-2 promotion.

## Receipt coverage

| Acceptance row | Execution and inspected coverage |
|---|---|
| `lean-proof` | [Lean log](lean-gate.log): exact source/signature/semantic pins, hostile gate controls, `lake build`, 53-theorem gate-owned axiom audit, real transaction fixture, recursive-call, normalization-fuel, structural-decrease and named-scope negative controls; final full `PASS`, not `PASS-PARTIAL`. |
| `renderer-authority` | [124-case log](kernel-focused.log): all five renderer exact-source cache checks; independent byte oracles, scalar/UTF-8/escape boundaries and real formatter shadow cases; all-five production counter traversal; authority fallback, candidate match, panic/re-entry, nesting, bounded-output and package-report cases. |
| `bootstrap-artifact` | Same 124-case log: six `rung_two_bootstrap::tests` cases cover two exact derivations, retained native/Wasm payloads, intentional authentic-v1 refusal, term structure, digest/schema/length/trailing hostility, reordered/duplicate entries and replaced compiler outputs. |
| `scalar-targets-recovery` | Same log: `all_five_retained_targets_recover_to_rust_and_reenter_after_corruption`, malformed-output and mismatch refusals, plus pointer-exact authoritative-borrow recovery. Required target environment prevents absent Clang/Node from becoming passing skips. The exact test executes native O0/O2 and Core-Wasm for each retained scalar lane. |
| `differential-corpus` | Same log: reference/compiler corpus agreement, native O0/O2 and Core-Wasm reference agreement, plus rung-one classifier interpreter/target cases. This is finite-corpus evidence, not a general equivalence theorem. |
| `owned-handoff` | [10-case log](owned-handoff.log): four retained-call cases cover actual last-owner release, pre-staging bounds, exhaustion, owner/receipt substitution, retained-alias refusal, panic and two-MiB-stack cleanup; three Kernel binding cases cover exact source/graph/target authentication and zero-owner refusal; two authority cases cover fallback/re-entry; one physical target case asserts all 13 wrapper rows at native O0/O2 and Core-Wasm with allocation/handle/settlement probes. |
| `baseline-preservation` | Explicit local waiver under the 2 October user instruction. No full-profile receipt or passing result is asserted. Hosted CI handles that profile separately. |

`kernel-focused.log` reports 124 passed, 0 failed, 0 ignored. The separate
`owned-handoff.log` reports 10 passed, 0 failed, 0 ignored. Six selected tests
occur in both runs; the totals must not be described as 134 distinct tests.
The separate handoff run closes the four retained-call tests omitted by the
`kernel_zero` filter.

## Provenance and tool versions

The retained [focused runner](run-focused.sh) and
[handoff runner](run-owned-handoff.sh) both check exact HEAD and a clean
checkout before invoking Cargo in the same network-disabled Linux container
image `semaprax-issue327-quality:curl-v2`, with required target flags,
Rust 1.97.1, one Cargo build job, two test threads and the same private target.
[Tool versions](tool-versions.log) were captured during the handoff invocation:
Rust/Cargo 1.97.1, Debian Clang 14.0.6 and Node 22.23.3. The earlier focused
run uses that retained image/toolchain setup; its log does not independently
print the version lines. The image tag is an execution-context label, not an
immutable image attestation.

The Lean gate ran locally on macOS against the same clean exact revision;
[invocation context](lean-invocation.txt) was confirmed by the coordinator
that ran it. [Lean/host tool metadata](lean-tool-versions.txt) was captured
read-only afterward from the same installed tools: pinned Lean 4.34.0,
Lake 5.0.0, Python 3.14.2 and host Rust/Cargo 1.98.0. This distinguishes its
host transaction-fixture compilation from the Linux runtime corpus.

[SHA-256 inventory](sha256.json) records the exact retained log, runner and
metadata bytes. It detects later changes to these copies; it is not a
signature, runner attestation or independent authentication of execution.
These are reviewed local receipts, never hosted CI evidence.
