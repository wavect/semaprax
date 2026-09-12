# Assurance Manifest v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors, including the
later SMT (#184), model-checking (#185), and proof-kernel (#186) backends and
the sibling consumers of the shared `ASSURANCE-SUMMARY` contract (#129, #202,
#205).

Assurance Manifest v1 (`semaprax.assurance-manifest.v1`) is a deterministic,
read-only, per-obligation record of exactly what was checked about one
verified single-file SEMAPRAX module, how strongly it was checked, under
which explicit assumptions, and what remains open. It replaces
undifferentiated "verified" language with one canonical document spanning
every obligation kind this repository can currently derive or accept evidence
for. It is proof data, not permission: the manifest grants no execution,
publication, signing, or review authority, matching the repository's evidence
capsule invariant.

This tranche introduces the schema, the assurance lattice, the canonical
producer, replay, and delta. It does **not** implement or require an SMT
solver, a model checker, or a theorem-proving kernel. A manifest that has none
of those backends available still renders a complete, valid document: every
obligation this producer cannot currently assure explicitly is absent rather
than silently implied, and any obligation an operator wants to track before a
formal backend exists is expressed with class `open` or `assumed` through the
external-record input, never inferred.

## Command and public API

`semaprax assurance-manifest <file> [--max-bytes N] [--max-obligations N]`
prints the envelope `generate` returns, exactly like the sibling
`capability-manifest`/`region-report` commands. The public library entry
points are:

```rust
pub fn generate(
    source_path: &Path,
    options: &AssuranceManifestOptions,
) -> Result<String, Vec<Diagnostic>>

pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic>

pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Diagnostic>

pub fn public_view(envelope: &str) -> Result<String, Diagnostic>

pub fn delta(
    base_envelope: &str,
    candidate_envelope: &str,
    as_of: Option<&str>,
) -> Result<String, Diagnostic>
```

`generate` reads bounded source bytes, parses, and runs the established
`verify::verify` diagnostic pass (the same bar `capability_manifest` and
`region_report` already use for "one verified single-file SEMAPRAX module").
It derives obligations from what that pass already proved (see "Obligation
derivation" below), optionally merges `options.external_records` (obligations
and assumptions supplied by a caller — a test harness, or a future SMT/model-
checking/proof-kernel producer), renders the canonical envelope, and rechecks
exact source identity, bytes, and revision before returning, exactly like the
other single-file report producers. It never executes a target, discovers or
runs project tests, or writes source.

## Obligation identity

An obligation identity is derived only from the owning declaration's
persistent `stable_id` (the `@id`), a closed obligation `kind` token, and a
locator that is stable under reformatting and under an unrelated rename but
changes when the obligation's own semantics change:

```text
semaprax.obligation.v1:<kind-length>:<kind>:<decl-id-length>:<decl-id>:<locator-length>:<locator>
```

Every variable-length segment is length-prefixed (the same technique
`hir::ids::FunctionInstanceId::derive` uses) so concatenation can never
alias two different identities. `kind` is one of the closed tokens below.
`locator` is a structural position, never a byte offset or a display name:

| Kind | Locator | Stability |
| --- | --- | --- |
| `precondition` | `require:<index>` | Stable across formatting/rename. Changes if a `requires` clause is added, removed, or reordered — a semantic change. |
| `postcondition` | `ensure:<index>` | Same rule, over `ensures`. |
| `ownership_parameter` | `param:<index>` | Stable across formatting/rename. Changes if the parameter list or a parameter's ownership mode changes. |
| `ownership_result` | `result` | Reserved for a later producer; not derived automatically in this tranche (see "Obligation derivation"). |
| `effect` | `effect:<name>` | Stable across formatting/rename. `<name>` is a declared effect token, deduplicated within one function's `uses { ... }` set; changes if the set of distinct declared effect names changes — a semantic change. Derived automatically today; see "Obligation derivation". |
| `exhaustiveness` | `match:<index>` | Stable across formatting/rename. `<index>` is the 0-based position of a variant `match` expression in a pre-order, left-to-right walk of the owning function's `requires` clauses, then its body, then its `ensures` clauses (`derive::walk_expr`); changes if a match is added, removed, or reordered ahead of it — a semantic change, exactly like `require:<index>`. Derived automatically today; see "Obligation derivation". |
| `resource_cleanup` | `cleanup:<path>` | Reserved; not derived automatically in this tranche. |
| `architecture_law` | `law:<name>` | Reserved; not derived automatically in this tranche. |
| `generated_interface` | `interface:<name>` | Stable across formatting/rename. `<name>` is the declaring `interface`'s own name; changes if the interface is renamed — a semantic change, exactly like a `stable_id`-scoped rename elsewhere in this table. Derived automatically today, one obligation per interface that declares at least one import; see "Obligation derivation". |

`kind` is a closed enum in `ObligationKind`; an unrecognized token is a
replay failure (`SPX-Z103`), not a silently-accepted extension. This mirrors
AGENTS.md: expression identities may be revision-scoped, so an index-based
locator that shifts when a sibling clause is added is acceptable; only the
declaration's own `stable_id` must be persistent. Per-obligation `@id`
attributes are an explicit non-goal of this tranche; see "Known limitations."

## Obligation derivation

`generate` derives obligations only from facts the existing, already-run
compiler pass proves for the current module, and only when this tranche can
state precisely which guarantee that fact carries:

- **`precondition` / `postcondition`.** One obligation per `requires` /
  `ensures` clause on an admitted function. `wasm::emit_contract_guard`
  compiles every such clause into a trapping runtime check on every admitted
  backend (native, Wasm, and the tree-walking interpreter all reject a
  violated clause at the call boundary), and RFC 0001 lists "cheap static
  contract discharge" as future work the completion matrix still marks
  Partial. So each clause's one method record has class `runtime_guarded`,
  never `compiler_proved`: the clause is checked on every execution, but a
  violating input is only rejected when it runs, not ruled out beforehand.
- **`ownership_parameter`.** One obligation per parameter on an admitted
  function. AGENTS.md states plainly that "ownership errors are compile-time
  diagnostics, never backend accidents," and `verify::verify` (via
  `source_verify`) is exactly the pass that raises them (for example
  `SPX-O104`). Because `generate` only reaches obligation derivation after
  `verify::verify` returned no error diagnostic for this module, every
  parameter ownership mode it derives already survived that pass; its
  method record has class `compiler_proved`.
- **`exhaustiveness`.** One obligation per variant `match` expression
  reachable from an admitted function's `requires` clauses, body, and
  `ensures` clauses (`derive::exhaustiveness_obligations`, walking child
  expressions with `derive::walk_expr`). `source_verify`'s iterative
  verifier rejects a variant match that does not cover every declared case
  (directly, or via a trailing wildcard) with `SPX-M101` "non-exhaustive
  match; missing case", at the same AST-level pass `ownership_parameter`
  relies on — no resolved-HIR dependency is needed for this one kind,
  unlike `ownership_result` below. Because `generate` only reaches
  obligation derivation after `verify::verify` returned no error
  diagnostic, every variant match it derives an obligation for already
  survived that check; its method record has class `compiler_proved`. A
  record match, a scalar match, or any other expression is not
  exhaustiveness-checked by `SPX-M101` the same way, so none of those
  derive an obligation here — deriving one for every `match` regardless of
  pattern kind would overstate what was actually proved.
- **`effect`.** One obligation per distinct effect name a function declares
  in its `uses { ... }` set (`derive::effect_obligations`; a name repeated
  in one function's own set, legal source, still derives exactly one
  obligation — see the negative-control tests). `source_verify` rejects,
  at the same AST-level pass `ownership_parameter` and `exhaustiveness`
  rely on: a declared effect the enclosing module does not `permit`
  (`SPX-E101`); a reachable call — to a host operation, a command-I/O
  operation, or another local function — that requires an effect this
  function does not declare (`SPX-E102`); and an owned parameter or result
  whose automatic finalization requires an effect this function does not
  declare (`SPX-E103`). Because `generate` only reaches obligation
  derivation after `verify::verify` returned no error diagnostic, every
  declared effect it derives an obligation for already survived all three
  checks; its method record has class `compiler_proved`. A function that
  declares no effects derives nothing here — there is no declared fact to
  attach a record to, exactly like a function with no `requires` clause.
- **`generated_interface`.** One obligation per `interface` declaration that
  has at least one import (`derive::generated_interface_obligation`). An
  `interface` with no imports generates no host binding at all, so nothing
  about a "generated interface" was checked; deriving one anyway would
  overstate what was proved. For an interface that does have an import,
  `source_verify::check_native_rust_imports` rejects, at the same AST-level
  pass the other automatic derivations rely on: an inadmissible parameter
  type crossing the import boundary (`SPX-T268`); an inadmissible
  parameter/result shape, symbol collision, or invalid status domain on the
  Native Rust Interop lane (`SPX-B107`); and, on the component lane, an
  inadmissible resource-consuming shape, an invalid failure domain, or a
  declared effect that is duplicated or falls outside the interface's own
  `permits` set (`SPX-I403`/`SPX-I404`). Because `generate` only reaches
  obligation derivation after `verify::verify` returned no error
  diagnostic, every such interface's imports already survived all of those
  checks; its method record has class `compiler_proved`.

Nothing else is derived automatically in this tranche. `ownership_result`
needs the resolved-HIR `result_ownership` helper (private, and defined over
`ResolvedProgram`, not the `ast::Program` this producer stays at); resource
cleanup order (`cleanup_plan::build`, over `ResolvedProgram`/
`ResolvedFunction`) needs the same resolved-HIR dependency this producer
does not have. `architecture_law` names no existing, single checked fact at
all yet — unlike the other reserved kinds, this repository has no producer
to audit for it. Mapping each of these three to a specific assurance class
needs its own audit (or, for `architecture_law`, its own design) of exactly
what a checker proves before this manifest can state it without overstating
it — the explicit failure case this issue calls out first. `ObligationKind`
already reserves their tokens (closed vocabulary, not an open string) so a
later change can add their derivation without a schema version bump; until
then they simply do not appear unless a caller supplies them through
`options.external_records`, which is also how `open`, `assumed`,
`test_evidenced`, `attempt_inconclusive`, `smt_proved`, `model_checked`, and
`theorem_proved` records reach the manifest today. A simple candidate
assurance summary — this tranche's automatic derivation — never waits on
any of those backends existing.

## The assurance lattice

`AssuranceClass` is a closed set of nine tokens, ordered bottom to top by
how much was actually checked, **not** as one total chain:

```text
open  <  assumed  <  attempt_inconclusive  <  { test_evidenced, runtime_guarded,
compiler_proved, model_checked, smt_proved }  <  theorem_proved
```

`open` means no obligation record exists yet for something this schema can
name. `assumed` means an explicit, owned, rationale-bearing assumption was
recorded instead of evidence (`AssumptionRecord`); it is not a proof, but it
is strictly more accountable than silence. `attempt_inconclusive` means a
solver, model checker, or proof kernel ran and did not reach a verdict
(timeout, resource limit, or explicit "unknown") — real evidence about an
*attempt*, never rounded up to a successful classification. A timed-out or
skipped run can never render as `smt_proved`, `model_checked`, or
`theorem_proved`; the producer and `verify_envelope` both reject that shape
closed.

Above `attempt_inconclusive`, `dominates(a, b)` is a **partial** order,
computed as reachability over this fixed, hand-reviewed edge set (see
`lattice::DIRECT_EDGES` and its exhaustive-pairs test):

```text
theorem_proved  > smt_proved, model_checked, compiler_proved, runtime_guarded, test_evidenced
smt_proved      > model_checked, runtime_guarded, test_evidenced
model_checked   > test_evidenced
compiler_proved > runtime_guarded, test_evidenced
runtime_guarded > test_evidenced
{ every class above } > attempt_inconclusive > assumed > open
```

`compiler_proved` and `smt_proved` (and `model_checked`, and `theorem_proved`
by way of not being reached from `compiler_proved`) are **deliberately
incomparable** in this table, even though intuitively both feel "strong":
they usually check different content (ownership discipline is a closed,
decidable, unconditional property of the type system; an SMT or model-
checking result is bounded by its own encoding, axioms, and search bound), so
asserting a general dominance between them would be exactly the "simplistic
total ordering" this issue calls out as a security-relevant failure case
(ranking a bounded proof above an unbounded property, or the reverse). The
same reasoning keeps `test_evidenced` from ever dominating anything except
`attempt_inconclusive`/`assumed`/`open`: passing tests are evidence about the
cases exercised, not a proof that no other case exists — using
`AssuranceClass::rank_for_display` (a fixed tie-break order, documented as
display-only) to imply otherwise would misstate what the evidence proves.

When one obligation carries several method records (a later solver
alongside today's runtime guard, for instance), `classification_of` reports
exactly one current classification: the unique class among that obligation's
own records that `dominates` every other record's class, if one exists;
otherwise the lowest-ranked-by-`rank_for_display` member of the Pareto
frontier (the incomparable maximal records), so the choice is deterministic
without asserting a dominance the lattice does not support. The full
`methods` array is always retained beside the single `classification`, so a
reader who needs the untelescoped picture never loses it.

## Canonical envelope

The report is exactly one UTF-8 JSON line, no terminal LF, matching
`capability_manifest` and `region_report`:

```text
{"schema":"semaprax.assurance-manifest.v1","digest":"sha256:<64 hex>","bytes":<payload length>,"payload":<payload>}
```

`digest` is `sha256:<64 lowercase hex>` over
`domain || little_endian_u64(byte_length) || exact_payload_bytes`, domain
`semaprax.assurance-manifest.payload.v1\0`, rendered through
`digest_hex::LowerHex`, the same domain-separated framing every sibling
report in this repository uses.

Payload top-level key order:

```text
schema, source, limits, counts, obligations, assumptions, nonclaims
```

```text
source:      path, revision, sha256
limits:      max_bytes, max_obligations
counts:      obligations_total, assumptions_total, by_class (one member per
             AssuranceClass token, always present, zero when empty)
obligations: [{id, declaration_id, kind, classification, methods, assumption_ids}]
methods:     [{class, tool, tool_version, inputs, bounds, assumption_ids,
              proof_ref, counterexample_ref, runtime_fallback, test_refs,
              target, artifact_digest, detail}]
assumptions: [{id, owner, rationale, scope, review_by, dependents}]
```

`source.sha256` is `sha256:<64 hex>` over
`semaprax.assurance-manifest.source.v1\0 || little_endian_u64(len) || source_bytes`,
the same convention `capability_manifest` and `region_report` use for their
own source digest, so a value computed by one producer over identical bytes
is byte-identical across all three. `obligations` and `assumptions` are each
sorted by their own `id` field in ascending byte order — never source
declaration order, which is not itself required to be deterministic across
an unrelated reordering of declarations in the file. `methods` within one
obligation preserve the order `generate` produced them in (automatic records
first, in derivation order, then external records in the order supplied),
which is itself deterministic because `options.external_records` is a `Vec`,
not a set.

Every optional method field (`bounds`, `proof_ref`, `counterexample_ref`,
`target`, `artifact_digest`, `detail`) renders as JSON `null` when absent,
never an omitted key: a fixed key set lets an independent reader index a
method record positionally without a presence check, and keeps
`verify_envelope` able to reject one malformed record instead of silently
treating a missing optional key as "not applicable."

## Determinism

Given byte-identical source and a byte-identical, order-identical
`options.external_records`, `generate` produces byte-identical output. Every
field traces to: the parsed `ast::Program` (itself a deterministic function
of source bytes), the fixed constants in this document (schema string,
digest domains, tool name/`env!("CARGO_PKG_VERSION")`), and the caller-
supplied external records, rendered by explicit hand-written JSON formatting
(no `HashMap`/`HashSet` iteration reaches output un-sorted). No wall-clock
time, process ID, random value, or filesystem-ordering-dependent traversal
ever reaches the payload. `AssumptionRecord.review_by`, when present, is
caller-supplied *data* describing when an assumption should be revisited —
never a value `generate` fills in from the current clock — so it does not
threaten determinism. Comparing a `review_by` date against "now" only
happens in `delta`'s optional `as_of` parameter, which the caller must pass
explicitly; omitting it leaves the `stale` bucket empty rather than making
`delta` itself impure by default.

A change that is purely cosmetic (whitespace, comment text, declaration
order in the source file, note: NOT identifier renames — see "Known
limitations") does not change any obligation `id` and does not change the
manifest bytes beyond `source.revision`/`source.sha256`, which always change
with the source regardless. A change to a `requires`/`ensures` clause's
condition, a parameter's ownership mode, or the set of admitted functions
changes exactly the `id`s of the obligations it touches, following
`graph::revision`'s own existing source-hash semantics for what counts as a
formatting-only change versus a semantic one.

## Drift and fail-closed replay

`generate` binds `source.sha256`/`source.revision` to the exact source bytes
it read via the shared `patch::canonical_source_path` /
`patch::read_source_snapshot` / `patch::validate_source_unchanged` sequence
every other single-file report producer in this repository uses, so a
concurrent edit between the read and the final check fails the whole call
closed with the standard patch source-changed diagnostic, before any bytes
are returned.

`verify_envelope` independently replays one envelope with no filesystem
access: exact envelope shape and key order, the payload digest recomputed
over the exact payload bytes, every `id` present exactly once (`SPX-Z103` on
a duplicate), `obligations`/`assumptions` in strict ascending `id` order
(`SPX-Z103` on any other order — canonical bytes are never repaired,
matching the cleanup-plan-ordering invariant applied here to report
ordering), every `kind`/`class` inside its closed vocabulary (`SPX-Z103` on
a forged or unknown token), every `assumption_ids` reference resolving to a
present `assumptions[].id` (`SPX-Z103` on a dangling reference), and
`counts` exactly re-derived from the listed obligations (`SPX-Z103` on a
mismatch). `verify_envelope_against_source` additionally rebinds the current
bytes of `source_path` to `source.sha256`/`source.revision`, failing closed
with `SPX-Z104` on any drift — wrong source, a source edited after the
manifest was generated, or a manifest carried over from a different file.

## Delta

`delta(base_envelope, candidate_envelope, as_of)` independently verifies
both envelopes, then classifies every `id` that appears in either payload's
`obligations` into exactly one bucket:

- `added` — present only in the candidate.
- `removed` — present only in the base.
- `strengthened` — present in both; the candidate's `classification`
  `dominates` the base's, strictly.
- `weakened` — present in both; the base's `classification` `dominates` the
  candidate's, strictly.
- `reclassified` — present in both; the classifications differ but neither
  `dominates` the other (an honest bucket instead of forcing an
  incomparable change into `strengthened` or `weakened`, which would
  overstate what changed).
- `assumption_changed` — present in both with the same `classification`,
  but a different `assumption_ids` set (an assumption was added, removed, or
  swapped without the headline class moving).
- `stale` — present in both; only populated when `as_of` is `Some`, and only
  for an obligation depending (via `assumption_ids`) on an assumption whose
  `review_by` is present and orders on-or-before `as_of` in ASCII byte order
  (both are ISO-8601 `YYYY-MM-DD`, so byte order is date order).

An `id` unchanged in every one of those respects is not reported at all —
the delta is a diff, not a re-statement of the whole manifest.

## Redacted public view

`public_view(envelope)` independently verifies the envelope, then re-renders
it with every `method.detail`, `method.inputs`, `method.proof_ref`,
`method.counterexample_ref`, and `assumption.rationale` field replaced by
JSON `null`, while keeping `schema`, `source.revision`
(**not** `source.path` or `source.sha256`, which can name a local filesystem
layout), every `id`, `kind`, `classification`, `bounds`, `runtime_fallback`,
`target`, `owner`, `scope`, `review_by`, `dependents`, and the full `counts`
section. It exists so a redacted classification summary can be shared
without also publishing free-text tool detail or a local absolute path; it
is still schema-`semaprax.assurance-manifest.v1` data, not a different
schema, and it is still proof data with no authority.

`public_view` renders through generic JSON object serialization rather than
the hand-written canonical formatter `generate` uses, so its key order is
alphabetical (Rust's `serde_json::Value` backs JSON objects with a
`BTreeMap`), not the "Canonical envelope" section's declared order. This is
still fully deterministic — identical input always serializes to identical
alphabetically-ordered bytes — but a consumer of `public_view` output must
index by key, never by position, and must not assume its bytes are
comparable to the primary envelope's bytes. It carries no outer
`digest`/`bytes` wrapper of its own for the same reason: those fields
describe the exact byte layout `generate` produces, which `public_view`
does not reproduce.

## Limits

| Limit | Value |
| --- | ---: |
| Source bytes | 16 MiB (shared `patch` source bound) |
| Payload bytes | 16 MiB |
| Obligations | 65,536 |
| Assumptions | 4,096 |
| `assumption_ids` per obligation | 64 |
| `dependents` per assumption | 4,096 |
| `test_refs` / `inputs` per method | 256 |

Exceeding `max_obligations` or the payload byte budget fails the whole call
closed with `SPX-Z102`; the producer never truncates a manifest.

## Diagnostics

This tranche uses the previously unused `SPX-Z1xx` family:

- `SPX-Z101` — invalid `AssuranceManifestOptions` (out-of-bounds `max_bytes`
  or `max_obligations`, or a malformed external record: unknown `kind`,
  empty `id`, or an `assumption_ids` reference with no matching
  `AssumptionRecord` in the same call).
- `SPX-Z102` — obligation-count or output byte-budget exhaustion; fail
  closed, never truncated.
- `SPX-Z103` — envelope or payload structural/replay consistency failure:
  malformed JSON, wrong schema or key order, a duplicate or out-of-order
  `id`, a class or kind outside the closed vocabulary, a dangling
  `assumption_ids` reference, an `attempt_inconclusive`/`assumed`/`open`
  record misrendered as a stronger class, or a `counts` section that does
  not match the listed obligations.
- `SPX-Z104` — source or artifact binding drift: the current bytes of
  `source_path` disagree with the manifest's bound `source.sha256`/
  `source.revision`.

## Exact nonclaims

The payload carries this ordered array verbatim:

```text
no_smt_solver_invoked
no_model_checker_invoked
no_proof_kernel_invoked
no_project_test_discovery_or_execution
no_target_execution
no_native_or_wasm_runtime_execution
not_human_approval_or_policy
not_signature_or_publication_authority
not_safe_compatible_or_target_conformant
no_repository_or_multi_file_analysis
no_ownership_result_resource_cleanup_or_architecture_law_derivation_yet
read_only_no_source_changes
```

This is not an SMT, model-checking, or proof-kernel result — those backends
are later, independent issues (#184, #185, #186) this manifest is designed
to accept evidence from without ever requiring them to exist. It is not test
execution, target execution, human approval, a signature, or publication
authority; it grants none of those and none of the ambient filesystem,
process, network, or signing authority AGENTS.md prohibits by default. It
does not yet derive `ownership_result`, `resource_cleanup`, or
`architecture_law` obligations automatically (see "Obligation derivation");
their tokens exist in the closed vocabulary so a later change can add that
derivation without a schema version bump, and a caller can already supply
such a record today through `options.external_records`.

## Known limitations

- **Single file, not managed workspace or `ProgramRoot`.** This producer
  takes one source path, exactly like `capability_manifest` and
  `region_report`. A managed-workspace or multi-target binding (source +
  `ProgramRoot` + target + compiled artifact, all in one top-level identity)
  is future work; the per-method `target`/`artifact_digest` fields exist
  today so that work can populate them without a schema break.
- **Entry-point-scoped by design: a library/provider module cannot receive
  its own envelope (#230).** `generate` parses and verifies exactly the file
  it is given through the single-file `verify::verify` pass, the same pass
  `capability_manifest`/`region_report` reuse rather than duplicate. That
  pass does two things no library module satisfies: it rejects any file with
  `module_uses` outright (`SPX-G172`, "source module imports require
  Workspace Semantic Graph resolution" — a single file is never resolved
  against other modules), and it requires the file to declare
  `fn main() -> i64` (`SPX-T105` otherwise). A real library/provider module
  is, definitionally, a file other modules `use` and that itself typically
  `use`s others — exactly the shape this pass refuses. Inside a project, the
  inverse rule holds: only the entry module (and a listed `tests` module) may
  declare `main`; every other listed source is a provider module and
  `SPX-G172` rejects `main` there too
  (`src/workspace_graph.rs`'s `"workspace scalar provider module may not
  declare main"`). No source edit satisfies both rules on the same file, so
  no library module can ever be pointed at directly.
  This is intended, not an oversight: `generate` derives obligations through
  one independent, self-contained parse-and-verify pass specifically so no
  second call site can re-derive them a different way and drift (see
  "Why this is a separate module from Assurance Manifest v1" in
  [Assurance Policy v1](ASSURANCE-POLICY-V1.md)). Accepting an arbitrary
  library module would mean resolving its cross-module imports first, i.e.
  routing `generate` through the Workspace Semantic Graph instead of a
  standalone parse — a materially larger, project-shaped tool this tranche
  does not build. Coverage of a library module's obligations today comes
  from the *entry* module's own envelope covering call sites that reach it,
  plus `ProjectCandidate::candidate_assurance_summary` (see
  [Project Candidate Assurance Acceptance v1](PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md))
  reporting every source it was not pointed at explicitly in
  `sources_not_observed`, rather than silently treating it as covered.
- **No per-obligation `@id` attribute.** Obligation identity is derived,
  not authored; nothing in the language grows a new attribute in this
  tranche. If a future RFC adds an explicit obligation-level `@id`, it can
  become an additional identity input without breaking existing derived
  IDs, because today's IDs never depend on one.
- **Automatic derivation is deliberately narrow.** See "Obligation
  derivation": only `precondition`, `postcondition`, `ownership_parameter`,
  `exhaustiveness`, `effect`, and `generated_interface` are derived from
  source today. `ownership_result`, `resource_cleanup`, and
  `architecture_law` remain undone: `ownership_result` and
  `resource_cleanup` each need a resolved-HIR dependency (`result_ownership`,
  `cleanup_plan::build`) this producer does not otherwise have, staying at
  `ast::Program`; `architecture_law` has no existing single checked fact in
  this repository yet to audit at all. This is a deliberate, incremental
  boundary, not an oversight.
