# Using the SEMAPRAX CLI

Status: public pre-alpha user guide.

Audience: language users running the compiler locally or from automation.

SEMAPRAX exposes a standalone `semaprax` compiler and, in source checkouts, an
unpublished `semaprax-full` toolchain for host-backed workflows such as project
creation. The standalone compiler is enough to format, check, run, inspect, and
build existing source and projects.

## Find the exact command shape

Start from the guided overview. It is one screen: the commands for writing,
checking, running, inspecting, and changing programs, grouped by task, each
with its purpose:

```sh
semaprax --help
```

List every command the installed binary accepts, including the protocol
surfaces intended for tool authors:

```sh
semaprax help all
```

Print the compiler-checked language quick reference, the one-page card of
admitted shapes, the diagnostics that habits from other languages trigger, and
their fixes, without a source checkout:

```sh
semaprax help language
```

Print the generated standard-library catalog, every `std.*` function with its
signature and contracts, without a source checkout:

```sh
semaprax help library
```

When you know a module, declaration name, or stable identity, request only
that exact generated entry to avoid reading the whole catalog:

```sh
semaprax help library compare
semaprax help library std.core.compare
```

Print every canonical declaration shape from the committed examples, or ask
for only one exact shape. A kind returns its smallest generated exemplar;
`path#stable-id` disambiguates identities reused by multiple examples:

```sh
semaprax help shapes
semaprax help shapes record
semaprax help shapes calculator.add
semaprax help shapes examples/calculator.spx#app.main
```

Show the exact accepted form of one command without reading source files,
probing tools, or starting a build:

```sh
semaprax check --help
semaprax build --help
semaprax help context
```

The standalone binary intentionally omits private-host commands and targets
that it cannot execute. If a command shown in source-install documentation is
absent, check which binary you installed before debugging the project.

## Work on one source file

A short edit loop checks formatting before semantic verification:

```sh
semaprax fmt examples/meaning.spx --check
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

Single-file `run` evaluates `@id("app.main")` in the bounded reference
interpreter, with no compiler or target process. `--max-steps`, `--max-bytes`,
and `--json` apply to this route. Use `run <file> --native` only when you
specifically need the generated C11 executable path. A module whose authority
is exactly `permit { process.stdout.write }` uses the success-published bounded
stdout interpreter profile automatically, so the language-card example is
directly runnable.

`fmt <file> --check` reports non-canonical source without rewriting it. Run
`fmt <file>` without `--check` when you want the compiler to rewrite that file
canonically. Write-capable formatting rejects symlink/reparse aliases for a
source, manifest, or project directory as `SPX-J102`, consistently with
Project input selection. `fmt` also takes a project directory or
`semaprax.toml`: `fmt . --check` names every drifting source and its first
differing line, in manifest order, and `fmt .` rewrites them, parsing every
file before writing any. Canonical expression formatting is compact: a
`match` remains on one line even when its source arms span several lines. `//`
comments survive formatting: each is printed on its own line above the
declaration, field, or statement it precedes, or right after the one it
followed; [canonical comments](CANONICAL-COMMENTS-V1.md) owns the exact
placement rules and lists the routes that preserve comments.

Inspect checked meaning by stable identity rather than searching formatted
source text:

```sh
semaprax graph examples/meaning.spx
semaprax context examples/meaning.spx app.main --depth 1
semaprax context examples/calculator-project calculator.add --direction both --depth 1 --max-bytes 2048 --max-nodes 16
```

`graph` and `context` produce deterministic JSON suitable for inspection or a
caller-owned file. Redirecting that output is the caller's publication action;
the query itself does not modify the source. A Project directory or manifest
selects authenticated cross-file context; its six structural edge families do
not accept the single-file `--filters` option. Its compact positional Project
schema retains exact revisions, traversal, truncation, and frontier facts while
avoiding repeated per-node and per-edge field names.

Search declarations by what they are, what they use, and what they call:

```sh
semaprax query examples/meaning.spx --kind function --effect clock.read
semaprax query examples/meaning.spx --calls math.add --json
semaprax query examples/calculator-project --id calculator.add
semaprax query examples/calculator-project --calls calculator.add
```

Each match is a declaration of the checked module with its identity and
canonical header; `--calls <id>` lists the callers of a declaration and
`--called-by <id>` its callees, from the same call index `impact` uses.
Selecting a Project directory or `semaprax.toml` searches every authenticated
source and prepends the owning path. Its call predicates cross module
boundaries, so agents can locate a library function and all retained callers
without transferring the complete Project graph.

Render the module's documentation from the same checked facts:

```sh
semaprax doc examples/meaning.spx
semaprax doc examples/meaning.spx --json
```

`doc` prints a Markdown page of every declaration: its `@id`, signature,
ownership modes, effects, contracts, members, and the `//` comments written
above it, bound to the graph revision `graph` prints for the same file.
`--json` emits the same facts as one `semaprax.doc.v1` document for tools.
[Documentation projection](DOC-PROJECTION-V1.md) owns the layout and the gate
that keeps the page and the graph naming the same declarations.

Replay any evidence capsule through one verb. The capsule's `schema` selects
the verifier, and the receipt is the owning route's own bytes:

