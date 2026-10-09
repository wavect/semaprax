# Bounded ASCII patterns with capture offsets — draft

Status: private source experiment. The OPT-702–704 baseline had four passing private witnesses; the OPT-706 source
revision is unchecked and has no package registration, completion change, public ABI, or performance claim.
Source is [ascii.spx](ascii.spx), based on `origin/main` at `ac3f3bfaa`.
The independent semantic oracle and corpus are in [fixtures](fixtures/README.md); exact compiled-table/work witnesses are in
[compiled-witnesses.json](compiled-witnesses.json). Execution and formatting for the new source revision remain deferred until
the complete shared source batch is ready. The earlier design proposal remains a proposal; this copy records the private source
choices without changing its product status.

## Problem and existing facilities

Completed LogLens SEM01 used 2,977 bytes for character helpers and validation (lines 5–39) and 766 bytes for repeated extraction
(line 118), within an 11,945-byte `loglens.spx`. These regions establish an opportunity, not an estimate of replaceable bytes or
token savings. Its two failed boundaries were candidate grammar errors: line 37 accepted `+` or `-` before timezone digits; the
unchanged SPEC requires literal `+`. Preserve that requirement.

Existing `std.bytes` supplies delimiters and trimming; Text Toolkit supplies byte lookup, search and checked slicing. Existing
`std.int.decimal`, admitted by command manifest v26, already replaces the candidate's 1,130-byte decimal arithmetic region
(lines 42–81). No new decimal API is proposed. The missing operation validates one ordered pattern and retains field offsets
without searching the same record again. Private indexed Rust regex is not a portable capturing source package; `std.regex` is
still Missing.

Target applications include log records, protocol headers and structured CLI records. This package does not encode LogLens,
validate calendar/range meaning, trim input, normalize bytes, render JSON or add filesystem/network authority. Independent
example patterns are `([A-Za-z-]+):[ \x09]*([^\x0D\x0A]*)` for a header-shaped record and
`([A-Za-z_][A-Za-z0-9_]*)=([^\x00\x0A]*)` for a key/value record; neither implies a complete HTTP or configuration parser.

## Pattern meaning

Proposed package: `std.pattern.ascii`, pure internal source library. Pattern syntax is ASCII; the input is an arbitrary borrowed
byte sequence.

- The match is anchored at both ends. There is no search mode or implicit multiline/dot policy. Empty pattern matches only empty
  input.
- Atoms are literal ASCII bytes, `.` (any byte), escaped punctuation, `\xHH` (one byte, two ASCII hexadecimal digits), or a
  character class.
- Classes contain explicit bytes/ranges; leading `^` complements over all 256 byte values. Ranges require ascending endpoints.
  `]`, `-`, `^`, and backslash have explicit escaping rules; empty classes are invalid.
- Atom suffixes are `?`, `*`, `+`, `{m}`, `{m,n}`. Decimal bounds are canonical unsigned ASCII, with `0 <= m <= n <= 255`. An
  atom consumes exactly one byte; `{0}` consumes none. Quantifiers cannot be stacked.
- Parentheses define numbered captures in opening-parenthesis order. Captures are flat, cannot nest, and cannot themselves carry
  a repetition suffix. Every successful capture is present, including an empty span.
- No alternation, backreferences, lookaround, lazy suffix, recursion, Unicode class/case folding, named capture, replacement, or
  host regex invocation.
- Repetition is greedy, in source order: among complete accepting paths, prefer the lexicographically greater vector of
  repetition counts, ordered by the quantifier's source position. Capture values come from that path. Thus `(a*)(a*)` on `aa`
  yields `[0,2)` and `[2,2)`; `([0-9]+)[0-9]` on `123` yields `[0,2)`. Failure to accept the entire input is no match.

