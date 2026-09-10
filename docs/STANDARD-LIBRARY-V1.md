# Standard Library v1

- Status: versioned reference; 36 packages are present under `std/`: nine
  `core`, eighteen `portable`, three `alloc`, three `hosted`, one `agent`, and two `test`. Every package remains
  Partial until its complete required scope and promotion evidence exist; every
  other module in the required set is Missing.
- Audience: standard-library authors, compiler contributors, and agents
  choosing between a compiler-owned function and a library declaration.

This document owns the standard-library contract: how the library is
composed, what every public declaration must carry, the portability tiers, the
required module set, the effect vocabulary hosted modules declare, and the
Everyday profile. The [completion matrix](COMPLETION-MATRIX.md) owns status;
the generated [standard library catalog](STANDARD-LIBRARY-CATALOG.md) lists the
declarations that exist today and `semaprax help library` prints it offline;
`semaprax help library <module|name|stable-id>` selects an exact compact entry
from its generated JSON companion; `tests/project.rs::standard_library` is the
executable gate.


## Released library evidence

The 34 packages of the v0.4.0 inventory have the accepted **HOSTED GREEN**
baseline for their admitted profiles. `std.io.lines` and `std.path.normalize`,
added after that baseline, have local evidence only and are not covered by it.
The inventory is nine core, eighteen portable, three alloc, three hosted, one
agent and two test packages.
The tables below retain Partial for incomplete module scope and Missing for
modules that do not exist; these are not pending first executions of the
implemented release corpus. Historical case counts or named-host observations
remain scoped to those runs. Additional physical providers, general streams,
public signatures, complete iterator interfaces and the full Everyday profile
are independent requirements.

The private scalar/Bytes iterator protocols, loops, callbacks and scalar-snapshot
closures already exist. They are not a completed `std.iter` package or a general
Iterator interface. Linked Agent migration and durable recovery likewise exist
under their own runtime contracts; running ordinary `std.agent` functions on
native/Wasm does not execute Agent stages on those backends.

## Library architecture

The standard library is composed of ordinary SEMAPRAX packages. A package is a
directory below `std/` holding a Project manifest and three modules:

| Module | Role |
| --- | --- |
| `std.<name>` | The library module. It declares only public functions and types; it never defines `main`. |
| `std.<name>.examples` | The Project entry. Its `main` demonstrates idiomatic use and returns `0`. |
| `std.<name>.tests` | The single Project test module. Its `main` is the conformance suite and returns `0`, or a bitmask naming the failed checks. |

Hosted packages with an effect profile follow the same three-module shape, but
their structural `main` functions are not conformance evidence. Their examples
and tests must each execute an explicit stable-ID `fn () -> bool` command
through an invocation-owned provider injected by the host, so the checked
effect path is exercised. A pure `main` that only returns `0` is present for
Project structure and does not demonstrate hosted behavior.

Host-specific operations may be implemented in Rust, C, Wasm, or a platform
language, but their public contract is a SEMAPRAX semantic interface: a
declaration in a `std.*` module with an `@id`, types, ownership modes,
effects, and contracts. The compiler-owned functions listed in the
[agent quick reference](AGENT-QUICK-REFERENCE.md#compiler-owned-functions) are
the current host surface; moving them behind `std.*` interfaces is an open
gate of this document, not a completed step. That surface now includes the
`bytes_zeroed`/`bytes_set` write-once owned buffer of
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md), which executes
on the interpreter, native C11, and internal Core Wasm through the
`env.spx_bytes_zeroed`/`env.spx_bytes_set` host-arena imports. The public Wasm
byte-export adapter still rejects it with `SPX-W115`, so it carries no public
ABI; no `std.*` package wraps it, and no required module below is satisfied by
it. The exact compiler-owned `Vec<T>` profile is different: the alloc-tier
`std.collections` package authenticates eight transparent aliases over its eight
intrinsics for the eight admitted Copy scalars. Those aliases add no public ABI,
iterator, owned-element, or broader collection support.
The three additive aliases select compiler prelude v3; collision-free source
using only the original five retains frozen prelude-v2 binding bytes and digest.
The separately specified
[Owned Bounded Vec For Traversal v1](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md)
is resolver syntax over the existing length/get/while vocabulary. It adds no
`std.collections` declaration and does not advance the Missing `std.iter`
package.
The separately bounded compiler-owned `Box<T>` allocation is exposed by the
alloc-tier `std.mem` package through exactly three authenticated aliases:
`new`, `get`, and `into_inner`, instantiated explicitly for the same eight Copy
scalars. Its scalar-result example and conformance package export no public
descriptor. These aliases retain the scalar Box slice. The compiler's separate
[Box v2](OWNED-BOUNDED-BOX-V2.md) implements owned Bytes payloads; it does not
widen the aliases. Regions, arenas, shared immutable values, general allocator
interfaces and public generic ABI remain absent.

Every public standard-library declaration must have:

1. a stable identity: an explicit `@id` below its module name, so
   `std.num.gcd` names one function across renames and moves;
2. types and ownership modes spelled in the signature;
3. effects, declared with `uses` and granted by the module's `permit`; the
   `core` tier declares none;
