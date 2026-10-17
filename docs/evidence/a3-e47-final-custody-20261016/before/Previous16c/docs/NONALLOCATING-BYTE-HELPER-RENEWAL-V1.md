# Nonallocating Byte Helper Renewal v1

Status: authored bounded contract; executable qualification remains separately gated.
Audience: coding agents, compiler contributors and reviewers.

This additive implementation profile addresses OPT #737. Owning regression
bodies are authored; execution and efficiency qualification remain pending.
It introduces no operation, allocation exemption, capacity increase, public
ABI or generated-name privilege.

An ordinary monomorphic, effect-free helper may return `Bytes` while accepting
exactly one `own Bytes` parameter. Other parameters are existing Copy scalars
or Copy records/variants, or authenticated borrowed Slice/str/record/Vec
carriers. Existing source and HIR type, loan, layout, field and origin checks
remain authoritative. Borrowed projected views and fixed-array slice views
use their ordinary provenance; they cannot alias the transferred byte owner.

The helper's complete body, requires and ensures, and every directly called
ordinary helper are inspected independently in source and typed HIR. The
closure must be acyclic. Each proof has at most 256 combined expression/function
cursor frames and 1,000,000 examined-node, child traversal and callee-lookup work
units. Full fixed cursor scratch is reserved before use. Missing declarations,
indirect calls, effects, yields, closures, lexical exits and proof-bound
exhaustion fail closed. No source or attached cleanup profile substitutes for
this independent proof.

Allowed operations are existing scalar arithmetic/conversions, Copy products,
borrowed byte/text reads and views, existing byte stores, Copy Vec length,
capacity and element reads, and authenticated Vec field reads. A literal
`vec_field` selector is static proof data, not a runtime String allocation.
Bytes copying or zeroed allocation, String construction (including literals,
formatting, numeric text, UTF-8 materialization and copying), Vec construction,
clone/reopen and every unknown operation are refused within the closure.
Existing byte fill admission still governs the helper's named same-owner stores.

`buffer = helper(..., buffer, ...)` renews an available mutable whole `Bytes`
binding when the helper's sole owning parameter consumes exactly that binding.
It is admitted inside ordinary bounded loops and outside loops. All arguments
still evaluate left to right; ordinary loan replay forbids an overlapping view
or second transfer. The old record renewal restriction to bounded loops is
unchanged. Reservation preserves the existing cleanup position; failure keeps
the selected status and settles actual staged owners. Postconditions protect
the provisional result before non-result cleanup and publication.

Only functions carrying helper-renewal sites select cleanup v19. Its existing
canonical reservation/renewal vectors are independently rederived; they are
never sorted, repaired or inferred downstream. Lexical functions additionally
retain their exact lexical-control-flow v1 certificate, continuations, resume
domains and loan v3 facts. Helper-only functions have no lexical certificate.

Graph v76 composes all existing graph facts with exact declaration/site/binding
updates under `semaprax.byte-helper-renewal.v1`. Complete typed HIR is required
to authenticate the transitive closure; frozen from-parts and evidence routes
remain closed. Programs without this profile retain their prior schemas/bytes.

Owning definitions are `loop_calls::byte_writer::tests::` and
`owned_data::byte_helper_renewal::`. They cover nested helper closure proof,
ordinary/loop reservation, contract allocation and recursion refusal, hostile
actual-owner substitution, canonical and graph projections, physical native
O0/O2 and private Core-Wasm settlement, and protected-result postcondition
failure. Definitions are not current-source execution evidence.

Compiler census cost is selective at the module signature level: a module with
no `Bytes` result plus `own Bytes` parameter signature skips the new binding
cursor. A module containing a potentially relevant signature performs the
bounded per-function census even when that particular function has no proven
helper-renewal site. Prior schemas and graph bytes remain unchanged in that
case; zero additional compiler work or scratch is not claimed. This body
census reserves 514 cursor slots, matching the ordinary cleanup traversal's
512-depth allowance and framing slots. The separate transitive closure proof
retains its stricter 256 combined function/expression frames.

