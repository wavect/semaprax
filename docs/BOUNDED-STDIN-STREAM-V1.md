# Bounded Standard Input Streaming v1

Status: additive compiler and provider contract under integration. The compiler
foundation and target implementations require the affected executable gates
before this profile is reported as implemented. This specification assigns the
identities left open by [the design draft](DRAFT-STDIN-STREAM-V1.md).

## Selection and authority

The profile is `language-command-io.stream.v1`, with input
`argv-utf8+stdin-stream.v1`. Project v23 selects this input explicitly; its first
product route is native. Project Wasm and npm routes refuse this profile until
their complete admitted backend and provider gates exist. Existing command,
line, network, filesystem, environment, and process profiles continue to refuse
Reader values and streaming operations. The snapshot `stdin_read()` contract,
including its 65,536-byte complete-input limit, remains unchanged.

The selected public command is an explicit stable-ID `fn () -> bool`. Its
invocation receives an injected input provider and one reusable 4096-byte buffer.
The compiler and generated module do not acquire a process descriptor or ambient
input authority. The standalone process adapter binds stdin only for this
explicit profile. The canonical command permit inventory remains
`process.args.read`, `process.stderr.write`, `process.stdin.read`, and
`process.stdout.write`; each function declares the effects it actually uses.
Streaming composes with argv inspection and existing single-transcript output
operations. It excludes snapshot stdin, append-output operations, and the other
host I/O families in this version.

## Closed source vocabulary

| Source operation | Identity | Signature | Effect |
| --- | --- | --- | --- |
| `stdin_stream_open` | `core.host.stdin-stream-open` | `() -> own StdinReader` | `process.stdin.read` |
| `stdin_stream_next` | `core.host.stdin-stream-next` | `(reader: own StdinReader) -> own StdinReader` | `process.stdin.read` |
| `stdin_stream_chunk` | `core.stdin-stream.chunk` | `(reader: borrow StdinReader) -> borrow Slice<u8>` | none |
| `stdin_stream_eof` | `core.stdin-stream.eof` | `(reader: borrow StdinReader) -> bool` | none |

The source return type spelling is `StdinReader`; ownership follows its unique
compiler-owned type facts. The reader identity is `core.stdin-stream.reader`,
with zero type arguments. Its facts are `copy=false`, `needs_drop=true`,
`sized=true`, `contains_resource=false`, and layout key `stdin-stream-reader.v1`.
Here `contains_resource` describes authored resource payloads; it does not make
the reader copyable or transportable. The empty record declaration is compiler
metadata. Source has no constructor, fields, patterns, projection, shared mode,
or explicit destructor for it. Its cleanup leaf is `core.stdin-stream.drop`.

Open synchronously prefills the buffer before publishing its Reader. A read of
1 through 4096 bytes publishes a chunk, including a short positive read. Zero
bytes is genuine EOF; empty input therefore opens at EOF. EOF is latched. Next
on an EOF reader returns that same owner without another provider read. Eof and
Chunk inspect one exact available named Reader and do not allocate or consume
it. An EOF chunk has length zero.

There is no cumulative streaming input-byte limit. Fixed input storage does not
remove ordinary application work/fuel bounds, checked arithmetic, output
bounds, or existing owned allocation quotas. Next reuses the one buffer rather
than creating another owned Bytes allocation for each iteration.

## Ownership, loans, and helpers

A source owner may renew only by exact same-owner assignment:
`reader = stdin_stream_next(reader)` or an authenticated forwarding helper call
with that same named owner. The right-hand argument stages first; successful
commit transfers the unique carrier and republishes it in the same binding.
This profile does not extend Vec reserve renewal.

Chunk uses the existing immutable SliceView loan, rooted in the Reader's exact
unprojected storage place. Byte ranges and aliases retain that root. A chunk
may be consumed by synchronous borrowed-Slice helpers. Any later use after a
refill, ownership transfer, or settlement is rejected by the source checker and
independently by HIR loan derivation. Ending the last use before renewal is
sufficient; a dead descriptor local is not an additional owned buffer. A copied
owned Bytes value is independent storage governed by the existing copy and
capacity rules. Chunk cannot escape through a result, aggregate, capture,
import ABI, or suspended continuation.

