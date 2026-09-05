# Quickstart

Audience: new SEMAPRAX users and contributors.

Status: pre-alpha bounded calculator workflow; not a production-readiness claim.

This quickstart uses the standalone `semaprax` CLI to create the built-in
calculator Project v1 template and then exercise it. Install it from the
checkout root:

```sh
cargo install --locked --path .
```

[Install](INSTALL.md) covers the prerequisites, the release-archive route,
`PATH` setup, and what a first failed command means.

Ensure Cargo's binary directory is on your `PATH`. Installation may fetch Rust
dependencies. The project generator itself does not access a network,
initialize Git, or install dependencies. The optional full toolchain
(`cargo install --locked --path crates/semaprax-toolchain`) and the tag
archives accept the same `new` grammar and create the same files; they publish
through a held-parent staged rename instead of the standalone create-new route.

To inspect or hand to another tool the exact calculator files without granting
SEMAPRAX a destination or write authority, print the public scaffold capsule:

```sh
semaprax project-scaffold --name first-semaprax
```

This writes one canonical `semaprax.project-scaffold.v2` document to stdout and
does not materialize a project. The capsule is not a publication primitive; a
consumer that writes its files owns that filesystem and publication policy.

The generator requires a fresh destination beneath an existing parent and
never replaces an entry. The standalone route is owned by
[standalone project creation](NEW-PROJECT-STANDALONE-V1.md) and the full
toolchain's by [calculator project publication](NEW-PROJECT-PUBLICATION-V1.md);
neither deletes a reported failure's output or residue automatically.

From a directory where `first-semaprax` does not already exist, run:

```sh
semaprax new first-semaprax
cd first-semaprax
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax graph semaprax.toml
semaprax build semaprax.toml --target web -o dist/web
```

The run command prints `42`. The graph command emits deterministic JSON for
the authenticated project, including the generated application and its imported
modules. The final command creates the single
missing `dist` parent and publishes the Web package at `dist/web`.

SEMAPRAX remains pre-alpha. This flow demonstrates the bounded calculator
project contract; it is not a production-readiness or broader ecosystem claim.

The executable quickstart suite's previously recorded nine tests passed locally on macOS arm64
with Rust 1.98, including the seven-command flow and hostile output-parent
cases. The additive stdout-only scaffold case also passed locally. That suite
invokes freshly built CLI paths and checks the install instructions as text; it
does not prove installation, `PATH` setup, release archives, or Windows
behavior.
