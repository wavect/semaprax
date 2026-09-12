# Streaming Proposal Decode v1

Status: **LOCAL** bounded implementation with an executable reference and
focused regression corpus, implemented in `src/streaming_proposal_decode.rs`.
This is issue #178 ("Derive streaming structured-output decoding from the
checked Proposal type"). No hosted evidence exists for this document; every
claim below is a local `cargo test` run against the fixture schema built in
`src/streaming_proposal_decode/tests.rs`.

Audience: implementers wiring a provider transport's chunked response bytes
into a decoded Proposal, and reviewers of the streaming-safety layer this
document adds around the existing whole-document decoder.

## Why a new module instead of extending `agent_interaction_schema`

`src/agent_interaction_schema/` derives `CompiledInteractionSchema` from one
checked source record or variant and decodes one complete `&[u8]` response
through `CompiledInteractionSchema::decode`. Its own module documentation is
explicit that this is deliberate: "Whole-value, not streaming... A streaming
transport can buffer provider output into one complete response before
calling this same boundary — this module needs nothing from a streaming
extension to exist." That module (and `src/live_invocation/`, which defines
the `ProposalDecoder` seam a real deployment binds a whole-document decoder
to) is leased to other issues' ownership for this round and is read-only
here regardless.

This document does not change either module's admission rules, wire format,
or public surface. It adds one new, independent module that depends only on
`agent_interaction_schema`'s already-public `CompiledInteractionSchema` type
and introduces a streaming-safety layer *around* the exact same `decode`
call: a bounded byte buffer, an incremental structural scanner that can
refuse a malformed or oversized prefix before the document completes, and
the explicit incomplete/invalid/accepted three-way state a chunked provider
transport needs. It is not wired into `live_invocation::kernel` or
`model_invoke::ProposalDecoder` — those files are this round's read-only
lease boundary; a later issue that owns that wiring can adapt
`ProposalStreamDecoder` behind that seam without this module changing.

## Why the streaming and whole-document decoders cannot disagree

`ProposalStreamDecoder` adds **no new admission rule** of its own for what a
Proposal document may contain. The only way it ever produces `Accepted` is
by calling `CompiledInteractionSchema::decode` on the complete buffered
bytes — the identical function, on the identical bytes, that the
whole-document path already calls and already tests
(`src/agent_interaction_schema/tests.rs`,
`src/agent_interaction_schema/live_bridge.rs`). There is no second,
hand-written grammar for unknown/duplicate/missing fields, variant tags,
integer bounds, or canonical-rendering equality to drift out of sync with
the checked type: every one of those rules is decided exactly once, by the
compiled decoder, regardless of which path (whole-document or streaming)
reached it.

The streaming layer's own incremental scanner (see below) only ever
*refuses earlier* than the whole-document path would — it never accepts
(i.e. reaches the terminal `decode` call on) a document shape that path
would reject, because every early-refusal rule it enforces is a strict
subset of a rule the whole-document decoder already requires of a complete
document:

