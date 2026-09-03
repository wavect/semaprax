# Installing SEMAPRAX

Status: public pre-alpha installation guide; not a production-readiness claim.

Audience: new SEMAPRAX users and contributors putting a working toolchain on a
local machine.

This document is the single owner of "how do I get a working SEMAPRAX". It
covers the prerequisites and why each one is needed, the two installation
routes and their different binary names, `PATH` setup, how to confirm the
install works, and what the first failure messages mean. The
[quickstart](QUICKSTART.md) then walks the calculator project flow, and the
[CLI user guide](CLI-GUIDE.md) covers day-to-day command shapes.

SEMAPRAX is pre-alpha research software. Installing it does not make any
feature production-ready; the [completion matrix](COMPLETION-MATRIX.md) is the
sole authority for what is implemented and what evidence backs it.

## Which binary do you need?

Three different names appear across the repository, the release archives, and
the documentation. They are not three products.

| Name | Where it comes from | What it can do |
| --- | --- | --- |
| `semaprax` | `cargo install --locked --path .`, or the crates.io compiler package | The standalone compiler: format, check, run, test, inspect, patch, and build existing source and projects. |
| `semaprax-full` | `cargo install --locked --path crates/semaprax-toolchain`, from a source checkout only | Everything the standalone compiler does, plus the private host surfaces the published package excludes: `new`, `doctor`, Native Rust package publication, and Windows revision-store host operations. |
| `semaprax` inside a tag archive | The [v0.2.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.2.0) archives | The archive's `semaprax` *is* the `semaprax-full` binary, renamed during staging, so archive users write `semaprax new`, not `semaprax-full new`. |