OPT #738 adds a phase-owned typed module index for HIR validation and the
cleanup replay schema, typed skeleton consumers, and complete module cleanup
construction. Each phase independently
constructs its index from the immutable resolved program. One 514-slot body
cursor is reused across functions, and each directly selected helper target's
complete transitive proof is performed once per index. Subtrees shared by
different selected targets are still replayed within each complete proof, so
the original combined 256-frame and closure-work bounds remain authoritative.
Positive and negative
results remain local to that program borrow; attached plans, graph schema and
names supply no authority. A changed program requires a new index.

The function/site carriers and temporary proof table reserve their exact Vec
backing before allocation. Helper identity and binding references borrow the
program; no identity Strings or opaque map nodes are introduced. All helper
site backing remains charged, and the validator's retained-capacity probe
includes the index. A module without a potentially relevant signature creates
no new backing or cursor reservation. Index lookup construction has a bounded
1,000,000-unit module budget in addition to the existing per-function census
and per-target closure limits. Reservation failure is H006, not a legacy
fallback. An allocating ordinary helper still supplies no renewal authority
and retains the ordinary profile.

Source body verification now owns a separate immutable AST proof memo for its
function-checking phase. Repeated direct targets reuse a completed proof and
one reserved 256-frame cursor; each different target still replays its complete
transitive body, requires and ensures. Assignments retain their ordinary
per-site ownership checks. The independent recursive oracle remains separate.

Workspace projection cost and materialization invocations each own a separate
provider memo over the held authored facts and source programs. Actual provider,
module and alias provenance is authenticated before a memo lookup; a generated
prototype supplies no authority. Borrowed keys and cursor backing are reserved
before allocation, replacement backing remains charged, and memo reservation and lookup-work
exhaustion is sticky G171 with no ordinary-profile fallback. An irrelevant
signature creates no memo backing. These consumers do not reuse proofs across
invocations, revisions or mutable source.

Retained Workspace Graph schema selection independently owns a positive-only
memo over its exact held typed module slice and declaration facts. Every site
still checks its complete call arguments, owner binding and selected cleanup
schema; a completed transitive proof is reused only for the same retained
callee pointer in that immutable context. One separately reserved 256-frame
cursor and an exact function-reference Vec are reused across module selection.
Changed providers, copied contexts and missing type facts require fresh replay.
No negative result or attached plan grants authority. Sticky reservation refusal
remains G171 and cannot be relabeled as successful ordinary-profile selection.
This is authored compiler scratch reuse, not measured heap or speed evidence.

Resolver and workspace-link construction hold one immutable typed index while
building canonical plans for all ordinary functions and retained instances.
Schema selection, renewal reservations and transfers reuse exact function-site
facts; copied functions or another program cannot borrow that authority. The
plan staging Vec is reserved before allocation, with the index, existing program
backing and previously completed plans still live. The complete physical capacity
of old attached plans is additionally reserved before index or builder creation,
including spare backing kept alive by delayed publication. A fixed 514-frame
cursor bounds old recursive type/shape census before its physical sum. Conditional
variant inventories include every case and field carrier capacity and identity
backing. All flat nested inventories are charged against the census work bound
before the physical sum, including the path that needs no recursive cursor;
attached plan contents supply no renewal authority. Publication consumes this
owned carrier only after the index borrow ends; construction failure replaces
no attached plans. A module without a relevant signature still reserves its
ordinary plan carrier but adds no helper index backing.

Direct and detached cleanup construction, standalone schema queries, and cleanup
path-state replay retain independent proof paths. The construction index does
not substitute for validation or replay, which rebuild current HIR facts. This
implements bounded reuse in the named consumers, rather than a global cache or
complete elimination of repeated helper proof work. Construction and replay
performance and accepted-task efficiency remain unmeasured. The direct phase's
exact/one-short reservation contract covers helper indexes, census scratch,
staging carriers and retained old-plan overlap; it does not enforce every nested
PlanBuilder allocation. Existing workspace AST-to-HIR prebounds remain independent.
Full direct nested construction accounting and a proof of that outer upper across
all staged/old/new contours remain follow-up obligations; the test-only capacity
upper is observational and cannot replace a preallocation reservation.

The helper-site census stores sorted borrowed identity/binding references in
an explicitly grown Vec. Each replacement backing is reserved before its
allocation while the previous backing remains charged; duplicate keys replace
only the borrowed value and incur no growth reservation. Existing legacy
renewal tree maps remain separate, so helper sites introduce no opaque tree
nodes or copied identity backing. The checked census refuses before insertion
when its backing reservation fails.

