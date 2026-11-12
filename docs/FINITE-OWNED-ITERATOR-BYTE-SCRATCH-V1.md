# Finite owned iterator byte scratch v1

Status: focused local interpreter, native C11 and strict Core-Wasm checks
passed. Import-free provider qualification remains open.
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

The focused source/HIR and backend gates passed, including the original
`lexical_exits::iterator_resume::` eight-copy/eight-drop oracle and hostile
resume controls. Native O0 and O2 ran the original normal/continue/break
fixture and its all-abrupt control four times per context. The observer at
the actual `bytes_zeroed` constructor recorded seven payload allocations
and seven frees per invocation, totaling 22 requested bytes and peaking at
five live payloads. Both variants settled every tracked allocation and
vector handle. The separate seven-String assertion retains its own fixture.

The import-free public generic provider has no owned Bytes iterator runtime.
A probe routed the original loop through a reachable provider helper and
reached module emission. Node v24.15.0 rejected the emitted call signature
during validation. The provider now checks the reachable function inventory
for owned Bytes iterators before emission.

The accumulating 4096-row witness and outer Vec accumulation are outside
this profile. General quota/recovery work and selected String allocation
remain separate OPT724 obligations.