The `semaprax-toolchain` package is `publish = false`; it is never fetched
from a registry. The naming split and what the standalone package excludes are
owned by the [release process](RELEASE-PROCESS.md#tag-admission).

Every command shown in this document runs on the standalone `semaprax` except
`new` and `doctor`, and so does every command in the [quickstart](QUICKSTART.md)
after the project has been created. If a documented command is missing from
`semaprax --help`, you installed the standalone compiler and the command is
private; that is a capability boundary, not a broken install.

## Prerequisites

| Prerequisite | Version | Why it is needed | Where the requirement is recorded |
| --- | --- | --- | --- |
| Rust toolchain (`cargo`, `rustc`) | 1.88 or newer | Builds and installs both CLIs from source. | `rust-version` in [Cargo.toml](../Cargo.toml); the CLI reports it as `rust_min` in `semaprax version --json`; CI runs a dedicated "Rust 1.88 minimum" job. |
| Clang | any C11-capable driver | The native lane emits C11 and invokes `clang` to produce the executable, so `--target native` and `--target native-callable` fail without it. | `.github/workflows/ci.yml` resolves `clang` for every native job; the compiler spawns `clang` by name. |
| Node.js | 22 or newer | Runs the repository's WebAssembly and Web verification scripts and the generated npm packages. Not needed to check, run, or build source. | `node-version: 22` in `.github/workflows/ci.yml`. |
| Git | any recent version | Only to obtain a source checkout. SEMAPRAX itself never initializes or invokes Git during a build. | — |

There is no `rust-toolchain.toml` in this repository, so your default
toolchain is used; a newer stable Rust is fine.

Neither the compiler nor generated code acquires ambient filesystem, process,
network, home-directory, or signing authority from being installed. The
project generator uses only compiled-in files and does not touch the network.

## Route 1: install from source

This is the route that gives you both CLIs.

```sh
git clone https://github.com/wavect/semaprax.git
cd semaprax
```

Install the standalone compiler:

```sh
cargo install --locked --path .
```

Install the private full toolchain beside it, from the same checkout root:

```sh
cargo install --locked --path crates/semaprax-toolchain
```

The first command installs `semaprax`; the second installs `semaprax-full`.
`--locked` keeps the recorded dependency graph. Installation fetches Rust
dependencies from the network; nothing in a later SEMAPRAX build does.

You can also skip installation entirely and drive the compiler out of the
checkout, which is what the repository's own documentation uses when it wants
to be unambiguous about which build is running:

```sh
cargo run --locked -p semaprax -- check examples/meaning.spx
```

### Put Cargo's binary directory on your PATH

`cargo install` writes into Cargo's binary directory, `$CARGO_HOME/bin`, which
defaults to `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on Windows). If that
directory is not on your `PATH`, the install succeeds and every later command
reports `command not found`.

For `bash` or `zsh`, add this to `~/.bashrc`, `~/.zshrc`, or your shell's
equivalent, then open a new shell:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

Rust installations made with `rustup` ship `~/.cargo/env` for the same purpose;
sourcing it is equivalent. On Windows, add `%USERPROFILE%\.cargo\bin` to the
user `Path` environment variable and open a new terminal.

Confirm the directory is the one your shell resolves:

```sh
command -v semaprax
```

## Route 2: install from a release archive

The [v0.2.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.2.0)
publishes one archive per admitted host plus a `SHA256SUMS` file:

| Host | Archive |
| --- | --- |
| Linux x86-64 | `semaprax-v0.2.0-x86_64-unknown-linux-gnu.tar.gz` |
| Apple Silicon macOS | `semaprax-v0.2.0-aarch64-apple-darwin.tar.gz` |
| Windows x86-64 | `semaprax-v0.2.0-x86_64-pc-windows-msvc.zip` |

Each archive contains `semaprax`, the `semapraxd` daemon, `LICENSE`,
`README.md`, a fixed smoke program, and a deterministic
`semaprax.release-artifact.v1` manifest. Verify the download against
`SHA256SUMS` before unpacking, then unpack it:

```sh
shasum -a 256 -c SHA256SUMS
tar -xzf semaprax-v0.2.0-aarch64-apple-darwin.tar.gz
```

Use `unzip` for the Windows archive. Put the unpacked directory on your `PATH`
the same way as Cargo's binary directory above, or invoke the binary by path.

**The archives are unsigned and are not notarized.** SHA-256 checksums are
integrity facts, not signatures, provenance, or publisher authentication. The
exact published digests, the build evidence behind them, and the full set of
nonclaims are owned by the release process:
[hosted release evidence](RELEASE-PROCESS.md#v020-hosted-release-evidence) and
[nonclaims](RELEASE-PROCESS.md#nonclaims). Do not treat an archive install as
promotion of any completion-matrix row.

### The archive uses a different command name

Because the archive's `semaprax` is the renamed `semaprax-full` binary, the
private commands are available under the plain name. With the unpacked
directory on your `PATH`:

```sh
semaprax --version
semaprax new first-semaprax
```

Wherever the quickstart writes `semaprax-full new`, an archive user writes
`semaprax new`. The reverse substitution does not work: the standalone
compiler refuses `new` outright, as shown in the failure table below.

## Confirm the install works

Run these from a source checkout, where the example programs live:

```sh
semaprax --version
semaprax version --json
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
semaprax graph examples/meaning.spx
```

Expected shapes, from a local `0.2.0` standalone build:

```text
semaprax 0.2.0 (commit unknown)
```

A CLI built from a tag archive reports its injected commit instead of
`unknown`. The JSON form is the machine-readable version of the same identity:

```text
{"schema":"semaprax.version.v1","version":"0.2.0","commit":null,"maturity":"pre-alpha","rust_min":"1.88"}
```

`check` prints the verified path and its source digest, and `run` prints `42`:

```text
verified examples/meaning.spx (sha256:42aeae2650d15b1e44b8fd6d8a7ce6018d61f43e0e7988a58da2426b2f0c1657)
```

`graph` emits deterministic JSON beginning `{"schema":"semaprax.graph.v`.

### Confirm the native lane, which needs Clang

```sh
semaprax build examples/meaning.spx --target native -o target/meaning-native
```

On success it prints `built native executable <path>`. This is the step that
proves Clang is usable.

### Confirm the Wasm lane, which needs Node.js

```sh
semaprax build examples/meaning.spx --target web -o target/meaning-web
node scripts/verify-web.mjs target/meaning-web
```

The build prints `built web package <path>` and the verifier prints `42`.
`scripts/verify-web.mjs` is a repository script, so this check is available in
a source checkout, not from an unpacked archive.

### Confirm the private toolchain, if you installed it

```sh
semaprax-full new first-semaprax
```

This creates and validates the built-in calculator project. Its publication
rules are owned by [calculator project
publication](NEW-PROJECT-PUBLICATION-V1.md); do not delete a reported
failure's output or staging residue automatically. The quickstart continues
from here.

## When your first command fails

Every symptom below was reproduced against a local `0.2.0` standalone
`semaprax` on macOS arm64. Diagnostics go to stderr; global help goes to
stdout. Invocation errors exit `2` and compiler or execution failures exit `1`.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `zsh: command not found: semaprax` (or `sh: semaprax: command not found`) | Cargo's binary directory is not on `PATH`, or the install never ran. | Follow [Put Cargo's binary directory on your PATH](#put-cargos-binary-directory-on-your-path), open a new shell, and check `command -v semaprax`. |
| Global help on stdout, empty stderr, exit `2` | No subcommand was given. | Pick a subcommand from `semaprax --help`, or read the [CLI user guide](CLI-GUIDE.md). |
| `new is unavailable in the standalone crates.io package; use the unpublished semaprax-full toolchain CLI` | A private full-toolchain command was run on the standalone compiler. Same message for `doctor`. | Install the full toolchain from the same checkout and run `semaprax-full new`, or use an archive binary, where the command is simply `semaprax new`. |
| ``unknown command `new` `` followed by global help | Same cause, reached through `semaprax new --help`. Names hidden by the capability boundary get no suggestion, by design. | As above. |
| ``unknown command `chekc`; did you mean `check`?`` | A misspelled command name. The suggestion compares only names already visible in that binary's catalog; see [capability-aware CLI typo guidance](CLI-HELP-V2.md). | Run the suggested name. |
| `error[SPX-I001]: cannot read missing.spx: No such file or directory (os error 2)` | The source path does not exist, usually because the shell is in the wrong directory. | Check the working directory and the path. Paths are resolved relative to the process working directory. |
| `error[SPX-I001]: cannot read examples: Is a directory (os error 21)` | A directory was passed where a file was expected. | Pass the `.spx` file, or the project's `semaprax.toml`, not its directory. |
| `error[SPX-J102]: cannot inspect declared Project v1 manifest <dir>/semaprax.toml: No such file or directory (os error 2)` | `check`, `run`, or `test` defaulted to a Project v1 manifest that is not in the working directory. | `cd` into the project, or pass `--manifest-path`. See [Project Manifest v1](PROJECT-MANIFEST-V1.md). |
| ``unsupported target `webb`; available: native, native-callable, web, wasm, npm`` | An unknown `--target` value. | Use one of the listed targets. |
| `graph requires exactly <file>` | A required operand is missing. Every rejected known command appends a scoped-help hint; see [capability-aware CLI recovery](CLI-HELP-V3.md). | Run the hinted `semaprax <command> --help` for the exact accepted shape. |
| `error[SPX-B101]: failed to start clang; install a C11 toolchain: No such file or directory (os error 2)` | The native lane could not spawn `clang`. | Install Clang and make sure it is on the `PATH` of the shell running the build. |
| `project-scaffold requires --name project-name` | The scaffold capsule command was run without its required name. | Supply `--name`. See [Public Project Scaffold Capsule v1](PROJECT-SCAFFOLD-V1.md). |
| `error[SPX-J115]: project scaffold name must match lowercase [a-z][a-z0-9-]*` | The project name used uppercase or an unadmitted character. | Use a lowercase name such as `first-semaprax`. |
| `TypeError: instance.exports.semaprax_main is not a function` | `scripts/verify-web.mjs` calls the entry point of a program that has none, such as a library-only example. | Verify a package built from a program with a `main`, and read exported functions through the generated bindings instead. See [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md). |

### Exact reproduced output

A missing operand, with the recovery hint appended to stderr:

```text
graph requires exactly <file>
hint: run `semaprax graph --help` for usage
```

An unknown build target:

```text
unsupported target `webb`; available: native, native-callable, web, wasm, npm
hint: run `semaprax build --help` for usage
```

A missing scaffold name:

```text
project-scaffold requires --name project-name
hint: run `semaprax project-scaffold --help` for usage
```

A private command on the standalone compiler:

```text
new is unavailable in the standalone crates.io package; use the unpublished semaprax-full toolchain CLI
```

A misspelling, whose global help still goes to stdout:

```text
unknown command `chekc`; did you mean `check`?
```

The native lane with no C11 driver reachable:

```text
error[SPX-B101]: failed to start clang; install a C11 toolchain: No such file or directory (os error 2)
```

Diagnostics that carry a source location print `path:line:column`; that format
is owned by [human diagnostic locations](HUMAN-DIAGNOSTICS-V1.md).

## Evidence and limits

The symptoms and success output above were observed locally on macOS arm64
against a `0.2.0` standalone `semaprax` built from this checkout. That is local
developer evidence, not hosted release evidence and not a support claim for any
platform.

Not verified here:

- The `semaprax-full` route. Its behavior is described from the documented
  contract in the [release process](RELEASE-PROCESS.md#tag-admission), the
  [quickstart](QUICKSTART.md), and the [CLI user guide](CLI-GUIDE.md); no
  `semaprax-full` binary was built or installed while writing this document.
- `cargo install` itself, `PATH` configuration, archive download and unpacking,
  and Windows behavior.
- Anything about the `semapraxd` daemon beyond its presence in the archive
  inventory.

`tests/quickstart_v1.rs` contains an executable gate over this document: it
checks that every `semaprax` and `semaprax-full` command line shown here names
a subcommand the CLI actually accepts, and that the error text quoted above
still matches what the CLI produces. See [quality
gates](QUALITY-GATES.md) for how to run it.
