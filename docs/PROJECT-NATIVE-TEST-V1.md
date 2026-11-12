# Project Native Tests v1

The Project v26 native-test gate passed on hosted macOS. The Project v28
effectful result-range gate passed locally on macOS. The completion matrix
owns product status.

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
A per-case host refusal publishes no result, including when a successful child
exit was observed before its output readers finish. The captured exit code and
first selected timeout/output/read failure remain visible; neither result
carrier can turn that refusal into a successful reported value.

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
output bounds, and scratch cleanup on ordinary outcomes.

The hosted macOS Project Manifest job at `80099513c` passed all 15 CLI tests,
including the original 100 ms recovery test. The local Project API tests
passed declared-root selection and manifest-bounded argv and file access.
Six CLI unit tests covered argument limits, canonical results, host refusal,
output capture, and timing observations.
[Hosted Project Manifest job](https://github.com/wavect/semaprax/actions/runs/43212603758/job/131164291990)

The focused integration selectors are:

```sh
cargo test --locked -p semaprax --test project source_command::source_command_native_tests_
cargo test --locked -p semaprax --test project_cli_v1 native_test::
```

## Opt-in diagnostic timings

`semaprax test --target native --native-timing --json` adds a per-case
`diagnostic_timings` object with schema
`semaprax.native-test-diagnostic-timing.v1`. Without this flag, the existing
v1 output shape is unchanged. The flag requires native target and JSON.

Monotonic observations relative to host `run_case` entry record spawn return,
existing deadline timer start, first wait, terminal exit/deadline/output/wait
observation, both reader settlements, and observation completion. Missing stages
are null, including stages unavailable after spawn failure. Setup time precedes
the existing deadline interval; reader settlement follows its terminal
observation. Compilation, sealing, scratch cleanup, and JSON serialization are
outside this observation. Loader and OS scheduling time remain inside the
original process deadline; the instrumentation neither isolates those costs
nor changes timeout, output refusal, result publication, or settlement.
The same opt-in object also records wait and pending observation counts, last
wait/pending timestamps, the maximum gap between observed waits, and terminal
deadline overshoot. These fixed-size observations distinguish repeated pending
checks from long parent polling gaps. They do not measure child CPU or identify
loader/scheduler causes. No polling counter or timestamp is sampled when the
flag is absent; the existing wait order and five-millisecond sleep are unchanged.
Timing values are diagnostic observations, not deterministic pins or acceptance
criteria. The original 100 ms recovery control passed in the hosted macOS job.

A local macOS run reached the unchanged 100 ms deadline for the infinite case
and its two constant-zero controls. The captured control executables have the
same SHA-256. Direct first launches of those bytes and byte-identical copies
took 143–176 ms and produced the expected `0\n`; repeat launches of the same
files took about 2 ms. The reproduction establishes cold-launch sensitivity
without attributing the delay to a particular OS component. A subsequent local
run reproduced the timeout of the constant-zero controls. Fresh, byte-identical
copies on both the internal and external SSD took 140–572 ms to launch and
return. A separate local run of the same timeout and reentry fixture passed
with an explicitly selected 2,000 ms deadline.

The v28 CLI range regression passed. An effectful case returned `256`, selected
exit code 1 and the range diagnostic, then the next declared case completed and
the scratch directory was empty.

The owning test harness alone accepts an explicit
`SEMAPRAX_NATIVE_TEST_DIAGNOSTICS=<directory>` output directory. It uses a local
compiler wrapper to snapshot C, compiler arguments and the linked executable
before entry, then retains only deadline-failed cases with source/command hashes
and the existing timing object. Each fresh attempt allows at most 64 capture
bundles (192 files) and 64 MiB of artifact bytes. Read-only receipts are diagnostic
data; they grant no execution or source authority. Compiler environment changes
and copying occur outside the deadline, so captured runs support no performance
comparison or loader-cost conclusion. The helper runs each program once and
leaves CLI scratch cleanup, the 100 ms control, and acceptance assertions intact.
Without the variable, the original harness path is unchanged.
