# First project

You will create a three-module calculator project, run its checks and tests,
build it for the web, and break one test on purpose. The project's
`semaprax.toml` is its **manifest**: it lists the modules, tests, and exports.

## 1. Create it

```sh
semaprax new first-semaprax
cd first-semaprax
```

The destination must not exist. Add `--name <project-name>` to override the
name, or `--template library` or `--template service` for another starter.

```text
first-semaprax/
├── semaprax.toml
├── README.md
├── AGENTS.md
└── src/
    ├── app.spx
    ├── core.spx
    └── tests.spx
```

| File | Holds |
| --- | --- |
| `src/app.spx` | The entry point, `main`. |
| `src/core.spx` | The logic: `add`. |
| `src/tests.spx` | The tests. |
| `AGENTS.md` | Commands and language rules for coding agents. Keep it. |

## 2. Run it

Run these inside `first-semaprax/`:

```sh
semaprax fmt . --check
semaprax check .
semaprax test .
semaprax run .
```

```text
verified project first-semaprax (sha256:...)
project tests passed
42
```

`fmt . --check` prints nothing when every file is canonical. Every command also
accepts `semaprax.toml` instead of `.`.

## 3. Read the files

<!-- handbook-project-file: {"group":"first","path":"semaprax.toml","stdout":"42\n","test":true} -->
```toml
schema = "semaprax.manifest.v1"

[package]
name = "first-semaprax"
version = "0.1.0"

[modules]
entry = "first_semaprax.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["first_semaprax.tests"]

[exports]
web = ["first-semaprax.add"]
```

<!-- handbook-project-file: {"group":"first","path":"src/core.spx"} -->
```semaprax
module first_semaprax.core;

@id("first-semaprax.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}
```

`src/app.spx` imports `add` by stable ID and calls it:

<!-- handbook-project-file: {"group":"first","path":"src/app.spx"} -->
```semaprax
module first_semaprax.app;
use function @id("first-semaprax.add") from first_semaprax.core as add;

@id("first-semaprax.app.main")
fn main() -> i64
{
    add(19, 23)
}
```

<!-- handbook-project-file: {"group":"first","path":"src/tests.spx"} -->
```semaprax
module first_semaprax.tests;

@id("first-semaprax.tests.main")
fn main() -> i64
{
    if 19 + 23 == 42 { 0 } else { 1 }
}
```

Three names do three jobs:

| Name | Example | Job |
| --- | --- | --- |
| File path | `src/core.spx` | Where the source lives. |
| Module | `first_semaprax.core` | Which module declares the function. |
| Stable ID | `first-semaprax.add` | What other modules import. |

`use function @id("...") from <module> as <name>;` imports by stable ID. See
[Modules and imports](../projects/modules.md).

## 4. Inspect and build

```sh
semaprax query . --kind function
```

```text
src/app.spx	function	first-semaprax.app.main	fn main() -> i64
src/core.spx	function	first-semaprax.add	fn add(left: i64, right: i64) -> i64
src/tests.spx	function	first-semaprax.tests.main	fn main() -> i64
```

```sh
semaprax build . --target web -o dist/web
```

The `[exports]` table in the manifest picks which functions the web package
exposes (`first-semaprax.add`). Building does not start a server. The output
directory must not exist yet. See [Targets](../projects/targets.md).

## 5. Break a test

Open `src/tests.spx` and change `19 + 23 == 42` to `19 + 23 == 41`. Run
`semaprax test .` and read the failure. Restore the line and run it again.

To add named cases, write `fn test_<name>() -> i64` functions with an `@id`
that return `0` on success. See [Testing](../practices/testing.md).

## Add a module

1. Create `src/<name>.spx` with `module first_semaprax.<name>;`.
2. Add its path to `sources` in `semaprax.toml`. List a test module under `tests`.
3. Run `semaprax check .`. Check the whole project, not a single file: a lone
   file that imports another module reports `SPX-G172` or `SPX-T105`.

## When the manifest is rejected

Semaprax accepts one canonical manifest layout. Keep the generated table order,
one-line arrays, and blank lines, and follow the first `SPX-J100` hint. The
[manifest guide](../projects/manifests.md) lists every field.

`semaprax project-scaffold --name <name>` prints a starter as one JSON capsule
without writing files. It is for tools; use `new` to create a project.

**Next:** [Language essentials](../language/essentials.md), or
[write your own modules](../projects/modules.md).
