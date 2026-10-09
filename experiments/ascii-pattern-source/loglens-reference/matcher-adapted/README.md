# LogLens

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
arithmetic, and rendering live in `loglens.spx`.

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
