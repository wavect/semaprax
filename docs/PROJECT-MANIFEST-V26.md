# Project Manifest v26: linked native source commands

Status: bounded native-only profile authored for OPT #667. Owning regression
`cargo test --locked -p semaprax --test project source_command` is pending
current-compiler verification. No interpreter, Wasm, Web, npm, Windows or hosted
qualification is claimed by this addition.

`source-command.v1` lets an ordinary native CLI import bundled source packages,
including `std.int.decimal`, without copying package implementations into its
application. It retains the exact SourceCommand invocation contract in
[Text Toolkit v1](TEXT-TOOLKIT-V1.md#command-line-programs).

## Manifest and entry

The canonical table layout `semaprax.manifest.v1` selects semantic contract
`semaprax.project.v26`. There is no positional frozen-v26 layout.
[The executable example](../examples/source-command-project/semaprax.toml)
imports decimal normalization, addition and division through the ordinary
bundled dependency registry. Its selected declarations keep their source IDs,
contracts, ownership facts and cleanup plans.

```toml
[package]
name = "decimal-command"
version = "0.1.0"
profile = "source-command.v1"

[command]
function = "decimal.command.main"
input = "argv-utf8+file-text.v1"

[capabilities]
required = ["fs.read", "process.args.read", "process.stderr.write", "process.stdout.write"]

[targets]
matrix = ["native64"]
```

The normal modules, test module, empty `[exports] web = []`, and optional
ordinary `[dependencies]` tables remain required or admitted by
[Package Manifest v1](PACKAGE-MANIFEST-V1.md). Source paths and module names,
strict ordering, capacity bounds and canonical bytes retain that specification.
The selected command must be the entry module's explicit-ID `fn main() -> i64`.
Another retained function as command is `SPX-J130`; an absent, duplicate,
implicit-ID or wrongly typed main retains the owned representation linker's
`SPX-G172` diagnostic. Public web exports are forbidden.

The capability list is a strictly byte-sorted, unique, nonempty subset of
`fs.read`, `process.args.read`, `process.stderr.write`, and
`process.stdout.write`, excluding exactly stdout alone. This is the existing
SourceCommand selector. Empty lists, stdin, filesystem writes, environment,
process execution, network and unknown effects fail closed. Every retained
module must stay inside this four-effect inventory; reachable functions' effect
union must equal the exact manifest list (`SPX-J131`). Every operation is checked
again by SourceCommand authority validation and native feature admission before
publication. Inert dependency declarations do not grant extra authority.

## Invocation and checked failure

Build with `semaprax build examples/source-command-project --target native
--output /fresh/path/decimal-command`, then run the binary from the example
directory with `digits`. It prints `333333333333333333333333` and returns zero.

The runtime is the unchanged SourceCommand runtime: argv excludes argv0; at most
16 UTF-8 arguments and 65,536 total argument bytes are admitted. File text reads
require `fs.read` and canonical relative paths below the invocation directory;
absolute paths, `.`/`..` components, symlinks, nonregular files, and invalid text
are refused. The compiler does not read runtime files while checking or building
the Project. No stdin or filesystem write operation is admitted.

Source `main` returns a checked status in `0..=255`. Staged stderr and stdout are
published, in that order, only after successful postconditions and non-result
cleanup; at most one write per channel and path is admitted. Failure is sticky,
discards staged transcripts, emits the existing status line to stderr, and exits
with 1. The decimal library uses checked contracts for malformed digits,
unsigned subtraction underflow and divisor zero.

## Projections, targets and publication

The canonical manifest, Project graph, subject digest and lock retain the exact
v26/profile/command/input/capability/native64 facts. The lock labels this route
`source-command.v1` with no public-interface digest. It grants no reusable
invocation authority. Public Wasm or owned-data descriptors are not constructed.

`native64` is the only target matrix. Other matrices fail manifest admission;
CLI target selection retains `SPX-J122`. Direct Web/npm/test-Wasm routes refuse
with `SPX-W120`, and Project interpreter entry/test/cancellable execution refuses
with `SPX-F102`: those routes have no v26 invocation-owned argv/file provider.
The old four-effect single-file selector and `owned-data-api.v1` remain unchanged.

Source authentication, full workspace validation, immutable Project generations,
held-input rechecks and fresh native destination publication are unchanged.
A stale or invalid source transaction cannot publish a binary or replace source;
a failed native build removes its private output. Managed transactions still
publish one immutable generation through `ACTIVE`, with no raw-source rewrite.

The owning fixture verifies imported arbitrary-length arithmetic, canonical
manifest/graph facts, invalid entry and authority, native-only refusals, runtime
usage/file/contract statuses, and held-source drift. The existing single-file
SourceCommand and decimal conformance suites remain parity controls for the
shared runtime and arithmetic; the full repository gate remains separately
required for completion claims.
