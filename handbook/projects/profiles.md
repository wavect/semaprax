# Choose a project profile

A profile fixes which values may cross a function boundary, who owns them, and
which targets can build the project. After this page you can pick the profile
for your interface and fix `SPX-G174`.

## Pick by interface

A function's **boundary** is its parameters and result. Locals inside a function
can use records and variants even when the boundary is a single `i64`.

| You are building | `[package] profile` | Example project |
| --- | --- | --- |
| Calculator or numeric library | omit it (scalar) | `examples/calculator-project` |
| Function taking borrowed text | `useful-text-consumer.v1` | `examples/config-validator-project` |
| Byte data, fixed arrays, borrowed slices | `useful-data.v1` | `examples/binary-frame-project`, `examples/task-service-project` |
| Owned bytes in and out | `owned-data-api.v1` | `examples/frame-payload-project` |
| Owned UTF-8 text | `owned-utf8-api.v1` | see the spec below |
| One owned record result | `flat-owned-record-api.v1` | see the spec below |
| Nested owned records, agents, routing | `nested-owned-record-api.v1` | `examples/support-routing-project`, `examples/job-service-project` |
| Command: stdin bytes plus one UTF-8 argument | `useful-data-command.v1` / `.v2` | `examples/spxgrep-project`, `examples/spxgrep-native-command-project` |
| Command: argv and stdin | `language-command-io.v1`, `line-command-io.v1` | `examples/spxgrep-language-command-project`, `examples/spxgrep-lines-project` |
| Command with HTTP or HTTPS | `network-command-io.v1`, `https-command-io.v1` | `examples/network-http-project`, `examples/https-project` |
| Local futures | `source-local-future.v1` (and `-indexed-rust.v1`) | `examples/ri13-m3-local-http` |

These profiles are private. They exist for the bundled `std` packages, have no
public ABI and no `web` exports, and may change. Do not rely on them:

| Profile | Gives a command | Package | Spec |
| --- | --- | --- | --- |
| `filesystem-io.v3` | `fs.read` and `fs.write` | `std.fs` (`examples/everyday-agent-project`) | [Filesystem I/O v2](https://github.com/wavect/semaprax/blob/main/docs/FILESYSTEM-IO-V2.md) |
| `environment-io.v1` | a read-only snapshot of the environment the host passes in (`process.environment.read`); never the real process environment | `std.env` | [Environment I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-ENVIRONMENT-IO-V1.md) |
| `process-io.v1` | `process.execute`: run one registry tool by number, with argv and stdin, and get its output back; no shell, no `PATH` lookup | `std.process` | [Process I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-PROCESS-IO-V1.md), [Project v18](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V18.md) |
| `useful-data.v2` | owned Reader and Writer values inside the project; exports stay on the `useful-data.v1` boundary | `std.data.json.write`, `std.email`, `std.format`, `std.export.policy` | [Project v16](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V16.md) |

`https-command-io.v1` is not in that list. It adds `network.http` for
`https_get` and `https_post`, and `examples/https-project` builds on it
([Input and output](../language/io.md#https-in-one-call)).

Every row is one bounded contract. A profile name is not a switch you flip on
an existing project: change one type, then run `check`, `test` and your
consumer.

## Why did `SPX-G174` fire?

A function crosses the boundary with a type the profile does not admit.
Read the named signature, then pick one:

1. Keep it private: remove it from `[exports]`.
2. Return a scalar: add a small wrapper.
3. Move the project to a profile that admits the type.

`SPX-G174` has more than one cause, so read the whole message.

## Scalar boundary (the default)

Parameters and results are Copy scalars (`i64`, `bool`, `u8`, `f64`, ...). The
scalar profile builds to `web`, `native` and `oci` (and `rust` with the full toolchain).

## Owned data

An owned boundary says who keeps the bytes and who frees them. A borrowed
input stays owned by the caller. An owned input or output has a defined
transfer and cleanup. Pick it when JavaScript or Rust calls your code with
buffers. The `npm` target needs a profile that admits it (the scalar profile
fails with `SPX-W120`). Start from the
[frame-payload project](https://github.com/wavect/semaprax/tree/main/examples/frame-payload-project)
and its [web](https://github.com/wavect/semaprax/blob/main/examples/frame-payload-web/README.md)
or [Rust](https://github.com/wavect/semaprax/blob/main/examples/frame-payload-rust/README.md)
consumer.

## Command I/O

A command profile has a `[command]` entry point and a fixed
`[capabilities] required` list, so the project declares exactly the authority it
uses. The `input` value is fixed per profile:
`stdin-bytes+one-utf8-arg.v1` for `useful-data-command.v2`,
`argv-utf8+stdin-bytes.v1` for the four `-io.v1` profiles.

- `semaprax run <project>` runs the ordinary project entry, not the command.
- `semaprax network-run <project> --fixture f.json [--arg UTF8]... [--stdin path]`
  runs a `network-command-io.v1` command against a recorded fixture
  (`semaprax.network-fixture.v1`, at most 1 MiB and 8 connections). No real
  socket opens.
- Build the command with `build --target native`; see [Targets](targets.md).

See [Input and output](../language/io.md) for the source operations.

## Check before you change

1. Which input and output types cross the boundary?
2. Who owns each non-Copy value before and after the call?
3. Which target and host supply external operations?
4. Which committed example covers that combination?

Build that example first, then change one thing at a time.

**Next:** [Integrate with Rust, C or a browser](integrations.md).
References: [Package Manifest v1 (profile table)](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V1.md),
[Public Owned Data API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-OWNED-DATA-API-V1.md),
[Public Flat Owned Record API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-FLAT-OWNED-RECORD-API-V1.md),
[Public Owned UTF-8 API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-OWNED-UTF8-API-V1.md),
[Bounded Language Network I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-NETWORK-IO-V1.md).
