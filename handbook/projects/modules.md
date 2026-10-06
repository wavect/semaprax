# Modules and imports

Split a program into modules: reusable logic in one, `main` in another, tests
in a third. After this page you can build, test and run a three-module project
by hand.

Want a ready-made layout instead? Run `semaprax new <dir>`
(see [Manifests](manifests.md#start-from-a-template)). To follow along, create a
`tutorial/` directory with a `src/` subdirectory and save each block at its
stated path.

## 1. Describe the project

Save this as `tutorial/semaprax.toml`:

<!-- handbook-project-file: {"group":"modules","path":"semaprax.toml","stdout":"42\n","test":true} -->
```toml
schema = "semaprax.manifest.v1"

[package]
name = "handbook-demo"
version = "0.1.0"

[modules]
entry = "tutorial.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["tutorial.tests"]

[exports]
web = ["tutorial.add"]
```

`entry` is the module with `main`. `sources` lists the files. `[exports] web`
lists the functions a web build exposes, by stable ID.

## 2. Write the reusable function

Save this as `tutorial/src/core.spx`:

<!-- handbook-project-file: {"group":"modules","path":"src/core.spx"} -->
```semaprax
module tutorial.core;

@id("tutorial.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}
```

This module has no `main`: it only provides `add`. Check it through the
project, not alone (a single file without `main` fails with `SPX-T105`).

## 3. Import it into the entry module

Save this as `tutorial/src/app.spx`:

<!-- handbook-project-file: {"group":"modules","path":"src/app.spx"} -->
```semaprax
module tutorial.app;
use function @id("tutorial.add") from tutorial.core as add;

@id("tutorial.main")
fn main() -> i64
{
    add(19, 23)
}
```

Read the import as a sentence: use the function with ID `tutorial.add` from
module `tutorial.core`, and call it `add` here. An import names a stable ID, not
a file path. Imports come right after the `module` line, before any `permit`
block.

## 4. Add tests

Save this as `tutorial/src/tests.spx`:

<!-- handbook-project-file: {"group":"modules","path":"src/tests.spx"} -->
```semaprax
module tutorial.tests;
use function @id("tutorial.add") from tutorial.core as add;

@id("tutorial.tests.add")
fn test_add() -> i64
{
    if add(19, 23) == 42 { 0 } else { 1 }
}

@id("tutorial.tests.zero")
fn test_zero() -> i64
{
    if add(0, 0) == 0 { 0 } else { 1 }
}

@id("tutorial.tests.main")
fn main() -> i64
{
    0
}
```

Each `test_*` function returns `0` to pass. The manifest's `tests` entry tells
the runner which module to read. See [Testing](../practices/testing.md).

## 5. Run the project

From inside `tutorial/`:

```sh
semaprax fmt .
semaprax check .
semaprax test .
semaprax run .
semaprax query . --id tutorial.add
```

`test` prints `project tests passed`, `run` prints `42`, and `query` prints
the `tutorial.add` declaration. A directory operand (`.`) selects its
`semaprax.toml`.

## Keep names and paths separate

```text
src/core.spx          file containing source
    tutorial.core     module declared by that file
        tutorial.add  persistent identity of one function
            add       local name used at a call site
```

A display rename keeps `tutorial.add`. Moving a file means updating the
manifest path. Renaming a module means updating its imports. Three separate
edits, which is why the ID is the stable handle.

## Fix common mistakes

| Symptom | Check first |
| --- | --- |
| The new file seems invisible | Its path is present in `sources`. |
| An import cannot be resolved | The provider's module name and declaration ID both match. |
| Tests are not running | The module is in `tests`, and test functions use the `test_` prefix. |
| A helper works alone but fails when linked | The project's [profile](profiles.md) admits its signature. |
| A previous semantic preview is stale | Re-query the project after changing source or the manifest. |
| `SPX-G172` or `SPX-T105` on one file | Check the project (`semaprax check .`), not the single file. |

## Use a standard-library module

Declare the package, then import from it:

```sh
semaprax add . std.num "^0.1.0"     # adds one [dependencies] row
semaprax help library std.num       # signatures and stable IDs
```

`add` edits only the manifest. Bundled `std.*` packages are version `0.1.0`
and need no download. See the [standard library](../reference/stdlib.md).

**Next:** [Choose a profile for richer data](profiles.md), or
[learn how the test runner reports failures](../practices/testing.md).
