# Using the SEMAPRAX CLI

Status: public alpha user guide.

Audience: language users running the compiler locally or from automation.

> New to the CLI? The user-facing [Semaprax Handbook](../handbook/README.md)
> covers everyday commands in its [First project](../handbook/getting-started/first-project.md)
> and [Cheatsheet](../handbook/reference/cheatsheet.md) pages. This guide
> remains the complete command reference.

Use `semaprax` for ordinary source and project work. A source checkout can
also build `semaprax-full`, which adds private host-backed operations. See
[Install](INSTALL.md) if a command is missing from your binary.

## Find the exact command shape

Start with the short command overview:

```sh
semaprax --help
```

List every command your installed binary accepts:

```sh
semaprax help all
```

Get the compiler-checked language card without a source checkout:

```sh
semaprax help language
```

Look up a diagnostic code or list the codes with short help:

```sh
semaprax help diagnostic SPX-T208
semaprax help diagnostic codes
```

Codes are exact and case-sensitive.

Find standard-library functions and their contracts:

```sh
semaprax help library
```

Ask for one entry when you know its name:

```sh
semaprax help library compare
semaprax help library std.core.compare
```

Read one language-card topic:

```sh
semaprax help language topics
semaprax help language scalars
semaprax help language ownership
```

Find a canonical declaration example. A kind returns its smallest example;
`path#stable-id` distinguishes repeated identities:

```sh
semaprax help shapes
semaprax help shapes record
semaprax help shapes calculator.add
semaprax help shapes examples/calculator.spx#app.main
```

Check the accepted arguments for one command:

```sh
semaprax check --help
semaprax build --help
semaprax help context
```

The standalone binary omits private host commands. If help does not show a
command, confirm which binary you installed before debugging the project.

## Work on one source file

A short edit loop:

```sh
semaprax fmt examples/meaning.spx --check
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

By default, single-file `run` evaluates `@id("app.main")` in the bounded
reference interpreter, not a generated executable. Set `--max-steps` and
`--max-bytes` to control limits and `--json` for machine-readable results.
Choose `--native` to run the generated C11 executable. The exact
`permit { process.stdout.write }` profile uses bounded stdout publication.

Use `fmt <file> --check` to report formatting changes without writing, or
`fmt <file>` to write canonical source. Both accept a project directory or
`semaprax.toml`. For projects, `fmt . --check` lists differences in manifest
order. `fmt .` parses every file before rewriting any. Write-capable formatting rejects path aliases
with `SPX-J102`. The formatter keeps `//` comments; the
[comment contract](CANONICAL-COMMENTS-V1.md) defines exact placement.

Inspect checked meaning by stable identity:

```sh
semaprax graph examples/meaning.spx
semaprax context examples/meaning.spx app.main --depth 1
semaprax context examples/calculator-project calculator.add --direction both --depth 1 --max-bytes 2048 --max-nodes 16
```

Choose `graph` for the whole checked graph or `context` for a bounded answer
about one identity. Both return deterministic JSON without changing source.
Project inputs authenticate cross-file context. Project `context` does not
accept single-file `--filters`; its compact schema still records revision,
traversal, and truncation.

Search declarations by what they are, what they use, and what they call:

```sh
semaprax query examples/meaning.spx --kind function --effect clock.read
semaprax query examples/meaning.spx --calls math.add --json
semaprax query examples/calculator-project --id calculator.add
semaprax query examples/calculator-project --calls calculator.add
```

Each match includes a checked declaration's identity and canonical header.
`--calls <id>` finds callers; `--called-by <id>` finds callees. Project queries
search every authenticated source, including cross-module calls, without
transferring the whole graph.

Render the module's documentation from the same checked facts:

```sh
semaprax doc examples/meaning.spx
semaprax doc examples/meaning.spx --json
```

`doc` renders checked declarations, signatures, contracts, effects, and source
comments. `--json` returns the same facts as a `semaprax.doc.v1` document.
See [Documentation projection](DOC-PROJECTION-V1.md) for the exact schema.

Replay an evidence capsule without granting it write authority. Its `schema`
selects the verifier:

