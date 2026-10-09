# Project Manifest v28: source-command resource output

Audience: language users and compiler contributors.

Status: additive native-only profile for OPT #678 with focused LOCAL native
owner gates passing. Evidence is recorded in
[`opt676-678-verification.json`](../benchmarks/opt-batch-verification-v1/opt676-678-verification.json);
no hosted or broader target support is claimed.

`source-command.resource-output.v1` keeps the Project v26 source-command ABI,
authority, file provider, and native-only target. It changes only the maximum
length-delimited String and authenticated borrowed-text view, and the staged
output policy needed to publish a larger report without rerunning the program.

## Manifest and authority

The canonical table layout selects `semaprax.project.v28`:

```toml
[package]
name = "report-command"
version = "0.1.0"
profile = "source-command.resource-output.v1"

[command]
function = "report.command.main"
input = "argv-utf8+file-text.v1"

[capabilities]
required = ["fs.read", "process.args.read", "process.stdout.write"]

[targets]
matrix = ["native64"]
```

The selected root remains the entry module's explicit-ID `fn main() -> i64`.
The capability list keeps Project v26's exact rule: a strictly sorted,
nonempty subset of `fs.read`, `process.args.read`,
`process.stderr.write`, and `process.stdout.write`, excluding stdout alone.
No stdin, filesystem write, environment, process, network, public carrier,
`Bytes`, or `Vec` authority is added. Web exports remain empty.

The argv and file-text provider is unchanged. It admits at most 16 UTF-8
arguments and 65,536 aggregate argument bytes. Each `file_read_text` operation
reads at most 65,536 UTF-8 bytes below the held invocation directory; the
existing maximum of 64 reservations and 1,048,576 reserved file bytes remains
exact. Absolute paths, dot components, symlinks, nonregular files, invalid text,
and source drift retain the v26 refusals. A v28 command without a reachable
file-text read does not open the invocation directory.

## Resource output

One owned length-delimited String may contain at most 1,048,576 bytes. A
borrowed byte view may use that bound only when retained HIR provenance proves
that its root is an immutable borrowed `str`. Ordinary borrowed `Slice<u8>`
roots remain limited to 65,536 bytes and owned `Bytes` values and views retain
their existing 131,072-byte internal limit. `bytes_copy` is not widened.

Combined staged stdout and stderr are limited to 1,048,576 bytes. Before the
root executes, the adapter allocates one 2,097,152-byte block containing one
full-capacity partition for each channel; allocation failure executes no source
code. The profile admits `stdout_append` and `stderr_append`, including bounded
repetition. Append overflow records the existing sticky command-output status.
Legacy `stdout_write`/`stderr_write` remain available with the frozen pre-HIR
rule: direct writes stay outside loops, at most one write per channel is
reachable on a path, and multiple unknown borrowed roots or a fixed plus
dynamic direct transcript remain refused during source admission. A reachable
closure may not mix legacy writes with appends. The v28 checked direct-write
helper defensively maps any admitted combined-cap overflow to command-output
status; it does not widen direct-write source admission.

Every output operation copies into staging while its authenticated source view
is live. Root failure, failed postconditions, cleanup failure, invalid exit
status, or checked append/direct-write capacity failure wipes and frees both
partitions without publishing application bytes. Success publishes stderr and
then stdout only after postconditions and non-result cleanup, then wipes and
frees staging. Infallible String and view-shape invariant violations retain the
existing abort behavior and carry no cleanup claim. The contract states per-String
and staged-output caps; it does not claim an invocation-wide cumulative
heap-allocation quota.

## Targets and preservation

Only `native64` is admitted. Interpreter execution retains `SPX-F102`; Web,
Wasm, and npm retain `SPX-W120` before artifacts. The opt-in native Project
test route is separately specified by
[Project Native Tests v1](PROJECT-NATIVE-TEST-V1.md). The manifest, lock, semantic
graph, Project digest, held-source checks, and fresh native publication bind
the v28 profile and schema exactly. The project graph's
`source_command_resource_output` object records the 1 MiB String, authenticated
borrowed-text, and combined staged-output envelope. The source graph's retained
65,536-byte portable capacity summaries remain pre-HIR source-site admission
facts; v28 does not reinterpret them as its native adapter's runtime envelope.

Project v26 `source-command.v1`, its generated C bytes, 65,536-byte borrowed
view/output limit, operation admission, and all target refusals remain frozen.
The raw-source SourceCommand selector continues to select v26; only an explicit
table manifest can select v28.
