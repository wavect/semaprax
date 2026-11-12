# Selected-file LogLens adoption

Status: native O0/O2 and original CLI49 passed locally on macOS arm64.
Audience: compiler maintainers and LogLens application authors.

This new reference derives from the immutable accepted source at
`experiments/ascii-pattern-source/loglens-reference/qualified-source/`.
Historical source, provenance, SPEC, oracle, corpus and results remain unchanged.
Its retained acceptance does not qualify this adaptation or measure savings.

The application selects `source-command.selected-file.resource-output.v1`
(Project v34), input `argv-utf8-256+selected-file.follow.v1`, native64 Unix.
Only checked source validates filename/option usage. `run.sh` locates the
executable and prepends the fixed host selector for application argv index 0;
it never resolves, rewrites, normalizes or opens the input path or its options.
The native provider follows the explicit host-selected path on first checked
read and holds that descriptor until settlement. File bytes remain bounded at
65536, raw argv at 256 arguments/65536 aggregate bytes, selected spelling at
4096 bytes, and String/staged output at the inherited v28 1 MiB bounds.
Numeric byte counts have no machine-integer ceiling inside the bounded file;
these physical limits are not a new numeric range restriction.

Set `SEMAPRAX_BIN` to a compiler built from the joined source and run:

```sh
./build.sh
./run.sh ../../../benchmarks/cli-tokens-v1/sample.log --top 3 --json
./test.sh
```

`tests.mjs` retains every original test and assertion, including the
33,000-digit report, thirty repeated `--json` flags, option order and repeated
`--top`, usage before missing-file acquisition, leading-zero top values,
Unicode, line endings, malformed input, exact output bytes and exit statuses.
Only the public sample's repository-relative location changes. Source takes
over the prior shell's filename-flag and ASCII-digit checks; report parsing,
aggregation, sorting, exact decimal arithmetic and rendering remain unchanged.
The owning Project gate passed all 33 historical and 16 SPEC boundary checks
against both native O0 and O2 builds from compiler source
`7d27641abb9a80b357beaf602b96ee3ffc4fd752`. Its selected-path controls
also passed for absolute and parent-relative paths, hidden names and spaces,
explicitly selected symlinks, and usage errors before file acquisition. The
public CLI build and checked-in wrapper passed the same CLI49 acceptance.
Fresh matched agent trials remain pending.