The private compiler pins the following lexical choices. Ordinary raw literals are printable ASCII 32..126. Outside a class,
`. \ [ ] ( ) ? * + { } | ^ $` are reserved; raw `-` is a literal. Escaping any printable punctuation 33..126 emits that byte;
escaped alphanumerics are invalid except `\xHH`, whose two digits accept either hex case. Raw control/non-ASCII pattern bytes
are invalid; `\xHH` can emit any of the 256 input-byte values. In classes, `]` closes, leading `^` complements, and a later raw
`^` is invalid. A raw `-` is literal only immediately before `]`; otherwise it joins ascending endpoints. A raw `-` cannot be a
range endpoint. Escape `]`, `^`, `-`, or backslash when using them elsewhere as members. Thus the original `[A-Za-z-]` header
class still includes hyphen. Empty/complement-only classes are invalid.

The first malformed byte is selected left to right. EOF is pattern length. A descending range diagnoses the first byte of its
right endpoint; an empty class diagnoses its closing bracket; nested captures diagnose the second opening parenthesis;
unmatched closing parentheses diagnose that byte, and an unclosed capture diagnoses EOF. Decimal leading zeros diagnose the
second digit, a value above 255 diagnoses the digit that exceeds the bound, a reversed bound diagnoses the closing brace, and
missing/extra punctuation diagnoses the byte encountered (EOF if absent). Stacked/group quantifiers diagnose the suffix byte.
Unsupported syntax is invalid, with no literal fallback. The independent oracle intentionally keeps previously underspecified
edge cases pending until its separate corpus is reconciled against these source choices.

## Reusable owned carrier and proposed surface

Use the existing internal nongeneric record shape with exactly one `Bytes` field and one `usize` field; this is the structural
renewal shape owned by [IO Lines v1](../../docs/IO-LINES-V1.md). Proposed `Matcher` fields are `storage: Bytes` and `position: usize`.
No stored borrowed view, new `Vec<Span>`, owning Option, public nominal descriptor, mutable field projection, or opaque handle
is needed.

Allocate one 3,072-byte buffer outside the caller's loop. Construct Matcher from it; compile consumes/returns the whole Matcher;
full-match consumes/returns that Matcher for each record. No input copy, new allocation site, growth or owner duplication occurs
in compile/full-match. Exact loop admission relies on OPT-702's narrow independent named borrowed-input extension. The compiler
hook and earlier private witnesses passed their focused baseline; the new source revision still needs its own gate and grants
no package admission claim.

The complete proposed layout is:

| Byte interval, inclusive | Meaning |
| --- | --- |
| 0–63 | compiled header, counts, flags; no trusted authority |
| 64–1087 | 128 atom records, 8 bytes each |
| 1088–1599 | 16 character-class bitmaps, 32 bytes each |
| 1600–1663 | 16 capture boundary pairs, two little-endian u16 atom indices |
| 1664–2695 | 129 start/count frames, two little-endian u32 fields each |
| 2696–2983 | result header and up to sixteen span slots |
| 2984–3015 | compiler-only temporary class bitmap; no matching meaning |
| 3016–3071 | unused storage; no matching meaning |

An atom stores kind/value in bytes 0/1, minimum in bytes 2/3, maximum in bytes 4–7. Kinds are literal, any-byte and
bitmap-class; value is respectively one byte, zero or class index. Maximum 0xffffffff means unbounded; otherwise minimum <=
maximum <= 255. Class bitmaps represent all 256 membership bits. Duplicate classes share a bitmap by exact comparison during
compile. A seventeenth distinct class is a resource refusal. There is no repeat expansion. Capture boundary indices are ordered,
nonoverlapping and in 0..atom_count. Frame atom_count is the end sentinel; its start is input length and count zero.

Compile clears the old ready byte first, constructs a temporary class bitmap, and writes its used program/header regions.
The candidate bitmap is zeroed for each class; complement, deduplication, and copying are individually charged. No full-buffer
clear occurs. A successful compile writes all 64 header bytes once after preflighting that commit plus its packet. Before matching, validate the header, every used atom and capture pair:
at most 64 + 128*8 + 16*4 = 1,152 byte reads, plus charged scalar/control checks. Bitmap bytes are all legitimate sets; unused
table bytes have no meaning and need no scan. Each entered atom overwrites its own frame. Only frames at or below the live
search depth are readable. No full-buffer or full-scratch clear occurs per input.

