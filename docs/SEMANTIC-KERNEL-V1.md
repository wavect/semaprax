# Semantic Kernel v1: trusted computing base, a proved kernel language, and the self-hosting gate ladder

- Status: proposed; trust-reduction programme opened, TCB inventoried, Kernel-0
  defined with a paper (not mechanized) type-safety sketch, three capacity
  ceilings measured with exact regression fixtures, and the self-hosting gate
  ladder defined with rung 0 reached and evidenced. No rung above 0 is
  reached. No proof in this document is machine-checked.
- Audience: compiler contributors, language designers, and any agent asked to
  extend, self-host, or formally verify part of SEMAPRAX.

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
proof sketch of its safety properties, an explicit and unproved
compiler-HIR-to-kernel-term translation relation, and a **self-hosting gate
ladder** with an honest statement of which rung is reached today: **rung 0**,
evidenced below, and no further.

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
- **No self-hosting milestone is reached.** Rung 0 of the ladder below is
  reached and evidenced; every higher rung, including the formatter
  self-hosting target the issue names as the first realistic candidate, is
  unreached and is recorded as a target, not a result.
- **The compiler-to-Kernel-0 translation is manual and unproved.** Section
  "Reification: HIR to Kernel-0, and its unproved edge" names the exact gap
  the issue's own failure-cases section warns about.
- **The `SPX-G171` byte figures below are not independently re-measured at
  full scale in this session.** The 18,874,368-byte `MAX_BUILDER_BYTES`
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
| Lexer/parser | `src/lexer.rs`, `src/parser.rs`, `src/parser/*` | Tokenization and grammar admission are correct; `SPX-P207`'s nesting pre-check is correct (see ceiling 3 below -- it is not, in one specific way) |
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
─────────────────────────────── (== != < <= > >=)   ─────────────────────── (&& ||)
Γ ⊢ e1 BinOp e2 : bool                               Γ ⊢ e1 BinOp e2 : bool

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

> A `ResolvedFunction` **reifies into Kernel-0** iff every parameter and its
> return type is `i64` or `bool`, its `uses`/effect set is empty, it has no
> `own`/`borrow` parameter or return mode, its body's expression tree uses
> only `Int`, `Bool`, `Binary`, `Unary` (on `i64`/`bool`), `If`, `Let`
> (scalar-typed), `Var`, and `Call` to other functions that themselves
> reify into Kernel-0, and the subgraph of reifying functions reachable from
> it by `Call` is acyclic.

This predicate is stated here, in this document, but **it is not implemented
as executable code against `ResolvedFunction` in this session**, and no test
enforces it. This is exactly the risk the issue names in its own "Failure and
security cases" section: *"the compiler-to-model translation can become the
unproved weak link."* Concretely:

- The three new regression tests added by this session
  (`tests/cleanup_backends/kernel_boundary.rs`,
  `src/parser/depth/tests.rs`) check **admission** (does the real toolchain
  accept or reject a given source text with a given diagnostic), which is
  necessary evidence but is not the same claim as "this admitted program's
  HIR reifies into the Kernel-0 term whose safety is proved above, and the
  compiler's interpreter/native/Wasm lowerings agree with Kernel-0's
  operational semantics on it." The latter needs the reification predicate
  above implemented as a real HIR walk plus a differential test against a
  from-scratch Kernel-0 reference interpreter, neither of which exists yet.
- Until that reifier exists and is itself tested, this document's Kernel-0
  proof is **necessary but not sufficient** evidence about the real
  compiler: it proves a term-rewriting system on paper is safe; it does not
  yet prove any specific compiler pass reduces real source to that system
  faithfully.

This gap is named, not closed, in this iteration. It is the highest-priority
follow-up this document identifies (see "Immediate follow-ups").

## What is and is not mechanically checked

| Claim | Mechanically checked today? | Evidence |
|---|---|---|
| Kernel-0 syntax/typing/semantics are internally consistent (progress, preservation) | **No.** Paper proof only. | This document, "Paper safety proof" |
| A given `.spx` source text is admitted or rejected by the real toolchain, with a named diagnostic, at a named boundary | **Yes**, for the three ceilings below | `tests/cleanup_backends/kernel_boundary.rs`, `src/parser/depth/tests.rs` |
| A real `ResolvedFunction` reifies into a Kernel-0 term | **No.** Predicate stated in prose only. | "Reification" above |
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

