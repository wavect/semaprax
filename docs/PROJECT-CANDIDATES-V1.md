# Project Candidates and Semantic Change IR v1

Status: authored, unrun. This is a partial implementation of the
[graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md).

Audience: agent builders, compiler contributors, and reviewers.

## Immutable source-derived candidates

`project::ProjectCandidate` retains an immutable base `Arc<ProjectRevision>`
and a separately admitted candidate revision. `open(base, expected_revision)`
creates the initial candidate. `apply(expected_candidate_digest, change)`
returns a new candidate, leaving the previous candidate, siblings, and source
files unchanged. Dropping a candidate discards the overlay. There are no
filesystem handles, cache locations, source locks, publication methods, or
automatic execution in this API.

Every change names the exact current Project revision. Every application also
requires the exact candidate digest, which binds the complete intention
history and evidence. Two histories that reach the same source revision need
not have the same candidate digest. A stale selector rejects before source
transformation.

## Closed Semantic Change IR

`SemanticChange::new(base_revision, &intent)` builds canonical bytes.
`SemanticChange::from_json(bytes)` accepts only those exact bytes: recursively
lexically sorted object keys, compact JSON, arrays in declared order, and one
terminal LF. Unknown or duplicate members, alternative ordering/escaping,
pretty printing, missing LF, and omitted/weakened requirements reject.
An object constructed in memory is still subject to node, depth and byte limits.
Construction does not grant semantic admission; `apply` validates the operation.

The top-level fields are `schema`, `base_revision`, `intent`, and
`requirements`. Schema is `semaprax.semantic-change.v1`. The mandatory ordered
requirements array is the exported `SEMANTIC_CHANGE_REQUIREMENTS`:

1. `preserve_stable_identity`
2. `preserve_public_exports`
3. `update_all_callers`
4. `no_new_effects`
5. `no_new_capabilities`
6. `preserve_contracts`
7. `revalidate_ownership_and_cleanup`
8. `preserve_project_profile_admission`
9. `preserve_admitted_core_targets`

These have the bounded meanings below. They do not assert external consumer
compatibility, general formal equivalence, runtime behavior, or every platform
target in the mature language contract.

Three operation shapes are currently admitted:

| Kind | Exact additional fields | Behavior |
| --- | --- | --- |
| `rename_declaration` | `target`, `name` | Rename an explicitly identified monomorphic top-level non-main function and its local call spellings. Imports continue to address its unchanged stable ID and retain aliases. |
| `change_function_signature` | `target`, `append_parameters` | Append 1–16 by-value scalar parameters and append the supplied exact scalar literals to every authenticated local/import call. Existing parameter and argument order is unchanged. |
| `replace_function_body` | `target`, `body` | Construct a new expression AST and admit the complete resulting Project through the real verifier. Existing contracts and declared effects remain. |

An appended parameter has exactly `name`, `type`, and `argument`. Its type is
`i64`, `i32`, `u8`, `usize`, or `bool`; its argument has matching `kind` and
`value`. For example, the intent value (shown pretty-printed for reading) is:

```json
{
  "kind": "change_function_signature",
  "target": "calculator.add",
  "append_parameters": [
    {"name": "offset", "type": "i64", "argument": {"kind": "i64", "value": 0}}
  ]
}
```

This is an initial signature-evolution operation, not arbitrary parameter
reordering, renaming, removal, conversion, or return-type migration. It does
not silently guess defaults. Literal append keeps existing effects and owned
argument evaluation in left-to-right order. Authenticated caller migration
visits contracts, ordinary and generic bodies, class bodies, loops, match
guards, and nested expressions; it does not discover external consumers.

Body constructors are closed objects: scalar `kind`/`value` literals;
`place` with `name`; `binary` with `op`, `left`, `right`; `unary` with `op`,
`value`; `if` with `condition`, `then`, `else`; and `call` with stable-ID
`target` and `arguments`. Places select existing function parameters. Calls
select existing local functions or explicit imports and cannot add an import.
Constructors cannot submit source text, HIR, graph fields, or unresolved holes.
Types, effects, contracts, ownership, and cleanup are checked after canonical
source materialization. An invalid constructor never becomes a public candidate.

## Validation and replay

Application parses only the admitted revision's canonical sources and invokes
the closed AST transformation. Module permits, per-function declared effects,
and contract inventories must remain unchanged. The constructors preserve
predicate ASTs except for the declared signature call-site migration; the
inventory comparison alone is not a formal proof of predicate equivalence.

