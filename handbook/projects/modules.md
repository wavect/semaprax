# Modules and imports

Split a program when one file starts doing several jobs. Put reusable logic
in one module, the entry point in another, and tests in a third. You will
build that layout here without relying on a generated template's names.

Create a fresh `tutorial/` directory with a `src/` subdirectory. Save each
block below at its stated path. The names in these four files belong together.

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

`entry` names the application's module. `sources` names files. The export
selects the addition function by stable ID.

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

This module has no `main`. It provides a function for other modules to use.
Its two preconditions describe the accepted arguments.

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

Read the import as a sentence: “Use the function identified by `tutorial.add`
from module `tutorial.core`, and call it `add` in this file.”

Imports go immediately after the module declaration, before an optional
`permit` block and the ordinary declarations. An import names a semantic
identity, not a relative file path.

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

The `test_*` functions return `0` when their assertion passes. The manifest's
`tests` list tells the runner where to find them.

## 5. Run the project

From inside `tutorial/`:

```sh
semaprax fmt .
semaprax check .
semaprax test .
semaprax run .
semaprax query . --id tutorial.add
```

The tests should pass, the application should print `42`, and the query should
locate the addition function. You have now connected a manifest, two importing
modules, and one shared declaration.

## Keep names and paths separate

```text
src/core.spx          file containing source
    tutorial.core     module declared by that file
        tutorial.add  persistent identity of one function
            add       local name used at a call site
```

A display rename can preserve `tutorial.add`. Moving a file still requires an
updated manifest path. Changing a module name affects imports. These are
separate edits, which is why a stable ID is useful.

## Fix common mistakes

| Symptom | Check first |
| --- | --- |
| The new file seems invisible | Its path is present in `sources`. |
| An import cannot be resolved | The provider's module name and declaration ID both match. |
| Tests are not running | The module is in `tests`, and test functions use the `test_` prefix. |
| A helper works alone but fails when linked | The project's [profile](profiles.md) admits its signature. |
| A previous semantic preview is stale | Re-query the project after changing source or the manifest. |

**Next:** [Choose a profile for richer data](profiles.md), or
[learn how the test runner reports failures](../practices/testing.md).