Proposed signatures (design notation, not a checked source example):

```text
matcher_from_bytes(own Bytes) -> Matcher
compile(own Matcher, borrow Slice<u8> pattern, usize work_limit) -> Matcher
full_match(own Matcher, borrow Slice<u8> input, usize work_limit) -> Matcher
result_valid(borrow Matcher) -> bool
status(borrow Matcher) -> usize
capture_count(borrow Matcher) -> usize
capture_start/end(borrow Matcher, usize capture) -> usize
capture_bounds(borrow Matcher, usize capture) -> Capture::Span { usize start, usize end }
work_used(borrow Matcher) -> usize
```

Construction requires exact storage extent. `position` must be within the fixed storage extent; it may not stand for trusted
compilation authority. Public source records are forgeable: each consumer validates its accessed region; full_match validates
the used compiled header/atoms/capture indices, while result observers validate only the logical packet. Compile/full-match reads are charged to their local logical meter. Borrow-only result observers are different: they debit
ordinary interpreter AST fuel, cannot mutate the owner, and never claim to add to `work_used`. Failed compile invalidates its previous compiled program. Failed match keeps the compiled program and sets the result
capture count to zero. Matcher construction/shape contracts run first; extent preflight follows, then left-to-right parsing or
table validation, then matching. The first encountered malformed token or exhausted engine bound selects its packet status;
later checks cannot replace it. A runtime/contract failure remains sticky. Result observers use checked preconditions for forged
carriers or invalid capture indices. Querying status does not reinterpret a failed runtime call.

Read a borrowed byte into a Copy scalar, end that loan, then replace the whole owned buffer/Matcher. No view may survive a
consuming update. Named string owners can supply `string_as_str` then `str_as_bytes`; the pattern/input owners remain with the
caller. The ordinary verifier and independent replay must admit these exact call and renewal shapes before the API is accepted.

## Result packet and decoding

At byte 2696, encode a 32-byte header: bytes 0–3 are `SPAT`, then version 1, status, capture count and reason. Little-endian u64
fields at offsets 8, 16 and 24 are input length, actual work used and detail offset. Each matched capture adds sixteen bytes,
start then end as u64. The logical packet extent is exactly 32 + 16*capture_count, at most 288; it is not the whole Matcher.

Statuses:0 ready/no attempt,1 matched,2 no match,3 invalid source/program,4 resource refusal. Only matched exposes captures.
Status3 reason1 is malformed source pattern and detail is a pattern-byte offset (EOF is pattern length, at most1024); its input
length is zero. Status3 reason2 is a forged/malformed compiled table and detail is an absolute carrier-table byte offset0..1663.
That table offset is never described as an original pattern offset. The compiled table does not retain source positions.

Status4 reasons are1 pattern extent (input length zero; detail is actual pattern length),2 input extent (detail equals actual
input length),3 atom count,4 distinct class count,5 capture count (input length zero; detail is the offending pattern token
start), and7 logical work (detail zero). Reason6 is reserved and rejected by `result_valid`. Status0 has length/count/reason/detail
zero; statuses1/2 have reason/detail zero. Compile's result length is zero. These domains are shared by the packet writer and
borrow-only decoder. Non-matched count is zero; no stale span is exposed by a capture
accessor. Unused slot bytes are outside the logical packet and need neither reading nor zeroing. Serializing whole storage is
not a canonical result projection.

Checked decode validates exact logical extent, magic/version, status/reason, count and start <= end <= input length. It checks
remaining bytes before a load and representability before converting to usize/i64, never truncating. Source helpers use existing
indexed bytes. The packet has no proof authority.

Offsets are byte offsets. A capture may split UTF-8. Passing offsets to `string_slice` retains its exact range and UTF-8
boundary failures; successful byte matching is not a UTF-8 boundary assertion. Examples keep the original input owner and check
the relevant boundary before text extraction.

## Bounds, work and settlement

