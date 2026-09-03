# Using the SEMAPRAX CLI

Status: public pre-alpha user guide.

Audience: language users running the compiler locally or from automation.

SEMAPRAX exposes a standalone `semaprax` compiler and, in source checkouts, an
unpublished `semaprax-full` toolchain for host-backed workflows such as project
creation. The standalone compiler is enough to format, check, run, inspect, and
build existing source and projects.

## Find the exact command shape

List the commands available in the installed binary:

```sh
semaprax --help
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
canonically.

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

Compiler diagnostics carry stable `SPX-...` codes so tests and tools can bind
to the diagnostic kind instead of matching an entire human-readable message.
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
contract are defined by [Capability-Aware CLI Help v1](CLI-HELP-V1.md).
