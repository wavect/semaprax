# Semantic Kernel v1: trusted computing base, a proved kernel language, and the self-hosting gate ladder

- Audience: compiler contributors, language designers, and any agent asked to
  extend, self-host, or formally verify part of SEMAPRAX.

- Status: proposed trust-reduction programme. The TCB is inventoried; Kernel-0
  has a paper, not machine-checked, type-safety sketch; three capacity ceilings
  have exact fixtures (the `SPX-P207` ceiling was later fixed); and the
  reification predicate is an executable HIR admission check in
  `src/kernel_zero.rs`. The self-hosting ladder has evidence for rung 1 only;
  no proof in this document is machine-checked.
  A separate reference interpreter, HIR translator, and 100-program
  deterministic corpus (`src/kernel_zero/{term,value,eval,reify,corpus,differential}.rs`)
  compare Kernel-0 with the compiler interpreter, native C11, and Core Wasm.
  The corpus grew from 74 after issue #188's strengthening passes and found
  **zero disagreements**; finite differential tests address reification
  faithfulness only for those programs, not as proof. The comparison fragment
  now covers `bool == bool` / `bool != bool`, but the stated Progress theorem
  still omits `i64` overflow and division/remainder by zero.

## Why this document exists

[Issue #188](https://github.com/wavect/semaprax/issues/188) asks for a
long-term trust-reduction programme: define the smallest semantic kernel that
every source, graph, change, ownership, effect, and lowering claim in
[RFC 0001](RFC-0001.md) depends on, prove a small first subset of it, and
define progressive, independently-checkable self-hosting gates rather than a
single "compiler is self-hosted" claim. It explicitly separates that proof
programme from the unrelated later goal of rewriting compiler stages in
SEMAPRAX itself, and explicitly forbids treating tests as proof or self-hosting
as evidence of correctness by itself.

This document is that programme's opening chapter: a trusted-computing-base
inventory, a tiny kernel language (**Kernel-0**) with syntax, typing, and
operational semantics stated independently of the Rust implementation, a paper
proof sketch of its safety properties, an explicit compiler-HIR-to-kernel-term
admission predicate now implemented and tested as executable code (its
*faithfulness to real evaluation* remains unproved -- see "Reification"), and
a **self-hosting gate ladder** with an honest statement of which rung is
reached today: **rung 1**, evidenced below, and no further.

Two independently measured compiler capacity ceilings
([issue #241](https://github.com/wavect/semaprax/issues/241)) bound what a
kernel-sized program can express today, and this document adds a third,
previously undocumented one found while producing this kernel's own evidence.
All three are given with the owning source location, the exact constant, and a
minimal reproducible fixture, most of them now committed as regression tests
(`tests/cleanup_backends/kernel_boundary.rs`,
`src/parser/depth/tests.rs`) so a future change cannot silently move a ceiling
without a red test.

## Non-claims

Read this section before citing this document elsewhere.

- **No property below is machine-checked.** "Proved" below means a paper
  proof sketch, in the traditional programming-languages sense (a syntactic
  progress/preservation argument a reader can follow and attack), not a
  Lean/Coq/Isabelle artifact. Section "What is and is not mechanically
  checked" states the exact boundary.
- **No compiler-component self-hosting milestone is reached.** Rungs 0 and 1
  of the ladder below are reached and evidenced; rung 2's formatter target and
  every higher rung remain targets, not results.
- **The compiler-to-Kernel-0 translation's *admission* half is mechanically
  checked; its *faithfulness* half now has a differential test, which is
  evidence, not a proof.** `src/kernel_zero.rs` decides, mechanically,
  whether a given `ResolvedFunction` matches Kernel-0's grammar.
  `src/kernel_zero/differential.rs` now checks a from-scratch reference
  interpreter (`src/kernel_zero/eval.rs`) against the compiler's real
  interpreter backend, driven through its ordinary public
  `interpreter::interpret` entry point, over a deterministic 100-program
  corpus (26 hand-written edge cases, up from 14 in issue #188's
  corpus-strengthening pass, plus 60 seeded, 9 fault-selection, and 5
  structural adversarial programs, 782 target concrete-argument comparisons
  total, seed `0x4b65726e656c3021` --
  see "Differential testing" below for exact counts and what was found).
  This is **not** a proof that the interpreter always agrees with Kernel-0's
  operational semantics: the corpus is finite. **A later session extended
  the same comparison to native C11 (`-O0`/`-O2`) and Core Wasm**
  (`src/kernel_zero/differential/cross_backend.rs`) over the identical
  corpus and seed: the latest executed target run covered 627 comparisons with
  0 disagreements; the expanded gate now selects 2,346 and was not run in this
  tranche. This remains evidence over one finite corpus, not a proof, and is
  silent on any program outside it. Section "Reification: HIR to Kernel-0, and its
  unproved edge" and "Differential testing" together name what is now
  checked and what remains open.
- **The `SPX-G171` byte figures below are not independently re-measured at
  full scale in this session.** The 67,108,864-byte `MAX_BUILDER_BYTES`
  constant is read directly from source and cited with its exact location;
  the "~35-40 KB of source" translation of that budget is the finding of the
  session recorded in `examples/catalog-normalizer-project/README.md` and
  issue #241, cited here rather than reproduced, because reproducing it needs
  a multi-module scratch project outside the scope of this session's budget.
  The `SPX-H006` and `SPX-P207` ceilings below **are** independently
  reproduced and reduced to committed regression tests this session.

## Trusted computing base (TCB) inventory

Every claim in RFC 0001 -- type preservation, effect soundness, ownership and
exactly-once cleanup, stable identity, deterministic graph projection,
semantic-transaction validity, and target-lowering correctness -- currently
rests on the following components being correct. None of them is formally
verified today; each is exercised only by the test suite and the completion
matrix's executable gates. Naming this list precisely is the point: a kernel
is only "minimal" once its own boundary and everything left outside it are
both explicit.

| Layer | Owning module(s) | What the TCB currently assumes |
|---|---|---|
| Lexer/parser | `src/lexer.rs`, `src/parser.rs`, `src/parser/*` | Tokenization and grammar admission are correct; `SPX-P207`'s token-level nesting pre-check had a counting defect found by this document's first draft ([issue #247](https://github.com/wavect/semaprax/issues/247)) and fixed by a later commit in this same programme (see ceiling 3 below) |
| Resolver / HIR | `src/hir.rs`, `src/hir/resolve_*.rs`, `src/hir/validation.rs` | Name resolution, type inference/checking, exhaustiveness, and the source verifier (`src/source_verify/*`) together enforce RFC 0001's static guarantees |
| Ownership / cleanup | `src/cleanup.rs`, `src/cleanup_plan.rs`, `src/cleanup_plan/build.rs`, `src/cleanup_plan/replay.rs`, `src/loan_plan.rs` | `CleanupPlan` construction is sound and its independent replay (`hir::validate`) is a genuine, not rubber-stamp, second check of exactly-once cleanup |
| Semantic graph | `src/workspace_graph.rs`, `src/workspace_graph/*`, `src/graph.rs` | Stable-ID assignment and deterministic graph projection from HIR are correct and injective across rename/move |
| Semantic transactions | `src/semantic_workspace_*.rs` | Precondition checking, replay-before-commit, and fail-closed staleness detection are correct |
| Interpreter | `src/interpreter.rs` | Tree-walking evaluation matches the static semantics the resolver/verifier admitted |
| Native lowering | `src/codegen/*`, the C11 bootstrap backend | Generated C (and eventually Cranelift/LLVM IR) preserves the checked program's observable behavior |
| Wasm lowering | `src/wasm.rs`, `src/wasm/*` | Generated Wasm preserves the checked program's observable behavior within the documented owned-ABI profile |
| External tooling | `wasmparser` (validates emitted Wasm), the host C/Rust toolchain, `rustc` itself | Each is trusted transitively; none is re-verified by SEMAPRAX |

Nothing in this table is newly claimed as correct by this document. It is
inventory, not proof; the proof work below covers a small fragment of the
"Resolver / HIR" and "Interpreter" rows only, for the language subset defined
next.

## Kernel-0: the smallest provable subset

Per the issue's implementation sequence step 1, Kernel-0 selects "scalars,
pure functions, records/variants, contracts/effects as appropriate, and
explicit ownership operations" -- narrowed further, for this first subset, to
the part reachable without touching ownership, effects, or generics at all,
so that progress/preservation can be stated and checked by hand in one
sitting. Ownership, effects, and generics are named as Kernel-1+ targets in
the gate ladder, not modeled here.

### Syntax

```text
Program  ::= Fn*
Fn       ::= "fn" name "(" (name ":" Ty),* ")" "->" Ty "{" Expr "}"
Ty       ::= "i64" | "bool"
Expr     ::= n                              -- integer literal
           | "true" | "false"
           | x                              -- parameter or let-bound name
           | Expr BinOp Expr
           | "-" Expr | "!" Expr
           | "if" Expr "{" Expr "}" "else" "{" Expr "}"
           | "let" x "=" Expr ";" Expr
           | f "(" Expr,* ")"               -- f a Fn name, call graph acyclic
BinOp    ::= "+" | "-" | "*" | "/" | "%"
           | "==" | "!=" | "<" | "<=" | ">" | ">="
           | "&&" | "||"
```

This is exactly the subset of the admitted language with `i64`/`bool` scalars,
`if`/`else`, `let`, arithmetic/comparison/boolean operators, and non-recursive
calls -- no `own`/`borrow`, no `uses`/effects, no records, variants, loops,
closures, or generics. Every Kernel-0 program is also an ordinary, admitted
`.spx` program: Kernel-0 is a syntactic restriction of the real grammar, not a
separate one, so an admitted Kernel-0 program is checked, compiled, and run by
the real, unmodified toolchain (Section "Rung 0 evidence" demonstrates this).

### Static typing

Standard structural typing, checked left to right per RFC 0001's evaluation-
order invariant:

```text
Γ ⊢ n : i64          Γ ⊢ true : bool          Γ ⊢ false : bool
Γ(x) = T
──────────           Γ ⊢ e1 : i64  Γ ⊢ e2 : i64
Γ ⊢ x : T            ─────────────────────────── (+ - * / %)
                     Γ ⊢ e1 BinOp e2 : i64

Γ ⊢ e1 : i64  Γ ⊢ e2 : i64                    Γ ⊢ e1 : bool  Γ ⊢ e2 : bool
─────────────────────────────── (== != < <= > >=)   ─────────────────────── (== !=)
Γ ⊢ e1 BinOp e2 : bool                               Γ ⊢ e1 BinOp e2 : bool

Γ ⊢ e1 : bool  Γ ⊢ e2 : bool
─────────────────────────────── (&& ||)
Γ ⊢ e1 BinOp e2 : bool

Γ ⊢ c : bool  Γ ⊢ e1 : T  Γ ⊢ e2 : T          Γ ⊢ e1 : T1  Γ, x:T1 ⊢ e2 : T2
───────────────────────────────────── (if)    ─────────────────────────────── (let)
Γ ⊢ if c { e1 } else { e2 } : T                Γ ⊢ let x = e1 ; e2 : T2

fn f(x1:T1,...,xn:Tn)->T ∈ Program   Γ ⊢ ei : Ti for each i
──────────────────────────────────────────────────────────── (call)
Γ ⊢ f(e1,...,en) : T
```

`&&`/`||` are call-by-need per RFC 0001's "lazy boolean operands execute only
when required" invariant; this is a semantics rule below, not a typing rule,
since both branches must still type-check as `bool`.

### Operational semantics

Small-step, left-to-right per RFC 0001's evaluation-order invariant. Values
`v ::= n | true | false`. Selected rules (the rest follow the same left-to-
right, call-by-value shape and are omitted for space):

```text
e1 -> e1'
────────────────────────────                n1 op n2 = n   (ordinary arithmetic)
e1 op e2 -> e1' op e2                        ─────────────────────────────────
                                              n1 op n2 -> n
v1 -> _ excluded (v1 is a value)
e2 -> e2'
────────────────────────                    if true  { e1 } else { e2 } -> e1
v1 op e2 -> v1 op e2'                        if false { e1 } else { e2 } -> e2

e1 -> e1'                                     true  && e2 -> e2      false && e2 -> false
──────────────────────────────                true  || e2 -> true    false || e2 -> e2
let x = e1 ; e2 -> let x = e1' ; e2

let x = v ; e2 -> e2[x := v]                 f(v1,...,vn) -> body_f[x1:=v1,...,xn:=vn]
                                              (Fn f defined as above; the call graph
                                               among Kernel-0 Fns is acyclic, so this
                                               reduction always terminates)
```

Substitution `e[x := v]` is the usual capture-avoiding scalar substitution;
Kernel-0 has no binders that could capture (`let` and parameters are the only
binding forms, and each Fn's parameter names are distinct by the parser's
existing admission rules).

### Paper safety proof (progress and preservation)

**Theorem (Preservation).** If `Γ ⊢ e : T` and `e -> e'` then `Γ ⊢ e' : T`.

*Proof sketch.* By induction on the typing derivation. Every reduction rule
above either (a) is a congruence rule stepping a strict sub-expression, so
preservation follows directly from the induction hypothesis applied to that
sub-expression, or (b) is one of the finitely many base reductions
(arithmetic/comparison, `if`, `&&`/`||` short-circuit, `let`-substitution,
call-substitution). For each base reduction, the result's type is read off the
same typing rule that assigned the redex's type: arithmetic and comparison
results have the fixed result type each `BinOp` rule already names
independently of its operands' values; `if`'s two branches were already
required to share `T`; `&&`/`||`'s reduced form is either the untouched
second operand (already typed `bool` by the `if` rule's premise) or a boolean
literal; substitution preserves typing because Kernel-0 has no dependent types
-- a value's type does not depend on which variable held it -- so
`Γ, x:T1 ⊢ e2 : T2` and `⊢ v : T1` together give `Γ ⊢ e2[x:=v] : T2` by a
standard substitution lemma (itself a routine induction on `e2`'s typing
derivation, omitted here). ∎

**Theorem (Progress).** If `⊢ e : T` (closed, i.e. typed under the empty
context plus the fixed, acyclic top-level `Fn` table) then either `e` is a
value or there exists `e'` with `e -> e'`.

*Proof sketch.* By induction on the typing derivation. Literals and variables
under the empty context are values or, for a variable, impossible (the empty
context has no bindings, and every `Expr` reaching this point through `let`
or a call body has already had its binder substituted away by the
Preservation argument's `let`/call rules, so no free variable can remain in a
closed derivation). Every compound form's immediate sub-expressions are typed
by the induction hypothesis and so are each either a value (letting the
form's own base rule fire) or step-able (letting the form's congruence rule
fire). The one form needing a side condition is `call`: `f(v1,...,vn)` always
steps, because Kernel-0's call graph is acyclic by construction (Kernel-0's
grammar has no direct recursion and this document does not admit indirect
recursion into Kernel-0 either -- see "What Kernel-0 deliberately excludes"),
so substituting into `f`'s body cannot loop forever within one reduction step,
and `body_f` always exists because every named `f` is required to be declared
in the fixed `Program`. ∎

**Corollary (Type soundness / "no admitted program gets stuck").** By
Preservation and Progress, no closed, well-typed Kernel-0 program's evaluation
gets stuck at a non-value, non-steppable term: every well-typed Kernel-0
program either diverges (impossible, since the call graph is acyclic and
every other reduction strictly decreases a term's size) or terminates at a
value of its declared type.

This is the "small first subset" the issue's implementation sequence step 3
asks for: type preservation and a termination/progress argument, stated and
proved on paper, for a fragment small enough to check by hand in one sitting.
It is **not** the mechanized artifact acceptance criterion 1 ultimately wants;
see "What is and is not mechanically checked."

### What Kernel-0 deliberately excludes, and why

- **Ownership and effects.** RFC 0001's exactly-once cleanup and effect-
  soundness claims are exactly the properties issue #188 asks a kernel to
  eventually cover, but Kernel-0's pure, Copy-scalar-only fragment has no
  cleanup obligations at all, so it says nothing about them yet. This is
  Kernel-1's job (see the gate ladder).
- **Recursion.** Proved by a plain structural-size argument above; a
  recursive or mutually-recursive Kernel-0 would need a well-founded
  decrease measure (a fuel parameter, or a proof that every named recursive
  call strictly decreases some argument) that this document does not supply.
  Issue #241 independently found the compiler's own cleanup-replay path
  budget currently rejects a recursive JSON-grammar call cycle outright
  (`SPX-H006`), so admitting recursion into a later Kernel-*n* needs that
  ceiling addressed first regardless of the proof side.
- **Records, variants, generics, closures, loops.** Each adds either new
  values (aggregates), new binders (closures, generic parameters), or new
  control flow (loops) that the proof sketch above does not cover. Each is a
  natural next Kernel-*n* increment; none is silently assumed safe by
  omission here.

## Reification: HIR to Kernel-0, and its unproved edge

Per the issue's implementation sequence step 4, a real Kernel-0 admission
predicate over the compiler's own HIR (`ResolvedProgram`/`ResolvedFunction` in
`src/hir.rs`) is:

*(Scope note on step 4's second half, "reject compiler states that cannot be
reified": the predicate below is now implemented and tested as a standalone
`pub(crate)` checker, but it is not wired into any compiler diagnostic path.
Calling it never rejects a real program -- it only answers "does this
function's HIR shape match Kernel-0's grammar" for a caller, today only its
own tests, that asks.

**Decision (issue #188, this session): recorded as permanently inert by
design, not left open as undone wiring.** `reifies_into_kernel_zero` names
proof-fragment membership -- "does this function's shape match the one
paper-and-Lean-proved calculus" -- which is orthogonal to admission: every
function outside Kernel-0 is already independently checked and admitted by
the compiler's real verifier, so nothing about whether a program compiles
should ever turn on it. A **mandatory** refusal wired from this predicate
would silently narrow the admitted language on a proof-infrastructure
pretext -- exactly the mistake a prior session made on issue #208 and had
rejected -- so that shape is out regardless of where it lived. The remaining
honest option was a **non-mandatory, queryable** surface (a new
`semaprax.agent-context.v1`/`v2` facet in `src/graph.rs`'s
`AgentContextFilter`, or a new top-level CLI verb) that only reports
membership for a caller that asks. Three independent reasons this session
did not build that surface, not only that this session's shared checkout
made it temporarily unsafe to edit the files it would touch:
1. This repository's own determinism invariant ("graph JSON... and
   contracted generated artifacts are deterministic") makes any such facet a
   new versioned wire-format/CLI commitment with golden fixtures and spec
   text of its own -- a materially larger and more permanent surface than
   the one boolean function the predicate itself is.
2. Every fact the predicate reads (effect emptiness, empty
   `requires`/`ensures`, `Value`-only ownership, `i64`/`bool`-only scalar
   types, and the absence of records/variants/match/closures/mutation in the
   body) is already independently visible through facts the compiler
   exposes today -- the existing `contracts`/`ownership`/`effects`/`types`
   context facets, or simply the function's own signature and body. A
   dedicated `kernel-zero` facet would package a boolean *derived* from
   information already exposed, not reveal anything a caller could not
   already determine.
3. Concretely, at the time of this decision, `src/graph.rs` sat exactly at
   its recorded `tests/module-size-budget.tsv` ceiling (5596/5596 lines),
   and that gate's own text is explicit that a budgeted file grows by moving
   code into a submodule, never by raising the recorded number -- so even
   the lightest version of this wiring was not a small, local edit.

A future session with independent evidence that a caller needs this exact
fact, and that it is worth the wire-format commitment point 1 names, can
revisit this; nothing here forecloses that. What is foreclosed is treating
"wire the predicate somewhere" as unfinished work this issue still owes --
it does not, by design.)*

> A `ResolvedFunction` **reifies into Kernel-0** iff every parameter and its
> return type is `i64` or `bool`, its `uses`/effect set is empty, it has no
> `own`/`borrow` parameter or return mode, its body's expression tree uses
> only `Int`, `Bool`, `Binary`, `Unary` (on `i64`/`bool`), `If`, `Let`
> (scalar-typed), `Var`, and `Call` to other functions that themselves
> reify into Kernel-0, and the subgraph of reifying functions reachable from
> it by `Call` is acyclic.

**Update (a later session in this same trust-reduction programme, per this
issue's own audit):** this predicate is now implemented as executable code,
`kernel_zero::reifies_into_kernel_zero` (`src/kernel_zero.rs`), checked
directly against `hir::ResolvedProgram`/`ResolvedFunction` with no parser or
codegen dependency, and its own `#[cfg(test)]` submodule exercises it against
real source through the ordinary `parse` -> `hir::resolve` path -- a positive
Kernel-0-shaped case (scalars, `if`, `let`, arithmetic/comparison, a
non-recursive call, and a `bool`-typed function using `&&`), and four negative
cases refused for exactly the reason named (a declared effect, a record
type reachable through a return value and transitively through a caller, a
`requires` contract clause, and a mutually recursive call pair). The
implementation is stricter than this section's prose predicate in one way,
found while implementing it: it also rejects a non-empty `requires`/`ensures`
list, since Kernel-0's grammar has no contract syntax at all and the original
prose above did not mention this case. Confirmed against the built CLI while
implementing it: `own`/`borrow` on a Copy value type (`i64`/`bool` included)
is refused by the compiler itself with `SPX-O002`, so the predicate's
`param.ownership == OwnershipMode::Value` conjunct, while kept as the
faithful translation of "no `own`/`borrow` parameter... mode" above, is
unreachable for any admitted function that also passes the scalar-type
check -- no admitted source can combine an `i64`/`bool` parameter with a
non-`Value` ownership mode in the first place, so no test exercises that
specific conjunct directly.

This is exactly the risk the issue names in its own "Failure and security
cases" section: *"the compiler-to-model translation can become the unproved
weak link."* What is closed now, and what remains open:

- The three regression tests from the prior iteration
  (`tests/cleanup_backends/kernel_boundary.rs`,
  `src/parser/depth/tests.rs`) check **admission** (does the real toolchain
  accept or reject a given source text with a given diagnostic). The
  reification predicate now additionally checks, mechanically, whether an
  *admitted* `ResolvedFunction`'s HIR shape matches Kernel-0's grammar --
  closing the "predicate stated in prose only" half of this section's
  original gap.
- **Partially closed, this session:** "this function reifies" is now checked
  against "the compiler's interpreter backend agrees with Kernel-0's
  operational semantics on it" for a finite corpus, by a from-scratch
  Kernel-0 reference interpreter (`src/kernel_zero/{term,value,eval}.rs`), a
  total-on-admission HIR translator (`src/kernel_zero/reify.rs`), and a
  differential test (`src/kernel_zero/differential.rs`) -- see "Differential
  testing" below for the corpus, the seed, and the exact result. **Extended
  to native C11 and Core Wasm in a later session**
  (`src/kernel_zero/differential/cross_backend.rs`): the same corpus, same
  seed, 627 comparisons (209 samples x native `-O0` x native `-O2` x Core
  Wasm), 0 disagreements. The expanded 753-sample corpus is wired to the same
  gate but was not run across those three target paths in this tranche. **Still
  open:** the corpus is finite and not exhaustive (a passing differential
  test is evidence a disagreement was not found, not a proof none exists),
  and Kernel-0's own operational-semantics rules turned out to need an
  explicit extension before a reference interpreter could even be written
  against them -- "Differential testing" documents that extension rather
  than treating it as already covered by the "Paper safety proof" above.

This narrower remaining gap was the highest-priority follow-up this document
identified; "Differential testing" below records what closing part of it
found, in both directions (the compiler's behavior, and two gaps in this
document's own stated grammar/semantics that a from-scratch reference
implementation surfaced by simply needing to make every case computable).

### Exact-source reification binding (issue #188)

`src/kernel_zero/reify.rs` now also owns a private `BoundTranslation` boundary.
Derivation parses the exact supplied source, resolves it, checks bounded
Kernel-0 admission, and independently replays HIR validation before producing
the term. Replay repeats this derivation and compares the complete translation,
the selected stable entry identity, a domain-separated SHA-256 digest of the
exact source bytes, and a separate digest of the canonical term structure.
Neither digest is a signature or authority. Even whitespace-only source drift
requires new evidence. Recomputing a digest on a substituted term cannot make
it match the freshly translated source.

The profile accepts at most 65,536 source bytes, 64 reachable functions, 8,192
charged expression/let nodes, and 128 active traversal levels (root depth
zero; depth 128 refuses, including traversal across call edges). The call walk memoizes completed functions and
rejects a repeated function on the active path. Unsupported effects (including
`yields`), contracts, non-scalar data, ownership modes, generic calls, mutation,
and other control flow have explicit internal refusal categories. These are
proof-profile refusals only; ordinary compiler admission stays unchanged.
Only the reachable call closure is translated, ordered by stable declaration
ID. Parameter, argument, operand, branch, and let order remain authored order.
Term hashing uses explicit tags, fixed-width counts and integers, and
length-prefixed UTF-8 identities, never Rust debug formatting.

The interpreter and target differential routes replay this binding before
evaluating the produced term. Their finite corpus remains the evidence for
selected return values and sticky arithmetic faults; the binding authenticates
which source and term were compared. It is not a mechanized translation or
termination theorem, does not connect Rust HIR to the Lean syntax, and does
not advance the self-hosting ladder. The existing Lean proofs and hosted
evidence claims are unchanged.

### Mechanized call-chain termination (issue #188)

`proofs/kernel0-lean/Kernel0.lean` extends its existing `Expr` and `Program`
with `callTargets`, which walks every constructor, every branch (including
lazy/unselected branches), and every call argument. `CallEdge` reads a caller's
actual body from that same function table. A `CallGraphRanked` certificate
requires a natural-number rank to decrease strictly on every derived edge.
Under that explicit hypothesis, Lean proves that a path of length `n` from
`f` to `g` satisfies `n + rank(g) ≤ rank(f)`, every closed path has length
zero, and no infinite call chain exists. An ordinary helper-call fixture
satisfies the certificate; a self-call nested in another call's argument
cannot satisfy it for any rank.

The existing proof gate pins these five added theorem statements and audits
their axiom sets. Its committed negative control offers a forged constant
rank for the recursive fixture: Lean must reject the non-strict `0 ≤ 0`
proof where strict `0 < 0` is required. Unexpected success, missing imports,
or an unrelated elaboration error fails the control. The source-only gate
also pins the control so replacing it with an unrelated failing theorem
does not preserve a passing gate.

The gate also pins the full-language Progress pair, the fuel-bounded theorem,
the structural/rank bridge, and the concrete two-beta-step helper fixture. A second committed hostile
control reuses that exact two-step witness while claiming a one-step
`NormalizesWithin` budget; Lean must reject it specifically at the `2 ≤ 1`
obligation. A third hostile control attempts to treat the helper's equal-size
call beta as a strict syntax decrease and must fail exactly at `1 < 1`.
Success or any unrelated failure is a gate failure.

The gate itself does not trust proof-source report text. It locates pinned
signatures only after stripping comments and strings, rejects top-level
`constant` as well as `axiom`, and runs all 46 axiom queries from a generated
driver bracketed by unpredictable markers. Hostile self-tests pin rejection of
a commented-signature shadow, forged reports after source-report removal, and
a live `constant` declaration. One authoritative digest covers the complete
comment/string-stripped live source, including commands between declarations
and every proof body, so later notation/macro/syntax/scope/attribute/instance
changes cannot silently alter elaboration. Fifteen narrower region pins bind the
headline statements to `Expr`, `Step`, the complete `FaultRedex` and
`ArgsProgress` relations, `Steps`, `Terminal`, `NormalizesWithin`, and their
semantic dependencies with more precise diagnostics. Further hostile self-tests
inject a zero-cost `Steps.teleport`, a universal `FaultRedex` constructor, and
a local notation rebinding later `FaultRedex` occurrences to `True`; all must
be rejected before any Lean build is trusted.

This is a call-graph termination property of the Lean calculus. It aligns
with the Rust reifier's active-path recursion refusal but does not prove
that the Rust traversal produces a Lean rank certificate. A later tranche
proves full-language Progress and fuel-bounded iteration over the real `Step`
relation: for every supplied fuel, a closed well-typed term reaches a
value/fault within budget or consumes the exact budget and has a witnessed
next step. The structural tranche proves value substitution preserves exact call
targets and raw node count, classifies every real step as either a strict node
decrease or a contextual call beta, and proves every classified beta exposes
an actual substituted callee body whose calls have strictly lower rank. The
weighted tranche adds a separately checked `WeightedCallCertificate`: each
function weight dominates its body potential, calls pay their callee weight,
value substitution preserves potential, and every real step strictly
decreases it. Lean therefore proves finite small-step normalization to a value
or modeled fault for every closed well-typed term under that certificate, with
the helper-call fixture as a non-vacuous positive case. The private corpus
harness described below now derives weights and bounded typing proof terms from
real reified HIR, then emits concrete Lean certificates and a complete
well-formed table for each finite fixture; the general theorem does not prove
either derivation correct for arbitrary HIR. The numeric extension below bounds
real small steps by the initial certified potential. Universal source-to-Lean correspondence,
resource-limit equivalence, hosted execution, and every self-hosting rung
above zero remain separate work. See
[Kernel-0 proof mechanization](KERNEL-PROOF-MECHANIZATION-V1.md#ranked-call-graph-extension-issue-188)
for the exact proof and gate scope.

### Named identity lowering inside the Lean model

The next #188 tranche makes one previously implicit representation boundary
executable and proved: the Rust reified `Term` carries `ValueId` and
`DeclarationId` identities, while Lean `Expr` carries de Bruijn variable indices
and function-table indices. `NamedTerm` independently models every Kernel-0
constructor with opaque natural-number identities; `lowerNamed` resolves those
identities into the existing `Expr` calculus. Operators are already classified
as arithmetic, comparison, or lazy boolean operators at this boundary.

`NamedHasType` uses an association-list variable context and an explicit
function-identity list naming the slots of the target signature table. Its
variable lookup is independent of target index lookup. Lean proves that the
two lookups agree, that every named typing derivation produces a successful
lowering with the same target type, and that the executable lowering's exact
returned expression has that type. The mutual argument-list theorem preserves
types and authored order. Closed named terms therefore inherit the existing
value/step/fault Progress theorem; under the existing well-formed-program and
weighted-call hypotheses, their actual lowered expressions also normalize.

The executable fixtures cover nested shadowing, an initializer that must still
see the outer scope, an outer variable beneath a distinct binding, unknown
callee refusal, and nontrivial argument ordering. A named helper entry resolves
to the established two-step `42` fixture, with target typing and both real
`Step` witnesses. The fourth compiled negative control forges the outer
variable's index as zero instead of one. The gate requires exactly that type
mismatch; missing imports, unrelated errors, or unexpected success fail it.
Another source-level hostile check injects the new binding into its own
initializer scope and requires the named-lowering semantic pin to reject it.
The axiom audit now also accepts Lean's genuine empty-axiom report format,
while rejecting duplicate reports across both formats.

This is a type-preserving identity-to-index translation **inside Lean**, not a
proof that Rust's `Term`, `ValueId`, `DeclarationId`, HIR, parser, operator
classification, or source bytes match this model. Lowering resolves identities;
it is not itself a type checker. The theorem's named typing premise is
substantive. The function table and its identity list are supplied explicitly;
their construction from Rust functions, uniqueness admission, and body
translation remain outside this theorem. Likewise, no independent named-term
evaluation relation or general observational-equivalence theorem is claimed.
`Int` retains the existing Lean model's literal range rather than imposing a
new Rust `i64` admission proof. No completion-matrix or self-hosting rung changes.

### Executable weights from the reified corpus (issue #188)

The private proof harness now computes function weights directly from the
`KernelProgram` returned by exact-source `BoundTranslation` replay.
`src/kernel_zero/weights.rs` walks the complete syntactic call closure, memoizes
each finished function, and assigns `weight(f) = potential(body(f)) + 1`.
Its potential uses the existing Lean equation: values and variables cost one,
ordinary constructors add one to their child potentials, and a call adds its
argument potentials to the callee's weight. Lazy branches and calls nested
inside arguments participate even when they would not execute. Stable identity
orders the calculation independently of the supplied function-table order.

The derivation refuses duplicate or unknown function identities, active-path
cycles, more than 64 functions, more than 8,192 charged nodes, traversal depth
128, and checked `u64` overflow. These are private proof-harness refusals,
not compiler diagnostics or language-admission changes. An acyclic program can
have an exponentially large mathematical weight; refusing an unrepresentable
weight does not mean the program fails to terminate. A separate replay checks
the exact function inventory and every strict body-potential inequality using
the supplied weights, without replacing an invalid certificate by a new one.

`src/kernel_zero/lean_fixture.rs` extends the existing real-corpus lowering
witness with the actual parameter types, result types, lowered function bodies,
and derived weights. For each corpus program it emits a complete
`WeightedCallCertificate` proof by exhaustion of the finite function table.
Lean's own `weightedPotential` reduction checks each inequality. The witness
therefore links certificate arithmetic to the same concrete lowered terms
whose identity-to-index translation is independently checked by `lowerNamed`.
The deterministic-rendering regression runs even without Lean; the existing
`real_reified_corpus_lowers_identically_in_lean` selector checks the generated
proofs when its Lean toolchain is available.

This is executable finite-corpus evidence, not a general theorem about the Rust
derivation, HIR typing, or source translation. A weight certificate alone does
not discharge `WellFormedProgram` or `HasType`; the bounded fixture producer
now discharges those separately for the exact table and bodies it renders. It
does not authorize execution or establish a concrete runtime or
interpreter-fuel bound. No proof axiom, language feature, public format, or
self-hosting rung is added. The implementation sessions ran the focused weight
and typing tests through small standalone Rust harnesses using the actual term,
weight, and renderer modules. An operator-complete 17-function generated
fixture was accepted by Lean with the current numeric theorems overlaid because
the cached `Kernel0.olean` predated them. Full Cargo/corpus and a clean Lean
build remain unrun.

### Bounded typing witnesses for reified fixtures (issue #188)

`src/kernel_zero/lean_fixture/typing.rs` independently walks each reified named
term rather than trusting HIR type annotations or the resolver's result. It
checks variable scope, operator families, branch agreement, return types,
function identity uniqueness, exact call arity and ordered argument types, and
the complete function inventory. It refuses more than 64 functions, 8,192
parameters plus term nodes, or traversal depth 128. These are private harness
limits and refusals, not compiler diagnostics.

For every admitted body the producer renders a concrete `NamedHasType` proof,
uses `named_lower_output_has_type` to type the exact lowered expression, and
constructs `WellFormedProgram` by exhausting the actual function table. The
numeric fuel witnesses therefore no longer accept caller-supplied `HasType` or
`WellFormedProgram` premises. Lean remains the checker of every emitted proof
term; the Rust producer cannot make a malformed constructor application true.
This closes those assumptions only for the finite rendered corpus. It is not a
universal HIR correspondence theorem and does not promote a self-hosting rung.

### Exact graph-v10 correspondence for the finite corpus (issue #188)

`src/kernel_zero/graph_projection.rs` binds one exact source/reification pair
to the exact bytes and domain-separated digest of its existing
`semaprax.graph.v10` projection. Its private decoder does not call the graph
renderer. It independently checks every selected persistent function identity,
signature, exact result identity, complete expression and let structure,
scalar/value identity, authored child order, repeated call occurrence,
canonical call set, and the exact acyclic closure reached from the entry.
Duplicate JSON keys, unknown schemas, drifted bytes, extra semantic fields,
unstable identities, and bounded-capacity violations fail closed.

The gate covers all 65 generated and adversarial structural corpus programs
plus explicit-ID display rename, declaration reorder, and module-move cases.
That execution exposed and fixed a malformed extra closing brace in the
adversarial arithmetic fixture. This is finite executable correspondence
evidence, not a proof that arbitrary HIR and graph projections agree. Cleanup
plans and unselected graph metadata remain opaque but byte-bound, and the
checker introduces no public format, compiler admission rule, or authority.

### Mechanized stable-ID graph facts for the exact fixture (issue #188)

`proofs/kernel0-lean/GraphProjection.lean` now gives the selected persistent
function portion of that same finite graph check a small, hole-free Lean model.
Its `project` is driven by a supplied canonical stable-ID inventory, refuses a
missing ID, and emits only the stable ID and authored call-occurrence list.
Lean proves that every successful projection has exactly the requested stable
identity inventory and that arbitrary display-name/module-location rewrites
preserve the projection. The concrete theorem uses `app.main`, `math.adjust`,
and `math.pair`; it fixes `app.main`'s four ordered call occurrences.

`real_graph_projection_stable_ids_and_calls_match_the_lean_fixture` derives
those IDs and calls from the real exact-source graph projection, renders them
as a fresh Lean witness, and asks the pinned kernel to apply the concrete and
general theorems. A changed compiler fixture can therefore not silently reuse
the handwritten Lean facts. The fixture is deliberately bounded and only runs
Lean when its pinned executable is available.

This remains a theorem about the finite model and that one compiler-derived
fixture. It does not prove JSON parsing/rendering, duplicate-ID admission,
canonical ordering construction, HIR/source correspondence, graph-byte
binding, call acyclicity, cleanup semantics, or any universal stable-ID claim.
No public graph format, authority, self-hosting rung, or compiler admission
rule changes.

### Numeric normalization fuel from the checked weights

`steps_spend_weighted_potential` telescopes the already-proved strict decrease
over the actual `Steps` relation: after `n` steps, `n + potential(out)` is at
most `potential(entry)`. `normalizes_within_weighted_potential` combines that
inequality with full-language bounded progress. An unfinished frontier after
the entire initial potential would have potential zero and a next strictly
decreasing step, which is impossible over natural numbers. Thus a closed,
well-typed term in a well-formed program with a `WeightedCallCertificate`
reaches a value or modeled arithmetic fault within its initial potential.
The source/signature pins and gate-owned axiom audit include both theorems.

The private Rust `value_call_fuel` replays the complete weight certificate and
computes `weight(entry) + parameter_count` with checked arithmetic. This is
the exact Lean potential of a call whose arguments are already scalar values,
because each value has potential one. Unknown entries, invalid certificates,
and overflow are refused. The real-corpus witness emits a numeric theorem for
each function at a zero/false scalar call, checks the computed potential by
Lean reduction, and applies the general numeric theorem. Its bounded typing
producer now discharges program well-formedness and call typing for each exact
rendered fixture; it still does not prove the Rust HIR type translation
universally.

The implementation session checked the two new proofs and a helper-call
application in a small Lean module importing the existing compiled kernel;
their reported axioms were subsets of the gate's existing allowed set. Eight
weight tests passed in a standalone Rust harness importing the actual term,
weight, and witness modules. Source pins and hostile gate self-tests passed.
The complete rebuilt proof and real HIR corpus witness were not run in this
session. This is a bound on modeled small steps, not compiler interpreter
steps, machine instructions, elapsed time, memory, or a new self-hosting claim.

### Concrete typed normalization traces from exact source

`src/kernel_zero/lean_fixture/normalization.rs` narrows one remaining bridge
between the numeric theorem and real reified inputs. A private
`BoundNormalizer` replays the exact-source translation, the independently
derived named typing proof terms, and the complete weighted-call certificate.
For one exact, type-checked scalar argument vector it then starts from the
literal `Call(entry, values)` term and executes the Kernel-0 small-step rules:
left-to-right strict operands and call arguments, lazy `&&`/`||` and `if`,
capture-avoiding scalar substitution for `let` and calls, and the existing
eight arithmetic fault terminals. After every real reduction it independently
rechecks the resulting closed term's type and recomputes the certified
potential; a non-decrease or a trace longer than the initial fuel refuses.
Replay re-derives the complete normalizer and outcome from the exact source,
entry identity, and arguments rather than trusting caller-authored trace data.

The focused corpus route covers the 60 seeded programs, nine exhaustive fault-
selection programs, and five structural/certificate programs: 74 programs and
750 concrete invocations. A lightweight standalone Rust harness against the
cached compiler library completed all 750 witnesses: 219 values, 531 modeled
faults, and a longest trace of 97 reductions. The committed non-vacuity and
hostile controls require both value and fault traces, retain the 40-edge deep
call case, and refuse source/entry drift, wrong argument count/type, and a
forged under- or overweight certificate. The same harness structurally compiled the new
module, while a separate no-Cargo `rustc` harness structurally compiled its
test bodies; `rustfmt` and diff checks passed. Cargo, a rebuilt compiler
library, the Lean build, and the full quality profile did not run in this
tranche.

This checker deliberately shares the reference evaluator's primitive unary
and binary arithmetic functions, so it is not a second outcome oracle and does
not add backend differential evidence. It checks the concrete transition,
typing, and potential-descent obligations that the prior Rust certificate
producer did not. It is finite local executable evidence, not a universal
Rust-to-Lean step-correspondence proof, not a proof of the checker itself, not
an external proof-kernel judgment, and not progress beyond self-hosting rung 1.

## Differential testing: reference interpreter vs. the compiler's interpreter

**What ran.** `src/kernel_zero/differential.rs`'s single test,
`reference_interpreter_agrees_with_the_compiler_over_the_kernel_zero_corpus`,
runs every case below through two independent paths and asserts the two
outcomes are identical:

1. **Reference side:** derive and replay `reify::BoundTranslation` from the
   case's exact `.spx` source and selected stable entry ID (fresh parse,
   resolution, bounded profile admission, and HIR validation), then evaluate
   the replayed term with `eval::eval_program`. This path shares no evaluation code
   with `src/interpreter.rs`: no function, type, or constant from that module
   is imported anywhere under `src/kernel_zero/`.
2. **Compiler side:** the same source, written to a temp file, run through
   `interpreter::interpret` -- the same public entry point
   `tests/interpreter_v1.rs`'s own golden-envelope tests call -- never by
   constructing an `Evaluator` or calling `evaluate`/`combine` directly.

**Corpus.** 14 hand-written edge cases (`i64` add/sub/mul overflow,
division/remainder by zero, `i64::MIN` divided/remaindered by `-1`, negating
`i64::MIN`, an untaken `if` branch that would fault if evaluated, `false &&
<would-fault>`, `true || <would-fault>`, `bool == bool` over all four
argument combinations, a three-level non-recursive call chain with a
non-commutative operator to make left-to-right argument evaluation
observable, and the nested `let`/`if`/call/arithmetic shape "Rung 0
evidence" above hand-ran) plus 60 generated programs from a deterministic
xorshift64 PRNG seeded with the fixed constant `0x4b65726e656c3021`
(`src/kernel_zero/corpus.rs::CORPUS_SEED`; ASCII `"Kernel0!"`), each 1-4
helper functions plus one entry function combining `if`/`let`/arithmetic/
comparison/`&&`/`||`/non-recursive calls, sampled at up to four concrete
argument tuples biased toward `0`, `1`, `-1`, `i64::MAX`, and `i64::MIN`. 197
concrete-argument comparisons total.

**Result, this session: zero disagreements.** Every one of the 197
comparisons produced an identical outcome (the same returned `i64`/`bool`
value, or the same one of the eight checked-arithmetic fault codes) on both
sides. This is evidence over one finite, seeded corpus that the interpreter
backend's checked-arithmetic, short-circuit, evaluation-order, and call
semantics agree with Kernel-0's operational semantics (as extended below) on
every case this corpus reached -- it is not a proof that no disagreement
exists outside this corpus. The native and Wasm backends are covered
separately below, in "Extension to native C11 and Core Wasm."

### Extension to native C11 and Core Wasm (a later session, issue #188)

**What ran.** `src/kernel_zero/differential/cross_backend.rs`'s
`native_c11_and_core_wasm_agree_with_the_kernel_zero_reference_interpreter_over_the_corpus`
runs the identical corpus above (same source text and same seed; now 100 cases
and 782 target comparisons after the strengthening sections below grew it from the
74-case, 197-comparison corpus this section originally measured)
through two additional paths per case, each compared
against the from-scratch reference interpreter side above (not against the
compiler's interpreter a second time, since the test above already
establishes that agreement):

1. **Native C11:** `crate::codegen::emit_hir_c` on the case's resolved
   program, plus one small generated `main` (compiled with
   `-DSPX_NO_ENTRY_WRAPPER` so it replaces the emitted decoy entry wrapper)
   that calls the entry function's raw `spx_decl_<hex(id)>` symbol directly,
   once per sample argument tuple, and prints either the returned value or
   the resolved `spx_normalized_status`'s exact `domain_id`/`code`. Compiled
   with the local `clang` at both `-O0` and `-O2`.
2. **Core Wasm:** `crate::wasm::build_web_with_scalar_exports` (Public
   Scalar Export Profile v1, the same profile
   `tests/wasm/scalar_exports_v1.rs` proves Node-executable) exporting only
   the entry function, then one Node script that calls the generated
   `semaprax.bindings.js` runtime's `call(id, ...args)` once per sample and
   prints either the returned value or `status.domain_id`/`status.code`.

Both paths reuse the `spx_decl_<hex(id)>` raw-symbol convention
`tests/scalar_status_backend_equivalence.rs` and
`tests/wasm/scalar_exports_v1.rs` already rely on externally, and both
compile/build once per corpus program (not once per sample) to keep the
100-program corpus's wall-clock cost bounded.

**Result: zero disagreements.** 591 comparisons (197 samples x native `-O0`
x native `-O2` x Core Wasm), every one an identical outcome against the
reference interpreter -- the same returned `i64`/`bool` value, or the same
`semaprax.arithmetic.v1` domain and code. This is evidence over the same
one finite, seeded corpus that native C11 and Core Wasm compute the same
values, and reach the same checked-arithmetic fault, as the interpreter
backend already proved to agree with -- it is not a proof for any program
outside the corpus, and it says nothing about programs
`reifies_into_kernel_zero` rejects. Gated on a local `clang` and `node`
(skips with a printed notice when either is absent; set
`SEMAPRAX_REQUIRE_KERNEL_ZERO_CROSS_BACKEND=1` to make their absence a hard
failure instead, following `tests/scalar_status_backend_equivalence.rs`'s
own convention).

**Two gaps this exercise found, neither of them a compiler bug:**

1. **Kernel-0's stated operational semantics do not cover `i64` overflow or
   division/remainder by zero, so its Progress theorem is incomplete as
   literally written for those inputs.** "Operational semantics" states one
   total-looking rule, `n1 op n2 = n ("ordinary arithmetic")`, with no side
   condition. But `Div`/`Rem` are undefined in ordinary arithmetic at a zero
   divisor, and `Add`/`Sub`/`Mul`/`Div`/`Rem`/(unary) `Neg` are undefined on
   `i64` outside its representable range (`i64::MIN`/`-1` for `Div`/`Rem`
   specifically). A term such as `1 / 0` is well-typed under the stated
   typing rules (both operands are `i64`) but has no reduction rule that
   fires and is not a value -- exactly the stuck state "Progress" claims
   cannot happen. This was found by trying to write a reference evaluator
   directly against the stated rules and discovering `n1 op n2 = n` has no
   answer for these inputs; `src/interpreter.rs`'s existing checked-
   arithmetic handling was read afterward only to see how the real compiler
   already resolves the identical gap (permitted by this task's own
   instructions), not copied from -- the reference evaluator's `Fault`
   arithmetic is written from Rust's `checked_add`/`checked_sub`/
   `checked_mul`/`checked_div`/`checked_rem`/`checked_neg` directly against
   which `i64` operations are partial, and the two independently-derived
   resolutions turn out to name the same eight cases because there is
   essentially one sensible way to make `i64` arithmetic total via an
   explicit stuck/fault outcome, not because one copied the other.
   **Resolution recorded here, not silently folded into the proof above:**
   `src/kernel_zero/value.rs`'s `Fault` type and `src/kernel_zero/eval.rs`
   extend the calculus with a third outcome alongside "reduces to a value"
   and "diverges" (which Kernel-0's acyclic call graph already rules out):
   "reaches one of eight named stuck points," matching exactly the family of
   partial operations named above. The real compiler resolves the same gap
   the same way, independently: `src/interpreter.rs`'s checked arithmetic
   (`combine`, read-only, never imported) turns each of these into a
   `Flow::Failure`/`NormalizedStatus` outcome rather than panicking or
   wrapping, under the domain `semaprax.arithmetic.v1` with the eight codes
   `StatusCase::code()` assigns (`src/cleanup_plan.rs`) -- the differential
   test's 0-disagreement result is exactly the claim that the reference
   evaluator's from-scratch `Fault` resolution and the compiler's
   independently-existing checked-arithmetic resolution agree on which stuck
   points arise and in which order. The paper proof's Preservation/Progress
   theorems above are unaffected in spirit (every non-arithmetic rule is
   still total on well-typed terms) but are **not restated** here to cover
   the extended three-outcome calculus; doing so rigorously is future work,
   not claimed by this section.
2. **The former `bool == bool` / `bool != bool` specification and proof
   mismatch is closed.** The real language and `reifies_into_kernel_zero`
   admit these two boolean operations, while rejecting `bool < bool` with
   `SPX-T208`. The table above now states the equality-only boolean rule.
   `proofs/kernel0-lean/Kernel0.lean` matches it with a `BoolEquality`
   premise on `HasType.cmpBool` and `Step.cmpBoolVal`; the Progress and
   Preservation proofs both cover that constructor. Its four additional
   headline theorems prove the admitted equality case, the exact equality
   and inequality steps on representative values, and the rejected ordering
   case; the no-holes gate pins every statement and audits every axiom set.
   The `BoolEquality` premise is deliberate: it prevents the model from
   accidentally widening boolean comparison to ordering merely because its
   evaluator is total.
   The existing reference interpreter and differential corpus still exercise
   all four boolean equality outcomes independently; this Lean change does
   not promote finite differential evidence into a backend theorem.

### Corpus strengthened (a later session, issue #188)

12 new hand-written cases were added to `src/kernel_zero/differential.rs`'s
`hand_written_cases`, targeting boundaries the deterministic generator is
unlikely to hit by chance rather than growing it randomly: which fault is
observed (not merely that one is) when both operands of a binary
operator -- arithmetic, comparison, or `&&`/`||`'s own left operand -- would
fault differently, pinning left-to-right evaluation order at the
observable-outcome level; `i64::MIN * -1` (`mul`'s own boundary, distinct
from `div`/`mod`'s dedicated `i64::MIN`/`-1` case); the boundary that must
**not** fault (`i64::MIN / 1`, `i64::MAX % -1`); a helper parameter and a
caller's `let` reusing the identical name `x` across disjoint function
scopes; and two 20-deep generated-by-a-small-Rust-helper (not the corpus's
own seeded generator) programs, one nesting `let`s and one nesting `if`s
with every untaken branch a `1 / 0`. One of these surfaced a real admitted-
grammar boundary while under construction: an initial draft of the deep-nesting
`let` case reused one name at every level (literal shadowing) and the
resolver rejected it outright with twenty `SPX-T209` diagnostics -- SEMAPRAX
does not admit local shadowing at all, narrower than this task's own
suggested adversarial target, so the case was rewritten with fresh distinct
names per level instead, still nesting `let`s 20 deep. The corpus is now 26
hand-written plus 60 seeded and 9 adversarial generated programs (95 programs,
seed and determinism unchanged), with 753 interpreter-vs-reference comparisons
and **zero disagreements** run locally. The same route now selects 2,259
native-C11-plus-Core-Wasm-vs-reference comparisons (753 samples x native `-O0`
x native `-O2` x Core Wasm), but that expanded target run was **not executed in
this tranche**; the latest executed target evidence remains the earlier 627
comparisons with zero disagreements.

### Fault-selection corpus (a later session, issue #188)

The original seeded generator is intentionally biased toward hazardous values,
but random combinations are poor evidence for a more exact rule: a strict
context must publish the **left** checked-arithmetic fault when both operands
would fail, and a lazy context must avoid evaluating its inactive operand or
branch altogether. `src/kernel_zero/corpus.rs` now constructs nine additional,
byte-deterministic source modules around one selector covering all eight named
Kernel-0 arithmetic faults plus one successful sentinel. Six strict contexts -- arithmetic, comparison,
call-argument staging, `let` binding, and the left operands of `&&`/`||` --
each run all ordered 8-by-8 pairs. Two lazy-right modules run every fault
through `false &&` and `true ||`; a conditional module crosses every selected
and unselected fault through both branches, then pairs a successful selected
branch with each fault in the inactive branch. This adds 544 samples without
altering the fixed-seed generator: 64 samples in each strict context, 8 in
each lazy-right context, and 144 conditional samples.

The same `generated_cases` route feeds this corpus to the reference
interpreter, the ordinary compiler interpreter, native C11 at `-O0` and
`-O2`, and Core Wasm. A mismatch therefore records the exact source and input
tuple plus the distinct named fault, rather than accepting an undifferentiated
"failed" result. It remains finite local differential evidence, not a
termination, translation, or backend-correctness theorem; it does not close
issue #188.

### Structural and certificate corpus (a later session, issue #188)

Five byte-deterministic modules now cover shapes the fixed-seed expression
generator and exhaustive fault selector do not emphasize: non-faulting `i64`
extrema for every arithmetic family (including signed division/remainder
truncation), 48 nested fresh lexical bindings, a 40-edge acyclic call chain,
nested dead `if` branches containing distinct checked faults, and staged
three-argument calls under a boolean selector. They add 29 argument tuples,
so the current target is 100 programs and 782 samples.

`adversarial_structure_corpus_reifies_and_replays_weight_certificates` checks
these modules without executing a backend: every module parses and resolves,
reifies through exact-source `BoundTranslation`, replays the translation, and
derives, verifies, and obtains numeric call fuel from its concrete
weighted-call certificate. The same modules feed the Lean witness renderer.
The enlarged interpreter and native/Wasm targets select 782 and 2,346
comparisons respectively, but were not run in this tranche; the preceding
753/2,259 corpus target is still the latest executed evidence. Certificate
readiness is not a claim of backend execution.

## What is and is not mechanically checked

| Claim | Mechanically checked today? | Evidence |
|---|---|---|
| Kernel-0 syntax/typing/semantics are internally consistent (progress, preservation) | **No.** Paper proof only. | This document, "Paper safety proof" |
| A given `.spx` source text is admitted or rejected by the real toolchain, with a named diagnostic, at a named boundary | **Yes**, for the three ceilings below | `tests/cleanup_backends/kernel_boundary.rs`, `src/parser/depth/tests.rs` |
| A real `ResolvedFunction`'s HIR shape matches Kernel-0's grammar (the admission predicate itself) | **Yes.** | `src/kernel_zero.rs`, its `tests` submodule |
| A reifying function's real evaluation agrees with Kernel-0's operational semantics on it, for the **interpreter backend**, over a finite corpus | **Partially.** The current target is 782 comparisons across 100 deterministic programs (26 hand-written, 60 seeded, 9 fault-selection, 5 structural adversarial); latest executed prior-target evidence is 753 comparisons across 95 programs with 0 disagreements. Not exhaustive; a passing run is evidence, not a proof. | `src/kernel_zero/differential.rs`, "Differential testing" above |
| The same, for the **native or Wasm backends** | **Partially.** The latest executed target evidence is 627 comparisons over the pre-expansion 209 samples, with 0 disagreements. The current expanded gate is wired for 2,346 comparisons (782 samples x native `-O0` x native `-O2` x Core Wasm) but was not run in this tranche. Not exhaustive; a passing run is evidence, not a proof. | `src/kernel_zero/differential/cross_backend.rs`, "Differential testing" above |
| Interpreter, native, and Wasm backends agree on one concrete Kernel-0-shaped program's observable output | **Partially, for one hand-run example this session**, not as an automated, repeatable gate | "Rung 0 evidence" below |
| Deterministic stable-ID graph projection | **No new checking added by this session.** Existing coverage is in `tests/workspace/semantic_graph.rs` and `src/workspace_graph/*`; this document does not extend it. | out of scope this session |
| Semantic-transaction precondition/replay validity | **No new checking added by this session.** Existing coverage is in `src/semantic_workspace_*.rs`. | out of scope this session |

Acceptance criterion 1 from issue #188 ("a clearly bounded semantic kernel has
mechanically checked properties") is therefore **partially met**: the kernel
is clearly bounded (Kernel-0's grammar above), and its **admission boundary**
at three specific ceilings is mechanically checked by committed, passing
tests. Its **safety properties** (progress/preservation) are not mechanically
checked; they are a paper proof. Widening this table's second column is
future work, not claimed here.

## Three measured capacity ceilings

A kernel-sized `.spx` program is bounded by at least three independent,
already-diagnosed compiler ceilings, two tracked by issue #241 and one found
by this session while producing the evidence above. Each entry gives the
owning constant's exact source location, its value, and a minimal
reproduction; the two this session could reduce to a committed regression
fixture now have one.

### Ceiling 1 — `SPX-G171`, the Workspace Semantic Graph builder-bytes budget

- **Constant:** `MAX_BUILDER_BYTES: usize = 64 * 1024 * 1024` (`67_108_864`
  bytes) — `src/workspace_graph.rs:57`, surfaced via `limit_error("builder_bytes", ...)`
  in `src/workspace_graph/diagnostics.rs:33-38`.
- **What it charges:** not raw source bytes. It is an in-memory structural-
  cost accumulator (`src/workspace_graph/expected_projection/cost.rs`) over
  the whole reachable closure — the project's own source **and every
  dependency actually reached**, weighted by node-footprint and identity-slot
  multipliers, not by which packages are merely named in `[dependencies]`.
- **Empirical translation to source size (cited, not re-run this session):**
  `examples/catalog-normalizer-project/README.md` (commit `e513a504`,
  referenced by issue #241) records a **zero-dependency** project hitting this
  ceiling once its own hand-ported modules reach roughly **35–40 KB** of
  source — well past a single `std.*` package's own tolerance (~20–22 KB) but
  well under the ~10.5 KB `apex-supply-chain` multi-module example, meaning
  the admitted budget sits strictly between "a working shipped example" and
  "a modest batch-validation application."
- **Status:** measured and cited; not independently re-derived at full scale
  by this session (a faithful repro needs a multi-module scratch project,
  out of this session's budget); no new regression fixture added for it here.

### Ceiling 2 — `SPX-H006`, the cleanup-replay path/work budget

- **Constants:** `MAX_REPLAY_PATHS: usize = 65_536` and
  `MAX_REPLAY_WORK_UNITS: usize = 32_000_000` —
  `src/cleanup_plan/replay.rs:60-61`, enforced by
  `validate_replay_size_budget` in `src/cleanup_plan/replay/path_summary.rs`.
- **What it charges:** the number of distinct terminal control-flow paths
  (and a separate structural "work units" count) cleanup replay must
  independently enumerate for **one function**, checked both while resolving
  to HIR and again in the independent `hir::validate` replay.
- **Refinement of issue #241's characterization, reproduced this session:**
  #241 describes the trigger as "roughly ten sequential `if`/`else`
  classification branches." This session found that framing imprecise in a
  way worth recording precisely:
  - A **nested** nine-`else`-chain classifier (ten branches, exactly one of
    which executes per call — the shape #241 calls "a per-record status
    dispatcher") does **not** exhaust this budget by itself: terminal paths
    grow additively with branch count (`~11` paths for ten branches), far
    under `65_536`. Regression:
    `tests/cleanup_backends/kernel_boundary.rs::a_ten_branch_nested_classifier_replays_within_budget`.
  - A function summing `count` **independent** Copy-scalar
    `(if v < i { 1 } else { 0 })` terms with `+` produces one CFG path per
    combination of branch choices, so the terminal path count grows
    combinatorially (exponentially) in `count`, not additively — a useful
    lower-bound estimate is `2^count`, though the exact count `hir::validate`
    enumerates runs higher than that naive estimate because the cleanup CFG
    carries extra per-term bookkeeping paths beyond the two value branches.
    The exact, reproduced crossover: **`count = 14` replays within budget;
    `count = 15` fails with exactly** (this session measured and pinned the
    literal count, 98,300, rather than repeating the theoretical `2^15 =
    32,768` estimate, since the two do not match)
    `` cleanup plan for function `app.main` failed independent replay: cleanup replay found 98300 terminal control-flow paths, exceeding the 65536 path budget: path count multiplies combinatorially (2^N) when N branch outcomes are combined independently within one function, not additively with branch count, so splitting into smaller functions only helps if it removes that combination -- restructure the branches to be mutually exclusive (a single dispatch chain, at most one branch executed per call) or combine their results across separate calls instead `` (`SPX-H006`; a later session rewrote this message to name the combinatorial driver and an actionable remedy directly, since the previous wording -- "cleanup replay path bound exceeds the global path budget" -- named only the budget, not the cause, and the natural fix an author reaches for reading it (splitting into helper functions) does not help unless it breaks the combination).
    Regressions:
    `tests/cleanup_backends/kernel_boundary.rs::fourteen_independent_scalar_comparisons_replay_within_budget`,
    `...::fifteen_independent_scalar_comparisons_exceed_the_cleanup_replay_path_budget`,
    and (pinning the diagnostic's content, not only its code)
    `...::fifteen_independent_scalar_comparisons_diagnostic_names_the_combinatorial_driver_and_remedy`.
  - **Implication:** the real cost driver is *combinatorial path
    multiplication from mutually-independent branch results combined in the
    same function*, not branch count in isolation. A ten-branch classifier
    is safe alone; the same ten branches feeding independent, later-combined
    booleans elsewhere in a larger function is not, well before `2^65_536`-
    scale branch counts would suggest. This reframing does not by itself
    explain every failure #241 records in the full catalog-normalizer
    pipeline (that investigation combined this ceiling with `SPX-G171`
    across multiple modules), but it gives issue #241 an exact, minimal,
    from-scratch reproduction to build on, which its own report says it
    lacked ("bisection reproduction commands are not preserved").
- **Status:** independently reproduced this session from real source text
  (not a synthesized `CleanupPlan` mutation) and reduced to a committed,
  passing regression fixture at the exact boundary.

### Ceiling 3 — `SPX-P207`, the token-level nesting pre-check (found and fixed)

- **Constant:** `MAX_SOURCE_NESTING: usize = 128` —
  `src/parser/depth.rs:4`. This one constant backs **two independent
  checks** sharing one diagnostic code:
  1. `src/parser/depth.rs::validate_program` — a legitimate, per-function,
     AST-based nesting-depth walk (existing coverage:
     `src/parser/depth/tests.rs::contract_clauses_are_walked_as_nesting_roots`,
     `...::class_method_bodies_are_walked_as_nesting_roots`).
  2. `src/parser/entry.rs::reject_token_nesting` — a **token-level**
     pre-check that runs before any AST exists, at `Parser::new()`.
- **The finding, as it stood when this document was first written:**
  `reject_token_nesting` incremented one running `delimiters` counter for
  `LParen`, `LBrace`, `LBracket`, **and `Lt`** (`<`) tokens alike, decrementing
  it only for `RParen`, `RBrace`, `RBracket`, and a literal `Gt` (`>`) token,
  never reset between sibling top-level declarations. A file of **127**
  syntactically independent, individually shallow functions (each just
  `fn f_i(value: i64) -> i64 { if value < i { value } else { value } }`,
  per-function AST depth ~4-5, nowhere near 128) was rejected with
  `SPX-P207` purely from the file-wide count of unmatched `<` tokens, not
  from any real nesting; 126 such functions parsed, 127 did not. This was
  filed as [issue #247](https://github.com/wavect/semaprax/issues/247).
- **Status: fixed**, by a later commit in this same trust-reduction
  programme (`3e560d41`, "fix(parser): stop counting every `<` as unclosed
  nesting in SPX-P207"), closing issue #247. `reject_token_nesting` now
  tracks `<`/`>` in a separate tentative `generic_depth` counter that only
  accumulates while the token run since the last unmatched `<` still looks
  like a real generic-argument list, and resets the instant a token appears
  that no generic-argument list could contain (or an enclosing bracket
  closes first). Genuine nested generic brackets (`T<T<T<...>>>`) deep
  enough to exceed the budget are still refused with `SPX-P207` before the
  type parser ever runs
  (`src/parser/depth/tests.rs::token_level_precheck_still_refuses_genuinely_nested_generic_brackets`);
  127 independent flat comparisons in one file now parse
  (`...::many_unmatched_less_than_tokens_no_longer_exhaust_the_token_level_nesting_precheck`,
  the renamed and inverted former regression for the bug —
  legitimate per this repository's rules because it pinned a bug, not a
  contract). The heuristic is narrower, not perfect: many bare comparisons
  joined only by commas inside one unclosed bracketed list
  (`f(a < b, c < d, ...)`) can still over-count, which is documented in
  `reject_token_nesting`'s own doc comment rather than repeated here.
  Verified passing on the current tree as part of this issue's (#188) audit;
  see the "Rung 0 evidence" verification note below the gate ladder.
- **TCB-table correction:** the row above ("Lexer/parser") previously read
  "`SPX-P207`'s nesting pre-check is correct... it is not, in one specific
  way," describing this ceiling's own bug as still live. That was accurate
  the session this document was written and is stale now that #247 is
  closed; do not cite the old wording elsewhere.

## Self-hosting gate ladder

Per the issue's requirement that "no broader self-hosting or correctness
claim is inferred automatically," each rung below is independently
checkable and must be separately accepted; reaching one never implies the
next.

| Rung | Gate | Reached today? | Evidence |
|---|---|---|---|
| **0** | A Kernel-0-shaped `.spx` program is admitted by the unmodified toolchain and produces the *same* observable result on the interpreter, the native backend, and the Wasm backend, for at least one concrete program and input. | **Yes.** | "Rung 0 evidence" below. |
| **1** | A kernel-sized program (bounded by the ceilings above, not merely "small") implements a non-trivial pure computation (validation, classification, or similar) entirely within Kernel-0/Kernel-1's admitted shapes, still with cross-backend agreement. | **Yes.** | "Rung 1 evidence" below: a 35,669-byte, 32-policy classifier passed 72 independent-oracle/interpreter cases and 216 native/Core-Wasm comparisons. |
| **2** | A pure, total, pipeline-safe compiler component with no ownership/effects of its own — a canonical formatter or renderer fragment is the issue's own suggested first candidate — is implemented in SEMAPRAX, differential-tested byte-identical against the Rust implementation over a broad corpus, and its build is bootstrap-reproducible under a stated environment. | **No.** | The exact-source renderer byte lanes now have a local target/recovery gate: it executes retained C11 at `-O0`/`-O2` and the retained bounded private Wasm companion, then preserves Rust bytes on a candidate refusal/mismatch. Exact regeneration checks reproducibility; semantic target-oracle execution compares the retained targets against Rust. The checked owned handoff wraps Rust-assembled scalar output; Kernel-0 exposes byte lanes and Rust retains formatter authority. [Accepted-Revision Validation v1](KERNEL-ZERO-ACCEPTED-REVISION-VALIDATION-V1.md) owns the still-pending exact-revision receipts and reviewer decision. Rust remains authoritative; see “Rung 2 renderer integration evidence”. |
| **3** | A parser fragment is self-hosted with the same differential-equivalence and bootstrap-reproducibility bar as rung 2, plus documented fallback/recovery if the self-hosted path disagrees with the Rust reference. | **No.** | Not attempted. |
| **4** | Semantic projection (HIR → semantic graph) is self-hosted with the same bar, plus stable-ID and determinism regression coverage carried over unchanged. | **No.** | Not attempted. |
| **5** | The semantic-transaction validator (precondition checking, replay-before-commit, fail-closed staleness) is self-hosted with the same bar. | **No.** | Not attempted. |

**Today's rung: 1.** No claim above rung 1 is made anywhere in this document,
the completion matrix, or the commits accompanying it.

### Rung 2 renderer integration evidence

`src/kernel_zero/canonical_char_renderer.spx`,
`canonical_bool_renderer.spx`, `canonical_int_renderer.spx`,
`canonical_operator_renderer.spx`, and `canonical_string_renderer.spx` own
narrow pure Kernel-0 components for the production formatter's character,
boolean, signed-integer, closed binary/unary operator-token, and decoded-string-
scalar primitives. Their Rust siblings own bounded compiler-side boundaries.
Kernel-0 cannot own a string or byte buffer, so every component exposes the
smallest lossless interface available at this rung: `render_length(value)` and
`render_byte(value, index)`. Each Rust boundary parses, resolves, translates,
and independently replays its exact embedded source bytes before every complete
byte-lane evaluation, without calling the compiler interpreter.

This is no longer only an isolated finite candidate. The ordinary character,
boolean, integer, binary/unary operator, and decoded-string-scalar formatter
paths now enter the closed production adapter specified by
[Rung-2 Formatter Authority v1](KERNEL-ZERO-RUNG-TWO-AUTHORITY-V1.md). It
replays the embedded source before each candidate, uses one caller-owned
20-byte token (the exact `i64::MIN` exception to the otherwise at-most-12-byte
lanes), and selects candidate bytes only after complete equality with the
Rust reference. Refusal, drift, oversize, or mismatch preserves the
pointer-identical Rust fallback before copying it to the bounded output. The
[Owned Handoff v1](KERNEL-ZERO-RUNG-TWO-OWNED-HANDOFF-V1.md) boundary
authenticates the checked `own Bytes -> Bytes` wrapper before staging, observes
last-owner release and `CopyOutAndSettleBytes`, and requires that settlement
before publishing a candidate. Rust assembles the scalar byte lanes and retains
formatter authority. No target executable is invoked in production. Candidate evaluation has a
thread-local panic-safe reentrancy scope: any nested canonicalization needed
by exact-source replay is Rust-only, and only the outer ordinary formatter
enters a lane. An active bounded-output scope also bypasses evidence before it
can spend the caller's formatter budget, leaving that caller Rust-authoritative.
Deterministic normal-formatter counters pin a pass through all five lanes; the
existing test-only shadows
continue to make independent disagreement observations. Broad scalar corpora
separately compare character and string-scalar components against independent
oracles for named escapes,
printable ASCII, lowercase variable-width `\\u{...}` escapes, and direct UTF-8;
a fixed literal byte oracle covers both boolean spellings, while independent
decimal and closed opcode-oracle tables cover signed-integer extrema and all
13 binary plus two unary token spellings. A wrong delimiter, spelling, hex
case, digit order, token byte, or length is therefore observable at the actual
formatter boundary.

It does **not** reach rung 2. Rust remains the only authoritative formatter;
production formatting depends only on its Rust reference bytes, never on a
candidate result. The byte-lane API is not a SEMAPRAX owned-buffer renderer,
and the finite corpus is not a universal equivalence proof. The private
[Rung-2 Bootstrap Artifact v2](KERNEL-ZERO-RUNG-TWO-BOOTSTRAP-V2.md) now
retains source-bound canonical term, generated C11-source, raw Core-Wasm, and
private executable scalar-export Core-Wasm payload bytes for all five lanes;
two local derivations must be byte-identical and an independent bounded decoder
exactly regenerates every retained payload. It is
only local compiler-output evidence: it carries no target-execution or hosted
claim, does not supply a public component wrapper, and does not make the
runtime-derived Kernel-0 program authoritative. The local
target/recovery gate is specified in
[Rung-2 Target and Recovery Evidence v1](KERNEL-ZERO-RUNG-TWO-TARGET-RECOVERY-V1.md):
its checks execute retained C11 at `-O0`/`-O2` and the retained private
scalar-export Wasm companion through a semantic target oracle against Rust,
then model candidate refusal/mismatch by preserving Rust bytes. Exact
regeneration is reproducibility authentication, not an independent semantic
check: a deterministic self-consistent bad producer is caught only by executed
Rust-oracle comparison. It is not a production fallback route or an authority
transfer. The existing checked owned handoff does not add an owned renderer to Kernel-0.
Exact-revision acceptance and an explicit rung decision remain independent
prerequisites before authority moves.

### Rung 0 evidence

Built and executed this session (scratch project, not committed — the
project itself is disposable scaffolding; the language behavior it exercises
is now covered by the committed regression tests above) using the ordinary,
unmodified CLI:

- Source: a two-function module — `add(left: i64, right: i64) -> i64` and a
  ten-branch nested `classify(value: i64) -> i64` matching the Kernel-0
  grammar and the "control fixture" from ceiling 2 above — composed as
  `main() -> i64 { add(19, 23) + classify(42) }`.
- `semaprax check <project>` → `{"status":"verified", ...}`.
- `semaprax run <project>` (interpreter) → `47`.
- `semaprax build <project> --target native` → built executable; executing
  it prints `47` and exits `0`.
- `semaprax build <project> --target wasm` → built Wasm package; loaded via
  its generated `semaprax.bindings.js` under Node and invoked directly:
  `add(19n, 23n)` → `42n`, `classify(42n)` → `5n` (`42 + 5 = 47`, agreeing
  with the interpreter and native results).

This is real, first-party, this-session evidence — not a fixture drawn from
an existing shipped example — that a Kernel-0-shaped program is admitted and
agrees across all three backends the completion matrix claims. It is
deliberately small (two functions, one call each), consistent with rung 0's
narrow scope; it is not evidence for rung 1, which needs a program actually
near the ceilings above, not comfortably under them.

### Rung 1 evidence

`src/kernel_zero/differential.rs` now constructs a pure, capacity-scale
multi-region access-policy classifier. Its 32 reachable regional variants use
different resource caps, privileged-operation roles, step-up thresholds, and
refusal codes; the classifier has seven typed inputs, shared helper
declarations, and a mutually exclusive dispatch chain. The source-size
assertion requires **30--35 KiB**, placing it at 75--100% of the documented
35--40 KiB zero-dependency ceiling without claiming to hit the separate
workspace-graph limit. It deliberately avoids independently combined branches,
which would exercise the unrelated cleanup-replay path bound rather than this
source-capacity question. Every declaration must reify into Kernel-0, and the
72-fixture known-answer corpus covers every regional policy on both an accepted
and a high-resource path plus priority and unknown-region boundaries.

`src/kernel_zero/differential/cross_backend.rs` owns the corresponding
`rung_one_capacity_classifier_agrees_across_native_o0_o2_and_core_wasm` gate.
It executes all 72 fixtures through native C11 at `-O0` and `-O2`
and through Core Wasm, comparing all 216 results with the independent reference
evaluator. Both focused gates passed locally: 72/72 independent-reference and
compiler-interpreter cases, then 216/216 native/Core-Wasm comparisons. This
reaches rung 1 without claiming a formatter, parser, compiler component, proof
of general backend equivalence, hosted evidence, or any evidence for rung 2.

## Immediate follow-ups (not done by this document)

In priority order, given the gaps this document names explicitly rather than
papering over:

1. **Implement the Kernel-0 reification predicate as an executable HIR
   walk: done** (`src/kernel_zero.rs`). **A from-scratch Kernel-0 reference
   interpreter plus a differential test against the compiler's interpreter
   backend over a generated corpus: done, this session** (see "Differential
   testing" above; `src/kernel_zero/{term,value,eval,reify,corpus,
   differential}.rs`), with the remaining document-level arithmetic-fault
   gap recorded rather than silently patched (Progress's incompleteness at
   `i64` overflow/division-by-zero). The `bool == bool`/`bool != bool`
   typing/evaluation mismatch found by that corpus is now modeled and
   mechanized. **The differential comparison now also covers native C11
   (`-O0`/`-O2`) and Core Wasm, done in a later session** (issue #188;
   `src/kernel_zero/differential/cross_backend.rs`): the same 74-program,
   197-comparison corpus run through `crate::codegen::emit_hir_c` compiled
   with the local `clang` and through
   `crate::wasm::build_web_with_scalar_exports`'s Public Scalar Export
   Profile v1 under Node, each outcome compared against the from-scratch
   reference interpreter -- 591 comparisons (197 samples x native `-O0` x
   native `-O2` x Core Wasm), zero disagreements (grown first to 86 programs
   and 209/627 comparisons by issue #188's corpus-strengthening pass, then to
   95 programs and 753 interpreter comparisons by the fault-selection corpus,
   then to a current 100-program/782-comparison target by the structural
   certificate corpus above). The expanded 2,346-comparison target gate is authored but unexecuted
   in this tranche. This is evidence over the
   same finite seeded corpus, not a proof, and it is silent on any program
   outside that corpus. **The admission predicate's own wiring question is
   now closed, not open:** "Reification: HIR to Kernel-0, and its unproved
   edge" above records the decision to leave it permanently unwired into any
   diagnostic path, with the reasoning. **Still open:** a **broader generated
   and adversarial corpus** beyond the current 60 seeded and bounded
   fault-selection cases (a passing differential test bounds the search that was
   actually done, not the space of possible disagreements), and a
   **restated paper proof** for the three-outcome (value/fault/divergence)
   calculus "Differential testing" above's first finding shows the original
   two-outcome (value/divergence) proof sketch does not, as literally
   written, cover.
2. **File issue #241's three required-evidence items** using this document's
   exact numbers: a boundary regression fixture for `SPX-H006` (now added,
   here, ahead of that issue landing it independently — coordination needed
   to avoid duplicate fixtures), the same for `SPX-G171` at realistic
   multi-module scale. Done for the third item: the `SPX-P207` token-counting
   finding was filed separately as
   [issue #247](https://github.com/wavect/semaprax/issues/247) (it was not
   previously known and is not one of #241's two named ceilings) and has
   since been fixed and closed — see "Ceiling 3" above.
3. **Decide, per ceiling, whether to raise the budget, make it incremental,
   or document it as a permanent limit** in the completion matrix and
   roadmap. A later session (issue #241) made this decision for both named
   ceilings with the evidence available: neither budget is raised, because no
   session has produced evidence that a higher `MAX_BUILDER_BYTES` or
   `MAX_REPLAY_PATHS` keeps the workspace graph and the semantic cache finite
   at the new value. Instead, both are documented honestly in the [completion
   matrix](COMPLETION-MATRIX.md#compiler-and-output-targets) with their exact
   constants, and the `SPX-H006` diagnostic was rewritten (message only, code
   unchanged) to name the actual cost driver — combinatorial multiplication of
   independently-combined branch outcomes, not raw branch count — and an
   actionable remedy, since making the diagnostic honest about the cause is
   itself a deliverable when raising the bound is not yet justified. Raising
   either bound, if ever justified, still needs the cross-backend
   near-boundary execution evidence this document's non-claims section notes
   is missing.
4. **Pick a proof engine** for the eventual mechanized version of the Kernel-0
   proof above (Lean 4, Coq, or Isabelle/HOL are the standard candidates for
   a small ML-like calculus's progress/preservation proof) and wire a CI gate
   that fails on any admitted axiom/`sorry`/`admit`, per the issue's
   implementation sequence step 5 — not attempted here; doing it honestly
   needs the reification work in item 1 first, or the mechanized proof would
   cover a calculus with no checked connection to the real compiler.