4. contracts: `requires` and `ensures` lines that state the admitted inputs
   and the guaranteed result; an authenticated transparent intrinsic alias
   instead inherits the intrinsic's exact preconditions and status identity
   and must not replace them with a second authored contract;
5. examples: the package's examples module imports and exercises it;
6. conformance tests: the package's tests module imports it and checks it,
   and the suite passes on every target the package lists;
7. compatibility metadata: the package's tier, targets, and status in
   `std/packages.json`;
8. generated human and agent documentation: the
   [catalog](STANDARD-LIBRARY-CATALOG.md) and `std/catalog.json`, both
   generated from the sources by the gate and pinned byte for byte.

The gate enforces 1, 3, 5, 6, 7, and 8 today. Contracts (4) are required by
this document and reviewed; a declaration without one is a review finding, not
yet a gate failure. The exact `std.collections` transparent aliases are the
contract-authoring exception described above, not contract-free new behavior.
Internal owned-data Project calls additionally admit explicit nongeneric
record trees over Bytes and Copy scalars. The `std.io` library uses this lane
for its Reader/Writer types, borrowed observations and consuming transitions.
Public descriptors remain separately restricted. The authenticated
`std.collections` transparent wrappers forward an explicit Copy-scalar argument
to their exact compiler intrinsics without changing HIR or status identity;
this does not admit general public generic signatures. Other library profiles
retain their existing scalar and borrowed-view boundaries. The Useful Text public export profile remains
contract-free, so `std.text` cannot yet satisfy the reviewed contract
requirement even though its bounded conformance package is executable.

`std/packages.json` lists every package directory with its module, tier,
targets, and status. The gate fails when the list and the directories under
`std/` disagree.

### Consuming a package

A canonical `semaprax.manifest.v1` Project may depend on one of the compiler's
exact bundled standard-library packages through `[dependencies]`, then import
its functions by stable identity:

```toml
[dependencies]
std.num = "^0.1.0"
```

```text
use function @id("std.num.gcd") from std.num as gcd;
```

The compiler admits only the closed `std.*` inventory at bundled version
`0.1.0`, validates the declared exact/tilde/caret range, adds its immutable
source and transitive standard dependencies to the authenticated in-memory
workspace, and performs ordinary stable-ID linking. It reads no cache and
gains no filesystem or network authority. Unknown packages and ranges that do
not contain the bundled version retain `SPX-J121`; ordinary resolved packages
are not yet linked by Project builds. A source file may still vendor a library
module explicitly. `std.bytes` requires the `useful-data.v1` profile and
`std.text` requires `useful-text-consumer.v1`.

## Portability tiers

| Tier | Scope |
| --- | --- |
| `core` | Allocation-free, effect-free operations available on every supported target |
| `alloc` | Collections, strings, and owned data using declared allocators or regions |
| `portable` | Interpreter, native, and Core Wasm behavior with equivalent semantics |
| `hosted` | Filesystem, environment, process, network, and clock operations |
| `browser` | Browser APIs through explicit host interfaces |
| `embedded` | Allocation-restricted and OS-free facilities |
| `agent` | Model, context, approval, tool, checkpoint, and evidence abstractions |
| `test` | Deterministic handlers, properties, fuzzing, and simulation |

A module need not exist on every target. Its availability is an explicit
fact: the package's `targets` list in `std/packages.json`, repeated in both
catalogs. Recording that availability inside the semantic graph itself, so a
`use function` of a module absent on the selected target fails at admission,
is an open gate; today the fact is package metadata that the gate verifies by
executing the conformance suite on each listed target.

