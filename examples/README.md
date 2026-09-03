# SEMAPRAX examples

Audience: newcomers to the language and contributors looking for a minimal
subject to point a command at.

Status: this index records what each committed example demonstrates and which
command was observed to succeed on it. It is not a status claim.
[Completion matrix](../docs/COMPLETION-MATRIX.md) is the status authority, and
each row's reference owns the exact admission rules.

New here? Read the [documentation entry point](../docs/index.md) first, then
work through the executable [quickstart](../docs/QUICKSTART.md). The
[CLI guide](../docs/CLI-GUIDE.md) covers the commands used below.

## How to read the tables

`semaprax` below means the standalone CLI. From a checkout you can substitute
`cargo run --locked -p semaprax --` for it; the [quickstart](../docs/QUICKSTART.md)
covers installing it. A few rows need the private `semaprax-full` toolchain
CLI instead, and say so.

- `check` verifies a single `.spx` file or a `semaprax.toml` manifest.
- `run` executes the entry `main` and prints the returned `i64`. The observed
  value is shown as `→ 42`.
- `test` runs a project's declared test modules.
- `graph` emits the deterministic semantic graph; `context` answers bounded
  queries about one declaration. `graph` succeeded on every `.spx` file listed
  here. `context` was exercised on `examples/meaning.spx`,
  `examples/ownership.spx` and `examples/refutable_match.spx` only.
- `build` emits a target package into a directory you name with `-o`.

Every command and result in the tables below was executed locally against the
standalone `semaprax 0.2.0` CLI on macOS arm64. That is local evidence for one
host and one build. It is not hosted, release, or cross-platform evidence, and
the repository's own gates in [quality gates](../docs/QUALITY-GATES.md) remain
the authority on what is covered.

## Start here

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/meaning.spx` | `requires`/`ensures` contracts on a two-function module; the fixed subject of the graph and revision snapshots in `tests/examples.rs` | `semaprax run examples/meaning.spx` → `42` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/calculator.spx` | Six arithmetic and boolean functions with stable `@id` identities; the subject shared by the web and Rust projections below | `semaprax run examples/calculator.spx` → `42` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/control_flow.spx` | `let` bindings and `if` used as a value-producing expression | `semaprax run examples/control_flow.spx` → `42` | [RFC 0001](../docs/RFC-0001.md) |

## Language basics

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/effects.spx` | A module-level `permit` set with a function that declares `uses { clock.read }` | `semaprax run examples/effects.spx` → `42` | [RFC 0001](../docs/RFC-0001.md), [Capability Manifest v1](../docs/CAPABILITY-MANIFEST-V1.md) |
| `examples/classes.spx` | A class with a field and two methods, one of them returning a new instance | `semaprax run examples/classes.spx` → `42` | [Class Inheritance v1](../docs/CLASS-INHERITANCE-V1.md) |
| `examples/inheritance.spx` | A three-level class hierarchy with method override, `super` dispatch and an upcast binding | `semaprax run examples/inheritance.spx` → `6` | [Class Inheritance v1](../docs/CLASS-INHERITANCE-V1.md) |
| `examples/strings.spx` | String literals and structural string equality, nothing else | `semaprax run examples/strings.spx` → `1` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/string_ops.spx` | The `string_concat`, `string_len` and `string_is_empty` intrinsics | `semaprax run examples/string_ops.spx` → `7` | [String Operations v1](../docs/STRING-OPS-V1.md) |
| `examples/string_ops_v2.spx` | `string_starts_with`, `string_contains`, `string_len_chars` and `string_from_char` over non-ASCII text, including astral-plane input | `semaprax run examples/string_ops_v2.spx` → `7` | [String Operations v1](../docs/STRING-OPS-V1.md) |

`examples/effects.spx` is a useful reminder that a report generator can refuse
a program that checks: `semaprax capability-manifest examples/effects.spx`
reports `error[SPX-K202]` because `clock.read` is outside that report's
bounded capability vocabulary. Use `examples/ownership.spx` for a manifest that
does emit.

## Scalar types

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/integers_i32.spx` | `i32` arithmetic, negation, comparison and the near-minimum value, alongside an `i64` result | `semaprax run examples/integers_i32.spx` → `7` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/bytes_u8.spx` | `u8` literals, division and subtraction, a `u8` record field, and a `requires` clause guarding saturation | `semaprax run examples/bytes_u8.spx` → `7` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/chars.spx` | `char` literals including an escape, `char` ordering, and a `char` record field | `semaprax run examples/chars.spx` → `7` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/floats.spx` | `f64` and `f32` arithmetic, unary negation, and float-typed record fields | `semaprax run examples/floats.spx` → `7` | [RFC 0001](../docs/RFC-0001.md) |
| `examples/useful_data_usize_v1.spx` | Target-independent `usize` arithmetic and `%`, driven by mutable locals | `semaprax run examples/useful_data_usize_v1.spx` → `0` | [Portable Indexed Byte Data v1](../docs/PORTABLE-INDEXED-BYTE-DATA-V1.md) |

## Data and matching

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/records.spx` | Nested records, out-of-order field initialization, and nested `with` update expressions | `semaprax run examples/records.spx` → `42` | [RFC 0002](../docs/RFC-0002-ALGEBRAIC-DATA.md) |
| `examples/refutable_match.spx` | Refutable `match` over `i64`, `u8` and `char` with or-patterns, guards, binding arms and `_` | `semaprax run examples/refutable_match.spx` → `-5` | [Refutable Match v1](../docs/REFUTABLE-MATCH-V1.md) |

