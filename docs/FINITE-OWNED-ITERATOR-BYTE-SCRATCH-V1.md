# Finite owned iterator byte scratch v1

Status: focused local interpreter, native C11, strict Core Wasm and
import-free public generic Core Wasm checks passed.
Audience: compiler contributors, agents and reviewers.

This OPT741 profile adds iteration-local literal-sized `bytes_zeroed` owners to
one closed consuming `Vec<Bytes>` traversal. It does not increase the cumulative
32 allocation events or 2,097,152 payload bytes, refund drops, introduce a
recoverable allocator, or admit an allocating arbitrary `while`.

## Independent count authority

The authored `for own` is a direct statement in its function body block. Its
iterator is exactly `vec_into_iter<Bytes>(value)`. Before that statement, the
immutable owner is derived only from `vec_with_capacity<Bytes>(Nusize)` and
successful `vec_push<Bytes>` calls, including immutable aliases. The literal
capacity is at most32. `vec_push` does not grow: its ordinary capacity failure
precedes insertion. The upper bound is the successful chain's actual length,
not the physical Vec type ceiling or merely the reserved capacity. Parameters,
unknown/imported factories, reserve, replacement, branch-produced owners and
mutable aliases do not supply a bound.

Typed replay independently reconstructs the exact ordinary seed/Step slot,
complete Done/Yield matches, item/remainder field identities and mandatory
`iter_next` tail. Lexical continue additionally replays its typed owned iterator
latch. An attached cleanup plan, graph count or source spelling grants nothing.

Scratch constructors are direct immutable locals in the authored body block,
with a literal count at most131072. A scratch owner is used only through an exact
whole-owner `bytes_as_slice` directly consumed by `byte_len` or `byte_get`.
Views, owners, aggregates, returned scratch and argument transfers cannot escape
this domain. Nested loops and closures are excluded. Other ordinary operations
still pass their existing source, HIR, effects, loans and ownership checks;
ordinary/helper, method and indirect calls are excluded from this first profile.
Scalar operators and already admitted byte operations retain their full ordinary checks.

## Capacity and settlement

Iterator initialization is evaluated and charged once. The complete body's
allocation-event and conservative payload summaries are multiplied by the
successful length bound using checked arithmetic. Conditions and stdin cannot
allocate in the repeated domain. Ordinary call/contract summaries remain part
of that body; nested allocating loops and cycles fail closed. Allocation sites
remain unique structural identities while event totals count actual possible
repetitions. Zero iterations contribute no body allocation events. Each issued
zero-sized owner still counts as one event when its constructor executes.

The independent proof has a fixed256-frame cursor and65,536 work units; its
scratch, including saved child/entry cursor frames, is reserved at its actual
layout before proof use. Child visits retain authored order without block
prefix rescans. Constructor provenance is iterative, with explicit predecessor lookup charges
and the same total work bound. Existing source/HIR
admission bounds still apply. No test-derived trip count or cleanup back-edge
enumeration supplies authority.

Ordinary canonical cleanup settles iteration-local scratch before each normal
latch, continue, break or return; the existing provisional result and sticky
failure rules remain unchanged. Proof replay must reject forged transfer,
reservation, owner, latch or generation. Runtime allocation policy is unchanged:
interpreter counters remain cumulative; import-free Wasm tokens are monotonic
and never refunded; native physical allocation failure remains fail-stop.

## Projections and gates

Graph v78 adds `semaprax.finite-owned-iterator-scratch.v1`: exact execution,
condition/body identities, upper bound and allocation site identities. Source
workspace selection and typed graph emission derive the profile independently;
canonical graph replay refuses missing, stale, reordered or forged fields.
Frozen graph/from-parts/evidence routes refuse this profile. Legacy programs
keep their old graph/cache bytes. HIR/cache nodes are unchanged and every cache
consumer replays the new capacity proof from actual nodes before execution.
The authenticated cache lifecycle restored and refreshed a three-module
Project that imports a finite-scratch function. Both stages reused all three
checked HIR modules and ran full HIR validation. Cold open and rebuild each
resolved three modules and produced identical frontend work and revisions.
Direct codec controls retained exact bytes across decode and re-encode,
re-derived the same four-event, nine-byte bound, and refused a forged latch
after decoding.

