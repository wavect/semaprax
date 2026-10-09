# Project Native Tests v1

Status: source implementation authored for Project v26 and v28
`source-command` profiles. The owning executable gate is pending current-head
verification; this document makes no hosted, physical-device, or production
qualification claim. The completion matrix owns product status.

Audience: users of native-only source-command Projects, CLI authors, and
compiler contributors.

This specification adds an opt-in native target to the Project test command.
The existing interpreter `semaprax test` route, its `semaprax.project-execution.v1`
envelope, and its `SPX-F102` refusal for v26/v28 are unchanged.

## Selection and authority

Use the route with a Project selector:

```sh
semaprax test . --target native
semaprax test semaprax.toml --target native --json
semaprax test --manifest-path semaprax.toml --target native \
  --native-timeout-ms 10000 --native-max-output-bytes 65536
```

The target is admitted only for Project v26 `source-command.v1` and Project
v28 `source-command.resource-output.v1`. It uses the authenticated,
manifest-declared test module and its `test_program`; it does not compile the
manifest command entry as a test. The test module's explicit-ID `main` is the
first root. Each selected named case follows in stable-identity order. A named
case has a `test_` display-name prefix, no parameters, an `i64` result, and an
explicit `@id`. Only roots selected from that test module are compiled; this
does not promise that every function in the module is compiled as a root.

Each root is compiled separately from the authenticated test closure. The
root's stable identity becomes the executable entrypoint. Reachable effects
must be contained in the exact capability grant of the Project manifest, and
the existing v26 or v28 SourceCommand native adapter supplies the already
admitted argument and file providers. The test wrapper supplies no user
arguments, runs with the Project directory as its working directory, and gives
the child a closed stdin. The interpreter receives no argv or file provider.
The route adds no manifest fields, capability grants, or input-shape changes.

## Results

Every root passes only when its observed `i64` result is zero. A pure native
root (one whose linked test closure retains no permits) must exit successfully
and write one UTF-8 line parseable as an `i64`, followed by one line feed. A leading
`+`, surrounding whitespace, missing line feed, multiple lines, invalid UTF-8,
or an unparseable value fails the root. The decimal form must be canonical:
leading zeroes and negative zero fail the root.

When the linked test closure retains one or more permits and satisfies the
existing SourceCommand authority admission, each executable uses the adapter's
process status as its result, even if that particular root does not call an
effect. Closure permits come from retained function effect declarations;
unused module permits do not add them. As with existing SourceCommand admission,
a closure whose only permit is `process.stdout.write` is refused. A successful
source result in `0..=255` is returned as the process status. A language status
failure and an out-of-range
result, including `256`, exit nonzero. The CLI treats status zero as a pass and
every nonzero status as a failed check. A native runtime trap also fails the
root.

`--json` emits `semaprax.native-test.v1` with `target`, overall `passed`, and a
`cases` array. Each result contains `stable_id`, `name`, `role` (`main` or
`case`), `passed`, nullable `result`, nullable `exit_code`, `outcome`, `stdout`,
and `stderr`. Captured byte streams are represented as lossy UTF-8 strings.
Authentication, compilation, or host failures before case results are
collected retain the command's diagnostic reporting path.

## Process bounds and cleanup

The default timeout is 10,000 ms for each root's child process, and the default
combined stdout and stderr bound is 65,536 bytes per root. Compilation is
outside the process timeout. `--native-timeout-ms` accepts a positive
integer through 600,000. `--native-max-output-bytes` accepts a positive
integer through 1,048,576. Exceeding either bound fails the root. Timeout
handling kills and waits for the child. Stdin is null; output is captured, not
forwarded as ambient host streams.

Native tests do not accept interpreter `--max-steps` or `--max-bytes` limits.
The native limit options require `--target native`; ordinary interpreter test
options and behavior remain as specified by
[Project Test Cases v1](PROJECT-TEST-CASES-V1.md).

Each executable is created in a fresh private scratch directory and the
authenticated scratch inventory is discarded on ordinary command outcomes,
including compilation errors, spawn failures, timeouts, and result failures.
Changed scratch identity fails closed. An abrupt termination of the CLI process
does not carry an unconditional physical-cleanup guarantee.

## Owning gate

The owning gate must exercise the native route for both source-command profile
versions, including the selected root set and stable ordering, pure and
effectful results, interpreter-route `SPX-F102` preservation, timeout and
output bounds, and scratch cleanup on ordinary outcomes. Until that executable
gate passes on the current head, this route remains pending in the completion
matrix.

The focused integration selectors are:

```sh
cargo test --locked -p semaprax --test project source_command::source_command_native_tests_
cargo test --locked -p semaprax --test project_cli_v1 native_test::
```