Proposed ceilings: pattern 1,024 bytes, input 65,536 bytes, 128 atoms, 16 distinct class bitmaps, 16 captures, 288 result bytes
and 262,144 logical work units per compile/match. Example callers initially request 8,192 units; that is a proposed witness
configuration, not evidence it fits normal fuel. Extents/counts above bounds are resource refusal, never truncation. No compiler
quota changes. Captureless and empty matches retain their full grammar meaning.

A logical unit is one byte read/write or one atom-dispatch/backtrack/control iteration. Every pattern/class scan, input
predicate, bitmap access, frame read/store, compile-table check and loop iteration debits before performing that event. A
u32/u64 load costs four/eight reads, not one. Codec arithmetic and meter bookkeeping are bounded straight-line scalar work;
there is no hidden unmetered loop or bulk copy. The implementation must pin each helper's AST cost as well as this byte/control
metric, which is not CPU time or fuel.

Full-match requires32 <= work_limit <=262144. Compile requires33 <= work_limit <=262144: invalidating the previous ready byte
costs one actual store before its 32-store refusal packet. A 32-unit compile cannot satisfy both invariants and is a checked
precondition failure. Maintain 32 unspent units while engine operations execute, sufficient to write a refusal
header. If the next event would leave less than 32, return resource refusal without performing it. Reserve is not a debit.
Checked subtraction avoids work-counter overflow.

Successful finalization has exact byte/control work F(c) = 32 + 29*c: 32 header stores, and per capture four boundary-index
reads, eight frame-start reads, then sixteen offset stores and one entered capture-loop iteration. Load both endpoints even for
an empty capture; use the sentinel for an end boundary. All frame/boundary checks were already charged before finalization.
Header values are retained Copy scalars. Preflight F(c) as a whole. If it does not fit, write the 32-byte resource header
instead. The encoded work_used includes those actual stores/reads, not reserved work. Header and u64 writes are straight-line
unrolled source helpers, not codec loops. No-match/invalid/resource packet finalization is exactly32 writes; no captures clear. Compile also has its earlier one-store
program invalidation. Successful compile finalization is64 header stores plus32 packet stores, preflighted as 96 units. A
failed contract or ordinary runtime/fuel failure publishes no Matcher and retains canonical sticky settlement, rather than
becoming a packet result.

Interpreter fuel remains ordinary per-node fuel: default 1,000,000 steps, maximum 160,000,000 at this source. A logical-work
ceiling does not demonstrate that an invocation fits either value. Native/Wasm retain the source meter; they do not
automatically inherit interpreter fuel. Future gates must accept the useful corpus under unchanged ordinary limits and record
aggregate fuel. If byte/codec helper overhead prevents that, revise the design or justify an exact portable helper with
source/HIR/replay/backend gates; do not raise fuel.

## Small greedy engine and explicit tradeoff

Flatten the no-alternation grammar into atoms plus capture boundary indices. For each atom at the current position, scan the
maximal permitted matching run, store start/count, and advance. On a failed minimum, later atom or whole input check, rewind to
the rightmost earlier variable atom whose count exceeds its minimum, decrement that count and rebuild the subsequent frames
greedily. Fixed atoms have no alternative count. Empty captures map to equal boundaries. Every scan/rewind/store is charged; no
recursion or unbounded host stack.

This depth-first order enumerates feasible count vectors in lexicographically descending source order. Therefore the first fully
accepting vector is exactly the specified greedy match. Captures follow its prefix boundaries. Bounds use remaining input length
before addition; count decrement requires count > min. The proof must cover absent/empty suffixes and replay of invalidated
frames; a bounded exhaustive oracle remains required before implementation acceptance.

This is bounded backtracking, NOT Thompson linear matching. `a*a*...a*b` on many `a` bytes without `b` can explore exponentially
many count vectors. Even an accepted suffix may require many failed partitions first. The meter bounds the attempted work and
returns resource refusal; it cannot turn that refusal into no-match or authorize silently skipping an input record. Memory is
fixed 3 KiB, but fast acceptance for ordinary formats is still unproved.