```sh
semaprax patch-evidence examples/meaning.spx change.spatch > evidence.json
semaprax verify examples/meaning.spx change.spatch evidence.json
semaprax verify semaprax.toml image.json
```

Inspect an agent definition without running it:

```sh
semaprax agent inspect agent.json
semaprax agent inspect agent.json --profile
```

[Unified CLI v1](UNIFIED-CLI-V1.md) lists the admitted capsule schemas and
the fail-closed selection codes.

## Work on a project

From a project directory, you can omit `semaprax.toml`. Name it explicitly in
scripts or when working from another directory:

```sh
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax build semaprax.toml --target web -o dist/web
semaprax lock semaprax.toml
```

A directory operand selects its `semaprax.toml`, so `semaprax run .` works.
Only `--manifest-path` is taken literally.

Check scoped help before choosing a target: standalone and full toolchains
have different target catalogs. Commands listing `--json` offer structured
output for automation.

| Input | Targets |
| --- | --- |
| Source file | `native`, `native-callable`, `web`, `wasm` |
| Project | `native`, `web`, `wasm`, `npm`, `oci`; full toolchain also offers `rust` |

`wasm` is an alias for `web`; both create a package directory with `app.wasm`,
not a bare Wasm file. `oci` currently accepts only the Project v1 scalar
profile and publishes an offline OCI Image Layout; see
[OCI Deployable Artifact v1](OCI-DEPLOYABLE-ARTIFACT-V1.md). `-o` and
`--output` mean the same thing.

Without a target, source files build as `native` beside the source and
projects build as `web` inside the project. Use `-o` when you want an explicit
destination. `build --json` reports `status`, `target`, `product`, and
`output`; native-callable bundles also report `manifest_sha256`.

Explicit single-file build outputs must be new. An existing path fails with
`SPX-I307`; an invalid parent fails with `SPX-I301`. Builds do not merge into
an existing directory or overwrite source.

For command-profile projects, `run` executes the ordinary project entry, not
the process-input command function. The output points to the built adapters
that exercise that function.

Declare and stage dependencies without any implicit network access:

```sh
semaprax add . examples.meaning ^1.0.0
semaprax fetch cache vendor/examples.meaning-1.0.0.subject.json
semaprax resolve . --target native64 --cache cache --write
```

`add` updates the manifest's dependency table. `fetch` verifies each named
Subject-v3 envelope and files it by digest. `resolve` selects only from that
explicit cache; none of these commands discovers a registry. See
[Unified CLI v1](UNIFIED-CLI-V1.md) for exact inputs and refusal codes.

## Diagnose command-line errors

Argument errors exit before compilation. Start with the command's help:

```sh
semaprax fmt --help
semaprax context --help
```

An unambiguous typo may suggest a command, such as
``unknown command `chek`; did you mean `check`?``. The standalone binary does
not suggest private commands. For a known command with bad arguments, follow
the scoped-help hint in its diagnostic.

Compiler errors have `SPX-...` codes. Use those codes in tests and tools,
not whole English messages. Human output includes `path:line:column` where
available; use `--json` for automation. SEMAPRAX is alpha, so check release
notes before assuming a cross-release diagnostic or ABI guarantee.

## Source checkout binaries

Install both source-checkout binaries with the locked dependency graph:

```sh
cargo install --locked --path .
cargo install --locked --path crates/semaprax-toolchain
```

The first command installs standalone `semaprax`; the second installs
unpublished `semaprax-full`. A release archive names its full CLI `semaprax`.

For a complete first project, continue with the executable
[quickstart](QUICKSTART.md). The exact capability boundary and byte-level help
contract are defined by [Capability-Aware CLI Help v1](CLI-HELP-V1.md), with
bounded typo guidance added by [v2](CLI-HELP-V2.md), known-command recovery
added by [v3](CLI-HELP-V3.md), and the guided overview plus `help all` added by
[v4](CLI-HELP-V4.md).
Human diagnostic rendering is defined by
[Human Diagnostic Locations v1](HUMAN-DIAGNOSTICS-V1.md).
