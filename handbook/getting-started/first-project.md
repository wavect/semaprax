# First project

A project connects source files, tests, dependencies, and exports. Its
`semaprax.toml` file is a **manifest**: a list of those inputs and choices.
You will create a calculator project and run its checks before changing it.

## 1. Preview the files

From a directory where you keep projects, run:

```sh
semaprax project-scaffold --name first-semaprax
```

A scaffold is a starter project. This command previews one without writing it.
Read the proposed manifest and source paths in the output.

## 2. Create the project

The destination must not already exist:

```sh
semaprax new first-semaprax
cd first-semaprax
```

The default template is `calculator`. To create a different project, use a new
destination with `--template library` or `--template service`.

The key files are:

```text
first-semaprax/
├── semaprax.toml
└── src/
    ├── app.spx
    ├── core.spx
    └── tests.spx
```

`app.spx` contains the entry point. `core.spx` contains the reusable logic.
`tests.spx` checks that logic. The generated project also supplies guidance
for coding agents; keep it with the project.

The standalone `new` command creates the standard table-layout manifest.
Use `semaprax help project-scaffold` for preview layout options. Do not add
`--layout` to `new` unless your installed executable's help accepts it.

## 3. Run the existing project

These commands now run **inside `first-semaprax/`**:

```sh
semaprax fmt . --check
semaprax check .
semaprax test .
semaprax run .
```

The calculator entry returns `42`, and its tests should pass. Establish this
working starting point before editing. You can also pass `semaprax.toml`
instead of `.`.

## 4. Follow one call

Open `src/app.spx`. Find its import and the call that uses it. Then open
`src/core.spx` and find the matching `@id`.

Three names have separate meanings:

| Name | Example | Purpose |
| --- | --- | --- |
| File path | `src/core.spx` | Where the source is stored. |
| Module name | `calculator.core` | Which module declares a function. |
| Stable ID | `calculator.add` | Which declaration another module imports. |

The generated manifest and source are the authority for your template's exact
names. The [modules tutorial](../projects/modules.md) builds a complete small
example and explains each part of an import.

## 5. Inspect and build

List functions without dumping the entire semantic graph:

```sh
semaprax query . --kind function
```

Then build the calculator's selected web exports:

```sh
semaprax build . --target web -o dist/web
```

The manifest chooses which functions become callable from the generated
package. Building a package does not start a web server. See
[Targets](../projects/targets.md) for the consumer step and native builds.

## Make your first change

Open one `test_*` function in `src/tests.spx`. Change its expected result to a
wrong value and run `semaprax test .`. Notice the failing test's stable ID.
Restore the correct value and run the suite again.

This small exercise teaches the feedback loop you will use for larger work:
change one behavior, run the test that describes it, and inspect the result.

## When the manifest is rejected

Semaprax expects a **canonical** manifest, meaning one accepted spelling and
layout. Preserve generated table order, one-line arrays, and blank lines.
Follow the first `SPX-J100` formatting hint instead of trying arbitrary TOML
layouts. The [manifest guide](../projects/manifests.md) explains the fields.

For richer function boundaries, choose the matching
[project profile](../projects/profiles.md). You do not need those profiles to
finish the calculator learning path.

**Next:** [Learn the language essentials](../language/essentials.md), or
[write your own multi-file project](../projects/modules.md).
