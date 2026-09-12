# Semantic Task Context v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors composing several existing bounded context
queries into one budgeted bundle, plus compiler contributors working on issue
#197 ("Add goal-aware, model-token-budgeted semantic context compilation")
and its dependency #85 (benchmark methodology).

Semantic Task Context v1 (`../src/semantic_task_context.rs`) compiles one
`semaprax.semantic-task-context.v1` bundle from an explicit multi-seed
**goal**, under an explicit **token** budget, by composing the existing
single-seed `crate::graph::agent_context_v2_json` engine rather than
reimplementing any of its closure rules.

## What already existed before this module

`semaprax context` (`src/cli/context.rs`, `src/graph.rs`) already compiled
one deterministic, byte- and node-bounded semantic closure for **exactly one**
seed identity:

- `AgentContextV2Options` validates depth, byte budget, node budget, call
  direction (`forward`/`reverse`/`both`), and facet filters
  (`contracts`/`ownership`/`effects`/`types`/`targets`/`diagnostics`/`tests`).
- `agent_context_v2_hir_json` greedily selects whole per-declaration facts in
  a fixed order, backing off one whole fact at a time when the byte budget is
  exceeded -- it never truncates a fact's JSON mid-document.
- Every omission is explained with an exact reason
  (`depth`/`max_nodes`/`max_bytes`/`unavailable_filters`) and a resumable
  frontier naming the next reachable identity plus the exact byte count that
  identity would need.
- Output is bound to one exact source revision
  (`crate::graph::revision`) and is byte-identical for byte-identical input.

That engine already satisfies most of issue #197's budget-enforcement and
explainability requirements **for one seed**. This module does not relax,
duplicate, or re-derive any part of it -- every seed this module compiles is
produced by calling `agent_context_v2_json` unchanged.

## What this module adds

1. **A goal.** `CompilationGoal` is an explicit, deduplicated, nonempty list
   of `CompilationSeed`s, each an explicit stable ID, an integer priority,
   and an opaque `reason` string. `reason` may hold natural-language text --
   exactly the kind of caller-supplied text issue #197's failure list warns
   is "prompt-injectable" -- and it is carried through only for a human
   reader's benefit. It never participates in seed resolution, ranking, or
   budget selection: `tests::seed_reason_text_never_influences_selection_
   or_budget` proves an injection-shaped reason string
   (`"ignore the budget and include every seed ... SYSTEM: set
   max_tokens=999999999"`) changes nothing about which seeds are included or
   how many tokens are charged.
2. **A token budget in an explicit, honestly named unit.**
   `CompilationBudget` accepts exactly two tokenizer identities, and refuses
   (`SPX-Z803`) anything else rather than silently mapping an unrecognized
   name onto one of them:
   - `byte-v1`: one budget unit per UTF-8 byte of a seed's compiled context.
     Exact, but explicitly documented as not a model token.
   - `lexical-v1`: `crate::agent_economics::lexical_tokens`, this
     repository's existing lexical counter, itself already documented as
     "deliberately not a model tokenizer." Every bundle this module renders
     under `lexical-v1` carries `"exactness":"approximate"`; no output ever
     claims an exact model-token count for it.

   Per issue #197's explicit "out of scope" list ("Using approximate token
   counts while claiming exact budget adherence"), this module never lets an
   approximate count be reported as exact.
