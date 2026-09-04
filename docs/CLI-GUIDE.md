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

`fmt <file> --check` reports non-canonical source without rewriting it. Run
`fmt <file>` without `--check` when you want the compiler to rewrite that file
canonically. `//` comments survive formatting: each is printed on its own line
above the declaration, field, or statement it precedes, or right after the one
it followed; [canonical comments](CANONICAL-COMMENTS-V1.md) owns the exact
placement rules and the routes that still drop comments.

Inspect checked meaning by stable identity rather than searching formatted
source text:

```sh
semaprax graph examples/meaning.spx
semaprax context examples/meaning.spx app.main --depth 1
```

`graph` and `context` produce deterministic JSON suitable for inspection or a
caller-owned file. Redirecting that output is the caller's publication action;
the query itself does not modify the source.

## Work on a project

From a directory containing `semaprax.toml`, the manifest argument can be
omitted. Keeping it explicit is useful in scripts and from parent directories:

```sh
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax build semaprax.toml --target web -o dist/web
```

A directory operand means the `semaprax.toml` inside it, so
`semaprax check examples/calculator-project` and `semaprax run .` are the
same as naming the manifest. Only `--manifest-path` is taken literally.

Use each command's scoped help before selecting a target or profile; the
available build targets differ between the standalone and full toolchains.
Commands that list `--json` in scoped help provide their structured form for
automation.

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