Target names are `interpreter`, `native-c11`, and `core-wasm`, matching the
lanes in [Architecture](ARCHITECTURE.md#compiler-and-execution-lanes).

## Required standard modules

| Module | Required scope | Status |
| --- | --- | --- |
| `std.core` | Option, Result, ordering, equality, ranges, conversion, and core traits/interfaces | Partial: `i64` ordering as `-1`/`0`/`1`, extrema, clamping, range membership, `bool` conversions and connectives; `Option` and `Result` remain compiler-owned |
| `std.num` | Checked, wrapping, saturating, and conversion operations | Partial: sign, absolute value, parity, Euclidean division and remainder, greatest common divisor, checked power, integer square root, digit count, power-of-two test, and floor logarithms in base 2 and 10 in `std.num`; overflow predicates and wrapping and saturating addition, subtraction, negation, absolute value, and multiplication in `std.num.overflow`; checked arithmetic is the language default, and additive `wrapping_mul` closes wrapping multiplication with exact two's-complement results at `i64::MIN`, `i64::MAX` and every sign combination |
| `std.iter` | Iterators, adapters, folds, collection, and ranges | Missing as a standard-library package. Private consuming scalar/Bytes iterators, loops, map/filter/fold and scalar-snapshot closures are implemented separately. A general public iterator interface, associated types, lifetime rules and supported package surface remain required |
| `std.mem` | Ownership helpers, regions, arenas, boxes, shared immutable values | Partial: authenticated transparent wrappers for compiler-owned `Box<T>` `new`, synchronous `get`, and consuming `into_inner` over exactly eight Copy scalars, with explicit conformance and no public exports; regions, arenas, shared immutable values, allocator interfaces, owned payloads, and broader ownership helpers remain Missing |
| `std.collections` | Vector, deque, map, set, heap, and fixed-capacity collections | Partial: authenticated transparent wrappers for `with_capacity`, `push`, `reserve_exact`, `set`, `clear`, `len`, `capacity`, and `get` over exactly the eight Owned Bounded Vec v1 Copy scalars have focused local Project/package evidence, explicit conformance instantiations, generated catalogs, and no public exports; `push`, `reserve_exact`, `set`, and `clear` consume and return the one owner, while every broader collection operation remains Missing |
| `std.bytes` | Buffers, spans, readers, writers, endian operations, and encoding | Partial: byte-to-integer conversion, guarded indexing, first-index search, counting, ASCII classification, slice equality, prefix and suffix tests, and little- and big-endian 16- and 32-bit reads over `borrow Slice<u8>`. Additive span cursors add ASCII whitespace classification and trimming (`is_space`, `trim_start`, `trim_end`, `is_blank`) and delimiter-separated field walking (`field_end`, `field_start`, `field_count`) that preserves empty fields, so `a,,b` is three fields and a trailing delimiter opens one final empty field; these compose directly with the `std.io.lines` line content and carry local three-backend evidence. Buffers, writers, encodings, and any Unicode whitespace or quoting policy are Missing |
| `std.text` | UTF-8 strings, Unicode iteration, search, split, trim, and normalization policy | Partial: borrowed byte length, emptiness, exact equality, prefix, and substring search; iteration, split, trim, and normalization are Missing |
| `std.format` | Type-safe formatting without runtime format-string ambiguity | Partial: source-authored `append_str`, `append_i64`, `append_usize`, and `append_bool` consume and return a caller-owned `std.io.Writer` after exact capacity preflight. Checked decimal/byte/length helpers provide the rendering policy without hidden allocation or effects; the private `useful-data.v2` profile imports bundled `std.io` and has no public exports. Seven owned-function-import unit tests pass, including borrowed-`str` and ordinary owned-byte-record positives plus non-byte-record refusal. Eight individually runnable named SPX tests pass on the interpreter, native C11 `-O0`/`-O2`, and repeated Core Wasm with a strict two-entry byte arena; the pure helper case uses zero allocation. Five short/forged-output preflight cases pass twice with exact `requires`-false status through the bundled `std.format` consumer and transitive `std.io`; metadata and catalog regeneration pass. Named tests run individually because the per-function static allocation limit is unchanged. Additive field padding renders aligned output: `pad_len` is the written field width (never narrower than the content), `append_fill` writes one repeated byte, `append_str_left` writes left-aligned text and `append_usize_right` right-aligned decimals, each preflighting the whole field rather than only the content and never truncating longer content. Five named padding cases run with the existing corpus as individual bounded projects on the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm, and four more short/forged-output cases reject padded writes with the exact `requires`-false status; this padding evidence is local. General format strings, arbitrary alignment modes, grouping separators and floating-point rendering remain Missing. [Format Writer v1](FORMAT-WRITER-V1.md) owns the additive contract |
| `std.io` | Reader, Writer, buffered I/O, streams, line processing, and standard streams | Partial: source-authored nongeneric Reader and Writer records compose caller-supplied `Bytes` buffers with `usize` cursors through consuming transitions and no public exports, using internal owned-data Project imports; interpreter, native C11, and Core Wasm consume the same checked HIR. Focused local package, dependency, contract, and cross-engine evidence passes. Line processing is supplied separately by `std.io.lines`, because one library module holding both halves pushes an ordinary three-package consumer past the `SPX-G171` pre-bound. Buffered I/O, streams, standard streams, and public generic or nominal widening are Missing |
| `std.io.lines` | Line meaning over the `std.io` Reader and Writer cursors | Partial: additive sibling package depending on `std.io`. `line_end`, `line_terminated` and `line_content_len` observe a borrowed `Slice<u8>`; `reader_line_len` and `reader_line_complete` observe a borrowed Reader; `reader_line_into(borrow Reader, own Writer) -> Writer` copies one line's content into caller-supplied capacity after an exact preflight, starting at the writer's live cursor; `reader_next_line` consumes and returns the Reader past the line and its terminator, clamped for an unterminated tail. One line feed terminates a line, whose content excludes that byte and one immediately preceding carriage return, so LF and CRLF inputs yield identical content and a bare carriage return stays content. The graph exposes the selected cleanup schema per shape: the copy and the record observers select the existing v5 contract schema, the pure view helpers v2, and the borrowed Reader never enters an owned inventory. Eight named cases run as individual bounded projects on the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm, and nine hostile cases reject short capacity, a live cursor with too little capacity, forged cursors and out-of-range view offsets with the exact `requires`-false status; this evidence is local. Buffered readers, streams, standard streams, in-loop cursor replacement, text or Unicode interpretation, and any public export are Missing. [IO Lines v1](IO-LINES-V1.md) owns the additive contract |
| `std.path` | Platform-neutral path values and explicit platform conversion | Partial: the original public package provides allocation-free inspection of canonical slash-separated path bytes for absoluteness, trailing separators, nonempty segment count, filename start, parent boundary, and extension boundary; typed Path values are supplied separately by `std.path.value` and lexical normalization by `std.path.normalize`, while safe joining beyond its admitted caller-buffer operation, traversal policy, and platform conversion remain Missing |
| `std.path.value` | Source-authored typed lexical Path values and caller-buffer composition | Partial: ordinary nongeneric `Path` records combine a `Bytes` backing value with a `usize` logical length; NUL-free POSIX lexical bytes, checked bounds, consuming parent/finish transitions, and caller-buffer join are admitted through the internal profile, with focused local interpreter, C11 O0/O2, Core-Wasm, graph replay, contract, and bundled-dependency evidence passing; UTF-8 interpretation, filesystem authority, public generic or nominal widening, and broader path policy remain Missing |
| `std.path.normalize` | Lexical normalization of typed Path values | Partial: additive sibling package depending on `std.path.value`. `normalized-len` and `normalized-byte` give the exact normalized length and each byte of a borrowed view with no buffer; `path-length` lifts that to a borrowed `Path`; `into(borrow Path, own Bytes) -> Path` writes the normalized form into caller-supplied capacity after an exact preflight and preserves the borrowed input. The policy is lexical: a separator run collapses, `.` vanishes, `..` cancels the nearest retained segment, an uncancelled `..` is kept for a relative path and dropped at an absolute root, a trailing separator is removed, and an empty result is `.` or `/` so a normalized Path is never zero bytes. Retention is decided without a stack, as the clamped maximum prefix sum of a forward walk that scores `..` as `+1` and an ordinary segment as `-1`. Ten named cases run as individual bounded projects on the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm, with hostile cases rejecting a short buffer, a forged Path, an out-of-range index and an out-of-range offset; this evidence is local. Platform conversion, symbolic-link or filesystem resolution, Unicode interpretation, traversal containment and any public export are Missing. [Path Normalization v1](PATH-NORMALIZATION-V1.md) owns the additive contract |
| `std.fs` | Scoped file and directory access, metadata, and atomic file operations | Partial: private `filesystem-io.v2` composes owned Path, Reader/Writer, and Copy FileInfo values through read, create-new write, metadata, canonical immediate-name listing, directory creation/removal, and atomic replacement. Explicit providers execute typed commands in the interpreter, C11 O0/O2 and repeated Core Wasm; fixture and Unix directory-descriptor providers retain bounded authority. Recursive traversal, streaming, richer metadata, platform path conversion and cross-platform physical-provider promotion remain open. [Filesystem I/O v1](FILESYSTEM-IO-V1.md) stays frozen and [v2](FILESYSTEM-IO-V2.md) owns the additive private contract |
| `std.env` | Explicit environment access with capability and deterministic test replacement | Partial: registered private `environment-io.v1` source package with bounded snapshot-backed count, UTF-8 name/value observations, key lookup, and caller-owned Writer copying. Focused local Project commands pass on the interpreter, native C11, and Core Wasm; broader Everyday environment support remains Missing |
| `std.process` | Bounded process launch, pipes, exit, and settlement | Partial: private `process-io.v1` composition now executes the example, conformance, and bundled consumer commands on the interpreter, native C11 `-O0`/`-O2`, and Core Wasm. The historical five-case physical Darwin witness passed, covering registered launch, pipe/deadline and settlement behavior; the admitted release regression corpus is HOSTED GREEN, while broader physical-provider, public and general process support remain open |
| `std.time` | Durations, monotonic time, wall time, and deadlines | Partial: nonnegative millisecond conversion/decomposition with floor and ceiling rounding, elapsed and remaining-duration calculation, deadline comparison, and saturating duration addition; duration types, clock reads, instants, sleeps, and timers are Missing |
| `std.random` | Deterministic seeded generators and separately capability-gated secure randomness | Partial: pure Park–Miller seed normalization, next-step generation, bounded advancement, and sampling below an upper bound; stateful generators, unbiased range sampling, byte filling, and capability-gated secure randomness are Missing |
| `std.net` | Addresses, DNS, TCP, UDP, and explicit target support | Partial: pure helpers and the v1 TCP client operations now have a hosted-only bind/accept extension; native/Wasm service ABI, structured addresses, DNS policy, and UDP are Missing |
| `std.tls` | Vetted provider-backed TLS interface and certificate policy | Partial: the explicit Rust host supports authenticated outbound TLS 1.2/1.3 and server-side TLS with caller-installed certificate/key policy; source-level server TLS and native-C11/Wasm lanes are Missing |
| `std.http` | HTTP request/response types, client and server interfaces, streaming, and limits | Partial: allocation-free HTTP/1.x parsing helpers remain portable; hosted source, Core Wasm, and the Project v13 npm/Web lane can call bounded `https_get`, and `examples/https-project` reads a typed status code and body length out of a borrowed view of its canonical bytes; the explicit Rust host exposes the typed HTTP/1.1/2 response, redirects, pooling, and body limits. The Project v13 `https-command-io.v1` profile emits a libcurl-backed native C11 executable with embedded roots. A `[dependencies]` import of this package under the frozen Project v13 manifest, an owned typed response record, a server parser, HTTP/3, and the browser-Fetch adapter are Missing |
| `std.data.json` | Typed and value-based JSON parsing and encoding | Partial, and split across seven sibling packages because one library module large enough to hold the whole slice exceeds the `SPX-G171` pre-bound. `std.data.json` is the allocation-free JSON string-token scanner over `borrow Slice<u8>`: whitespace skipping, escape classification, `\uXXXX` decoding, strict surrogate-pair and control-byte rules, and the byte offset of the first rejection carried in the same `usize` result. Escape *expansion* is `std.data.json.dec`; an owned document tree is Missing. [Bounded JSON Scanner v1](BOUNDED-JSON-SCANNER-V1.md) owns the result encoding and policy shared by all seven |
| `std.data.json.dec` | Decoded JSON strings | Partial: escape expansion. `decoded_len` is the exact decoded UTF-8 byte length of one JSON string, `decoded_size` the whole-input form, and `emit_len`/`emit_at` with `token_end` are the pull-based per-token surface a caller streams without any buffer. `decoded_eq` remains the bounded buffer-backed comparison helper with its fixed 256-byte allocation. The additive cursor adapter `decode_into(borrow Reader, own Writer) -> Writer` decodes into caller-supplied writer capacity with exact preflight, no hidden allocation, and unchanged borrowed-reader state; standalone interpreter, native C11 O0/O2, and repeated Core Wasm cases include decoded strings over 256 bytes. All eight simple escapes, `\uXXXX`, and surrogate pairs expand. Scanner helpers retain source-offset failure encoding; the cursor adapter rejects malformed input through checked preconditions. The additive `decoded_token_eq` compares two JSON string tokens in one input by their decoded bytes through the same pull surface and with no buffer, so `"a\u0062"` and `"ab"` are equal keys. Per-output-index decoding, duplicate-key *detection over a document* (the object walk lives in `std.data.json.doc`), and an owned document tree remain Missing. [JSON Cursors v1](JSON-CURSORS-V1.md) owns the additive adapter contract |
| `std.data.json.doc` | Structural JSON documents | Partial: the object and array grammar over a base-2 container stack in one `i64`, an explicit `depth_limit` clamped to 32 open containers, the RFC 8259 number grammar, exact `true`/`false`/`null`, string framing that rejects raw `0x00`-`0x1F` and an unterminated string, a mismatched closer rejected at its own offset, and trailing-byte rejection with the offset of the first trailing non-whitespace byte. a duplicate-key rule over byte-identical member names as `is_unique`/`unique_end`, leaving `is_document` accepting a repeated name. Escape-character and surrogate validity stay with `std.data.json`, raw UTF-8 with `std.data.json.utf8`, and duplicate detection over decoded names is Missing |
| `std.data.json.token` | JSON number and literal tokens | Partial: the complete RFC 8259 number grammar as separate integer, fraction, and exponent scanners, `true`/`false`/`null` recognition, and exact `i64` decoding that rejects overflow and any token carrying a fraction or exponent rather than rounding it. Shares the scanner's `usize` end-offset/rejection encoding. Floating-point decoding and arbitrary-precision numbers are Missing, and float decoding is blocked at the project boundary rather than in the language: `f64` literals, arithmetic, and comparison are admitted inside a function and implemented on all three backends, but no workspace project profile admits `f64` as a parameter or a return type (`useful_data_workspace_parameter_admitted` and `useful_data_workspace_return_admitted` in `src/hir/workspace_link.rs`), so any std function carrying one is rejected with `SPX-G174` |
| `std.data.json.utf8` | UTF-8 validation of raw JSON bytes | Partial: sequence-width classification, scalar decoding, and whole-input validation that rejects invalid lead bytes, missing or malformed continuations, overlong encodings, raw surrogates, and scalars above `U+10FFFF`, reporting the first offending byte through the scanner's rejection encoding |
| `std.data.json.write` | Deterministic JSON string encoding | Partial: a pull-based, allocation-free encoder. `quoted_len` gives the exact byte length of the quoted JSON encoding of a byte view and `quoted_byte` gives its byte at an index, applying the two-byte named escapes, `\u00XX` for the remaining control bytes, and byte-identical pass-through for everything else including raw UTF-8. The additive cursor adapters `quoted_into(borrow Reader, own Writer) -> Writer` and `count_into(value: usize, own Writer) -> Writer` use caller-supplied writer capacity, exact preflight, and no hidden allocation; standalone interpreter, native C11 O0/O2, and repeated Core Wasm cases pass. Pretty-printing and document assembly remain Missing |
| `std.data.json.digits` | Deterministic JSON number and literal encoding | Partial: exact decimal rendering of any `i64` including `-9223372036854775808`, by length and by byte index, with no value passing through an `f64`, plus the `true`/`false`/`null` literal bytes. Floating-point rendering is Missing, blocked by the same project-boundary `f64` admission as float decoding in `std.data.json.token` |
| `std.data.toml` | TOML parsing and encoding | Partial: allocation-free bare-key validation, blank/comment line recognition, and simple-quote/comment-aware assignment-delimiter location over borrowed bytes; escaped and complete quoted-key validation, values, tables, decoding, validation, and encoding are Missing |
| `std.data.csv` | Streaming CSV reading and writing | Partial: allocation-free single-record field counting with quoted-comma and escaped-quote handling, balanced-quote checks, and strict complete-record quote-placement validation; Additive quote-aware field cursors walk one record: `field_end` stops at the first comma outside quotes, `field_start` opens the next field, `field_is_quoted` reports the quoted form, and `content_start`/`content_end` bound the field's content excluding the surrounding quotes, with `""` inside a quoted field treated as one escaped quote that never ends the field and left in the content bytes. Empty records, empty quoted fields and consecutive commas all yield exact offsets, and a walk terminates because the last field's `field_start` is the record length. The cursors compose with the `std.io.lines` record content and the `std.bytes` trimming offsets; this cursor evidence is local. Typed fields, decoded content, dialects, streaming reads, and writing are Missing |
| `std.encoding` | Base encodings, hex, UTF, and safe binary conversion | Partial: ASCII-byte classification, hexadecimal nibble conversion, byte-pair decoding, lowercase/uppercase hex digit encoding, and standard Base64 digit conversion plus unpadded quad decoding; buffer codecs, padded/streaming base encodings, and UTF conversion are Missing |
| `std.encoding.base64` | Padded standard Base64 encoding over a borrowed view | Partial: additive sibling package depending on `std.encoding`. `len` is the padded output length and `byte` is its byte at one index, computed from the input alone with no buffer, so a caller writes the digits into any capacity it owns. A short final group pads with `=`, and the encoding matches the standard alphabet byte for byte on the empty input and every residue class. It is a sibling package because `std.encoding` is on the default Project v1 route, which rejects a borrowed-view parameter. Decoding of padded input, streaming, and URL-safe or unpadded alphabets are Missing. [Base64 v1](BASE64-V1.md) owns the additive contract |
| `std.url` | URL parsing, normalization, and query handling | Partial: RFC-style ASCII scheme and unreserved-byte classification plus percent-triplet validation and decoding through `std.encoding`; structured URLs, parsing, normalization, resolution, query handling, and encoding are Missing |
| `std.regex` | Bounded regular-expression API or a first-party bundled package | Missing |
| `std.sync` | Mutexes, read/write locks, atomics, and synchronization contracts | Missing |
| `std.task` | Structured tasks, cancellation, scheduling, and channels | Missing; [Scoped Task Model v1](SCOPED-TASKS-V1.md) is the design. `std.async` holds the pure bounded-readiness-loop helpers (timeout clamping, exponential backoff, retry policy, round-robin handle selection, stream-end detection) for `net_wait`-driven loops |
| `std.log` | Structured logging with field identities and redaction | Partial: source-authored `Event` values carry level, sequence, name, and message fields into one caller-owned Writer as a complete JSON-lines object. Level and fixed-byte helpers, decimal sequence formatting via `std.data.json.write.count_into`/`usize_len`, JSON quoting, UTF-8 validation, and exact whole-event capacity preflight are composed through the private `useful-data.v2` profile with exactly bundled `std.data.json.utf8`, `std.data.json.write`, and `std.io` dependencies. The canonical package has a 59-byte escaped-line golden and five unwritten zero suffix bytes; fifteen expanded cases are in `tests/project/standard_library/log_cases.spx`, with exact local call closures and required type imports. The original focused gates `logging::log_writer_executes_on_all_three_backends` and `logging::log_writer_preflight_rejects_invalid_events` pass: all fifteen cases plus the canonical package execute on interpreter, C11 `-O0`/`-O2`, and repeated Core Wasm with exact zero-to-three live Bytes bounds; nine rejection cases pass twice with exact contract status. The existing UTF-8 package conformance also passes after the direct ASCII scan optimization. Additive level filtering makes the policy explicit: `level_enabled` compares a level against a threshold on the existing 0-5 scale, `event_admitted` is the borrowed observer that is true only when the event both passes the threshold and fits the live capacity, `discard_event` is the named drop path that consumes the event and returns the Writer untouched, and `append_event_if` writes a passing event exactly as `append_event` does while requiring capacity only for an event actually written. A filtered event is dropped rather than buffered and releases its bytes through ordinary lexical cleanup. Three named filtering cases run with the existing corpus as individual bounded projects on the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm; this filtering evidence is local. General logging sinks, timestamps, redaction, queues and concurrency remain Missing. [Log Writer v1](LOG-WRITER-V1.md) owns the additive contract |
| `std.metrics` | Counters, gauges, histograms, and effect-neutral instrumentation | Missing |
| `std.test` | Assertions, fixtures, property tests, fuzz targets, and snapshots | Partial: scalar equality predicates and deterministic unit or caller-selected failure status helpers remain in `std.test`; the sibling `std.test.bytes` package adds exact `Slice<u8>` equality, Reader-suffix equality with cursor validation and position preservation, and failure-bit wrappers. Reusable named byte snapshots additionally return a checked Copy comparison with equality, lengths, and relative first difference while preserving both borrowed inputs; source-package and bundled-consumer checks have HOSTED GREEN release evidence across the interpreter, C11 O0/O2, and repeated Core Wasm. Additive failure-mask discipline makes a nonzero test result diagnosable rather than merely nonzero: `bit_for` is the bit an indexed case owns, `bit_is_set` reports membership, `record_failure` accumulates one case's verdict and refuses a bit another case already claimed, and `first_failure` and `failure_count` report the lowest failing index and the number of failures. Indexes run 0 to 62 and the accumulated mask is monotonic, so two cases cannot silently share a bit and hide one another; the package's own conformance and examples now use it. This mask evidence is local. Snapshot file discovery/updates, richer property tests, fuzz targets, and broader diagnostics remain open |
| `std.agent` | Agent types, model roles, context, Proposal grammar, approval, effects, checkpoints, and evidence | Partial: the private package supplies source-authored Task, Context, Observation, Outcome and lifecycle/outcome helpers. Package execution and injected-runtime linked-role regressions are HOSTED GREEN for v0.4.0. [Direct Runtime v2](AGENT-RUNTIME-V2.md), [linked lifecycle](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md) and [linked migration](PROJECT-LINKED-AGENT-MIGRATION-V1.md) implement their own typed, durable and revision-bound additions. Native/Wasm execution of ordinary library functions does not execute Agent stages there. Native/Wasm Agent stages, live providers, public support and full module scope remain open |

A module is Partial when its package passes the gate on every listed target
and Implemented only when its required scope is complete and the
[completion matrix](COMPLETION-MATRIX.md) row says so.

### Naming

Functions are `snake_case`. A module whose required scope does not fit the
per-package admission limits described below is split into dotted
sub-modules, each its own package: `std.num.overflow` is the wrapping and
saturating half of `std.num`. Stable identities follow the module:
`std.num.overflow.wrapping_add`. Ordering is the `i64` triple `-1`, `0`, `1`
until variants cross the Project boundary; `std.core.ordering.less` and its
siblings name the values.

### Admission limits that shape packages today

These compiler bounds decide how large one package can be:

- Project v1 links only functions whose parameters and result are by-value
  `i64` or `bool`, admits exactly one test module and one entry module, and
  requires between one and thirty-two `web_exports`. Records, variants,
  generics, strings, and bytes therefore stay inside one file until a wider
  Project profile admits them across files.
- The internal owned-data library profile used by `std.io` admits ordinary
  nongeneric records over Bytes and Copy scalars through authenticated Project
  imports. An empty export list selects checked entry/test execution without a
  public descriptor. Nonempty public selections retain their original rules;
  public generic and nominal ABI admission remains unchanged.
- The Workspace Semantic Graph pre-bound charges an upper estimate of resolver
  memory against a 16 MiB budget before linking. The split pre-bound
  described in [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget)
  admits the current packages and a consumer that links both `std.core` and
  `std.num`; before it, a 4.9 KiB module of twenty scalar functions was rejected with
  `SPX-G171`. An imported function is now charged as the stub the projection
  actually retains, so a conformance module no longer costs a second complete
  copy of the library it exercises. The budget is charged against the whole
  package - library, examples, and conformance modules together with any
  vendored `[dependencies]` source - not against the library module alone.
  Measured on the seven JSON sibling packages by padding each library with
  trivial `i64` helpers until `SPX-G171` fires, the admitted total package
  source is between 19.7 KB and 22.1 KB, varying with declaration and
  expression structure rather than with byte count alone. The same measurement
  before the identity term was re-derived gave 12.3 KB to 14.2 KB, which is
  why `std.data.json.doc` shipped at 12,216 bytes with no headroom; the
  re-derivation is recorded in
  [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget).
  A slice larger than that is
  authored as sibling packages that a consumer links, which
  `standard_library::package_manifest_links_json_scanner_and_token_siblings`
  and `..._json_writer_siblings` exercise; taking a dependency on a sibling
  spends that package's whole source against the budget, so a package that can
  restate a three-line helper locally stays cheaper standing alone.
- The owned bounded byte buffer of
  [Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) and the
  nongeneric Reader/Writer records compose internally under `owned-data-api.v1`
  or the additive `useful-data.v2` profile. These internal calls do not expose
  an owning cursor through the frozen public byte facade. Project v16 retains
  the actual entry closure alongside selected public exports, so an entry
  beyond that public emitter's profile requires an empty export list and
  private execution. The scalar profile still refuses `borrow Slice<u8>`
  parameters (`SPX-G174`). [Project v16](PROJECT-MANIFEST-V16.md) specifies the
  separate internal and public admission rules.
- `byte_range` over an owned buffer's view lowers to the Core Wasm byte-range
  *descriptor* carrier, which the standard-library conformance closure does not
  implement; a library function that must compare a prefix takes an explicit
  length instead.
- The cleanup-plan replay path budget (`SPX-H006`) admits at most 65,536
  terminal paths in one function. Lazy `&&` and `||` operands and `if`
  branches each double that count, so a function holds about sixteen
  independent decision points. A dense predicate written as one long lazy
  chain is rejected; the fix is to split it, or to replace the chain with a
  `match` table, which contributes one path per arm rather than doubling. This
  is why a conformance module keeps each check in its own function.
- Path counts *multiply* across sequential statements and only *sum* across
  `match` arms. Two tables of eight arms each cost sixty-four paths in one
  function body and sixteen in two, so packing several small tables into one
  function can trip `SPX-H006` where splitting them apart does not. This is
  the opposite of the usual intuition that fewer, larger functions are
  cheaper, and it compounds with the per-function path doubling above.
- `match` is not admitted in a `while` body (`SPX-T252`, `match expressions
  are not yet admitted in while bodies`). The single exception is a two-arm
  `Option::Some`/`Option::None` match, without guards, directly on a
  `byte_get(...)` call. Every other table lookup inside a loop is authored as
  a helper function the loop calls, which is also what the path budget above
  wants: the helper's arms sum inside the helper instead of multiplying into
  the loop body. `while` bodies also reject string literals, fixed-array
  literals, and `?` with the same code.
- The byte-data profile (`useful-data.v1`) admits contracts throughout its
  inventory since the data emitter and npm recipe learned to lower and record
  them; `std.bytes` is a Project v3 package on that profile. Its web exports
  may take only `borrow Slice<u8>` parameters, so a function with a scalar
  parameter such as `count(view, needle)` is exported to the interpreter and
  native lanes but not selected as a web export.
- The text profile (`useful-text-consumer.v1`) links imported functions with
  exact non-escaping `borrow str` parameters across files. Other borrowed,
  shared, owning, stored, or returned text shapes remain outside the profile.

Both size bounds are per function and per module, so the practical shape of a
large package is many small functions rather than a few long ones. `SPX-G171`
names the budget that overflowed in its message: `builder_bytes` is the
pre-bound estimate above, and it limits neither source size nor declaration,
call, or use counts directly, which have their own budgets and their own names.

These bounds are compiler facts, not library design. Lifting them is tracked
in the [roadmap](ROADMAP.md#standard-library-outcomes).

## Effect vocabulary

Hosted modules declare their authority in the signature. The vocabulary is the
dotted effect grammar the verifier already checks: a function that performs an
effect lists it under `uses`, the module grants it under `permit`, and a
caller must declare every callee effect (`SPX-E101`, `SPX-E102`). The
canonical hosted signatures are:

```semaprax
module std.effects.examples;

permit { clock.read, filesystem.read, network.connect, random.secure }

@id("std.effects.examples.read_text_length")
fn read_text_length(path_bytes: i64) -> i64
    uses { filesystem.read }
{
    path_bytes
}

@id("std.effects.examples.now")
fn now() -> i64
    uses { clock.read }
{
    0
}

@id("std.effects.examples.connect")
fn connect(endpoint: i64) -> i64
    uses { network.connect }
{
    endpoint
}

@id("std.effects.examples.secure_bytes")
fn secure_bytes(length: i64) -> i64
    uses { random.secure }
{
    length
}

@id("app.main")
fn main() -> i64
    uses { clock.read, filesystem.read, network.connect, random.secure }
{
    read_text_length(0) + now() + connect(0) + secure_bytes(0)
}
```

The mature signatures are
`fn read_text(path: borrow Path) -> Result<String, FsError> uses { filesystem.read }`,
`fn now() -> Instant uses { clock.read }`,
`fn connect(endpoint: borrow Endpoint) -> Result<Connection, NetworkError> uses { network.connect }`,
and `fn secure_bytes(length: usize) -> Result<Bytes, RandomError> uses { random.secure }`;
the block above spells only the effect discipline in the admitted scalar
subset, and `tests/documentation.rs` checks that it verifies. Tests must be
able to replace each effect with a deterministic handler without changing
application logic; the network fixture provider of
[Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md) is the
first such handler (local evidence, injected by the host, not selectable from
source); a general handler mechanism is Missing and belongs to the `test` tier.

## The Everyday profile

The default hosted distribution should include an Everyday profile with CLI
argument parsing; filesystem and path APIs; strings, bytes, and collections;
JSON, TOML, and CSV; logging; an HTTP client and server; structured tasks;
testing; package management; and Agent runtime APIs. Every item is Missing
except the compiler-owned command I/O described in
[Bounded Language Command I/O v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md).

Of the default project templates, `library` exists offline through the
public capsule: `semaprax project-scaffold --name <name> --template library`
prints a package in the shape described under
[library architecture](#library-architecture), verified and tested at
derivation. Both standalone `semaprax new --template library` and the full
toolchain's hardened held-parent route publish that exact six-file inventory.
The `cli`, `service`, `web`, and `agent` templates are Missing. `new` stays in
the full toolchain by release policy.

## Evidence and nonclaims

`tests/project.rs::standard_library` proves, for every package under `std/`:

- the library, examples, and conformance sources are canonical;
- every library function has an explicit `@id` below the module name, no
  effects, and an import in the conformance module, and the examples module
  imports at least one;
- examples and conformance return `0` on the interpreter, on native C11 at
  `-O0` and `-O2`, and, for the conformance closure, on Core Wasm under Node;
- the committed catalogs equal the generated ones.

`tests/examples.rs` additionally holds every `.spx` file below `std/` to the
canonical form. Nothing here claims a package registry, ordinary-package
build integration, hosted effects, deterministic handlers, or any module
outside the packages listed in `std/packages.json`.