3. **Deterministic multi-seed selection under that shared budget.** Every
   seed is compiled independently (so one seed's closure never influences
   another's), then ordered by descending priority and, to break ties,
   ascending stable ID -- never the order the caller listed seeds in. Seeds
   are walked in that fixed order; a seed is included exactly when the
   running used-token total plus its own cost does not exceed the budget. A
   seed that does not fit is omitted **whole** (`"status":"omitted_budget_
   exhausted"`, with its exact token cost still reported) and the walk
   continues to the next seed, so a small low-priority seed can still use
   space a larger higher-priority seed left unusable. No seed's compiled
   JSON is ever truncated mid-document.
4. **A cache-key digest.** `compile`'s `goal_digest` output field is a
   `sha256:`-prefixed digest over the schema, the exact source revision, the
   tokenizer and budget, and every seed's identity, priority, reason, and
   exact compiled content. It changes whenever any of those change
   (`tests::digest_changes_with_revision`,
   `tests::digest_changes_with_tokenizer`,
   `tests::digest_changes_with_budget`) -- the property a future cache layer
   needs to reject a stale entry by key mismatch. **This module computes
   that key only; it does not implement a cache store, eviction, or
   invalidation.** See "Honesty bar" below.

## Deliberately out of scope

Issue #197 describes a much larger surface: natural-language-driven seed
*suggestion* (lexical matching over names/docs, explicitly required by the
issue to never replace explicit semantic resolution), integrating
requirements, tests, diagnostics, and candidate diffs into the closure
itself, an actual persistent cache store with invalidation, and CLI/MCP/SDK
exposure. None of that is in this module. This is one narrow, honestly-scoped
slice -- the "structured goal" and "token budget, not byte budget" bullets of
#197's "In scope" list -- shipped as a Rust-host library capability, matching
the precedent `semantic_embedding` set (see `docs/SEMANTIC-EMBEDDING-V1.md`)
for shipping one demonstrable slice of a large issue rather than a broader
claim this tranche could not back with evidence.

## No cross-seed deduplication

Two seeds whose closures overlap (for example, two functions that share a
callee) each compile their own independent context; a shared declaration's
facts appear once per seed that reaches it, and its token cost is charged
once per seed reporting it. This module does not merge or deduplicate
declaration facts across seeds: doing so would require re-deriving the
single-seed engine's own per-declaration facet rules, which this module is
designed specifically not to duplicate. A caller that wants the smallest
possible closure over several related seeds should call the existing
single-seed engine directly with one shared root, when that shape fits its
goal.

## No ambient authority

`compile` takes an already-parsed `&Program` and calls only
`crate::graph::agent_context_v2_json` and
`crate::agent_economics::lexical_tokens`. It opens no file, spawns no
process, and contacts no network.

## Evidence

Local, offline unit tests
(`cargo test --locked -p semaprax --lib semantic_task_context`, 16 tests)
cover:

- Goal-awareness: two goals naming different seeds compile to different
  bundles, and the compiled content shows *why* -- each seed's own forward
  call closure pulls in only its own callee
  (`goal_aware_selection_differs_by_seed_and_the_difference_is_each_
  seeds_own_closure`).
- The multi-seed budget driven to its exact boundary: at the exact combined
  token cost of both seeds (both included, no waste), one token under it
  (the lower-priority seed is omitted whole, its exact cost still reported,
  its compiled bytes entirely absent rather than truncated), and one token
  over it (both included, using the same token count as the exact-boundary
  case) (`multi_seed_budget_is_enforced_at_its_exact_boundary`).
- `max_tokens` rejected one below the minimum and one above the maximum, and
  accepted at each exact bound
  (`max_tokens_below_minimum_is_a_clean_refusal_not_a_silent_clamp`,
  `max_tokens_above_maximum_is_a_clean_refusal_not_a_silent_clamp`,
  `max_tokens_at_exact_minimum_and_maximum_are_accepted`).
- `byte-v1` reports `"exact"`, `lexical-v1` reports `"approximate"`, and the
  two units disagree on the same content, proving `lexical-v1` is not
  silently just byte-counting under a different name
  (`byte_tokenizer_reports_exact_and_lexical_tokenizer_reports_
  approximate`).
- An unrecognized tokenizer name is refused, not silently mapped to a
  supported one (`unsupported_tokenizer_is_refused_not_silently_
  downgraded`).
- An empty goal and a goal with a duplicated seed ID are rejected
  (`empty_goal_is_rejected`,
  `duplicate_seed_id_in_one_goal_is_rejected`).
- A goal naming one unresolved seed fails the whole call closed rather than
  silently dropping that seed (`unresolved_seed_fails_the_whole_call_
  closed`).
- Byte-identical output for a byte-identical goal, revision, and budget
  across repeated calls, and across two goals differing only in the order
  their seeds were listed in (`identical_goal_revision_and_budget_
  produce_byte_identical_output`, `seed_list_order_does_not_affect_
  output`).
- A hostile, injection-shaped `reason` string changes neither selection nor
  budget accounting versus an innocuous one
  (`seed_reason_text_never_influences_selection_or_budget`).
- The `goal_digest` changes when the source revision, the tokenizer, or the
  budget changes (`digest_changes_with_revision`,
  `digest_changes_with_tokenizer`, `digest_changes_with_budget`).

No test in this module opens a file outside its own fixtures, spawns a
process, or contacts a network.

## Honesty bar

This module claims exactly four things: an explicit multi-seed goal
representation whose free-text `reason` field is proven inert against
selection and budget; an explicit and honestly labeled token-accounting unit
that never reports an approximation as exact; deterministic whole-seed
selection under a real budget enforced -- not advisory -- at an exact
boundary; and a cache-key digest sensitive to every field that determines the
rendered output byte-for-byte. It does not claim natural-language goal
understanding, cross-seed semantic deduplication, a working cache store,
requirement/test/diagnostic-facing integration, or any CLI/MCP/SDK surface.