Before choosing this engine, pin useful records and hostile adjacent repeats in the future corpus. Require all unchanged 49
LogLens obligations plus two independent header/key-value applications to succeed within their ordinary invocation fuel and
declared per-match limit, without changing their grammar. Record long valid records, nonmatching records and accepted ambiguous
repeats separately. Frequent refusal is a design failure, not evidence of an advantage. Do not ship this general matcher solely
because it is smaller. If that gate fails, review a separately specified delimiter-disjoint linear subset or a
suffix-feasibility algorithm; neither is silently equivalent to this grammar.

Interpreter, C11 O0/O2 and internal Core Wasm must execute the same checked source algorithm. No Rust/JS regex shortcut. Initial
targets are internal owned-data/useful-data v2; native command v26/v28 needs a real admission witness. No public byte facade or
broader Wasm command claim follows from this draft.

## Private source progress and remaining proof

`ascii.spx` contains the owned carrier codecs, complete flat grammar compiler, class deduplication, greedy frame traversal,
packet finalization, and borrow-only packet observers. It has no manifest and uses the private `experiment.pattern` identities.
The existing language harness owns four new private witnesses: grammar/capture examples, exact malformed offsets and failed
compile invalidation, handwritten compiled tables with exact work boundaries, and a single independent header witness for
interpreter/C11 O0/O2/Core-Wasm settlement. The earlier four witnesses passed on their baseline; none has executed this source
revision. Handwritten table fixtures pin every byte by zero initialization,
ordered segments, and SHA-256; unused frame/result bytes start zero but remain outside compiled validation meaning.

The important remaining uncertainty is ordinary source cost. Per-byte helpers perform checked calls, contracts, matches,
conversion, and record reconstruction; their logical byte costs are exact, but exact interpreter AST costs are not yet pinned.
The 65 KiB ceiling can require roughly 196608 input-predicate work units for one class scan alone. That does not establish that
such a record fits the unchanged 1000000-step ordinary default fuel. The complete unchanged 49-obligation corpus, long records,
nonmatches, and ambiguous accepted suffixes remain required before any useful-package claim.

## OPT-706 source cost and greedy pruning candidate

This revision retains every grammar rule, limit, table byte, packet field, status/reason domain, and public observer signature.
It adds no allocation site. Fixed-width loads destructure a borrowed Matcher once and use direct total `byte_get` reads.
Stores use one owning Matcher destructure/reconstruction, combining contiguous groups with the existing `bytes_set5` operation.
Both write widths retain exact byte-write charges; `frame` still costs 8, `packet` 32, `capture_result` 16, compiled header 64,
and successful capture finalization remains `F(c)=32+29*c`. Every fused writer checks the carrier before accessing it.
The reference interpreter's unique owner transfer remains the basis for expecting no payload COW copy, not a measured heap claim.

The old scan path remains for bounds below 64 bytes, preserving the handwritten small-input work boundaries. On a long run,
any-byte atoms advance through their checked bound without reading the input. A class receives a 32-read inspection of all four
u64 bitmap words. Exact bitsets select an optimized mode only when they are all bytes, one included byte, one excluded byte,
all except CR/LF, all except NUL/LF, token bytes excluding 0..32 and 127, or path bytes excluding those plus
34 (quote) and 92 (backslash). The token/path modes accept every byte from 128 through 255. Their scalar
literal-or patterns preserve the bitmap predicate exactly; the current interpreter does not debit a separate AST step for
each pattern alternative. Generated target comparisons still have a fixed per-byte cost, so this is no CPU speed claim.
The one-bit test is six fixed unsigned scalar stages, with no hidden scan or loop.
The inspection never trusts a class spelling or stores authority in a cache. All other bitsets use the original predicate.

A selected literal or exclusion scan returns the flat Copy `Scan::Done { count, work, exhausted }`. It has no stored view,
owned field, or escaping loan. Each 48-byte helper performs direct total reads in increasing order and stops at the first
rejected byte. A complete chunk costs 49 logical units: one committed scan iteration plus 48 reads. A chunk rejecting its
zero-based byte `j` costs `j+2`; no later byte is read. A scalar tail costs two units per attempted byte. The caller reserves its
32-write refusal packet before passing the remaining work; each chunk/tail preflights its complete worst-case cost. Budget
bookkeeping and a refused preflight perform no predicate event. Thus a successful selected run of `n` bytes costs
`49*floor(n/48)+2*(n%48)`, or 66,917 for 65,536 bytes, plus class inspection, table validation, frames, and finalization.
The logical meter records performed work, not reserved chunk width. Source interpreter fuel remains separate.

