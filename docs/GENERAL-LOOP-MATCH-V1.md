# General Loop Match v1

Audience: language users, agent authors and compiler contributors.

Status: Authored; consolidated executable verification is pending. This finishes
#589's Copy scalar/flat Copy variant loop-match scope. It does not admit owning
payload guards or arbitrary record/collection loop matches.

## Source and helper admission

A `while` or `for` body admits ordinary matches over Copy scalars and concrete
variants whose every substituted payload field is a direct Copy scalar. This
includes payload-free authored variants, `Option<i64>`, `Option<u8>` and
`Result<i64,bool>`. The scrutinee evaluates once; constructors, named values
and monomorphic helper-call results obey the same classifier. Such variants
may be passed by Value to helpers, returned by helpers and returned by match
arms. Results retain the exact concrete type and Copy ownership. String arm
results retain ordinary uniquely owned join/settlement rules.

User helpers retain the existing declared read-only effect set, ordinary
capability checks and the permitted scalar/String/borrowed-str/borrowed-byte
signatures. The addition admits flat Copy variant Value parameters/results;
it grants no effect, public aggregate ABI, generic instance or new allocation.
Records, owning/non-Copy variant payloads, non-admitted generic calls, unsafe
operations and write-effect loop calls retain their stable refusals. Loop
entry and successful exit still have identical ownership state.

## Guards and coverage

Copy-variant Value matches admit ordinary checked Boolean guards: calls,
blocks, projections, nested matches, branches and lazy Boolean expressions,
including temporary Strings and body-local owning String call arguments.
Closures/invocation, yield, unsafe and residual propagation remain outside
this guard profile (`SPX-T254`); a non-Bool result is `SPX-T256`.
Guards cannot leave an outer owner consumed or change its availability, moved-place
or partial-ownership facts (`SPX-T254`). Ordinary lexical loans independently
prevent a local borrowed view from escaping its guard. Loop allocation and
ownership rules still apply throughout every guard subtree.

Patterns may name an exact case, a payload-free variant case-or pattern, or a
wildcard. Guards execute once only after the pattern matches and binds its
payload. Case mismatch skips the guard; false falls through; checked failure
selects the sticky status and evaluates no later guard or arm. `&&`/`||` keep
left-to-right lazy semantics. Guarded patterns contribute no coverage.
Exhaustive unguarded fallback is required (`SPX-M101`); unreachable unguarded
coverage retains `SPX-M102`. Owning or borrowed payload guards remain refused.

```text
let selected = choose(index); // returns Option<i64>
let output = match selected {
    Option::Some { value: n } if positive(n) => Option<i64>::Some { value: n },
    _ => Option<i64>::None {},
};
```

## Authenticated guard region

The ordinary source verifier and recursive oracle check every child under
its arm scope, including guard-only effects/calls. Both HIR validators repeat
all type, identity, signature, capability, loan and ownership checks. The
source shape helper confers no authority. Scalar-operator guards preserve
older plan bytes. Other guards have a canonical child cleanup region.
Successful guard evaluation exits that region before its `BooleanResult`
edges; the cleaned FlowState must equal the surrounding case-decision state.
Guard locals and temporaries settle before both selection and fallthrough.
Failure exits include outstanding guard owners and preserve the selected
status. Owned call operands stage left to right and commit together.

The recursive builder oracle derives identical regions and transitions.
Independent typed-HIR replay derives all case-or/wildcard/Boolean paths and
normal/failure observations, and authenticates the canonical attached plan,
including scope slots and finalizer vectors. Every materialized path/prefix
uses existing work/path budgets; the conservative census includes case-or
alternatives. No cap is raised; vectors are never sorted or repaired. Native
and Wasm emit canonical normal exits after guard values are computed. Nested
lexical scopes settle themselves; ancestor selection identifies the guard's
own exit, avoiding duplicate physical finalizers. User-resource guard trees remain a
stable native profile refusal; this guard addition grants no user-resource
physical finalizer route. Loop source admission already excludes those trees.

No parser/HIR node, carrier ABI, graph schema, CleanupPlan schema or effect is
added. Existing Copy carrier semantics and Boolean/ScopeExit facts suffice.
Programs composing whole String replacement use its independently
validated v16/v67 wrapper; older profiles keep their existing meanings.

## Explicit standalone Wasm selection

`wasm::internal_strings::emit_general_loop_match_module` selects the additive
`profile: "general-loop-match-v1"` descriptor. It composes Copy variant String
settlement and checked String replacement with private Copy variant helper
signatures, Copy variant match results and general guards. Public exports
remain Value `i64`/`bool` arguments/results. No CLI/Project/Web/Target Evidence
route selects this library entry implicitly.

The exact selected monomorphic, effect-free, acyclic closure is authenticated
before emission. Existing String imports, digest-bound generated trusted
runtime, fixed memory, quotas, stack/depth/node/function/work limits, checked
status mapping and poisoned-on-uncertainty behavior remain unchanged. Fixed
array byte access keeps full-width bounds checks. No host authority or public
String/nominal/view value is added. Unsupported selected Vec traversal,
owned payloads, records, effects and other closed operations remain `SPX-W111`
(or the owning Text Toolkit `SPX-W116`). Frozen `emit_module` remains nominal
free; `emit_copy_variant_module` and `emit_string_replacement_module` refuse
general guards, nominal helper signatures and Copy variant match results.
Their existing admitted modules retain exact descriptor/runtime/Wasm bytes.

## Focused authored gates

`cargo test --locked -p semaprax --test language general_loop_match::` covers
canonical and graph round trips, private Copy helper parameters/results,
variant/scalar matches yielding variants, call/block/wildcard/or/nested guards,
true/false and wrong-case paths, lazy contract failure, earlier operand and
staged-argument failure, String arm joins, `while`/`for` repetition, exact
source diagnostics and declared read/write effect controls, profile selection and forged HIR/cleanup facts. The
interpreter and native O0/O2 compare all sixteen outcomes; native counts
allocations/frees after every repeated call. Fifteen selected cases execute
repeatedly on the explicit String-settling Wasm entry; Vec traversal has its
exact documented Wasm refusal. A source-location ownership refusal remains
mandatory even when a loop is statically false.

`cargo test --locked -p semaprax --lib general_variant_guards_` checks builder
oracle parity and independent replay refusal of altered Boolean decisions,
missing cleanup actions and omitted region storage.
`cargo test --locked -p semaprax --lib general_loop_match_guards_match_recursive_oracle`
checks iterative/recursive source parity. Existing guarded Copy, indexed-byte,
owned String loop and refutable-match controls retain invalid type, effect,
field, ownership and nonexhaustive coverage regressions; only intended old
shape refusals migrate to success with frozen-profile refusal controls.

The change removes the former guard/helper/result refusal-and-repair turns
for the matched corpus. It claims no measured token percentage or speedup.
