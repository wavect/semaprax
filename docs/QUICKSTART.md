# Quickstart

Status: public alpha example, not a production-readiness claim.
Audience: first-time SEMAPRAX users.

> Prefer a guided path? The user-facing [Semaprax Handbook](../handbook/README.md)
> covers this flow in [First project](../handbook/getting-started/first-project.md).
> This page remains the exact, test-pinned command reference.

From the repository root, install the standalone CLI:

```sh
cargo install --locked --path .
```

If `semaprax` is not found, add Cargo's binary directory to `PATH`; see
[Install](INSTALL.md) for prerequisites and release archives. Cargo may
download Rust dependencies during installation. The commands below do not
install project dependencies or initialize Git.

To inspect the template without writing files, run:

```sh
semaprax project-scaffold --name first-semaprax
```

This prints a canonical scaffold document to stdout. It does not create a
project or authorize another tool to publish one.

Next, from a directory that does not contain `first-semaprax`, run:

```sh
semaprax new first-semaprax
cd first-semaprax
semaprax check semaprax.toml
semaprax test semaprax.toml
semaprax run semaprax.toml
semaprax graph semaprax.toml
semaprax build semaprax.toml --target web -o dist/web
```

`run` prints `42`. `graph` prints deterministic JSON for the checked project.
`build` creates `dist` if needed and writes the Web package to `dist/web`.

`new` needs a fresh destination under an existing parent and never replaces an
entry. A failed creation may leave files that need manual inspection; it never
deletes them for you. The [standalone creation contract](NEW-PROJECT-STANDALONE-V1.md)
defines this route. The optional full toolchain and release archives accept
the same `new` arguments but use a different, staged publication route; see
[calculator project publication](NEW-PROJECT-PUBLICATION-V1.md).
