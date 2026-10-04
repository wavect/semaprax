# Install

Install the command-line tool first. You can run the introductory examples
without a model account, API key, or editor extension.

## What you need

For a source installation, have Git and Rust/Cargo available in your terminal.
The repository declares Rust **1.88** as its minimum; use a compatible current
stable toolchain for the recorded dependency set. Clang is needed when you
build native code. Node.js 22 or newer is used by the web-package examples.

Check the tools you already have:

```sh
git --version
rustc --version
cargo --version
```

## Build from main

These commands download the repository and install its standalone CLI:

```sh
git clone --branch main https://github.com/wavect/semaprax.git
cd semaprax
cargo install --locked --path . --bin semaprax
```

Cargo is Rust's package and build tool. `--locked` tells it to use the dependency
versions recorded in `Cargo.lock`. Installing may download those dependencies.
Keep the checkout: the examples and helper scripts in this handbook live there.

This edition was reviewed against commit
`508b851a5fda25002ec27453bb559755a6a0d930`. To reproduce that source snapshot,
run the following **before** the install command, in a clean checkout:

```sh
git switch --detach 508b851a5fda25002ec27453bb559755a6a0d930
```

A detached checkout is useful for following a fixed tutorial. Create a branch
before developing your own changes.

## Confirm that it works

Run these from the repository root, where `examples/` exists:

```sh
semaprax --version
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

`check` should report a verified file. `run` should print `42`. You now have
everything needed for [First program](first-program.md).

To try the tool without installing it globally, use Cargo directly:

```sh
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- run examples/meaning.spx
```

The `--` separates Cargo's options from Semaprax's options.

## Fix “command not found”

Cargo normally installs executables in `~/.cargo/bin`. In a macOS or Linux
shell, add that directory to your search path:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
command -v semaprax
```

In Windows PowerShell, the equivalent for the current terminal is:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
Get-Command semaprax
```

Custom `CARGO_HOME` or installation roots can change the location. The install
output tells you where the executable was placed.

## Choose the right installation route

| Route | Use it for |
| --- | --- |
| Source-built `semaprax` | The learning path, project checks, interpreter runs, semantic queries, and ordinary builds. |
| Source-built `semaprax-full` | Workflows that explicitly require private host integration, such as the Project Rust package route and source-live operations. |
| A release archive | A fixed published build for your host. Match the documentation and examples to that release. |

Install the full toolchain only when a chapter calls for it:

```sh
cargo install --locked --path crates/semaprax-toolchain
semaprax-full help all
```

“Private host” describes an implementation boundary in the source tree. It is
not an account tier. The standalone executable supplies no private host hooks.

The current source version is `0.8.0`; its release archives have not been
published. The [v0.7.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.7.0)
was published on October 1, 2026. Record the commit as well as the version
when reproducing an issue. Follow the release's verification instructions and
retain its supplied provenance files.

## Find help for your build

```sh
semaprax help run
semaprax help build
semaprax help diagnostic SPX-T208
```

A command absent from `help all` may require a newer build or the full toolchain.
Use that executable's help rather than guessing a flag.

**Next:** [Write your first program](first-program.md), or
[configure VS Code](editor.md).
