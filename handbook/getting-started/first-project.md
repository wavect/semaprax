# First project

Single files are for learning; projects are for building. A project is a
`semaprax.toml` manifest beside a `src/` directory.

## Scaffold it

Preview the template without writing anything:

```sh
semaprax project-scaffold --name first-semaprax
```

Then create the project (the destination must not exist yet):

```sh
semaprax new first-semaprax
cd first-semaprax
```

v0.6.0 scaffolds three templates — `calculator` (default),
`--template library`, and `--template service` — with `--layout tables`
(default) or `--layout frozen` for the one-line-per-key manifest. Both flags
also work on `project-scaffold` for previewing.

You'll get a layout like this:

```text
first-semaprax/
├── semaprax.toml      # manifest: identity, modules, tests, exports
└── src/
    ├── app.spx        # entry module, declares main
    ├── core.spx       # your logic
    └── tests.spx      # test module
```

## The daily commands

```sh
semaprax check semaprax.toml                 # verify the whole project
semaprax test semaprax.toml                  # run the test module
semaprax run semaprax.toml                   # run main (prints 42 for the scaffold)
semaprax graph semaprax.toml                 # checked semantic graph as JSON
semaprax build semaprax.toml --target web -o dist/web   # web package
```

`run` prints `42`. `graph` prints deterministic JSON for tooling. `build`
creates `dist/` if needed and writes the target package there.

## The manifest

```toml
schema = "semaprax.manifest.v1"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add"]

[dependencies]
std.num = "^0.1.0"
```

Rules that bite newcomers:

- **Manifest bytes are canonical.** Keep the table order, one blank line
  between tables, one-line arrays, no comments. A non-canonical manifest fails
  with `SPX-J100`, and its `help` line names the first differing line.
- **`entry` names the one module that declares `main`.**
- **Import by stable identity, not by path**, directly after the `module` line:
  `use function @id("calculator.add") from calculator.core as add;`
- **Project function signatures are Copy scalars only.** Records, variants,
  and `Option`/`Result` work fine *inside* a function body but can't cross a
  function boundary (`SPX-G174`).

The committed
[`calculator-project`](https://github.com/wavect/semaprax/tree/main/examples/calculator-project)
is the reference instance. Exact manifest rules live in the Project Manifest
specifications under
[`docs/`](https://github.com/wavect/semaprax/tree/main/docs).

## Next step

Learn the language core: [Essentials](../language/essentials.md).
