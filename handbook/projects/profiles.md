# Choose a project profile

A profile selects a concrete set of rules for a workflow. It tells the compiler
which data may cross a function boundary, how values are owned, and which build
or execution route handles them.

You encounter profiles when a function works in a standalone file but needs
extra setup in a project, or when an API starts returning text or bytes instead
of a number. Choose the interface you need before changing the manifest.

## Start with the interface

A function's **boundary** is its parameters and return value. Internal local
values and a package's exported values need not have the same representation.
For example, a function can work with a record internally and still expose a
single `i64` result to another application.

| You are building | Start with |
| --- | --- |
| A calculator or a small numeric library | The default scalar Project route. |
| An API receiving and returning byte data | The selected owned-data API project and consumer examples. |
| A command that reads arguments or standard input | The command-I/O project profile and its host runner. |
| A function calling Rust code | The Native Rust SDK/binding route. |
| An agent with state and typed effects | The Agent lifecycle and runtime profiles. |
| A project with selected law declarations | Manifest v2 with explicit `law_sources`. |

## Understand the default scalar route

A Copy scalar is a basic value such as `i64`, `bool`, or `u8` that can be copied
without transferring an owner. The ordinary scalar Project route is a useful
starting point for public functions and cross-language exports.

The implementation also has selected private helper paths for function values,
generic collections, generic variants, and native owners. Those paths have
their own signature checks. They do not require you to flatten every local
record into numbers, and they do not make every aggregate a public export.

When `SPX-G174` reports that a function is outside the linker profile, inspect
the named signature. Decide whether it should remain a private helper, expose
a scalar result, or move to an explicit data profile. Read the full diagnostic:
`SPX-G174` is used for more than one workspace/project admission condition.

## Move to an owned-data API

An owned-data boundary makes transfer explicit. A borrowed input remains owned
by its caller. An owned input or output has a defined transfer and cleanup path.
This matters when JavaScript or Rust calls generated code: both sides need to
agree on who keeps the bytes and who releases them.

Use the existing
[frame-payload project](https://github.com/wavect/semaprax/tree/main/examples/frame-payload-project)
with its matching
[web consumer](https://github.com/wavect/semaprax/blob/main/examples/frame-payload-web/README.md)
or [Rust consumer](https://github.com/wavect/semaprax/blob/main/examples/frame-payload-rust/README.md).
Keep the example's manifest schema, profile, export signatures, and consumer
route together while learning.

`profile = "owned-data-api.v1"` also appears in the extensible package manifest's
consumer selection. That setting alone is not a conversion of an arbitrary
project into a different ABI. An **ABI** is the agreement about arguments,
results, ownership, and failure at a compiled interface.

## Give command I/O a host

Reading arguments, standard input, a file, or a network response needs an
implementation of that operation. A host supplies it. A test host can return
fixed data, while a configured native host performs the actual operation.

The `useful-data-command.v1` route and the specialized project manifests spell
out their accepted command entry points and data. Start from their examples;
do not add an effect name and assume the pure `run` route has acquired a file
system or a network connection.

See [Input and output](../language/io.md) for the source operations and
[Targets](targets.md) for choosing the runner.

## A practical selection checklist

Before expanding an interface, answer these questions:

1. What exact input and output types should cross it?
2. Who owns each non-Copy value before and after the call?
3. Which target and host supply any external operations?
4. Which committed example exercises that combination?

Build that example first. Then change one type or operation at a time and
run its checks and consumer. This makes profile errors much easier to isolate.

**Next:** [Integrate with Rust, C, or a browser](integrations.md).
Implementation: [Project HIR linking](https://github.com/wavect/semaprax/blob/main/src/hir/workspace_link.rs).
References: [Project Manifest v18](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V18.md),
[Project Manifest v19](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V19.md),
and [Public Owned Data API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-OWNED-DATA-API-V1.md).
