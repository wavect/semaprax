# Standard Library v1

- Status: versioned reference; the `core`-tier slice under `std/` is
  executable, every other module in the required set is Missing.
- Audience: standard-library authors, compiler contributors, and agents
  choosing between a compiler-owned function and a library declaration.

This document owns the standard-library contract: how the library is
composed, what every public declaration must carry, the portability tiers, the
required module set, the effect vocabulary hosted modules declare, and the
Everyday profile. The [completion matrix](COMPLETION-MATRIX.md) owns status;
the generated [standard library catalog](STANDARD-LIBRARY-CATALOG.md) lists the
declarations that exist today; `tests/project.rs::standard_library` is the
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
slice holds functions over `i64` and `bool` only.

`std/packages.json` lists every package directory with its module, tier,
targets, and status. The gate fails when the list and the directories under
`std/` disagree.

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
| `std.num` | Checked, wrapping, saturating, and conversion operations | Partial: sign, absolute value, parity, Euclidean division and remainder, and greatest common divisor in `std.num`; overflow predicates and wrapping and saturating addition, subtraction, negation, absolute value, and multiplication in `std.num.overflow`; checked arithmetic is the language default; wrapping multiplication is Missing |
| `std.iter` | Iterators, adapters, folds, collection, and ranges | Missing; needs interfaces and closures |
| `std.mem` | Ownership helpers, regions, arenas, boxes, shared immutable values | Missing |
| `std.collections` | Vector, deque, map, set, heap, and fixed-capacity collections | Missing; needs the `alloc` tier |
| `std.bytes` | Buffers, spans, readers, writers, endian operations, and encoding | Missing; the compiler-owned byte functions are the current surface |
| `std.text` | UTF-8 strings, Unicode iteration, search, split, trim, and normalization policy | Missing; the compiler-owned string and `str` functions are the current surface |
| `std.format` | Type-safe formatting without runtime format-string ambiguity | Missing |
| `std.io` | Reader, Writer, buffered I/O, streams, line processing, and standard streams | Missing; `stdout_write`, `stderr_write`, and `stdin_read` are the current surface |
| `std.path` | Platform-neutral path values and explicit platform conversion | Missing |
| `std.fs` | Scoped file and directory access, metadata, and atomic file operations | Missing |
| `std.env` | Explicit environment access with capability and deterministic test replacement | Missing; `args_len` and `arg_utf8` are the current surface |
| `std.process` | Bounded process launch, pipes, exit, and settlement | Missing |
| `std.time` | Durations, monotonic time, wall time, and deadlines | Missing |
| `std.random` | Deterministic seeded generators and separately capability-gated secure randomness | Missing |
| `std.net` | Addresses, DNS, TCP, UDP, and explicit target support | Missing |
| `std.tls` | Vetted provider-backed TLS interface and certificate policy | Missing |
| `std.http` | HTTP request/response types, client and server interfaces, streaming, and limits | Missing |
| `std.data.json` | Typed and value-based JSON parsing and encoding | Missing |
| `std.data.toml` | TOML parsing and encoding | Missing |
| `std.data.csv` | Streaming CSV reading and writing | Missing |
| `std.encoding` | Base encodings, hex, UTF, and safe binary conversion | Missing |
| `std.url` | URL parsing, normalization, and query handling | Missing |
| `std.regex` | Bounded regular-expression API or a first-party bundled package | Missing |
| `std.sync` | Mutexes, read/write locks, atomics, and synchronization contracts | Missing |
| `std.task` | Structured tasks, cancellation, scheduling, and channels | Missing; [Scoped Task Model v1](SCOPED-TASKS-V1.md) is the design |
| `std.log` | Structured logging with field identities and redaction | Missing |
| `std.metrics` | Counters, gauges, histograms, and effect-neutral instrumentation | Missing |
| `std.test` | Assertions, fixtures, property tests, fuzz targets, and snapshots | Missing; conformance modules use the bitmask convention above |
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
  admits the three current packages; before it, a 4.9 KiB module of twenty
  scalar functions was rejected with `SPX-G171`. A conformance module keeps
  each check in its own function so the cleanup-plan replay stays under its
  path budget.

Both bounds are compiler facts, not library design. Lifting them is tracked
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
application logic; that handler mechanism is Missing and belongs to the `test`
tier.

## The Everyday profile

The default hosted distribution should include an Everyday profile with CLI
argument parsing; filesystem and path APIs; strings, bytes, and collections;
JSON, TOML, and CSV; logging; an HTTP client and server; structured tasks;
testing; package management; and Agent runtime APIs. Every item is Missing
except the compiler-owned command I/O described in
[Bounded Language Command I/O v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md).

The default project templates `semaprax new cli`, `semaprax new service`,
`semaprax new library`, `semaprax new web`, and `semaprax new agent` are
Missing; `semaprax new` and `semaprax project-scaffold` emit the calculator
template only, and `new` stays in the full toolchain by release policy.

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
canonical form. Nothing here claims a package registry, cross-package
imports, hosted effects, deterministic handlers, or any module other than the
three listed packages.