The focused source/HIR and backend gates passed, including the original
`lexical_exits::iterator_resume::` eight-copy/eight-drop oracle and hostile
resume controls. The same unchanged two-continue-site source and its late
postcondition-failure variant now also run through native C11 at O0 and O2.
The native probe checks exact result or contract status, untouched result
storage on failure, zero live allocations and zero live Vec authorities over
four invocations per context. The interpreter and strict Core-Wasm checks
exercise those same sources. Native O0 and O2 also ran the original
normal/continue/break fixture and its all-abrupt control four times per context. The observer at
the actual `bytes_zeroed` constructor recorded seven payload allocations
and seven frees per invocation, totaling 22 requested bytes and peaking at
five live payloads. Both variants settled every tracked allocation and
vector handle. The separate seven-String assertion retains its own fixture.

A boundary program with sixteen empty input owners and sixteen 131,072-byte
scratch owners runs at exactly 32 allocation events and 2,097,152 payload
bytes. The interpreter, native C11 at O0 and O2, and strict Core Wasm return
the same count on repeated invocations. A zero-iteration control creates no
scratch owner. The Wasm host observes 32 allocations and 32 drops at the
exact limit, and zero of each in the empty case. Source validation refuses
one additional zero-byte allocation or one additional payload byte with
`SPX-T267`.

The early-return fixture leaves the current item, its scratch owner and the
untransferred suffix live at the return site. The interpreter, native C11
at O0 and O2, and strict Core Wasm check successful return, a false
postcondition, and division by zero in the return operand. Division by
zero keeps its arithmetic status even when the postcondition would fail.
The Wasm host observes three allocations and three drops per invocation.

The import-free public generic Core Wasm provider reserves 8,960 bytes for
32 rows of up to 32 Bytes carriers each. In addition to ordinary HIR replay,
the provider bounds table demand from the selected endpoint's exact reachable
closure. It charges all reachable literal Vec constructions, including
multiple calls to the same helper and constructors on different branches.
Recursion, unknown calls, loop-local Vec construction and unsupported Vec
operations fail closed. The table occupies the existing reserved space
between the byte token table and its payload arena.

Vec mutation, iterator conversion and Yield renew the handle before
publishing its successor. Yield transfers the existing Bytes carrier and
advances the authenticated cursor. Drop consumes the untransferred suffix
in element order. Done retires the iterator and writes an all-zero Step.
The provider preserves generations across invocations and checks row
settlement on both successful and failed calls before returning to the host.

Node v24.15.0 ran the import-free artifact with the existing public export
inventory. The tests use one provider instance across repeated calls and
inspect the actual byte token table for issued lengths and retired pointers.
They cover the original two-continue loop (eight issued and eight retired),
normal/continue/break (seven), and early return with the current item and
untransferred suffix live (three). The early-return cases include division
by zero in the return operand, a false postcondition and successful reentry.
The provider also checks a full Vec push after staging a new Bytes owner.
The refused push leaves the staged owner, the previous Vec and its contents,
and the outer Bytes owner for canonical cleanup. The probe observes all three
payload tokens retired and successfully re-enters the same instance. The
provider returns public status 11 for contract, arithmetic and Vec failures.
A returned-item case checks that the selected owner reaches the output
carrier while scratch and suffix tokens retire. The same ownership
transfer passes the interpreter, native O0/O2 and strict Core Wasm, including
a false postcondition that discards the provisional result. The exact limit
test observed sixteen empty owners and sixteen 131,072-byte scratch owners,
all retired before publishing the result. An empty iterator produced zero
allocation events.

The frozen native owned-data and owned-UTF8 provider emitters and native
stdin stream emitters refuse finite scratch with `SPX-B103`. The public
owned-data Wasm route keeps its existing `SPX-W115` refusal. The explicit
import-free public generic route uses the qualified table described above.

The accumulating 4096-row witness and outer Vec accumulation are outside
this profile. General quota/recovery work and selected String allocation
remain separate OPT724 obligations.