```sh
semaprax patch-evidence examples/meaning.spx change.spatch > evidence.json
semaprax verify examples/meaning.spx change.spatch evidence.json
semaprax verify semaprax.toml image.json
```

Compile an agent definition and read its graph without running anything:

```sh
semaprax agent inspect agent.json
semaprax agent inspect agent.json --profile
```

[Unified CLI v1](UNIFIED-CLI-V1.md) lists the admitted capsule schemas and
the fail-closed selection codes.

## Work on a project

From a directory containing `semaprax.toml`, the manifest argument can be
omitted. Keeping it explicit is useful in scripts and from parent directories:

```sh
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax build semaprax.toml --target web -o dist/web
semaprax lock semaprax.toml
```

A directory operand means the `semaprax.toml` inside it, so
`semaprax check examples/calculator-project`, `semaprax run .`, and
`semaprax lock .` are the same as naming the manifest. Only `--manifest-path`
is taken literally.

Use each command's scoped help before selecting a target or profile; the
available build targets differ between the standalone and full toolchains.
Commands that list `--json` in scoped help provide their structured form for
automation.

`build` has separate target catalogs for its two input classes. A source file
admits `native`, `native-callable`, `web`, and `wasm`; a project admits
`native`, `web`, `wasm`, and `npm`, plus `rust` in the full toolchain. An
unsupported-target diagnostic lists only the catalog the current input and
toolchain can execute. `wasm` is an exact alias of `web`: both publish the
same Web package directory, including `app.wasm`, rather than a bare Wasm
file. `-o` and `--output` are equivalent.

When omitted, a source target defaults to `native` and its destination to
`<source-stem>.out` beside the source. A project target defaults to `web`; its
destination defaults inside the project root to `<name>-web` for `web` or
`wasm`, `<name>-npm` for `npm`, `<name>-rust` for `rust`, and
`<name>-out` plus the platform executable suffix for `native`. `build --json`
prints one success object with `status`, `target`, `product`, and `output`
(plus `manifest_sha256` for a native-callable bundle); build diagnostics use
the ordinary one-diagnostic-per-line JSON form.

Every explicit single-file build output is create-new. Native builds reserve
the exact destination before invoking the compiler and publish through that
retained file; Web/Wasm builds atomically create a fresh package directory.
An existing file, directory, symlink, or concurrent winner is rejected with
`SPX-I307` and left unchanged. An invalid or unavailable parent is rejected as
`SPX-I301`; builds never merge into an existing directory or overwrite their
own `.spx` input.

For command-profile projects, `run` deliberately executes the project's
ordinary entry rather than synthesizing process input for its command
function. Human output includes a note naming both identities and points to
the built native and Web/npm adapters that exercise the command function.

Declare and stage dependencies without any implicit network access:

```sh
semaprax add . examples.meaning ^1.0.0
semaprax fetch cache vendor/examples.meaning-1.0.0.subject.json
semaprax resolve . --target native64 --cache cache --write
```

`add` rewrites a table-layout `semaprax.toml` canonically with the new
`[dependencies]` row and changes nothing else; `fetch` replays each named
Subject-v3 envelope and files it in the content-addressed cache by digest;
`resolve` then selects from exactly that cache.
[Unified CLI v1](UNIFIED-CLI-V1.md) owns both grammars and their fail-closed
codes.

## Diagnose command-line errors

Command-line grammar errors exit without compiling the input. Start with
scoped help for the command that rejected the invocation:

```sh
semaprax fmt --help
semaprax context --help
```

A uniquely recognizable command typo includes capability-aware guidance, such
as ``unknown command `chek`; did you mean `check`?``. The standalone compiler
does not reveal private full-toolchain commands through suggestions.
If a known command's arguments are invalid, the diagnostic points directly to
its scoped usage, for example `semaprax check --help`.

Compiler diagnostics carry stable `SPX-...` codes so tests and tools can bind
to the diagnostic kind instead of matching an entire human-readable message.
Human-readable diagnostics include `path:line:column` when the compiler knows
the source location; `--json` remains the stable automation interface.
SEMAPRAX remains pre-alpha, so consult the release notes and versioned
references before treating a diagnostic, schema, or ABI as stable across
releases.

## Source checkout binaries

Install both source-checkout binaries with the locked dependency graph:

```sh
cargo install --locked --path .
cargo install --locked --path crates/semaprax-toolchain
```

The first command installs the standalone `semaprax` binary. The second
installs the unpublished `semaprax-full` binary. Tagged release archives expose
their full CLI as `semaprax`; follow the archive's release documentation rather
than installing the private source package beside it.

For a complete first project, continue with the executable
[quickstart](QUICKSTART.md). The exact capability boundary and byte-level help
contract are defined by [Capability-Aware CLI Help v1](CLI-HELP-V1.md), with
bounded typo guidance added by [v2](CLI-HELP-V2.md), known-command recovery
added by [v3](CLI-HELP-V3.md), and the guided overview plus `help all` added by
[v4](CLI-HELP-V4.md).
Human diagnostic rendering is defined by
[Human Diagnostic Locations v1](HUMAN-DIAGNOSTICS-V1.md).