The compiler canonically formats every source, reparses and checks canonical
round-trip equality, then runs the complete Project Phase-A build. That build
relinks entry/test/export closures, validates HIR and ownership/cleanup plans,
and replays the selected manifest profile's admission. A second complete build
from the same rendered source must reproduce the exact source facts, Project
revision, and complete Project graph. Explicit declaration identity facts and
the canonical manifest/export list must match the preceding revision.

Both entry and test closures undergo ordinary native C11 emission and ordinary
Core-Wasm emission plus wasmparser 0.258.0 structural validation. Reports name
the exact role/lane, admission result, diagnostic on rejection, artifact digest,
and byte length. A candidate may not lose a lane admitted by the preceding
candidate. An ordinary lane not admitted by the base is explicitly marked;
that is not a fallback or a claim that a command/package-specific lane failed.
The manifest's profile-specific admission is separately replayed in every case.
No native compiler, Node process, interpreter execution, or target runtime is
invoked. Tests remain `not_run` in the preview.

`ProjectCandidate::replay(base, expected_base, changes, evidence_bytes)` starts
from the base, reapplies the complete ordered history, and exact-compares the
resulting evidence. An attacker recomputing a digest over altered JSON does
not satisfy this replay. Retained APIs describe immutable source revisions;
the CLI authenticates actual held disk inputs before and after the preview.

## Review and comparison

`to_json()` returns `semaprax.project-candidate.v1` with one LF. It includes
base/candidate revisions and graph digests, complete ordered intentions,
operation targets and migration counts, changed-file source digests, exact
replacement source, a human-readable single-hunk unified diff per changed
file, structural before/after impact, target facts, and required execution gates.

The semantic-delta digest binds operation summaries and graph digests, not a
complete behavioral proof. Impact currently uses the existing six cross-file
edge families. Full local call migration is broader than that impact report.
`compare(other)` requires a common base and reports target overlap and source
revision equality. It is descriptive and cannot authorize semantic merge.

All digests use SHA-256 over `domain || u64_le(bytes.len) || bytes`. Domains are
`semaprax.project-candidate.v1\0`, `semaprax.candidate.source-diff.v1\0`,
`semaprax.candidate.semantic-delta.v1\0`, `semaprax.candidate.native-c11.v1\0`,
and `semaprax.candidate.wasm-core.v1\0` respectively. Candidate digest is
returned separately, not embedded in its own payload.

```text
semaprax project-candidate-preview <manifest> <change.json>
```

The CLI reads one explicit bounded regular change file and writes no source or
cache. It buffers stdout until the final held Project-input check succeeds.
Wrong arity exits 2; domain rejection exits 1 without stdout.

## Bounds and diagnostics

Change bytes are at most 1 MiB; intention data at most 8,192 JSON nodes and
depth 64; typed expressions at most 4,096 nodes/depth 64; migration at most
1,048,576 visited nodes/depth 256; history at most 32 changes. Canonical Project
source remains under its existing 16 MiB aggregate bound. Candidate evidence
is at most 64 MiB. Core target projections are bounded to 16 MiB each.
These are work/wire limits, not total resident-heap guarantees.

- `SPX-G222`: candidate grammar or invariant rejection.
- `SPX-G223`: candidate input, source, target, or output capacity exceeded.
- `SPX-G224`: stale candidate/revision or replay mismatch.
- `SPX-G225`: unsupported/invalid typed intention or constructor.
- `SPX-G226`: typed-constructor or migration bound exceeded.

Underlying parser/verifier/profile diagnostics remain intact. Input-file open
uses the image reader's existing `SPX-G219` host rejection boundary.

## Evidence and remaining programme

Tests in [project_candidates_v1.rs](../tests/project_candidates_v1.rs) and the
intent module cover append migration, stable-ID body calls, canonical source
round-trips, branching, sequential changes, stale/tampered replay, real type
rejection, and no incidental writes. They are authored, unrun at the user's
request; no local or hosted passing result is claimed.

Typed holes, generalized signature migration, expression selection, extraction,
new/moved declarations, record/interface/contract operations, affected test
execution, candidate persistence, semantic rebase, and separately authorized
source publication are still required by the full programme. The current
read-only image protocol does not advertise candidate or commit authority.