- **Constant:** `MAX_BUILDER_BYTES: usize = 18 * 1024 * 1024` (`18_874_368`
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
  `MAX_REPLAY_WORK_UNITS: usize = 8_000_000` —
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
    `(if v < i { 1 } else { 0 })` terms with `+` produces `2^count` terminal
    paths (every term's branch choice is independent of every other term's),
    and this **does** exhaust the budget, at a small, exact, reproduced
    crossover: **`count = 14` (16,384 paths) replays within budget;
    `count = 15` (32,768 paths) fails with exactly**
    `` cleanup plan for function `app.main` failed independent replay: cleanup replay path bound exceeds the global path budget `` (`SPX-H006`).
    Regressions:
    `tests/cleanup_backends/kernel_boundary.rs::fourteen_independent_scalar_comparisons_replay_within_budget`
    and
    `...::fifteen_independent_scalar_comparisons_exceed_the_cleanup_replay_path_budget`.
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

### Ceiling 3 — `SPX-P207`, the token-level nesting pre-check (new finding)

- **Constant:** `MAX_SOURCE_NESTING: usize = 128` —
  `src/parser/depth.rs:4`. This one constant backs **two independent
  checks** sharing one diagnostic code:
  1. `src/parser/depth.rs::validate_program` — a legitimate, per-function,
     AST-based nesting-depth walk (existing coverage:
     `src/parser/depth/tests.rs::contract_clauses_are_walked_as_nesting_roots`,
     `...::class_method_bodies_are_walked_as_nesting_roots`).
  2. `src/parser/entry.rs::reject_token_nesting` — a **token-level**
     pre-check that runs before any AST exists, at `Parser::new()`
     (`src/parser/entry.rs:44-77`).
- **The finding:** `reject_token_nesting` increments one running
  `delimiters` counter for `LParen`, `LBrace`, `LBracket`, **and `Lt`**
  (`<`) tokens alike, and only decrements it for `RParen`, `RBrace`,
  `RBracket`, and a literal `Gt` (`>`) token
  (`src/parser/entry.rs:46-56`). It does not distinguish a generic
  angle-bracket open (`Vec<...>`, matched by a later `>`) from an ordinary
  scalar less-than comparison (`x < 5`, matched by nothing, ever), and the
  counter is **never reset between sibling top-level declarations** — it
  runs once over the whole file's token stream. Consequently:
  - A file of **127 syntactically independent, individually shallow
    functions** (each just `fn f_i(value: i64) -> i64 { if value < i { value } else { value } }`,
    per-function AST depth ~4-5, nowhere near 128) is **rejected** with
    `SPX-P207` ("source nesting depth exceeds the admitted maximum (128)")
    purely from the file-wide count of unmatched `<` tokens, not from any
    real nesting. Reproduced exactly at the boundary this session:
    126 such functions parse; 127 do not.
  - The same shape with each `<` **immediately paired with a literal `>`**
    in the same condition (e.g. `value < i && value > -1_000_000`) parses
    fine at **200** functions and beyond — conclusively isolating "unmatched
    `<` count" as the trigger, independent of true nesting or declaration
    count.
  - Regressions committed this session:
    `src/parser/depth/tests.rs::many_unmatched_less_than_tokens_exhaust_the_token_level_nesting_precheck`
    and
    `...::the_same_less_than_count_parses_once_each_is_closed_by_a_literal_greater_than`.
- **Why this belongs in a kernel/ceiling report:** RFC 0001 states this bound
  as being about "expression trees" admitting "at most 128 nested
  constructs," and the diagnostic's own message says "nesting depth." Neither
  description matches what `reject_token_nesting` actually measures. Ordinary
  validation/classification code — exactly the kernel-sized shape this
  document's Rung 1 targets — leans on `<`/`>` comparisons far more than on
  real bracket nesting, so this is, in practice, the **cheapest of the three
  ceilings to hit by accident**: roughly 127 net unmatched `<` tokens
  anywhere in one file, an amount an ordinary range-checking module can reach
  with no deep nesting and no owned data at all. This is filed here as new,
  session-local evidence for issue #241's continued investigation, not as a
  fix — no source behavior is changed by this document or its tests.

## Self-hosting gate ladder

Per the issue's requirement that "no broader self-hosting or correctness
claim is inferred automatically," each rung below is independently
checkable and must be separately accepted; reaching one never implies the
next.

| Rung | Gate | Reached today? | Evidence |
|---|---|---|---|
| **0** | A Kernel-0-shaped `.spx` program is admitted by the unmodified toolchain and produces the *same* observable result on the interpreter, the native backend, and the Wasm backend, for at least one concrete program and input. | **Yes.** | "Rung 0 evidence" below. |
| **1** | A kernel-sized program (bounded by the ceilings above, not merely "small") implements a non-trivial pure computation (validation, classification, or similar) entirely within Kernel-0/Kernel-1's admitted shapes, still with cross-backend agreement. | **No.** Rung-0 evidence stays deliberately tiny; ceiling 3 above shows how little headroom ordinary comparison-heavy code has before hitting a ceiling this document did not expect going in. | This document's ceiling measurements are the negative evidence. |
| **2** | A pure, total, pipeline-safe compiler component with no ownership/effects of its own — a canonical formatter or renderer fragment is the issue's own suggested first candidate — is implemented in SEMAPRAX, differential-tested byte-identical against the Rust implementation over a broad corpus, and its build is bootstrap-reproducible under a stated environment. | **No.** | Not attempted this session. |
| **3** | A parser fragment is self-hosted with the same differential-equivalence and bootstrap-reproducibility bar as rung 2, plus documented fallback/recovery if the self-hosted path disagrees with the Rust reference. | **No.** | Not attempted. |
| **4** | Semantic projection (HIR → semantic graph) is self-hosted with the same bar, plus stable-ID and determinism regression coverage carried over unchanged. | **No.** | Not attempted. |
| **5** | The semantic-transaction validator (precondition checking, replay-before-commit, fail-closed staleness) is self-hosted with the same bar. | **No.** | Not attempted. |

**Today's rung: 0.** No claim above rung 0 is made anywhere in this document,
the completion matrix, or the commits accompanying it.

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

## Immediate follow-ups (not done by this document)

In priority order, given the gaps this document names explicitly rather than
papering over:

1. **Implement the Kernel-0 reification predicate** ("Reification" above) as
   an executable HIR walk, plus a from-scratch Kernel-0 reference
   interpreter, plus a differential test between that reference interpreter
   and the compiler's own interpreter/native/Wasm backends over a generated
   corpus of reifying programs. This closes this document's largest named
   gap: today's Kernel-0 proof is about a term-rewriting system on paper,
   not yet provably about any specific compiler pass.
2. **File issue #241's three required-evidence items** using this document's
   exact numbers: a boundary regression fixture for `SPX-H006` (now added,
   here, ahead of that issue landing it independently — coordination needed
   to avoid duplicate fixtures), the same for `SPX-G171` at realistic
   multi-module scale, and the `SPX-P207` token-counting finding as a new,
   separate report (it was not previously known and is not one of #241's
   two named ceilings).
3. **Decide, per ceiling, whether to raise the budget, make it incremental,
   or document it as a permanent limit** in the completion matrix and
   roadmap — explicitly out of this document's scope (both files are
   coordinator-owned in this session's assignment) but squarely #241's own
   "what this issue should produce" list.
4. **Pick a proof engine** for the eventual mechanized version of the Kernel-0
   proof above (Lean 4, Coq, or Isabelle/HOL are the standard candidates for
   a small ML-like calculus's progress/preservation proof) and wire a CI gate
   that fails on any admitted axiom/`sorry`/`admit`, per the issue's
   implementation sequence step 5 — not attempted here; doing it honestly
   needs the reification work in item 1 first, or the mechanized proof would
   cover a calculus with no checked connection to the real compiler.
