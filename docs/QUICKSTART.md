# Quickstart

Audience: new SEMAPRAX users and contributors.

Status: pre-alpha bounded calculator workflow; not a production-readiness claim.

This quickstart uses the source-installed `semaprax-full` CLI to create the
built-in calculator Project v1 template, then the standalone `semaprax` CLI
to exercise it. Install both CLIs from the same checkout root:

```sh
cargo install --locked --path .
cargo install --locked --path crates/semaprax-toolchain
```

The first command installs `semaprax`; the second installs the private
`semaprax-full`. Ensure Cargo's binary directory is on your `PATH`.
Installation may fetch Rust dependencies. The project generator itself does
not access a network, initialize Git, or install dependencies. Tag archives
expose the full CLI under the `semaprax` name instead; when using an archive,
use `semaprax new` in place of `semaprax-full new` below.

The generator requires a fresh destination beneath an existing parent.
Its [publication contract](NEW-PROJECT-PUBLICATION-V1.md) keeps a published
project intact if final verification fails; do not automatically delete a
reported failure's output or staging residue.

From a directory where `first-semaprax` does not already exist, run:

```sh
semaprax-full new first-semaprax
cd first-semaprax
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax graph src/app.spx
semaprax build semaprax.toml --target web -o dist/web
```

The run command prints `42`. The graph command emits deterministic JSON for
the generated application module. The final command creates the single
missing `dist` parent and publishes the Web package at `dist/web`.

SEMAPRAX remains pre-alpha. This flow demonstrates the bounded calculator
project contract; it is not a production-readiness or broader ecosystem claim.

The executable quickstart suite passed all nine tests locally on macOS arm64
with Rust 1.98, including the seven-command flow and hostile output-parent
cases. That suite invokes freshly built CLI paths and checks the install
instructions as text; it does not prove installation, `PATH` setup, release
archives, or Windows behavior.