## Deferred paired construction and replay measurement (OPT #738)

`scripts/measure_helper_phases.py` prepares one actual CollectionResponse
replacement through the ordinary authenticated Project derivation and replay
APIs. Its application calls both generated preflight and encoding, and includes
an actual generic instance. Setup, source checking, resolution and canonical
serialization are outside the measured intervals. Both routes use the same
complete immutable typed program, including monomorphic functions and retained
instances, and the same already attached plans. The owning definition checks
exact canonical plan bytes against production construction and independent
validation before collecting samples.

The private test-only construction control preserves old-plan overlap census,
staging reservation and complete delayed-publication carrier on both routes.
The indexed route builds a fresh module index inside each timed sample; the
independent route uses the existing per-function proof path. Both omit the
production test-only observational capacity-high-water probe. Replay likewise
uses one shared global budget over the complete program on each route, selecting
only whether fresh helper facts are shared or independently reconstructed.
The existing per-builder test-only capacity probes remain enabled on both
routes, including their route-dependent backing census. Reported construction
time is therefore instrumented test-harness phase wall time, not uninstrumented
production compiler latency; the aggregate staged-plan observation is excluded
on both controls. Returned plan destruction and canonical serialization are
outside both timed intervals. These controls expose no runtime switch and confer
no authority on cached plans.

After the owning definitions are qualified, run from a clean committed checkout:

```sh
python3 scripts/measure_helper_phases.py --root "$PWD" \
  --output /absolute/new/result-directory --warmup 2 --samples 10
```

The driver builds the owning release library test binary with all features,
locked offline dependencies, one Cargo job, and a private target directory under
this worktree. Compilation is separately logged and never included in phase
latency. Each iteration rotates indexed/independent ordering. Warmup records
remain in the raw JSONL and are excluded only from the median summary. Every
requested construction/replay pair must succeed; missing/duplicate samples,
source or binary drift, or mismatched generated/application source digests leave
the run unqualified. Raw stdout, stderr, build messages, actual source bytes,
toolchain, source revision, binary digest and all sample records are retained.

`elapsed_ns` is compiler-phase wall time. `reservation_ledger_bytes` is the
actual sticky reservation ledger debit under the unchanged 67,108,864-byte
measurement reservation limit; it is not measured heap use, allocation count
or RSS. Replay reports its existing work and skeleton budget debits separately
(including conservative skeleton preflight, rather than a complete count of all
helper proof node visits);
construction has no corresponding complete work counter and reports null.
Fixed harness/process context is not live application cost. This measures
cleanup construction and replay, not source verification or resolution, physical
JSON execution, kernel crossings (OPT #708), billing, or accepted-task efficiency.
No measurement result or improvement is claimed before this driver is run.

The normal owning definition is
`cleanup_plan::phase_measurement::paired_routes_preserve_complete_program_plans_and_independent_replay`.
The ignored measurement definition is
`cleanup_plan::phase_measurement::generated_codec_construction_and_replay_measurement`.
Both are authored in the existing library harness; the driver selects only the
latter by its exact name. The regular qualification selector must exclude the
ignored body from executed-success counts.

## Resolved saved-cursor accounting (OPT #750)

The resolved proof's authored child cursor uses the linear conservative work
model in [HIR authored child cursor v1](HIR-AUTHORED-CHILD-CURSOR-V1.md): one
block statement census and constant saved advances. It retains complete callee,
argument, active-call-cycle and ownership checks. Source indexed traversal keeps
its existing prefix charges. Physical reservations, work/depth bounds, runtime
fuel and helper schema authority are unchanged; owning executable qualification
of this authored accounting change remains pending.

The retained source-schema byte-helper census and held proof walker also use
saved authored child cursors. Census scratch is charged before construction;
the invocation-held proof Box is reserved once and reused without authority
across contexts. Logical node/advance and one-time statement work remain the
same; helper signatures, actual bodies, owner facts and schema selection still
replay independently. The fixed scratch charge can change a near-limit builder
receipt and is never refunded or hidden. Remaining indexed lookup calls in
independent test oracles are intentionally retained.
