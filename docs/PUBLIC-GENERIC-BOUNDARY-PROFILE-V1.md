# Public Generic Boundary Profile v1

Status: frozen specification, gate PG-... admission half of gate #150 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
This document freezes the admission predicate only. It defines no classifier
code, admits no diagnostic into the installed catalogue, and changes no
existing public projection. The classifier that implements this predicate
against real checked HIR is the next tranche (issue #150's implementation
half); it does not exist yet, and until it lands, exercised, and hosted, the
milestone's separation gate continues to prove that public generic ownership
is unsupported and unpublished.

Audience: ABI, package, evidence, and generated-consumer maintainers; the
implementer of issue #150 and every issue downstream of it.

## Why this document exists

Three overlapping issue packs describe the same callable public generic
boundary at different levels of detail: the original design issue (#132), the
granular implementation spine (#149-#166), and the descriptor/carrier pair
(#170-#171). Read together rather than singly, they disagree on how wide the
admitted shape is:

- #132's own "Bounded scope" section proposes "one nonrecursive owned record
  with one `Bytes` leaf and admitted Copy fields" as the experimental
  boundary;
- #150's "Required v1 profile" section requires substantially more: every
  reachable field may itself be "another fully concrete authored record
  admitted recursively by the same rule," and #150's own positive-test list
  requires "nested concrete records" and "maximum admitted record depth" as
  acceptance criteria;
- #170 and #171 gesture at a still wider descriptor/carrier vocabulary —
  "variant," "Option/Result" — that neither the admitted type grammar nor the
  settlement obligations support today.

A coordinator decision resolves the reading order: **#150-#165 is the
granular implementation spine, and the admitted profile is the union of the
requirements every pack actually commits to** — not the union of every shape
any pack merely mentions, and not narrowed to the smallest illustrative
fixture. This document is that one exact resolution, written down once so no
later agent re-litigates it per pull request.

## Profile identity

| Layer | Identifier |
| --- | --- |
| Admission profile | `semaprax.public-generic-boundary-profile.v1` |
| Depends on | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) (`semaprax.public-generic-type-grammar.v1`) |
| Depends on | [Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md) (`semaprax.public-generic-settlement-plan.v1`) |
| Feeds | [Public Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md) |
| Feeds | [Public Generic Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md) |
| Reserved diagnostic range | `SPX-PG6xx` (unallocated; no code below is defined in source yet — see [Reserved refusal vocabulary](#reserved-refusal-vocabulary)) |

Checked against `rg -n "public-generic-boundary-profile" docs src tests`: no
existing schema, profile, or diagnostic namespace collides with this
identifier at the time of freeze (commit `d45db653`, 2026-09-11).

## The scope conflict, resolved

| Question | #132 (minimal slice) | #150 (spine) | Frozen v1 decision |
| --- | --- | --- | --- |
| Record nesting | one nonrecursive record | recursive, bounded nesting ("another fully concrete authored record admitted recursively") | **Recursive, bounded nesting is IN.** #150 is the spine issue whose own acceptance criteria require nested records as a positive case; #132's "nonrecursive record" is its smallest illustrative fixture within that wider profile, not a ceiling on it. Treating #132's example as the whole profile would fail #150's acceptance criteria outright. |
| Owned leaves per instance | "one `Bytes` leaf" | unspecified count, bounded by the grammar's existing 256-leaf limit | **Zero or more `Bytes` leaves, up to the existing 256-leaf bound.** #150 explicitly requires a positive case with "zero-length `Bytes`" and does not cap leaf count at one; #132's phrasing describes its own fixture, not a profile-wide cap. |
| Copy scalar fields | "admitted Copy fields" | "all admitted Copy-scalar type arguments where the current grammar permits them" | **All eight grammar scalars, at any reachable position**, matching #150. |
| Variant / Option / Result | not mentioned | explicit non-goal ("generic variants") | **Excluded from v1** — see [Contested classifications](#contested-classifications). #170/#171 mention these conditionally ("if the grammar supports it safely" / as carrier representation guidance for a value model wider than v1); the frozen type grammar and settlement spec both refuse them today, so admitting them here would silently widen two already-hosted-green specifications. |
| Parameter count | one owned input, unstated result plurality | "exactly one owned aggregate input position and exactly one owned aggregate result... additional parameters are not admitted in v1" | **Exactly one owned input, exactly one owned result**, per #150, stricter than #132's silence. |

No shape admitted here is wider than what #150's own acceptance criteria
already commit the spine to. No shape is narrower than #150 requires merely
because #132's smallest fixture happened to need less. This is the reading
the task's own instruction demands: retain the union of *accepted*
requirements, not the union of every sentence any pack contains, and do not
mark a broader criterion satisfied because the smallest example works.

## Admission predicate

An export is admitted under this profile only if every rule below holds.
Nothing here is inferred from display names, native offsets, or target
layout; every rule is checked against persistent declaration identity and
checked HIR facts, exactly as [Public Generic Type Grammar
v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) already does for the types it admits.

### Export shape

1. The selected endpoint is a **monomorphic checked function**: no unresolved
   function-level type parameter remains. It may be authored directly as
   monomorphic code, or refer to concrete instances originating from generic
   record declarations; the exported symbol itself is never a generic
   template.
2. The endpoint is selected by persistent semantic identity (`@id`), never by
   display name.
3. Every concrete generic instance in its signature was resolved during
   checking. There is no runtime type-argument input.
4. The function is synchronous, deterministic, and effect-free for v1.
5. The function has **exactly one** owned aggregate input parameter and
   **exactly one** owned aggregate result. No additional parameters of any
   mode are admitted in v1.

### Input and result type shape

Both the input parameter's type and the result type must be fully concrete
authored generic record instances, admitted by [Public Generic Type Grammar
v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md)'s `instance` production, subject to:

- every declared type parameter of the root template has exactly one
  explicit concrete argument; arity matches exactly;
- the ordered argument identities are the grammar's own (template digest,
  position, parameter owner/index, argument digest);
- the complete substituted record closure is finite and acyclic;
- every reachable field, after exact owner-and-index substitution, is one of:
  - an admitted Copy scalar (`i64`, `i32`, `u8`, `usize`, `char`, `f32`,
    `f64`, `bool`);
  - direct owned `Bytes`, including zero-length;
  - another fully concrete authored record admitted recursively by this same
    rule, to the bounds in [Bounds](#bounds);
- no reachable field is a borrowed view, `String`, variant, resource,
  function value, closure, interface object, opaque host value, unresolved
  type parameter, or any type outside the grammar's closed vocabulary — see
  [Public Generic Type Grammar v1's rejection table](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md#admitted-vocabulary);
- every owned leaf (every transitive `Bytes` field) matches exactly one entry
  of the checked cleanup inventory and one settlement obligation derived by
  [Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md);
  a disagreement is a refusal, never a repaired order;
- the input instance and the result instance may share one template with
  different concrete arguments, or use two different templates; either way,
  each instance carries its own persistent template and instance identity —
  they are never merged or deduplicated because their rendered terms happen
  to differ only by argument.

### Ownership shape

- The input parameter's ownership mode is `own`: transfer into the provider.
- The result is a newly produced owned value, transferred to the consumer
  only after complete postcondition and non-result cleanup validation — the
  milestone's non-negotiable "result publication follows postconditions and
  non-result cleanup" invariant, applied at this boundary.
- No alias to an owned input leaf may survive the transfer.
- No borrowed field, borrowed parameter, or borrowed result is admitted
  anywhere in the shape.
- No implicit clone or copy of an aggregate is admitted. Copy-scalar fields
  may be copied as values (that is what makes them Copy); `Bytes` and
  recursively owned record leaves remain linear — moved, never duplicated.
- The canonical leaf order is the existing compiler-derived structural
  order — the cleanup inventory's declaration-order leaf tree — never a new
  sort invented by this profile or by any downstream consumer.

## Bounds

Every bound below is either reused from an existing frozen specification (the
smallest sound bound available, per issue #150's instruction to prefer reuse
over a new number), or newly defined and justified against an existing
sibling bound.

| Bound | Value | Basis |
| --- | --- | --- |
| Max canonical term bytes, per instance | 65,536 | reused: [Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) `MAX_TERM_BYTES` |
| Max record nesting depth | 64 | reused: Type Grammar v1 `MAX_RECORD_DEPTH`, which already matches [Concrete Generic Owned-Byte Records v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md)'s internal nested bound |
| Max transitive owned (`Bytes`) leaves, per instance | 256 | reused: Type Grammar v1 `MAX_OWNED_LEAVES` |
| Max visited type nodes, per instance closure | 4,096 | reused: Type Grammar v1 `MAX_VISITED_NODES` |
| Max declared template arity | 16 | reused: Type Grammar v1 `MAX_TEMPLATE_ARITY` |
| Owned aggregate input-parameter count | exactly 1 | new: v1 profile invariant (§ Export shape, rule 5) |
| Owned aggregate result count | exactly 1 | new: v1 profile invariant (§ Export shape, rule 5) |
| Max record declarations in the substituted closure | 4,096 | reused: identical to the visited-node bound, since every declaration node visited while deriving a term is counted there |
| Max fields per single record declaration | 256 | new, chosen for symmetry with the existing 256-count bounds used elsewhere in this corpus (the owned-leaf bound above; the existing 256-function public-export bound in [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md)) |
| Max total fields across the substituted closure | 4,096 | reused: identical to the visited-node bound |
| Max bytes per single owned `Bytes` leaf | 65,536 | reused: [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md)'s existing per-value/cumulative borrowed-input bound |
| Max total owned payload bytes, per carrier (input or result) | 16,777,216 (16 MiB) | reused: [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md)'s existing module-input bound. It is also exactly 256 × 65,536 — the owned-leaf bound times the per-leaf byte bound above — so the three numbers are mutually consistent rather than independently chosen. |
| Max live handles per carrier instance | 257 | new = the 256-leaf bound plus one root aggregate handle; see [Carrier v1](PUBLIC-GENERIC-CARRIER-V1.md) |
| Max canonical descriptor rendering bytes, total | 131,072 | derived: two instance terms (input, result) at 65,536 each, plus fixed-width identity fields; see [Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md) |
| Max independent-replay work per verification | bounded by the same closure bounds above: at most 4,096 node visits and 256 leaf visits per instance, no unbounded recursion | reused |

Where several existing bounds could apply and differ, the smaller one was
chosen; none of the numbers above widen any bound an existing hosted-green
specification already enforces. Exact `limit` and `limit + 1` cases are
required test fixtures for the future classifier (§ [Required test
matrix](#required-test-matrix-for-the-classifier)); this document does not
claim they exist yet, because the classifier does not exist yet.

## IN / DEFERRED / EXCLUDED shape table

This is the frozen answer to "which shapes cross the v1 boundary." It is
authoritative over any looser wording in #132, #170, or #171.

| Shape | Status | Why |
| --- | --- | --- |
| Direct owned `Bytes` leaf, including zero length | **IN v1** | admitted grammar leaf; explicit #150 positive case |
| Copy scalar leaf (`i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, `bool`), at any admitted position | **IN v1** | admitted grammar scalar; explicit #150 positive case ("all admitted Copy-scalar type arguments") |
| Single-level (nonrecursive) record with only `Bytes`/Copy-scalar fields | **IN v1** | the special case of the general rule below with nesting depth 1; #132's minimal fixture |
| Nested finite acyclic authored record (record containing another admitted record, bounded) | **IN v1** | explicit #150 requirement and positive-test case ("nested concrete records," "maximum admitted record depth"); matches the already hosted-green type grammar and the internal nested profile in [Concrete Generic Owned-Byte Records v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md) |
| Same template, different concrete arguments for input vs. result | **IN v1** | explicit #150 positive case |
| Different templates for input vs. result | **IN v1** | explicit #150 positive case |
| Exactly one owned aggregate input parameter, exactly one owned aggregate result | **IN v1** (and the only admitted arity) | explicit #150 export-shape rule |
| Multiple owned aggregate input parameters | **DEFERRED** | #150 phrases this as "not admitted **in v1**" rather than a categorical epic non-goal; a later profile version may widen the parameter count once carrier ordering for a single argument is proven |
| Multiple results | **DEFERRED** | same reasoning as above; #150's own non-goals list scopes this to "this profile," i.e., v1 |
| Owned generic variant (concrete instantiation of an authored `variant` declaration) | **DEFERRED, contested — see below** | the frozen [Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md) explicitly refuses a variant leaf today ("the grammar admits no variant... reaching one is a disagreement"), and the type grammar rejects any authored variant with `unadmitted_nominal_kind`; #149's phrasing ("generic variants ... in the first public profile") reads as v1-scoped rather than a permanent programme-wide non-goal |
| `Option<T>` / `Result<T, E>` (compiler-owned generic nominal) at the public boundary | **EXCLUDED from v1; separate internal profile, not reinterpreted** | the type grammar rejects any compiler-owned nominal (`compiler_owned_nominal`); the existing internal `Option<Bytes>`/`Result<Bytes, Bytes>` admission ([Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md)) is a distinct, non-public profile that [Concrete Generic Owned-Byte Records v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md)'s own nonclaims say is not reused here |
| Borrowed generic aggregate (any `borrow` field, parameter, or result) | **EXCLUDED** | explicit epic-wide non-goal (#149: "borrowed generic aggregate APIs"), and explicit v1 ownership-shape rule |
| Public generic function templates (consumer-side instantiation, runtime type-argument selection) | **EXCLUDED, contested — see below** | requires runtime generic specialization, an explicit epic-wide non-goal (#149), even though #170's own "out of scope" wording says only "in v1" |
| Resources, classes, interfaces, dynamic dispatch | **EXCLUDED** | explicit epic-wide non-goal (#149); no owning specification defines an ownership model for them at any public boundary |
| Callbacks, closures, function values | **EXCLUDED** | explicit non-goal, both epic-wide and profile-local |
| Effects, host calls, async functions | **EXCLUDED from v1** | export-shape rule 4 requires synchronous, deterministic, effect-free; no later profile is named for this yet |
| Runtime generic specialization / runtime type-argument selection | **EXCLUDED** | explicit epic-wide non-goal |
| Recursive or cyclic record graphs | **EXCLUDED** | both the type grammar and the internal nested profile require acyclic, finite closures |
| Owned `String` | **EXCLUDED from v1** | grammar rejects (`owned_string`); reserved for the separate String profiles the grammar document names, not this record-shaped boundary |
| Borrowed `str`, `Slice<u8>`, inline byte arrays, `unit`, function types | **EXCLUDED** | grammar rejects all of these outright |
| A target-width integer distinct from the grammar's own eight scalars | **EXCLUDED (see caveat)** | grammar rejects any integer type outside its closed eight-scalar vocabulary; **caveat:** the grammar's own `usize` is itself one of the eight admitted scalars and is defined as target-neutral, so #150's negative test case naming "a target-width integer" cannot mean grammar-`usize`. The next round's fixture author must identify the genuinely target-width type this case targets (for example a raw host-size type distinct from the checked semantic `usize`) before writing that fixture; this document does not resolve which type that is. |
| Higher-kinded types, constraints, trait bounds, general allocator ABI, concurrency | **EXCLUDED** | explicit epic-wide non-goal, and no owning ownership model exists for any of them |

### Contested classifications

Two rows above are marked contested because the source issues read two ways.
Flagging them here, rather than silently picking one, is the point of an
independent review gate.

**Owned generic variants.** Chosen: DEFERRED (not in v1, but not
permanently foreclosed). Alternative: EXCLUDED outright, on the grounds that
the frozen Settlement Obligations v1 document already states categorically
that a variant leaf "has no unconditional owned-leaf order" and treats
reaching one as a disagreement rather than an unimplemented case — which
reads less like "not yet" and more like "this settlement model cannot
represent it without new obligations for conditional liveness." A reviewer
who prefers the stricter reading should reclassify this row EXCLUDED and
require a new settlement-obligations version, not an extension of v1, before
any variant is admitted.

**Public generic function templates.** Chosen: EXCLUDED, because admitting a
template for consumer-side instantiation requires runtime type-argument
selection, which #149's epic-wide non-goals foreclose categorically
("runtime generic specialization or runtime type argument selection"), not
merely for v1. Alternative: DEFERRED, on the grounds that #170's own
"Explicitly out of scope" list writes "Exporting generic templates for
consumer-side instantiation **in v1**," which is v1-scoped phrasing
identical to the multiple-parameter and multiple-result rows this document
marks DEFERRED. A reviewer who prefers the issue's literal wording over the
epic's categorical non-goal should reclassify this row DEFERRED and record
which future profile would need to relax the epic non-goal first.

Neither contested row changes this profile's v1 admission predicate: both
shapes are refused in v1 either way. The classification only changes what
the next profile version is permitted to widen without also reopening the
epic's non-goals.

## Reserved refusal vocabulary

The future classifier (issue #150's implementation half) must return one of
the closed reasons below, never a partial admission and never prose parsing.
No code in this table is defined in source yet — defining an unimplemented
`SPX-PG6xx` constant in a `.rs` file would register it in the installed
diagnostic catalogue's automatic source scan ([Installed Diagnostics
v1](INSTALLED-DIAGNOSTICS-V1.md)) before any code path could produce it, so
this table exists only here until the classifier lands.

| Reserved code | Reason |
| --- | --- |
| `SPX-PG601` | selected export not found |
| `SPX-PG602` | selected item is a generic function template |
| `SPX-PG603` | wrong parameter count (not exactly one owned input) |
| `SPX-PG604` | wrong ownership mode |
| `SPX-PG605` | unsupported result shape (not exactly one owned result) |
| `SPX-PG606` | unresolved type argument |
| `SPX-PG607` | arity mismatch |
| `SPX-PG608` | type outside the admitted grammar |
| `SPX-PG609` | borrowed field or view present |
| `SPX-PG610` | variant, resource, or function value present |
| `SPX-PG611` | recursive or cyclic closure |
| `SPX-PG612` | ambiguous or repeated stable identity |
| `SPX-PG613` | record, field, depth, leaf, or payload bound exceeded |
| `SPX-PG614` | cleanup inventory mismatch |
| `SPX-PG615` | settlement-obligation mismatch |
| `SPX-PG616` | effectful function |
| `SPX-PG617` | incompatible retained facts |

The next free range after this table is `SPX-PG9xx`; `SPX-PG7xx` and
`SPX-PG8xx` are allocated below to the descriptor and carrier codecs this
same round.

Diagnostic precedence (which reason wins when several apply) is unresolved
here; issue #150 recommends an order (bounds and framing before schema,
before canonical-byte validation, before subject selection, before retained
facts, before digest/cross-pair checks, before lifecycle validation, before
execution). This document adopts that recommended order for the future
classifier and does not restate it, since #150 already owns it; the
classifier's own tests must pin the exact order once it exists.

## Required test matrix for the classifier

Not run in this round; recorded so the implementer does not have to
reconstruct the required coverage from three issues again. All of the
following are required, none are optional, and none may be satisfied by the
smallest fixture alone:

**Positive:** same template, different input/result arguments; different
input/result templates; nested concrete records at maximum admitted depth;
zero-length `Bytes`; maximum admitted leaf count; display-only rename with
unchanged stable identities; every admitted Copy-scalar type argument; exact
reconstruction of the same classified subject (determinism).

**Negative, one per reserved reason above**, plus: one missing argument; one
duplicated argument; two arguments swapped; a never-instantiated template; a
generic function selected as the export; a scalar-only function; a borrowed
slice or borrowed string in any reachable position; an owned variant under a
record; the target-width-integer case flagged as a caveat above; a cyclic
record graph; duplicated stable declaration identity; a second owned
parameter; no owned result; an effect declaration; every bound's exact
first-over-limit case from the [Bounds](#bounds) table.

**Separation:** a regression proving admission under this profile does not
change the Canonical ABI Report, the C header, Project v8/v9/v11, or scalar
Wasm export behavior — this profile is additive, never a widening of an
existing schema.

## Freeze and change procedure

This profile is frozen as of commit `d45db653` (2026-09-11). Changing it
requires:

1. A new document version (`PUBLIC-GENERIC-BOUNDARY-PROFILE-V2.md` or later),
   never an in-place edit of this file's admission predicate or bounds table.
2. An explicit statement of which row of the [IN / DEFERRED /
   EXCLUDED](#in--deferred--excluded-shape-table) table moves, and to which
   state.
3. Independent review — this document itself was produced under an
   independent-review gate and may not be self-approved by the agent that
   writes the successor version.
4. Updated positive and hostile fixtures for the widened shape *before* any
   descriptor, carrier, or consumer code depends on it, per the repository's
   change protocol.
5. No existing frozen bytes — grammar terms, settlement plans, descriptor or
   carrier bytes already emitted under v1 — may be reinterpreted; a widened
   profile describes new instances, never old ones differently.

## Nonclaims

This document defines no classifier code, generates no descriptor or
carrier bytes, executes nothing, and grants no filesystem, process, network,
or publication authority. It is not evidence that any gate has passed. It
does not widen [Public Generic Type Grammar
v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md), [Public Generic Settlement
Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md), or any existing public
projection; every admission rule above is already a consequence of those
frozen specifications, restated here as one composed predicate rather than a
new one. Public generic ownership remains unsupported and unpublished until
[PG-9](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md#standing-support-and-publication-decision)
says otherwise.