| Streaming rule | Whole-document rule it is a subset of |
|---|---|
| Buffered bytes ≤ `MAX_STREAM_BYTES` (65536) | `source.len() > MAX_DOCUMENT_BYTES` (65536) is already refused |
| First byte must be `{` | A non-object top level already fails `value.as_object()` |
| No raw whitespace/control byte outside a string, other than the single terminal `\n` | Canonical rendering never emits whitespace; any inserted whitespace already fails the exact byte-for-byte canonical-replay check |
| Exactly one trailing `\n`, nothing after | `text.strip_suffix('\n')` plus "no other `\n`/`\r`" is already required |
| UTF-8 validity | Uses `std::str::from_utf8` — the identical stdlib check the whole decoder itself runs |
| String/escape/`\u`-hex well-formedness | Already required for the JSON parse to succeed |
| Container nesting ≤ `MAX_STREAM_DEPTH` (64, generous relative to the compiled schema's own 16-level `MAX_DEPTH`) | A legal document's worst-case bracket depth is well under 64 (2 JSON object opens per semantic nesting level, plus the envelope) |
| String-literal token count ≤ `MAX_STREAM_STRING_TOKENS` (8192) | A minimal string token costs 2 bytes, so this bound is unreachable by any document within `MAX_STREAM_BYTES` unless it is almost entirely empty-string filler |

This table is the "prove they agree, don't assert it" evidence: every early
refusal is provably conservative, and the accept path is the same function
call. `src/streaming_proposal_decode/tests.rs`'s
`streaming_outcome_agrees_with_the_whole_document_decoder_for_every_case`
exercises this directly — for a corpus of one valid document and eight
distinct hostile mutations (unknown field, missing field, duplicate field,
wrong scalar type, out-of-range integer, oversized text field, trailing
data, unknown variant tag), it decodes each with
`CompiledInteractionSchema::decode` directly and separately drives the same
bytes through `ProposalStreamDecoder` three ways (one chunk, one byte at a
time, split at the midpoint), asserting the streaming outcome always agrees
— `Accepted` only with the identical decoded value, `Refused` whenever and
only whenever the whole decoder itself refuses.

## The three-way outcome: incomplete, accepted, refused

`PushOutcome` has three variants, and a prefix can never be mistaken for a
complete value:

- **`Incomplete`** — valid so far, more bytes are required. Carries no code
  or message at all, so it cannot be confused with any refusal by
  inspecting its text (there is no text to inspect).
- **`Accepted(DecodedInteractionValue)`** — the buffered document decoded
  successfully. Only [`ProposalStreamDecoder::finish`] produces this, never
  `push` alone: reaching the document's terminal newline only stops `push`
  from refusing more input under this document, it does not yet authorize
  anything, because a later chunk could still turn out to carry trailing
  data after that newline. Only the caller's explicit "no more bytes are
  coming" signal (`finish`) can turn a syntactically-complete-so-far prefix
  into a genuine acceptance.
- **`Refused(StreamRefusal)`** — a closed, stable `code` plus caller-facing
  `message` and a best-effort `at_byte` offset. `push` can refuse a document
  before it ever completes (a structural violation, or a bound driven past
  its limit); `finish` can additionally refuse a document that reached the
  terminal newline but that the compiled decoder itself rejects
  (`STREAM-SEMANTIC`), or one that never reached the terminal newline at
  all (`STREAM-TRUNCATED`).

`src/streaming_proposal_decode/tests.rs`'s
`every_strict_prefix_is_incomplete_only_the_complete_document_is_accepted`
feeds every strict prefix of a valid document and asserts each is
`Incomplete`, with only the complete byte sequence (after `finish`) reaching
`Accepted`.

## The closed refusal vocabulary

| Code | Meaning |
|---|---|
| `STREAM-START` | The document did not open with `{`. |
| `STREAM-WHITESPACE` | A raw whitespace or control byte appeared outside a string, other than the single terminal newline. |
| `STREAM-STRING` | An unescaped control byte, unknown escape character, or invalid `\u` hex digit inside a string literal. |
| `STREAM-BRACKET` | A closing `}`/`]` had no matching open, or did not match the innermost open container's kind. |
| `STREAM-UTF8` | The buffered bytes are not valid UTF-8 (via `std::str::from_utf8`, the same check the whole decoder uses). |
| `STREAM-DEPTH` | Container nesting exceeded `MAX_STREAM_DEPTH`. |
| `STREAM-TOKENS` | String-literal token count exceeded `MAX_STREAM_STRING_TOKENS`. |
| `STREAM-BYTES` | Buffered byte count exceeded `MAX_STREAM_BYTES`. |
| `STREAM-TRAILING` | A byte arrived after the document's terminal newline, or the top-level value closed without one immediately following. |
| `STREAM-TRUNCATED` | `finish` was called before the document reached its terminal newline. |
| `STREAM-CANCELLED` | The caller explicitly cancelled the stream via `cancel`. |
| `STREAM-SEMANTIC` | The complete, syntactically well-formed document was rejected by `CompiledInteractionSchema::decode` itself; the message carries that diagnostic's own stable code and text (mirroring `live_bridge::SourceInteractionProposalDecoder`'s own refusal-reason convention), so a caller sees exactly which admission rule failed. |

Every code is distinct from every other, and none overlaps the vocabulary
`Incomplete` would need if it carried one (it does not carry a message at
all) — `incomplete_and_refused_are_never_textually_confusable` asserts both
the pairwise distinctness and the absence of "incomplete" text in any code.

