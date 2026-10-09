# LogLens

This is a private source adaptation of the separate `matcher-adapted`
reference. It is not compiler-checked or qualified yet; the original directory
and its source remain unchanged.

Build and run with the verified SEMAPRAX compiler:

```sh
./build.sh
./run.sh ../sample.log
./run.sh ../sample.log --top 3 --json
./test.sh
```

`SEMAPRAX_BIN` may override the default verified compiler path. Node from
`PATH` runs the black-box automated tests; the application itself is compiled
SEMAPRAX. `run.sh` resolves file paths and normalizes command arguments for the
compiler's bounded file provider. Parsing, aggregation, ranking, decimal
arithmetic, and rendering live in `loglens.spx`. This private adapter variant
imports the public `std.pattern` matcher API declared by its package dependency;
it does not carry a copy of the matcher implementation.

The [pattern API guide](../../../../std/pattern/README.md) documents the
stable-ID imports, source-string escapes, and whole-owner renewal used here.
`report` compiles its fixed pattern once, keeps the returned `Matcher`, and
renews it against each independent line view. Its six captures are offsets
into that line, so extraction applies them to `line`, not the enclosing file.
The existing token/path/hour/decimal checks still decide the captured values'
application meaning. Invalid or resource-refused matcher packets take the
terminal error path; they are distinct from status 2 semantic no-match.
This integration description adds no qualification evidence and changes none
of the complete 49 acceptance obligations.

The resource-output project profile supports reports larger than 64 KiB.
Byte counts use exact decimal arithmetic, including totals beyond i64.
`unit.spx` provides a compiler-checked decimal test declaration; the native
source-command profile cannot run through the interpreter test command, so
`test.sh` verifies behavior by executing the compiled application.

The specification's inline golden example has 12 nonempty lines, whereas the
public `../sample.log` has 249. Tests reconstruct the inline example and
compare its two exact output strings (text with `--top 3`, as labeled in the
specification). They separately check the public sample in both formats
against an independent test reference. Additional tests cover line endings,
malformed/truncated input, Unicode, tie ordering, rounding, large totals,
argument errors, file errors, and exit statuses 0, 1, and 2.