A manual conservative ordinary-fuel estimate allocates 660 steps per complete exclusion chunk: roughly 595 for the called
48-byte helper including requires and arguments, and the remaining allowance for scan guard/body, scalar updates and implicit
statement tails. The direct `byte_get` extra operation charge, match scrutinees, selected arm values, true blocks, and
requires-clause charge are included. A 65,536-byte successful scan has 1,365 complete chunks and a 16-byte tail, putting the
scanner below approximately 904,000 steps on that estimate. Literal and singleton-exclusion helpers differ only by fixed scalar
argument/condition work. This is a source estimate pending exact retained-HIR counting and execution, not a measured fuel result.
The complete invocation must also fit compilation, table checks, frames, observers, and caller work under the unchanged
1,000,000 default. Repeated compile per record wastes that remaining margin; fixed-pattern applications should compile once and
renew the returned Matcher across records, including after no-match or resource packets. `full_match` preserves the compiled
table and overwrites only reached frames and the logical result packet.

On inputs of at least 64 bytes, terminal-atom pruning skips reductions when the maximal run cannot cover the remaining input:
for the fixed earlier prefix, every smaller count also leaves an uncovered suffix. Rewinding resumes at an earlier atom.
Required-follower pruning also skips reductions when the immediate following atom has a positive minimum and disjoint byte
membership. It supports literal/literal and literal/class in either order, inspecting six metadata bytes and at most one bitmap
byte (charged six or seven units). Each removed byte belongs to the current atom and cannot start the required follower, so
none of those count vectors can accept. The LogLens token/path modes therefore stop at their exact required
space/quote delimiter and avoid retreating across the same long field after a later suffix failure. Nullable followers, any-byte
intersections, class/class pairs and all other cases keep
the ordinary greedy rewind. The first accepting vector and capture boundaries therefore remain unchanged; no input is skipped
and every no-match conclusion still follows exhausted accepting alternatives. Every later greedy entry overwrites its own
frame before reading it.

The exact token bitmap words are `[18446744065119617024, 9223372036854775807, 18446744073709551615,
18446744073709551615]`; the path words are `[18446744047939747840, 9223372036586340351,
18446744073709551615, 18446744073709551615]`, in little-endian class-word order. These are checked against all
32 bytes at each long class entry; forged or differently spelled classes only select the mode when all bits agree.

The bounded engine still has a major proof obstacle: arbitrary long classes can exceed ordinary fuel, and intersecting adjacent
repeats can exhaust logical work before deciding a match. This source revision does not claim a linear matcher or all-pattern
65 KiB acceptance. The unchanged 49 LogLens obligations, independent header/KV meanings, long valid and nonmatching records,
accepted ambiguous suffixes, exact work boundaries, forged bitmaps, and interpreter/C11/Wasm settlement are required on the final
source. Package admission remains contingent on those results and the owning Standard Library/Project contracts. No manifest,
closed dependency inventory, catalog or package status is changed here.

The existing start/end observers now validate the complete packet once before directly checking its status/count header bytes;
they retain the same predicate and contract-failure boundary without recursively revalidating through `status` and `capture_count`.
The additive private `capture_bounds` observer performs that same complete validation once and returns flat Copy
`Capture::Span { start: usize, end: usize }`. It still rejects malformed sibling spans, non-matched packets and out-of-range indices.
No borrowed input escapes and neither observer path debits the logical engine meter. Callers extracting both endpoints can match
this Copy result once; existing start/end signatures remain available. These exact flat payloads use the same owning loop classifier
as Token/Quantifier/Scan, rather than assuming arbitrary Copy aggregates are scalar-call results.