## Bounds, each driven to its exact limit

No unbounded buffering: each bound below is checked as bytes arrive, not
only once a document completes, and each has a dedicated test that proves
the bound is enforced *at* the declared limit (not one before, not one
after) — `byte_bound_is_enforced_at_its_exact_limit`,
`depth_bound_is_enforced_at_its_exact_limit`, and
`string_token_bound_is_enforced_at_its_exact_limit` each construct a stream
that reaches the bound exactly (still `Incomplete`) and then push one more
unit past it (refused with the bound's own code). The byte bound is the
primary "no unbounded buffering" guarantee — the buffer literally never
grows past `MAX_STREAM_BYTES`, the bound is checked before appending a
chunk, not after — and it structurally caps the streaming scanner's total
work, since every scanner step is a single O(1) transition over at most
`MAX_STREAM_BYTES` bytes.

## Determinism across chunk boundaries

The single most valuable property this document proves: the same total
byte sequence produces the identical final outcome no matter how it is
split into chunks. `ProposalStreamDecoder` has no chunk-boundary-dependent
logic anywhere — every state transition (UTF-8 confirmation, container
depth, string/escape state, byte/token counters) is a per-byte decision
carried across `push` calls, never re-derived from where a chunk happened to
end. `identical_bytes_produce_identical_outcomes_regardless_of_chunk_boundaries`
drives one valid document, one document with a multi-byte-UTF-8 text value
(covering split UTF-8 continuation bytes), one document with an unknown
field (a semantic refusal), and one document with a corrupted UTF-8 byte
(a structural refusal) through every single-split-point chunking plus fully
one-byte-at-a-time chunking, asserting every chunking of a given byte
sequence produces the same outcome as feeding it in one piece.

## Cancellation and premature end of stream

`cancel(reason)` closes the stream early with `STREAM-CANCELLED`, but cannot
retroactively un-authorize an already-`Accepted` value — a stream that
already finished successfully returns that same `Accepted` value unchanged
if `cancel` is called afterward, matching this codebase's sticky
failure-selection discipline elsewhere
(`cancel_after_acceptance_cannot_retroactively_unauthorize_the_decoded_value`).
`finish` called before the document reaches its terminal newline refuses as
`STREAM-TRUNCATED` rather than leaving the caller waiting forever
(`finish_refuses_a_stream_that_never_reached_its_terminal_newline`). Once
any terminal outcome is reached (`Accepted` or any `Refused`), the decoder
is closed: further `push`/`cancel`/`finish` calls return that same stored
outcome (`a_refusal_is_sticky_across_further_pushes`).

## What this document does not claim

- **Not wired into `live_invocation` or `model_invoke::ProposalDecoder`.**
  Both are this round's read-only lease. A later issue that owns that
  wiring can drive provider chunk bytes through `ProposalStreamDecoder::push`
  and call `finish` when the transport signals end-of-response, without
  either module changing.
- **Not a JSON parser.** The incremental scanner validates only what it
  needs to bound work and detect the terminal byte early: container
  nesting, string-literal well-formedness, the absence of disallowed raw
  bytes outside strings, and the exact single-newline framing. It does not
  itself validate object key/value/comma grammar, number lexical form, or
  `true`/`false` spelling — full semantic legality is always decided by the
  one delegated `CompiledInteractionSchema::decode` call, never by this
  scanner accepting or rejecting on its own authority.
- **No provider prompt/schema projection change.** `provider_json_schema`
  is unchanged, untouched, and out of this module's scope.
- **No hosted evidence.** Every claim above is local
  `cargo test --locked -p semaprax --lib streaming_proposal_decode`.

## Executable reference

`src/streaming_proposal_decode.rs` (`ProposalStreamDecoder`, `PushOutcome`,
`StreamRefusal`, the `STREAM-*` closed vocabulary, and the `MAX_STREAM_*`
bounds) plus its `tests` submodule
(`src/streaming_proposal_decode/tests.rs`) is the complete reference
implementation this document describes. Focused gate:

```sh
cargo test --locked -p semaprax --lib streaming_proposal_decode
```
