# Assurance Policy v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, CI pipeline authors, and compiler
contributors integrating [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)
(#183), [Bounded SMT Discharge v1](SMT-DISCHARGE-V1.md) (#184),
[Bounded Model Checking v1](BOUNDED-MODEL-CHECKING-V1.md) (#185), and
[SMT Proof Certificate v1](SMT-PROOF-CERTIFICATE-V1.md) (#186) into a runtime
library, CLI, or CI surface (issue #187).

Assurance Policy v1 (`semaprax.assurance-policy.v1`) is a deterministic,
read-only evaluator over one already-generated Assurance Manifest v1
envelope. It answers exactly one question per obligation: does its current
`classification` satisfy one of four named policy profiles? It reports a
fixed, documented remediation suggestion when the answer is no, and a fixed
retention verdict for any `runtime_guarded` method record. It is proof data,
not permission, matching AGENTS.md's "evidence capsules carry no authority"
and "a settlement or concurrency model is proof data, not permission to
perform a physical finalizer, spawn runtime work, or publish an artifact."
This module never re-derives obligations, never touches source or a target
artifact, and never removes anything itself.

The implementing library module is
[`src/assurance_policy.rs`](../src/assurance_policy.rs). It is exposed
through:

- the `semaprax` library crate directly (`semaprax::assurance_policy`) --
  this is the "runtime" surface issue #187 asks for: any Rust caller,
  including candidate review or a CI script written against the crate, can
  call `evaluate`/`evaluate_delta` without a subprocess;
- two CLI subcommands, `assurance-policy` and `assurance-diff` (see "CLI");
- **not** LSP. See "Known limitations" below for why.

## Why this is a separate module from Assurance Manifest v1

`assurance_manifest::generate` binds one envelope to one real source file
that must compile with `verify::verify` and, per `SPX-G172`, must contain a
`main` (see #230, "Known limitations" in
[Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)). That file-and-`main`
binding is a fact about how obligations are *derived*, not about how a
policy is *applied* to obligations once derived. Folding policy evaluation
into `assurance_manifest` would place a second, unrelated concern (profile
comparison, remediation labeling, CI gating) inside a module this issue's
file lease explicitly excludes from edits, and would not let a caller
evaluate a policy against an envelope obtained any other way (for example, a
future `ExternalRecords` producer, or an envelope replayed from storage).
Keeping the profile vocabulary in its own module also means adding a fifth
profile later never requires touching `assurance_manifest`'s frozen schema
or diagnostics.

## Why this is not exposed through Universal Semantic Query v1

[Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md)'s eight
operations are all addressed by a `stable_id` inside one already-open,
already-parsed workspace snapshot revision
(`src/project/semantic_query.rs`; `src/project/next_construct_query.rs` is
the worked example for adding a ninth). Assurance Manifest v1 is
structurally incompatible with that model: `generate` requires its own
independent parse-and-verify pass over one exact file that must contain
`main`, not an arbitrary retained declaration inside a workspace revision.
The obligation-derivation logic itself
(`assurance_manifest::derive::derive_obligations`) is `pub(super)`, reachable
only from inside `assurance_manifest`, specifically so no second call site
can re-derive obligations by a different path and drift from it -- exactly
the "do not create a parallel source of truth" instruction issue #187 states
directly. Re-implementing obligation derivation against workspace-snapshot
HIR facts here (mirroring `next_construct_query`'s technique of walking
`programs(revision)` directly) would be precisely that parallel source of
truth. Extending the query kernel to accept a raw file-with-`main` target,
instead of a `stable_id`, is a larger structural change than this policy
tranche and is left as a documented gap (see "What remains").

## The four profiles

| Profile token | Accepts (in addition to every stricter profile's set) |
| --- | --- |
| `require-static` | `compiler_proved`, `smt_proved`, `model_checked`, `theorem_proved` |
| `allow-runtime-guard` | ...plus `runtime_guarded` |
| `allow-test-evidence` | ...plus `test_evidenced` |
| `report-only` | every classification; never fails a CI gate |

No profile ever treats `open`, `assumed`, or `attempt_inconclusive` as
satisfying an obligation except `report-only`, and `report-only` never
authorizes anything -- it only reports. This mirrors the issue's explicit
"Explicitly out of scope: converting an open obligation to assumed without
an explicit assumption record": no profile in this module ever performs that
conversion; `PolicyProfile::accepts` only reads an already-recorded
classification.

`PolicyProfile::resolve_precedence` is a pure combinator over up to four
already-resolved profile values in the issue's own precedence order
(source/module, Project, deployment/target, invocation/CI): it returns the
strictest of the sources actually supplied (lowest `precedence_rank`), or
`None` if none were supplied. It does not look up any of the four sources
itself -- there is no per-module or per-Project profile configuration format
in this repository yet, and inventing one was out of scope for this
tranche; see "What remains."

## Per-obligation verdict (`evaluate`)

`evaluate(envelope_json, profile)` independently replays the envelope with
`assurance_manifest::verify_envelope` first (fail closed on a malformed or
forged envelope, `SPX-Z103`/`SPX-Z104` from that call), then reports:

```text
schema, profile, source, counts, obligations, ci_status, nonclaims
```

Each `obligations[]` entry carries `id`, `declaration_id`, `kind`,
`classification`, `satisfied`, `suggested_next_action` (`null` when
`satisfied`), and `runtime_guards` (one entry per `runtime_guarded` method
record on that obligation; empty when none).

`ci_status` is `"fail"` only when `profile` is not `report-only` and at
least one obligation is unsatisfied; `report-only` is always `"pass"`.

### Suggested next action

A fixed, deterministic label, chosen only from the closed vocabulary the
issue names in its implementation sequence:

```text
strengthen_precondition, weaken_postcondition, add_invariant,
add_runtime_guard, split_function, mark_explicit_assumption
```

This is a suggestion until applied through a typed change (rename, replace,
add-contract, add-declaration); it is never itself an edit, and it never
proves the suggested change would verify. The mapping from
`(kind, classification)` to a label is total and documented in
`suggested_next_action` in `src/assurance_policy.rs`; it is a display label,
not a new proof.

### Runtime guard retention

For each `runtime_guarded` method record on an obligation, `evaluate` reports
whether it may be removed, per the issue's own item 4: "a guard can be
removed only when the exact target/artifact obligation is statically
discharged under an accepted policy; otherwise retain it." Concretely, a
guard's `target`/`artifact_digest` must both be present and byte-equal to a
sibling method record's `target`/`artifact_digest` on the *same* obligation,
where that sibling's class is one of `compiler_proved`/`smt_proved`/
`model_checked`/`theorem_proved` and `profile` accepts it. Two absent
(`None`) target/artifact pairs are never treated as an equal, unspecified
binding -- an unbound guard is always retained (`removable: false`,
`reason: "guard_target_or_artifact_not_bound"`), matching the issue's named
failure case "removing runtime guards based on a proof for a different
target or artifact." Under `report-only`, every guard verdict is
`removable: false` with `reason: "report_only_profile_grants_no_removal_authority"`,
regardless of any static evidence present, because `report-only` grants no
authority by name. `evaluate` never removes a method record from the
envelope; it only reports the verdict.

## Delta policy (`evaluate_delta`)

`evaluate_delta(base_envelope, candidate_envelope, as_of, profile)` calls
`assurance_manifest::delta` (unchanged, #183) and applies a CI fail policy
over its buckets:

```text
schema, profile, delta, new_open_obligations, ci_status, nonclaims
```

`ci_status` is `"fail"` when `profile` is not `report-only` and any of the
following holds: the delta's `weakened` bucket is non-empty; its `stale`
bucket is non-empty (an expired assumption, only computed when the caller
passes `as_of`); or any obligation in its `added` bucket has a candidate
classification `profile` does not accept (a genuinely new obligation that
does not meet policy -- reported separately in `new_open_obligations`, not
folded into `delta.added`). `report-only` never fails.

This directly covers the issue's "Required tests and evidence" item "CI
policy rejects new open obligations or weakened classes when configured,"
scoped to what `assurance_manifest::delta` already classifies; it does not
invent a new delta bucket.

## CLI

Assurance Manifest v1 shipped with no CLI subcommand ("Known limitations" in
[Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md): wiring one was left to
"whichever worker owns that surface next"). This issue wires both the
manifest generation and the policy evaluation together, since a bare
manifest with no profile applied is exactly `--profile report-only`'s
output (every obligation reported, `ci_status` always `pass`) -- there is no
separate "just show me the envelope" command:

```text
semaprax assurance-policy <file> --profile <require-static|allow-runtime-guard|allow-test-evidence|report-only> [--max-bytes N] [--max-obligations N]
semaprax assurance-diff <base-file> <candidate-file> --profile <...> [--as-of YYYY-MM-DD] [--max-bytes N] [--max-obligations N]
```

Both generate the envelope(s) from real source file(s) with
`assurance_manifest::generate` (standalone, `main`-requiring, exactly like
`capability-manifest`/`region-report`), then evaluate the named profile, and
print the resulting JSON to stdout. A process exit code of `1` (via the
CLI's shared `report`/error path) accompanies `ci_status: "fail"` only in
the sense that a downstream CI step can additionally check `ci_status` in
the printed JSON; this CLI does not fail its own exit code on `ci_status:
"fail"` -- an unsatisfied obligation is not a CLI-level error, it is
reported data a caller decides how to act on, matching "assurance operations
remain read-only unless a separate typed source change is explicitly
applied" (issue #187, acceptance criteria).

## MCP

No separate MCP wiring is added by this tranche. `assurance-policy` and
`assurance-diff` are single-file report generators in the same family as
`capability-manifest`/`region-report`, none of which sit on the
`SemanticWorkspaceService` kernel that the CLI's `query`/`service --mcp`
surfaces already share (see
[Semantic Service Surface Consolidation Audit v1](SEMANTIC-SERVICE-SURFACE-CONSOLIDATION-AUDIT-V1.md)).
Exposing them over MCP would require either adding a raw-file operation
outside that kernel's `stable_id` model, or the kernel-model extension
described in "Known limitations" below; this tranche does neither, so it
does not claim MCP exposure for the same honest reason it does not claim
one for LSP.

## Known limitations

- **No LSP surface.** `ls src/ | grep -i lsp` and `rg -il '\blsp\b' src/`
  return no Language Server Protocol implementation anywhere in this
  repository; the string `lsp` appears only in authority-nonclaim prose
  (see [Semantic Service Surface Consolidation Audit
  v1](SEMANTIC-SERVICE-SURFACE-CONSOLIDATION-AUDIT-V1.md), "Fact: there is
  no LSP module in this repository," independently re-confirmed for this
  issue). Writing an LSP server (a `textDocument/*` state machine,
  incremental document sync, editor diagnostics translation) is a separate,
  much larger effort explicitly out of scope for this tranche. Nothing in
  this module is LSP-specific, so a future LSP server can call
  `semaprax::assurance_policy::evaluate`/`evaluate_delta` directly once one
  exists.
- **Coverage is capped at entry-point files (#230).** Because
  `assurance_manifest::generate` requires `main`, a plain library module can
  never receive an assurance envelope, and therefore never receives a
  policy verdict either, in this repository's v1. This is inherited, not
  introduced or fixed, by this tranche.
- **`SPX-Z1xx`'s `no_smt_solver_invoked` nonclaim is unconditional
  (#184).** `assurance_manifest::render.rs`'s fixed `nonclaims` array always
  includes `no_smt_solver_invoked` on every envelope, whether or not an SMT
  method record is actually present. This module reads whatever
  `classification`/`class` values the envelope declares (already replayed
  by `verify_envelope`, which independently confirms each obligation's
  `classification` was correctly re-derived from its own `methods`) and
  never overrides or reinterprets that fixed nonclaim array itself.
- **No per-module or per-Project policy configuration file.**
  `PolicyProfile::resolve_precedence` is a pure combinator; nothing in this
  tranche defines a file format, project-manifest field, or CI-config key
  that would supply its four source values automatically. A caller (CLI
  flag, CI script, or future config reader) must resolve each of the four
  sources itself today.
- **No prove/counterexample CLI operations.** `assure`, `assurance show`,
  `prove`, and `counterexample` as named CLI verbs in issue #187 are not
  added by this tranche beyond `assurance-policy`/`assurance-diff`.
  `assurance_manifest::smt_discharge`/`proof_certificate` (#184, #186)
  already carry proof/counterexample data inside method records this module
  reads (`proof_ref`, `counterexample_ref`); a dedicated `prove`/
  `counterexample` CLI verb that invokes those backends directly, rather
  than reading their already-recorded evidence through an envelope, is
  further CLI surface than this tranche adds.
- **Not schema-equivalent across CLI/MCP/SDK, only runtime+CLI.** Issue
  #187's "All frontends share one underlying schema/service" acceptance
  criterion is met only for the runtime (library) and CLI surfaces this
  tranche actually adds; see "MCP" above for why MCP is not claimed, and
  "No LSP surface" above for LSP.

## Diagnostics

`SPX-Z6xx` (previously unused):

- `SPX-Z601` -- invalid policy input: an unrecognized profile token, or a
  malformed envelope/delta shape this module's own defense-in-depth parsing
  rejects independently of `assurance_manifest::verify_envelope`.
- `SPX-Z602` -- output byte-budget exhaustion; fail closed, never truncated.

`assurance_manifest::verify_envelope`/`delta`'s own diagnostics (`SPX-Z101`
-- `SPX-Z104`) propagate unchanged when either call fails before this
module's own logic runs.

## Exact nonclaims

```text
proof_data_not_authority
evaluated_only_over_the_supplied_envelope_no_live_recheck_of_source_or_target
grants_no_execution_publication_signing_or_repair_authority
suggested_next_action_is_a_deterministic_label_not_a_verified_repair
never_converts_an_open_obligation_to_assumed_on_its_own
profile_precedence_is_caller_supplied_not_looked_up_from_project_or_deployment_configuration
```
