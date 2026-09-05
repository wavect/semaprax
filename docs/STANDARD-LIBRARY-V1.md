# Standard Library v1

- Status: versioned reference; eight `core`-tier packages, eight
  `portable`-tier packages, and one `test`-tier package under `std/` are
  executable; every other module in the required set is Missing.
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

## Library architecture

The standard library is composed of ordinary SEMAPRAX packages. A package is a
directory below `std/` holding a Project manifest and three modules:

| Module | Role |
| --- | --- |
| `std.<name>` | The library module. It declares only public functions and types; it never defines `main`. |
| `std.<name>.examples` | The Project entry. Its `main` demonstrates idiomatic use and returns `0`. |
| `std.<name>.tests` | The single Project test module. Its `main` is the conformance suite and returns `0`, or a bitmask naming the failed checks. |

Host-specific operations may be implemented in Rust, C, Wasm, or a platform
language, but their public contract is a SEMAPRAX semantic interface: a
declaration in a `std.*` module with an `@id`, types, ownership modes,
effects, and contracts. The compiler-owned functions listed in the
[agent quick reference](AGENT-QUICK-REFERENCE.md#compiler-owned-functions) are
the current host surface; moving them behind `std.*` interfaces is an open
gate of this document, not a completed step.

Every public standard-library declaration must have:

1. a stable identity: an explicit `@id` below its module name, so
   `std.num.gcd` names one function across renames and moves;
2. types and ownership modes spelled in the signature;
3. effects, declared with `uses` and granted by the module's `permit`; the
   `core` tier declares none;
4. contracts: `requires` and `ensures` lines that state the admitted inputs
   and the guaranteed result;
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
yet a gate failure. Records, variants, and generic declarations are admitted
by the language but not yet by the cross-file Project route, so the current
slice holds functions over `i64`, `bool`, `u8`, `usize`, `borrow Slice<u8>`,
and `borrow str` only. The Useful Text public export profile remains
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
| `std.num` | Checked, wrapping, saturating, and conversion operations | Partial: sign, absolute value, parity, Euclidean division and remainder, greatest common divisor, checked power, integer square root, digit count, power-of-two test, and floor logarithms in base 2 and 10 in `std.num`; overflow predicates and wrapping and saturating addition, subtraction, negation, absolute value, and multiplication in `std.num.overflow`; checked arithmetic is the language default; wrapping multiplication is Missing |
| `std.iter` | Iterators, adapters, folds, collection, and ranges | Missing; needs interfaces and closures |
| `std.mem` | Ownership helpers, regions, arenas, boxes, shared immutable values | Missing |
| `std.collections` | Vector, deque, map, set, heap, and fixed-capacity collections | Missing; needs the `alloc` tier |
| `std.bytes` | Buffers, spans, readers, writers, endian operations, and encoding | Partial: byte-to-integer conversion, guarded indexing, first-index search, counting, ASCII classification, slice equality, prefix and suffix tests, and little- and big-endian 16- and 32-bit reads over `borrow Slice<u8>`; buffers, writers, and encodings are Missing |
| `std.text` | UTF-8 strings, Unicode iteration, search, split, trim, and normalization policy | Partial: borrowed byte length, emptiness, exact equality, prefix, and substring search; iteration, split, trim, and normalization are Missing |
| `std.format` | Type-safe formatting without runtime format-string ambiguity | Missing |
| `std.io` | Reader, Writer, buffered I/O, streams, line processing, and standard streams | Missing; `stdout_write`, `stderr_write`, and `stdin_read` are the current surface |
| `std.path` | Platform-neutral path values and explicit platform conversion | Partial: allocation-free inspection of canonical slash-separated path bytes for absoluteness, trailing separators, nonempty segment count, filename start, parent boundary, and extension boundary; typed path values, normalization, safe joining, traversal policy, and platform conversion are Missing |
| `std.fs` | Scoped file and directory access, metadata, and atomic file operations | Missing |
| `std.env` | Explicit environment access with capability and deterministic test replacement | Missing; `args_len` and `arg_utf8` are the current surface |
| `std.process` | Bounded process launch, pipes, exit, and settlement | Missing |
| `std.time` | Durations, monotonic time, wall time, and deadlines | Partial: nonnegative millisecond conversion/decomposition with floor and ceiling rounding, elapsed and remaining-duration calculation, deadline comparison, and saturating duration addition; duration types, clock reads, instants, sleeps, and timers are Missing |
| `std.random` | Deterministic seeded generators and separately capability-gated secure randomness | Partial: pure Park–Miller seed normalization, next-step generation, bounded advancement, and sampling below an upper bound; stateful generators, unbiased range sampling, byte filling, and capability-gated secure randomness are Missing |
| `std.net` | Addresses, DNS, TCP, UDP, and explicit target support | Partial: pure helpers and the v1 TCP client operations now have a hosted-only bind/accept extension; native/Wasm service ABI, structured addresses, DNS policy, and UDP are Missing |
| `std.tls` | Vetted provider-backed TLS interface and certificate policy | Partial: the explicit Rust host supports authenticated outbound TLS 1.2/1.3 and server-side TLS with caller-installed certificate/key policy; source-level server TLS and native-C11/Wasm lanes are Missing |
| `std.http` | HTTP request/response types, client and server interfaces, streaming, and limits | Partial: allocation-free HTTP/1.x parsing helpers remain portable; hosted source can call bounded `https_get` and parse its canonical bytes, while the explicit Rust host exposes the typed HTTP/1.1/2 response, redirects, pooling, and body limits. A source-level typed response API, server parser, HTTP/3, and generated target adapters are Missing |
| `std.data.json` | Typed and value-based JSON parsing and encoding | Partial: an allocation-free JSON string-token scanner over `borrow Slice<u8>` with whitespace skipping, escape classification, `\uXXXX` decoding, strict surrogate-pair and control-byte rules, and the byte offset of the first rejection carried in the same `usize` result. Number and literal tokens, structural document validation, UTF-8 validation, decoded strings, an owned document tree, and a writer are Missing; the `SPX-G171` pre-bound, not the design, is what bounds the admitted scope. [Bounded JSON Scanner v1](BOUNDED-JSON-SCANNER-V1.md) owns the result encoding and policy |
| `std.data.toml` | TOML parsing and encoding | Partial: allocation-free bare-key validation, blank/comment line recognition, and simple-quote/comment-aware assignment-delimiter location over borrowed bytes; escaped and complete quoted-key validation, values, tables, decoding, validation, and encoding are Missing |
| `std.data.csv` | Streaming CSV reading and writing | Partial: allocation-free single-record field counting with quoted-comma and escaped-quote handling, balanced-quote checks, and strict complete-record quote-placement validation; typed fields, record iteration, dialects, streaming reads, and writing are Missing |
| `std.encoding` | Base encodings, hex, UTF, and safe binary conversion | Partial: ASCII-byte classification, hexadecimal nibble conversion, byte-pair decoding, lowercase/uppercase hex digit encoding, and standard Base64 digit conversion plus unpadded quad decoding; buffer codecs, padded/streaming base encodings, and UTF conversion are Missing |
| `std.url` | URL parsing, normalization, and query handling | Partial: RFC-style ASCII scheme and unreserved-byte classification plus percent-triplet validation and decoding through `std.encoding`; structured URLs, parsing, normalization, resolution, query handling, and encoding are Missing |
| `std.regex` | Bounded regular-expression API or a first-party bundled package | Missing |
| `std.sync` | Mutexes, read/write locks, atomics, and synchronization contracts | Missing |
| `std.task` | Structured tasks, cancellation, scheduling, and channels | Missing; [Scoped Task Model v1](SCOPED-TASKS-V1.md) is the design. `std.async` holds the pure bounded-readiness-loop helpers (timeout clamping, exponential backoff, retry policy, round-robin handle selection, stream-end detection) for `net_wait`-driven loops |
| `std.log` | Structured logging with field identities and redaction | Missing |
| `std.metrics` | Counters, gauges, histograms, and effect-neutral instrumentation | Missing |
| `std.test` | Assertions, fixtures, property tests, fuzz targets, and snapshots | Partial: scalar equality predicates and deterministic unit or caller-selected failure status helpers for the current return-code test model; rich diagnostics, fixtures, property tests, fuzz targets, and snapshots are Missing |
| `std.agent` | Agent types, model roles, context, Proposal grammar, approval, effects, checkpoints, and evidence | Missing; [Agent Runtime v1](AGENT-RUNTIME-V1.md) is the design |

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

Two compiler bounds decide how large one package can be:

- Project v1 links only functions whose parameters and result are by-value
  `i64` or `bool`, admits exactly one test module and one entry module, and
  requires between one and thirty-two `web_exports`. Records, variants,
  generics, strings, and bytes therefore stay inside one file until a wider
  Project profile admits them across files.
- The Workspace Semantic Graph pre-bound charges an upper estimate of resolver
  memory against a 16 MiB budget before linking. The split pre-bound
  described in [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget)
  admits the current packages and a consumer that links both `std.core` and
  `std.num`; before it, a 4.9 KiB module of twenty scalar functions was rejected with
  `SPX-G171`. A conformance module keeps
  each check in its own function so the cleanup-plan replay stays under its
  path budget.

- The byte-data profile (`useful-data.v1`) admits contracts throughout its
  inventory since the data emitter and npm recipe learned to lower and record
  them; `std.bytes` is a Project v3 package on that profile. Its web exports
  may take only `borrow Slice<u8>` parameters, so a function with a scalar
  parameter such as `count(view, needle)` is exported to the interpreter and
  native lanes but not selected as a web export.
- The text profile (`useful-text-consumer.v1`) links imported functions with
  exact non-escaping `borrow str` parameters across files. Other borrowed,
  shared, owning, stored, or returned text shapes remain outside the profile.

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
