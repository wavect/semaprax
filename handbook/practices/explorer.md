# Explore a project's meaning

Use the semantic explorer when you want to understand a project before editing
it. It turns the compiler's project information into a view you can read, share,
or compare. Start with one function rather than the entire graph.

## Build a focused HTML view

From the repository root, choose a new output filename and run:

```sh
semaprax explore examples/calculator-project/semaprax.toml --target calculator.add --depth 1 --format html --output calculator-explorer.html
```

Open the generated file in a browser. The view is centered on `calculator.add`.
Depth limits how far the selected neighborhood extends; it does not change
the underlying source.

Generated reports should stay out of source commits unless you intentionally
version them as review artifacts.

## Choose the format for the task

The command supports `html`, `json`, `markdown`, and `svg`. Use HTML to browse,
JSON for a tool, Markdown for a written review, and SVG for a diagram.

For example:

```sh
semaprax explore examples/calculator-project/semaprax.toml --target calculator.add --depth 1 --format markdown --output calculator-explorer.md
```

Use separate output filenames for separate formats and subjects. Retain the
report's revision information when sharing it.

## Ask smaller questions from the terminal

You do not need a visual export for every question:

```sh
semaprax query examples/calculator-project --id calculator.add
semaprax query examples/calculator-project --calls calculator.add
semaprax query examples/calculator-project available-operations calculator.add
```

The first command locates the function. The second finds its callers. The
third asks which typed change operations the current project exposes for that
target.

A **caller** uses the function. A **callee** is a function it calls. Keeping
those directions straight helps when estimating the effect of a change.

## Preview a rename

```sh
semaprax change preview examples/calculator-project rename-display-name calculator.add sum
```

The proposal changes the display name to `sum` while addressing the existing
stable ID. Inspect the preview before moving to an authorized change workflow.

The explorer can also read a candidate capsule with `--candidate-capsule` and
`--expect-candidate`. Use the exact digest of the candidate you intend to review;
run `semaprax help explore` for the full command shape. A **capsule** packages
revision-bound candidate data for later inspection or replay.

## Review the useful questions

Before accepting a change, ask what declaration changed, which callers are
involved, whether contracts or effects changed, and which tests cover the
behavior. Refresh the report after editing source. An old diagram can remain
readable while no longer describing the current project.

**Next:** [Use the same workflow from an AI coding agent](agents.md).
Implementation reference: [installed command catalog](https://github.com/wavect/semaprax/blob/main/src/cli/help.rs).
