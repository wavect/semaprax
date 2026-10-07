# Bounded Stdin Command Exit Status v1

Status: proposed additive native Project profile. This proposal does not change
the existing Bool-returning streaming profile or claim an implemented route.

## Purpose

The existing `language-command-io.stream.v1` profile selects a command with
type `fn() -> bool`. Its native adapter maps `true` to process status 0 and
`false` to process status 1. Status 2 means that command execution or the
adapter failed. A command therefore cannot return an application-defined
status 2 for an ordinary rejected request while publishing its own diagnostic.

This profile gives a streaming command an explicit process result while
preserving the existing stdin provider, reader ownership, output staging, and
capability rules. It is intended for applications that need to distinguish
ordinary invalid input from an execution or host failure.

## Selection

The additive profile name is `language-command-io.stream.v2`, selected by
Project v24 (`semaprax.project.v24`). Its input remains
`argv-utf8+stdin-stream.v1`: the bytes, EOF, and checked read-failure protocol
are unchanged. Its selected command is an explicitly identified `fn() -> i64`.

The command result is an application process status in the portable range
0 through 255, inclusive. The process adapter returns that exact status after a
successful semantic invocation. Status 0 conventionally means success; every
nonzero status is an application-defined outcome. In particular, an
application may use status 2 for invalid input and write one diagnostic to
stderr while leaving stdout empty.

The adapter validates the returned integer before publishing the staged
output transcript. A result below 0 or above 255 is an adapter failure: staged
output is discarded and the adapter emits its one generic failure diagnostic
with process status 2. A checked stdin failure remains
`semaprax.command-input.v1`, code 3, and follows the existing runtime-failure
path. Neither failure is represented as a normal application result.

## Compatibility and authority

Project v23 and `language-command-io.stream.v1` retain their exact Bool result
and status-0/status-1 behavior. They do not silently accept `i64` commands.
Project v24 admits the same exact command capability inventory and the same
reachable streaming operation closure as v1; it adds no source operation or
host capability. `stdin_stream_open`, `stdin_stream_next`, chunk loans, true
EOF, the reusable 4096-byte provider buffer, and the normalized read failure
remain those of [Bounded Standard Input Streaming v1](BOUNDED-STDIN-STREAM-V1.md).

This proposal selects the native executable route only. Project v24 must refuse
Wasm and npm requests until those backends define and pass their own process
result contract. It must not fall back to Project v23 or snapshot input.

## Focused evidence required

Before this profile is implemented, its owning gates must establish:

- canonical Project v24 parsing, formatting, and table-layout lowering;
- exact admission of a stable-ID `fn() -> i64` command and the unchanged v1
  streaming operation closure;
- native execution preserving application statuses 0, 1, 2, and 255, including
  a status-2 diagnostic with empty stdout;
- rejection of negative and greater-than-255 results before transcript
  publication, with the generic adapter-failure status and diagnostic;
- unchanged Project v23 bytes, Bool mapping, earlier profile admission, and
  existing streaming-reader cleanup and read-failure behavior; and
- explicit Wasm/npm refusal before candidate artifacts are created.

The implementation must extend the Project profile, retained-link validation,
and native result handling together. A test wrapper that translates Bool
status 1 to 2 does not establish this profile.
