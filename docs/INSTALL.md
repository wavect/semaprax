# Installing SEMAPRAX

Status: public alpha installation guide; not a production-readiness claim.
Audience: new users and contributors.

> Prefer a shorter path? The user-facing [Semaprax Handbook](../handbook/README.md)
> covers installation in [Install](../handbook/getting-started/install.md).
> This page remains the complete, test-pinned installation reference.

Install from source for the newest local build, or use the last published
[v0.5.0 archive](https://github.com/wavect/semaprax/releases/tag/v0.5.0).
Then follow the [quickstart](QUICKSTART.md) to run a calculator project. For
command syntax, use the [CLI guide](CLI-GUIDE.md). For feature status and
evidence, use the [completion matrix](COMPLETION-MATRIX.md).

## Which binary do you need?

There are two builds. Release archives rename the full build:

| Name | Where it comes from | What it can do |
| --- | --- | --- |
| `semaprax` | Standalone source install or compiler package | Common commands: `new`, `fmt`, `check`, `run`, `test`, `graph`, `build`, and `doctor`. |
| `semaprax-full` | Source checkout only | Common commands plus private host and publication operations. Its `new` uses staged publication. |
| `semaprax` in a release archive | Published [v0.5.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.5.0) | The full build, renamed when packaged. |

The full toolchain is unpublished (`publish = false`), so build it from a
checkout. Both binaries accept the common commands, including `doctor`. The
standalone `new` uses a [create-new route](NEW-PROJECT-STANDALONE-V1.md); the
full build uses a [staged route](NEW-PROJECT-PUBLICATION-V1.md). If a command is
missing from `semaprax help all`, check whether it is private before treating
the installation as broken. The [release process](RELEASE-PROCESS.md#tag-admission)
owns the exact split.

## Prerequisites

| Tool | Version | When you need it |
| --- | --- | --- |
| Rust (`cargo`, `rustc`) | 1.88 or newer | Build or install from source. [Cargo.toml](../Cargo.toml) records the minimum. |
| Clang | C11 support | Build native executables; SEMAPRAX calls `clang`. |
| Node.js | 22 or newer | Run repository WebAssembly/Web verification scripts and generated npm packages. Not needed for ordinary source checks. |
| Git | Any supported version | Clone the source repository; SEMAPRAX does not invoke Git during builds. |

There is no `rust-toolchain.toml`; a newer stable Rust is fine.

Neither the compiler nor generated code acquires ambient filesystem, process,
network, home-directory, or signing authority from being installed. The
project generator uses only compiled-in files and does not touch the network.

## Route 1: install from source

Use this route if you need either CLI or current source changes.

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
`--locked` uses the recorded dependencies. Cargo may fetch them while
installing; a later SEMAPRAX build does not fetch dependencies.

To try the compiler without installing it, run it from the checkout:

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

At this document's 2026-09-24 update, the
[v0.6.0 tag gate](https://github.com/wavect/semaprax/actions/runs/36047757697)
was still running; v0.6.0 had no published archive or signature. The last
published [v0.5.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.5.0)
publishes one archive per admitted host plus a `SHA256SUMS` file:

| Host | Archive |
| --- | --- |
| Linux x86-64 | `semaprax-v0.5.0-x86_64-unknown-linux-gnu.tar.gz` |
| Apple Silicon macOS | `semaprax-v0.5.0-aarch64-apple-darwin.tar.gz` |
| Windows x86-64 | `semaprax-v0.5.0-x86_64-pc-windows-msvc.zip` |

Each archive contains `semaprax`, the `semapraxd` daemon, `LICENSE`,
`README.md`, a fixed smoke program, and a deterministic
`semaprax.release-artifact.v1` manifest. Verify the download against
`SHA256SUMS` before unpacking, then unpack it:

```sh
shasum -a 256 -c SHA256SUMS
tar -xzf semaprax-v0.5.0-aarch64-apple-darwin.tar.gz
```

Use `unzip` for the Windows archive. Put the unpacked directory on your `PATH`
the same way as Cargo's binary directory above, or invoke the binary by path.

**The archives are unsigned and are not notarized.** SHA-256 checksums are
integrity facts, not signatures, provenance, or publisher authentication. The
exact published digests, the build evidence behind them, and the full set of
nonclaims are owned by the release process:
[v0.4.0 hosted release evidence](RELEASE-PROCESS.md#040-hosted-release-evidence) and
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

Wherever this document writes `semaprax-full doctor`, an archive user writes
`semaprax doctor`. The standalone compiler also exposes `doctor` through the
same shared driver. An unavailable offline profile produces the documented
fail-closed report; command availability is not production tool authority.

## Confirm the install works

Run these from a source checkout, where the example programs live:

```sh
semaprax --version
semaprax version --json
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
semaprax graph examples/meaning.spx
```

Expected shapes, from a local `0.6.0` standalone build:

```text
semaprax 0.6.0 (commit unknown)
```

A CLI built from a tag archive reports its injected commit instead of
`unknown`. The JSON form is the machine-readable version of the same identity:

```text
{"schema":"semaprax.version.v1","version":"0.6.0","commit":null,"maturity":"alpha","rust_min":"1.88"}
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

### Create a project

```sh
semaprax new first-semaprax
```

This creates and verifies the built-in calculator project and prints
`created calculator project first-semaprax`. The standalone route is owned by
[standalone project creation](NEW-PROJECT-STANDALONE-V1.md): it writes into a
fresh destination under an existing parent and never replaces an entry. It
does not delete a reported failure's output automatically.

### Confirm the full toolchain, if you installed it

```sh
semaprax-full new first-semaprax
```

This creates the same files through the held-parent staged publication owned
by [calculator project publication](NEW-PROJECT-PUBLICATION-V1.md), which
also never deletes a reported failure's output or staging residue. The quickstart continues
from here.

## When your first command fails

Every symptom below was reproduced against a local
`semaprax` on macOS arm64. Diagnostics go to stderr; global help goes to
stdout. Invocation errors exit `2` and compiler or execution failures exit `1`.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `zsh: command not found: semaprax` (or `sh: semaprax: command not found`) | Cargo's binary directory is not on `PATH`, or the install never ran. | Follow [Put Cargo's binary directory on your PATH](#put-cargos-binary-directory-on-your-path), open a new shell, and check `command -v semaprax`. |
| Global help on stdout, empty stderr, exit `2` | No subcommand was given. | Pick a subcommand from `semaprax --help`, or read the [CLI user guide](CLI-GUIDE.md). |
| `failed profile: an explicit offline profile is required; use --profile <id>` with exit `1` | `doctor` ran without an offline tool profile. It never discovers or runs tools ambiently. | Pass `--profile <id>` naming an admitted offline profile; the report lists which required checks stay unavailable. |
| `new: cannot create project first-semaprax: an entry already exists` | The destination already exists. `new` never replaces or writes into an existing entry. | Choose a fresh destination name, or remove the entry yourself. |
| ``unknown command `chekc`; did you mean `check`?`` | A misspelled command name. The suggestion compares only names already visible in that binary's catalog; see [capability-aware CLI typo guidance](CLI-HELP-V2.md). | Run the suggested name. |
| `error[SPX-I001]: cannot read missing.spx: No such file or directory (os error 2)` | The source path does not exist, usually because the shell is in the wrong directory. | Check the working directory and the path. Paths are resolved relative to the process working directory. |
| `error[SPX-J102]: cannot inspect declared Project v1 manifest <dir>/semaprax.toml: No such file or directory (os error 2)` | `check`, `run`, `test`, or `build` was given no input or a directory, and that directory holds no Project v1 manifest. A directory operand always means the `semaprax.toml` inside it. When no input was given, the diagnostic carries a `help:` line naming the admitted inputs. | Pass the `.spx` file, the project directory, or its `semaprax.toml`; from inside a project, `semaprax check .` works. See [Project Manifest v1](PROJECT-MANIFEST-V1.md). |
| ``unsupported target `webb`; available: native, native-callable, web, wasm`` | An unknown single-source `--target` value. | Use one of the listed targets; project and full-toolchain catalogs differ. |
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
unsupported target `webb`; available: native, native-callable, web, wasm
hint: run `semaprax build --help` for usage
```

A missing scaffold name:

```text
project-scaffold requires --name project-name
hint: run `semaprax project-scaffold --help` for usage
```

`doctor` without an offline profile (exit `1`, report on stdout):

```text
failed profile: an explicit offline profile is required; use --profile <id>
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
against a standalone `semaprax` built from this checkout. That is local
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
