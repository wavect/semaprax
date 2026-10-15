# Static borrowed UTF-8 literals v1 — design proposal

Status: design reference for [OPT #740](https://github.com/wavect/semaprax/issues/740).
An authored direct-source tranche is described in
[Static borrowed UTF-8 literals v1](STATIC-BORROWED-UTF8-LITERALS-V1.md);
qualification of both direct and retained Workspace/Project adoption remains pending.
Audience: compiler contributors, language designers, and coding agents planning a future batch.

This document records the original design for an explicit allocation-free
literal root. The owning implementation document distinguishes authored direct
routes from the authored retained Workspace/Project adapter. No public profile
or completion status is promoted by this design reference.
The [RFC 0001](RFC-0001.md), [completion matrix](COMPLETION-MATRIX.md),
[Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md),
[Owned String Borrowed View v1](OWNED-STRING-BORROWED-VIEW-V1.md), and
[Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) remain owners
of existing behavior. The proposed acceptance map below records future owners;
it does not add entries to executable regression registries before definitions
exist.

## Motivation and spelling

The issue records actual `SPX-T263` for treating an ordinary String as borrowed
text, `SPX-U105` for an allocating String construction in a renewing byte helper,
and `SPX-T252` for a literal byte array inside a bounded loop. These refusals
remain correct. Explicit owning views cannot make the originating String
allocation disappear. Numeric JSON-key arrays are a current workaround; their
source size is authoring evidence, not a measured task or runtime saving.

Recommend the adjacent prefix form `str"priority"`. It denotes an immutable
UTF-8 byte sequence with type `borrow str`, rather than an owning `string`.
Reuse the existing String token's decoding and canonical escaping. The parser
recognizes the adjacent contextual identifier `str` plus String token; an
ordinary identifier named `str` remains an identifier elsewhere. Canonical
formatting always emits the adjacent prefix. This introduces no implicit
conversion, interpolation, new escape language, byte-array literal, overload,
user function, or magic call name.

Proposed authoring, **not currently admitted**:

```semaprax
let key = str"priority";
next = writer_bytes(str_as_bytes(key), cursor, next);
```

An immutable local initialized directly by this constructor, plus existing
verified local aliases, is the initial use shape. Keep byte-view production on
named borrowed-text locals and ordinary synchronous borrowed-parameter calls;
do not broaden temporary-owner borrowing or general borrowed storage. Such
locals may be created in bounded loops because evaluation creates only a view
descriptor over existing module bytes. Ordinary `"priority"` still constructs
an owning String with its unchanged allocation, failure and cleanup behavior.

## Typed root and independent authority

Add a distinct AST and resolved-HIR leaf, provisionally
`BorrowedUtf8Literal`, with a boxed rare payload where that avoids increasing
common enum or binding layout. Its resolved type is Str, ownership Borrow, and
its decoded payload is exact immutable UTF-8. It is not a String Call, a
`BorrowPlace` over fabricated owned storage, or a new allocation operation.

`SourceExprStatic` is a **proposed** root class, not an existing authority token.
It identifies the actual literal producer by source module/declaration origin,
exact FunctionExecutionId and expression site, decoded bytes/length, and held
source revision. A local ValueId identifies only the local view descriptor.
The static producer does not acquire ownership authority from that ValueId,
an intrinsic-looking name, a graph schema, a digest alone, or an attached plan.
A byte view retains the producer and existing checked range derivation; offsets
and lengths use checked subtraction and semantic u64 extents.

Prefer an additive rare static-origin fact/payload beside the current local
view tables over silently treating a producer ExpressionId as an owning
ValueId. Define one representation before implementation: every consumer must
independently distinguish a local descriptor's identity from its literal root.
Both source projection and hostile-HIR validation reconstruct the constructor,
its exact site and owner, type/ownership, bytes and extent. Imported helpers use
the actual retained provider source and declaration origin. Transitive helper
proof inspects complete bodies, requires and ensures under its existing shared
depth/work limits; the new leaf is allowed only after ordinary static-root
validation. No signature-only, generated-name, stub, or attached-index shortcut
may authorize an allocating or unknown producer.

Source edits invalidate revision-scoped literal facts even if display names or
payload length remain equal. Generic instances bind the actual instance owner
and originating source site; a monomorphic proof cannot be transplanted.
Native constant-pool deduplication may share equal byte storage, but distinct
literal sites remain distinct proof identities. Stored graph/cache facts are
replayed against current source/HIR before use. Unchanged old source retains
its existing canonical bytes and selected versions.

## Lifetime, input classification and accounting

The immutable compiled module generation retains the literal backing until its
last executing frame finishes. A runtime view additionally expires with its
existing synchronous lexical/iteration scope. Module lifetime does not add a
language-level escaping or universally static reference type. Returning a
view, placing it in a record/Vec/Box/variant or Agent state, capturing it,
yielding it, or sending it through a callback/async/foreign-result boundary
remains closed. Initial admission need not extend the existing special
borrowed-parameter closure profile to literal-root locals.

Ordinary own-root views still have their exact shared loans and prevent owner
moves. A validated immutable literal root has no move-capable owner to freeze;
it creates no owning cleanup slot, owner epoch, or physical finalizer. The
absence of an ownership-conflict loan does not replace source/HIR nonescape
validation or waive ordinary loans on other arguments and the Bytes output.
Continue/break/return must still expire local descriptors on their actual
canonical contours; no module pointer is published at result commitment.

Literal roots are internal source constants. They contribute **zero foreign
invocation input bytes**, while their full physical storage contributes to
module/artifact and compiler retention accounting. Aliases, ranges and repeated
loop evaluation do not mint external roots. A real external parameter remains
foreign and subject to the existing 65,536-byte cumulative input limit even
when its address or contents match a compiled constant. Internal borrowed calls
substitute authenticated caller provenance; public entries cannot claim a
static root from host metadata. Do not add a runtime pointer-range heuristic.

Recommended initial bound: each decoded literal and the selected immutable
UTF-8 literal pool fit the existing 65,536-byte constant-data ceiling. On Core
Wasm the pool is shared with other UTF-8 literal data already using that fixed
segment; no additional memory page, enlarged arena or discounted owned literal
is introduced. Pin the same selected-pool rule in the new native/interpreter
profile. A program without this feature preserves its old admission and bytes.
Check decoded UTF-8 bytes, not source characters or escaped spelling. Empty
literals have length zero and a valid non-dereferenced empty representation.
Existing source-size, node/work, call-depth and compiler memory limits also
remain independent and unchanged.

Preflight pool size and real metadata backing before allocation. Account AST
and HIR payload capacities, boxed carriers, identities, literal-site tables,
cache buffers and the actual pool. Deduplication discounts require an actual
shared allocation/pool inventory; guessed per-node constants or digest-only
sharing cannot reduce retained charges. Old/new module generations retained
simultaneously each retain their real backing. No source-size cap increase is
part of this proposal.

## Backend contract and blast radius

| Owner | Required implementation and independent checks |
| --- | --- |
| `src/lexer.rs`, `src/parser.rs`, new parser child | Contextual adjacent prefix over the existing String token, exact spans, depth/work budget, malformed syntax and escape refusals. |
| `src/ast.rs`, AST visitors, `src/format.rs`, `src/format/literals.rs` | Separate leaf and deterministic prefixed projection; complete traversal and physical payload census; editor grammar updated without changing ordinary literals. |
| `src/hir/nodes.rs`, `resolve_expr.rs` and reference resolver, `resolve_expr/byte_view.rs`, `byte_slice_provenance.rs` | Exact Borrow Str producer, separate static origin, aliases/ranges and generic-instance provenance. No invented owned root. |
| `src/source_verify/declared_type.rs`, iterative verifier and independent oracle, `src/hir/validation/borrowed_str.rs` and `borrowed_argument.rs` | Independently authenticate source/HIR producers and scoped aliases; enforce nonescape and left-to-right argument staging. Loop rules and both nonallocating helper proofs recognize only the checked constructor. |
| `src/loop_calls/byte_writer/{source,resolved}.rs`, module indexes, cleanup/loan consumers | Complete closure traversal, current provider provenance, depth/work reservations and rebuild on revision drift; no owned-literal relaxation or fabricated cleanup root. Preserve unrelated loans and sticky failures. |
| `src/graph.rs`, graph profile child, `src/hir/cache_codec.rs`, workspace source projection and source call walkers | Selective version/tag and explicit literal origin/byte length; exact source/HIR replay, bounded cache decoding and stale/forged root refusal. Allocate graph/cache/prelude or target-profile versions during implementation wherever the new facts require them, preserving old tags and bytes. |
| `src/codegen/native_emit/literals.rs`, borrowed text/byte lowering and public entry guards | Immutable length-delimited constant data; reuse admitted Str carrier with authenticated internal origin. No String constructor, strlen, heap owner, allocator call or external-input debit at literal evaluation. Empty, NUL and Unicode bytes exact at O0/O2. |
| `src/interpreter.rs`, public argument/root adapters | Module-owned immutable backing established before evaluation; literal evaluation borrows the retained entry. Abstract Arc bookkeeping is not physical allocation evidence. External/public input classification stays separate. |
| `src/wasm/aggregate.rs` and new static-data child, profile admission and runtime adapters | Existing fixed guest constant segment and length-aware borrowed carrier; static origin propagated through internal calls. No per-evaluation owned-string mint, copy or heap import. Imported host parameters retain external admission. |
| `src/wasm/internal_strings/admission.rs` and opaque standalone profiles | Explicit refusal until an independently versioned static-view carrier is implemented; an arena-token String conversion cannot be called allocation-free. No broad claim from aggregate Core Wasm evidence. |
| `src/private_capacity_contract.rs`, retained workspace/loan/HIR walkers, interop-builder capacity readers | Charge actual carrier/payload/pool overlap and work; preserve closed public generator profiles until separately versioned. Join every extracted child into existing source-text gates. |
| Agent/native/CLI public boundaries and generation retention | Retain exact executing module image, prevent borrowed escape and stale cross-generation use, preserve status and publication rules; no new public borrowed-result ABI. |

Initial qualification must include the interpreter, native C11 at O0/O2 and
aggregate Core Wasm in the profiles actually claiming this form. Existing
opaque/frozen profiles must refuse it explicitly until their owning contracts
support it. Wasm linear memory is not physically read-only. Every admitted guest write and
host runtime writer must reject the constant segment as writable storage,
including forged owned-buffer headers and overlaps. A module export or runtime
adapter must not grant an otherwise closed raw-memory mutation path. Admission
of constant data is not permission to trust an arbitrary host pointer. This
write fence and the selected host boundary require hostile tests alongside the
static carrier; existing owned-write APIs keep their ordinary ownership checks.
No allocator-aborting native result proves Wasm carrier support.

## Proposed acceptance and hostile-case map

These are requirements for future definitions in existing harnesses, **not test
registrations or executed evidence**.

| Owning harness | Required positive and negative definitions |
| --- | --- |
| Language/IR | Prefixed canonical round-trip and graph/cache replay for empty, ASCII, Unicode, embedded NUL, quotes and escapes. Verify decoded lengths and exact prefix; ordinary owning literals and their old bytes remain unchanged. |
| Language/HIR | Named static Str and byte views inside/outside bounded loops; same-owner helper with actual key views and whole Bytes output; direct/imported/transitive helpers with body/requires/ensures. Preserve T263 for ordinary String-as-Str, U105 for allocating helpers and T252 for literal arrays in loops. |
| HIR/cache hostile | Change payload with unchanged extent; wrong site, declaration/module origin, instance or revision; fake type/ownership, root class, producer, extent or range; forged cached/attached facts. All independently refuse H006 before execution/publication. |
| Borrow/cleanup | Aliases retain exact root; a scoped view cannot return, enter an aggregate, capture, yield or cross a forbidden boundary. Literal arguments do not unlock moves of unrelated String/record/Bytes owners. Repeated aliases/calls neither debit nor refund foreign input budget. |
| Boundary/lifetime | Host input matching literal bytes/address still charges its distinct external parameter root. Selected module revision retained throughout execution; old views cannot enter a new revision or survive callback/settlement. Failure and reentry publish no borrowed pointer. |
| Capacity | Exact/+1 decoded-byte and combined pool bound; Unicode byte count rather than character count; exact/one-short metadata reservation, spare backing and old/new module overlap. Refuse before pool/table allocation and preserve source/plans. |
| Native settlement | Genuine generated JSON-key helper inside/outside caller loops; prepare the Bytes owner before measurement, then abort every attempted allocator call during key-view/helper execution at O0/O2. Independent byte oracle and allocation/copy hooks; no owned String substitute or fake logical allocation receipt. |
| Interpreter/Core Wasm | Same byte results, scopes, external-input classification, evaluation order, empty/max-index behavior and constant-pool refusal. Authenticate the actual Wasm static carrier rather than host String materialization. |
| Generated-code owning Project | Actual current codec derivation, ordinary source checking, graph/cache round-trip, full backend parity and stale generated-source refusal; preserve schema/key/field ordering and complete preflight. |

Only then measure generated source bytes and actual payload execution separately.
A smaller spelling, zero logical owner count, or one physical witness does not
establish accepted-task cost. Fresh matched cohorts remain necessary for token,
repair-turn or billing advantage claims. Construction/replay measurements for
[OPT #738](NONALLOCATING-BYTE-HELPER-RENEWAL-V1.md) remain a separate compiler
phase experiment; this proposal supplies no such measurement result.