## Mutation and loops

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/explicit_mutation.spx` | `let mut` locals and reassignment at both `i64` and `i32` widths | `semaprax run examples/explicit_mutation.spx` → `500016` | [Explicit Mutation v1](../docs/EXPLICIT-MUTATION-V1.md) |
| `examples/field_mutation.spx` | Assignment to record and class fields, including inside both arms of an `if` | `semaprax run examples/field_mutation.spx` → `96` | [Field Mutation v1](../docs/FIELD-MUTATION-V1.md) |
| `examples/while_loops.spx` | Two `while` loops with their loop-continuation expressions, over mutable locals | `semaprax run examples/while_loops.spx` → `41` | [While Loops v1](../docs/WHILE-LOOPS-V1.md) |

## Ownership, resources and cleanup

These three declare a `resource`. All three verify, but `run` does not reach a
result: on this CLI build each reports
`error[SPX-B104]: native resource lowering requires lifecycle declarations and the verified cleanup ABI`.
Treat them as `check`, `graph` and `context` subjects.

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/ownership.spx` | `own` and `borrow` parameter modes, a `drop trivial` resource, and an `ensures` clause over a borrow-then-consume pipeline | `semaprax check examples/ownership.spx` → `verified …` (`run` fails with `SPX-B104`) | [RFC 0003](../docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| `examples/lifecycle.spx` | A resource whose `drop` names an imported finalizer, plus the `interface`/`permits`/`effects`/`failure`/`consumes` declaration that supplies it | `semaprax check examples/lifecycle.spx` → `verified …` (`run` fails with `SPX-B104`) | [RFC 0003](../docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| `examples/native_callable.spx` | The smallest owned-resource identity function; the subject for a native-callable bundle | `semaprax build examples/native_callable.spx --target native-callable --function example.token.identity -o /absolute/out/callable` → `built native-callable bundle …` | [Native Callable ABI v3](../docs/NATIVE-CALLABLE-ABI-V3.md), [RFC 0004](../docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md) |

## Projects and manifests

Each directory here is a multi-file project rooted at its own `semaprax.toml`.
Pass the manifest, not a source file. `check`, `test` and `run` all accept it,
and `build` accepts it too — `--target web` needs no `--export` list because
the manifest's `web_exports` already selects the surface.

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/calculator-project` | Project schema v1: three modules, cross-module `use function @id(...)`, a `tests` module and six `web_exports` | `semaprax test examples/calculator-project/semaprax.toml` → `project tests passed`; `run` → `42` | [Project Manifest v1](../docs/PROJECT-MANIFEST-V1.md) |
| `examples/config-validator-project` | Project schema v2 under the `useful-text-consumer.v1` profile: four modules over borrowed UTF-8 input | `semaprax test examples/config-validator-project/semaprax.toml` → `project tests passed`; `run` → `0` | [Project Manifest v2](../docs/PROJECT-MANIFEST-V2.md), [Useful Text Consumer v1](../docs/USEFUL-TEXT-CONSUMER-V1.md) |
| `examples/binary-frame-project` | Project schema v3 under the `useful-data.v1` profile: indexed byte data with a checksum and magic-number check | `semaprax test examples/binary-frame-project/semaprax.toml` → `project tests passed`; `run` → `0` | [Project Manifest v3](../docs/PROJECT-MANIFEST-V3.md), [Portable Indexed Byte Data v1](../docs/PORTABLE-INDEXED-BYTE-DATA-V1.md) |
| `examples/spxgrep-project` | Project schema v4 under `useful-data-command.v1`: a `command` entry with a single declared `process.stdout.write` capability | `semaprax test examples/spxgrep-project/semaprax.toml` → `project tests passed`; `run` → `0` | [Project Manifest v4](../docs/PROJECT-MANIFEST-V4.md), [Bounded Stdout Transcript v1](../docs/BOUNDED-STDOUT-TRANSCRIPT-V1.md) |
| `examples/spxgrep-native-command-project` | Project schema v5 under `useful-data-command.v2`: the same command shape with a declared `input` contract and four capabilities | `semaprax test examples/spxgrep-native-command-project/semaprax.toml` → `project tests passed`; `run` → `0` | [Project Manifest v5](../docs/PROJECT-MANIFEST-V5.md) |
| `examples/frame-payload-project` | Project schema v8 under `owned-data-api.v1`: an `SPX1` frame decoder returning owned `Bytes`, `Option<Bytes>` and `Result<Bytes, i64>`, with a nine-case `corpus.json` | `semaprax test examples/frame-payload-project/semaprax.toml` → `project tests passed`; `run` → `0` | [Public Owned Data API v1](../docs/PUBLIC-OWNED-DATA-API-V1.md), and the directory's own [README](frame-payload-project/README.md) |
| `examples/spxgrep-language-command-project` | Project schema v6 under `language-command-io.v1`: `argv-utf8+stdin-bytes.v1` input read through `arg_utf8` | `check`, `test` and `run` on the manifest all fail here — see the note below | [Bounded Language Command I/O v1](../docs/BOUNDED-LANGUAGE-COMMAND-IO-V1.md) |
| `examples/spxgrep-lines-project` | Project schema v7 under `line-command-io.v1`: line-at-a-time filtering with `byte_range` over the same argv/stdin input | `check`, `test` and `run` on the manifest all fail here — see the note below | [Project Manifest v1](../docs/PROJECT-MANIFEST-V1.md), section "Additive Project Manifest v7 line-command profile" |

The last two projects both bind a borrowed `str` local from `arg_utf8`. On this
CLI build `semaprax check`, `test` and `run` against either manifest each stop
at
`error[SPX-H006]: borrowed-str local must be an exact alias or authenticated owning String view`,
so neither project can be exercised end to end from the command line here.
Their committed sources are still compiled as fixtures by the repository's own
gates —
`cargo test --locked -p semaprax --test project` (modules
`language_command_native`, `line_command_native`, `manifest_v7`) and
`cargo test --locked -p semaprax --test useful_data` (module
`line_filter_project_v7`) — which is where their evidence lives.
[Completion matrix](../docs/COMPLETION-MATRIX.md) owns the status of these
profiles.

## Target and host projections

These directories are not programs to run. Each is a committed host-side
consumer or shell that expects a *generated* package next to it, and the
generated output is deliberately not committed. Build the package first, then
run the consumer with its own toolchain. Each directory carries the exact
commands; this table says which toolchain is needed and links there.

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/calculator-web` | A browser shell that calls the generated scalar package by stable ID and renders normalized semantic failures | Standalone CLI is enough: `semaprax build examples/calculator-project/semaprax.toml --target web -o /absolute/out/web` → `built project web package …`. Then follow the directory's [README](calculator-web/README.md) | [Wasm Scalar Exports v1](../docs/WASM-SCALAR-EXPORTS-V1.md) |
| `examples/calculator-rust` | Three separate Cargo consumers of a generated safe-Rust SDK: direct exports, a host-implemented callback, and the six Project exports | Cargo-driven, not a `semaprax` subcommand; the setup package and its environment variables are in the directory's [README](calculator-rust/README.md). `examples/calculator-rust/callback.spx` itself checks (`semaprax check examples/calculator-rust/callback.spx`), but `run` reports `error[SPX-B103]: native Rust imports are unavailable for the ordinary native target` | [Native Rust Interop v1](../docs/NATIVE-RUST-INTEROP-V1.md) |
| `examples/owned-data-rust` | A single-file owned-byte API (`examples/owned-data-rust/owned_data.spx`) plus the Cargo setup and consumer packages for its generated safe-Rust SDK | The `.spx` subject alone runs: `semaprax run examples/owned-data-rust/owned_data.spx` → `0`. SDK generation is the Cargo setup package in this directory, not a `semaprax` subcommand | [Public Owned Data API v1](../docs/PUBLIC-OWNED-DATA-API-V1.md) |
| `examples/frame-payload-web` | A Node and browser npm consumer that pins stable-ID access through `runtime.functions[...]` and replays the shared corpus | Use the private `semaprax-full` for the `--target npm` route the directory's [README](frame-payload-web/README.md) documents. The standalone CLI does emit something for this manifest — `semaprax build examples/frame-payload-project/semaprax.toml --target npm -o /absolute/out/npm` → `built Project v2 npm package …` — but that output announces itself as a Project v2 package, not the Project v8 owned-data package this consumer reads | [Public Owned Data API v1](../docs/PUBLIC-OWNED-DATA-API-V1.md) |
| `examples/frame-payload-rust` | A safe-Rust consumer of the generated owned-data SDK, reading the same corpus through Serde | Needs the private `semaprax-full`: the standalone CLI answers `build --target rust is unavailable in the standalone crates.io package; use the unpublished semaprax-full toolchain CLI`. Commands are in the directory's [README](frame-payload-rust/README.md) | [Public Owned Data API v1](../docs/PUBLIC-OWNED-DATA-API-V1.md) |

## Semantic change input

| Example | Teaches | Command (observed) | Reference |
| --- | --- | --- | --- |
| `examples/rename.spatch` | A three-line semantic patch that renames `math.add` in `examples/meaning.spx` and requires no new effects | Read-only preview: `semaprax impact examples/meaning.spx examples/rename.spatch`. As committed it fails closed with `error[SPX-G409]: stale semantic patch: expected graph GRAPH_REVISION, current graph sha256:42aeae…` — its first line tells you to substitute the current revision, which `semaprax check examples/meaning.spx` prints. With the real revision substituted into a copy of the patch, `impact` emits a `semaprax.semantic-impact.v1` document whose `source_consumers` list names the `math.add` declaration and its one reference site in `app.main` | [Semantic Patch v2](../docs/SEMANTIC-PATCH-V2.md) |

Patch application (`semaprax patch`) rewrites the file you pass it. Copy
`examples/meaning.spx` somewhere else before trying it, so the committed
example stays canonical for `tests/examples.rs`.

## Keeping this index honest

`tests/examples.rs` carries a `readme_index` module that fails if a committed
example is missing from this file, or if this file names a path under
`examples` or a relative link that does not resolve:

```sh
cargo test --locked -p semaprax --test examples
```

Add a row here in the same change that adds an example.
