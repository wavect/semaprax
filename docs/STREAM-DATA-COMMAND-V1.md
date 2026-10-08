# Stream Data Command v1

Audience: Project command authors and compiler/backend contributors.

Status: additive native profile with a passing focused local native execution,
source/graph, old-profile, carrier-authentication, public-ABI, and target-refusal
gate. Broader product support remains separate.

## Selection and compatibility

Project v27 is selected only by the table manifest profile
`language-command-io.stream-data.v1`; its semantic schema is
`semaprax.project.v27`. It uses input `argv-utf8+stdin-stream.v1` and the exact
sorted capability inventory `process.args.read`, `process.stderr.write`,
`process.stdin.read`, and `process.stdout.write`.

This profile inherits the Project v25 [Stream Text Command v1](STREAM-TEXT-COMMAND-V1.md)
runtime, quotas, ownership and cleanup rules, stream epochs, command operation
inventory, length-delimited String representation, local Map/Reader helper
rules, and native process adapter. Project v23, v24, and v25 remain frozen.
Web, Wasm, and npm refuse v27 with `SPX-W120` before producing artifacts.
Native-only describes the selected command target and streaming runtime. The
inherited authority-free Project interpreter may still evaluate the ordinary
pure `main` and test closures; it supplies no stdin provider or command adapter.

The ordinary entry and selected command remain separate exact external roots.
The entry is an explicit authored `fn main() -> i64`; the selected command is
an explicit stable-ID `fn () -> i64`. The manifest exports exactly that command
identity. Neither root may expose Vec, String, Map, Reader, float, char, or any
other carrier through its parameters or result.

## Authenticated private helper extension

Every retained non-root semantic function still requires an explicit stable
identity and the v25 signature/effect closure. V27 additionally admits a
parameter with this exact shape:

```text
borrow Vec<T>
```

`T` must be exactly one compiler-owned Copy scalar: `i64`, `i32`, `u8`,
`usize`, `char`, `f32`, `f64`, or `bool`. The borrow is immutable, synchronous,
non-escaping, and governed by Shared Loan Plan v1. Native lowering passes the
existing private vector carrier; it creates no public adapter or descriptor
surface and transfers no ownership.

Helper results retain the v25 set. V27 adds no owned or borrowed Vec result. It
does not admit `own Vec<T>`, Vec of Bytes or authored nominal elements, mutable
borrowing, Map widening, borrowed String, authored generic wrappers, function
values or captures, additional Reader shapes, or new effects. The entry and
command are excluded from this private predicate even when a candidate
signature would otherwise match it.

The same exact borrowed Vec shape is admitted when one retained helper is
called from a bounded `while` or `for` body. Loop calls do not widen the
profile: ownership, element identity, result and effect checks replay at the
source verifier, recursive oracle, HIR resolver and independent HIR validator.
Canonical compiler Vec operations remain the admitted generic operations in a
loop; importing a generic wrapper under another name does not turn it into an
intrinsic.

## Verification and projections

Workspace Graph reachability first authenticates the retained entry, command,
and helper closure. The HIR linker independently rechecks root identity and ABI,
private signature ownership, compiler-owned Vec identity and exact scalar type,
effects, operation profile, byte capacity, loan facts, and cleanup metadata.
The native backend consumes only checked HIR and selects the v25
length-delimited streaming runtime. No verifier work limit, vector capacity,
loan limit, allocation quota, capability, or ambient authority is increased.

Graph, Prelude, LoanPlan, Cleanup Inventory, and CleanupPlan selection follow
the inherited program features. V27 adds no semantic-graph field or proof
carrier; `project_schema = "semaprax.project.v27"` is the Project-level
selection fact.

The owning focused gate is:

```sh
cargo test --locked -p semaprax --lib \
  project::tests::stdin_stream_command::stream_data::private_vec_and_copy_scalar_helpers_keep_command_abi_closed \
  -- --exact
```

It covers a private borrowed Copy-scalar Vec helper called from a bounded loop,
native execution/output, exact command export and capability inventory,
v24/v25 refusal, owned and non-Copy Vec refusal, exact public-root ABI refusal,
and pre-artifact Web/npm refusal.

The source import gate rejects owned or non-Copy Vec parameters with `SPX-G172`.
Authenticated Copy-vector candidates reach the selected profile: v24 refuses
with `SPX-G174`, while the v25 and v27 HIR signature closures use `SPX-H006`.
The selected command root is refused earlier with `SPX-G172` unless its
explicit identity and exact `fn() -> i64` ABI are present.
Read the accompanying message to distinguish signature/ABI refusal from work
limits that share the same diagnostic code. V27 also independently refuses
forged resolved carrier identities and non-Copy elements before native lowering.