Owned forwarding helpers are monomorphic and synchronous: exactly one
`own StdinReader` parameter, a `StdinReader` result, and no effects other than
optional `process.stdin.read`. Their terminal expression returns that exact
parameter, its Next successor, or another authenticated forwarding helper's
successor. They may inspect a chunk before its loan ends. Borrowed Reader
helpers may inspect it without returning the view. Reader helper call graphs
are acyclic. This version excludes function-value/closure execution and agents
in streaming programs rather than guessing an indirect Open count.

The complete reachable call path may execute Open at most once. Open is refused
in every loop condition/body and any relevant call cycle. A function that
already receives a Reader cannot Open another. Dropping a Reader does not
restore Open permission. Branch alternatives use a maximum Open count; guard
prefixes and sequential expressions accumulate counts in authored order.

```semaprax
module app.stream_count;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("app.count")
fn count() -> bool uses { process.stdin.read } {
    let mut reader = stdin_stream_open();
    let mut total = 0usize;
    while !stdin_stream_eof(reader) {
        let ignored = {
            let chunk = stdin_stream_chunk(reader);
            total = total + byte_len(chunk);
            0
        };
        reader = stdin_stream_next(reader);
        0
    }
    total >= 0usize
}
@id("app.main") fn main() -> i64 { 0 }
```

## Failure and settlement

Open and Next normalize input read failure to
`semaprax.command-input.v1`, code 3. No Reader/result initializes on failed Open.
On failed Next, the staged Reader remains the caller's cleanup responsibility;
no successor publishes. Failure selection is sticky, ordinary canonical cleanup
settles the reader once, and no source result publishes on that path. Internal
scratch bytes need not be rolled back after invocation failure.

Malformed provider success, invalid length, stale token, or epoch overflow is a
target invariant failure, separate from the checked code-3 read error. Provider
Drop and final settlement are infallible cleanup operations. Settlement releases
invocation-owned buffer/token state without closing process-global stdin.

The streaming native private Slice carrier additionally records an epoch pointer
and captured epoch. Reader-backed views check the epoch before access, ranges
propagate it, and refill/drop invalidate it. Nonstream-backed slices use a null
epoch sentinel in this explicit profile. Old profile Slice layouts and artifacts
remain unchanged. Static loans remain mandatory; runtime checks carry no
admission authority. The JS provider similarly authenticates its private chunk
generation. No raw Reader token or stream Slice is a public language ABI.

## Semantic projections and proof boundaries

Programs using this vocabulary select Prelude v10 and Graph v65. Graph v65
preserves prior applicable graph facts and adds `stdin_stream`, schema
`semaprax.stdin-stream.v1`, containing the Reader/drop/profile/input identities,
4096 buffer bytes, one buffer, Open-prefill and true-EOF rules, status domain/code,
and each function's independently derived Open path bound, forwarding parameter,
and ordered Open/Next/Chunk/Eof expression identities. Chunk provenance adds the
`stdin_stream_reader` root kind with zero offset, symbolic current root length,
and the authenticated producer. It does not turn a view into owned data.

LoanPlan v1 and the existing cleanup-plan versions remain in force. Ordinary
leaf transfers, storage availability, and canonical finalizer vectors prove
renewal and settlement. The source scanner and HIR derivation independently
recompute singleton and forwarding facts; graph facts are evidence, not authority.
HIR replay rejects forged Reader constructors, roots, ownership, helper shapes,
or stale proof metadata before a backend runs.

Each stream analysis pass charges at most 1,000,000 node/call work units, in
addition to existing function, loan, graph, and cleanup limits. These work bounds
bound replay/materialization work; they are not a claimed peak-heap quota.
Existing byte-capacity summaries account one fixed 4096-byte Open allocation in
the input-site category; Next contributes no new input allocation. Existing
application copy sites and their quotas retain their ordinary meaning. Programs
without streaming vocabulary retain their previous prelude selection, graph
schema, and canonical artifacts.
