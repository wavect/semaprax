# Stream Text Command v1

Status: implemented additive native profile with focused local executable evidence.

Project v25 (`semaprax.project.v25`) selects
`language-command-io.stream-text.v1`. It retains the exact
`argv-utf8+stdin-stream.v1` input, four command capabilities, and explicit
stable-ID `fn() -> i64` command. The process result and transcript publication
are [Bounded Stdin Command Exit Status v1](BOUNDED-STDIN-COMMAND-EXIT-V1.md):
application statuses 0 through 255, checked failure distinct from an application
result, and invalid process status rejected before transcript publication.
The private runner/result remain v2; the input and provider remain v1.

This profile selects existing language surfaces for streaming applications:

- length-delimited native Strings and the five pure operations from
  [Text Toolkit v1](TEXT-TOOLKIT-V1.md): `string_slice`, `string_find`,
  `string_to_i64`, `string_trim`, and `string_byte_at`, with their existing
  UTF-8, offset, sentinel, and `semaprax.text.v1` failure rules;
- the existing local [String Collections v1](STRING-COLLECTIONS-V1.md) operations
  under their unchanged capacities and owned-map rules; this does not add Map
  parameters, results, or record fields;
- private ordinary helper calls taking owned `string` and returning `string`,
  alongside the existing Copy, named `borrow str`, borrowed byte-slice, and
  independently authenticated sealed Reader forwarding signatures.

These are private compiler-linked boundaries, not a public owned UTF-8 ABI.
The selected command itself has no parameters and returns only `i64`.
Borrowed views still cannot escape; aggregate/capture storage, generic wrappers,
and new mutable borrows are not added. A helper returning Reader must still
pass the bounded acyclic carrier-forwarding proof. The pure `main`/test closure
admits the same String helper boundaries but no command effects.

The invocation has the same immutable argv and one 4096-byte stdin buffer,
reader ownership, chunk epochs, and source-loan expiration as
[Bounded Standard Input Streaming v1](BOUNDED-STDIN-STREAM-V1.md).
`file_read_text` is excluded: there is no `fs.read`, filesystem adapter,
network, environment, or process-execution capability. The snapshot input and
output-append host families remain excluded by the closed streaming operation
inventory. Existing quotas, source/loan work limits, and String behavior are
unchanged. Separating parsing into helpers avoids inlining pressure; it does
not increase the 4096 loan-program-point limit.

Native selection combines the existing length-delimited String runtime and
present-String cleanup tracking with streaming Slice epoch authentication and
command carriers. Calls stage owned arguments once, left to right, and transfer
at the existing authenticated Call commit. Result handoff precedes canonical
non-result cleanup, failures remain sticky, and both normal and failed helper
calls settle owned Strings and Reader. Cleanup/LoanPlan and source graph
schemas remain their existing versions; this is a target/profile addition.
Project v23/v24 retain their original String runtime and closed helper
boundaries. Programs selecting them do not acquire this addition.

## Selection and usage

Use the ordinary extensible manifest table layout:

```toml
schema = "semaprax.manifest.v1"

[package]
name = "stream-text"
version = "0.1.0"
profile = "language-command-io.stream-text.v1"

[modules]
entry = "app"
sources = ["a/app.spx", "b/tests.spx"]
tests = ["app.tests"]

[exports]
web = ["app.command"]

[command]
function = "app.command"
input = "argv-utf8+stdin-stream.v1"

[capabilities]
required = ["process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
```

This lowers exactly to Project v25. Its frozen eleven-field manifest has the
same field order as Project v24, with only the schema/profile values replaced.
Schema/profile or input mismatches are refused. Build with
`semaprax build --manifest-path semaprax.toml --target native --output app`.
`semaprax run` still executes ordinary `main`; it does not acquire command stdin.
Wasm/npm requests are `SPX-W120` before candidate creation. The compiler-native
entry is `emit_hir_c_with_stdin_stream_text`.

A private helper can consume an owned input and return the independent slice:

```semaprax
@id("app.prefix")
fn prefix(text: string) -> string
{
    string_slice(text, 0, 3)
}
```

As on the single-file text route, `prefix("é\u{0}")` returns three exact UTF-8
bytes including NUL. Invalid offsets remain checked failures; they do not
publish partial output. General owned String mutation and allocation-bearing
while conditions remain outside this addition.

## Focused local gates

`project::tests::stdin_stream_command::text` covers frozen/table selection,
exact capability/input refusals, owned String helpers across modules in both
pure entry and command closures, native Unicode/NUL output, earlier-profile
`SPX-G174`, and Wasm/npm refusal without artifacts.
`language::stream_text_command` covers canonical graph round trip, deterministic
native output, old native Text Toolkit `SPX-B103`, String helper transfers and
text failure inside a stream loop and local map operations at native O0/O2, with allocation and provider
settlement balance and failure without transcript publication.
`codegen::native_emit::output_profile::tests` locks the old runtime selections;
`hir::workspace_link::stdin_stream::tests` locks owned-only String boundaries
and refuses forged borrowed or escaping-view signatures. All six selected unit
checks and both owning native integration checks passed locally. The native
helper table anchors optional stream operations so Open/EOF-only and
Open/Next/EOF-without-Chunk programs compile under `-Werror`. This is local
evidence, not hosted or production promotion.

Focused local test-call evidence covers cross-module owned-String helpers in
entry and test modules, named cases, cancellable execution and prepared traced
tests. Legacy interpreter APIs still refuse the same helper closure. Exact Own
modes and pure effects remain required, and a selected Text failure survives
owned-argument cleanup (`interpreter::resolved_case::tests::stream_text_owned_`).
