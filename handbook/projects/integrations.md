# Use Semaprax from another language

Start by moving one well-defined function across the boundary. A price
calculation, parser, or validation routine is easier to integrate and test
than an entire application at once.

Choose the direction first. Calling Semaprax from Rust and calling Rust from
Semaprax use related tooling, but they are different jobs.

## Choose a route

| Your goal | Route and starting example |
| --- | --- |
| Call scalar Semaprax functions from a browser | A generated web package; [calculator-web](https://github.com/wavect/semaprax/blob/main/examples/calculator-web/README.md). |
| Call Semaprax from Rust | A generated safe-Rust SDK; [calculator-rust](https://github.com/wavect/semaprax/blob/main/examples/calculator-rust/README.md). |
| Exchange owned bytes with Rust | The owned-data SDK; [owned-data-rust](https://github.com/wavect/semaprax/tree/main/examples/owned-data-rust). |
| Let Semaprax call a Rust host operation | A checked native Rust import and generated host callback interface. |
| Embed checking and semantic inspection in a Rust tool | The [embedding API example](https://github.com/wavect/semaprax/blob/main/examples/embedding-api/README.md). |
| Inspect a C-facing scalar interface | The `c-header` and `abi-report` commands. |

## Call Semaprax from Rust

The calculator example separates preparation from consumption:

```text
checked Semaprax source
        ↓ SDK builder and explicit native tools
generated safe-Rust SDK package
        ↓ ordinary Cargo dependency
Rust application
```

The **setup package** reads the source and builds the SDK. The **consumer
package** depends on that generated package. It does not need to compile the
Semaprax compiler as part of the application.

Follow the [calculator setup instructions](https://github.com/wavect/semaprax/blob/main/examples/calculator-rust/README.md)
for your host. They identify the compiler, Clang, archiver, manifest, and output
paths. Tool selection matters: the documented Darwin archive plan uses
`/usr/bin/libtool`, rather than substituting an arbitrary archiver.

The example includes direct function exports, a host callback, and a Project
consumer using the manifest-selected exports. Build the SDK first, then run the
matching consumer. Keep generated output out of source control when the example
expects it to be prepared locally.

The separate Project `build --target rust` command uses `semaprax-full`.
Read `semaprax-full help build` and the matching example before selecting that
route; it is not the same invocation as the standalone Cargo setup package.

## Call Rust from Semaprax

A Rust import declares the operation Semaprax may call. The generated adapter
connects that checked declaration to the Rust implementation. Keep the input
types, result type, effects, and failure behavior explicit.

The indexed route adds a prepared Rust API index. An **index** lists discoverable
items and their signatures. Prepared-index replay is included in the compiler
at `src/rust_api_index`; a separate extraction step prepares the metadata.
The binding code checks the selected package,
version, source digest, target, features, Rust path, and signature against the
checked import. The later Rust build checks the generated call against the
actual crate.

The current source also contains selected owned Regex and Url binding paths,
receiver-tied Url views, and generated callback adapters. These are useful
starting points when extending an integration with richer Rust data.

For a **receiver-tied view**, the returned view borrows from a particular owner.
Keep that owner alive while using the view. Moving, replacing, or releasing it
is an ownership operation, not an implementation detail to hide in a wrapper.

Implementation starting points:
[checked Rust bindings](https://github.com/wavect/semaprax/blob/main/src/native_rust_binding.rs)
and the [native Rust builder](https://github.com/wavect/semaprax/tree/main/crates/semaprax-native-rust-interop-builder).
Use the selected adapter's tests when changing its lifetime or cleanup behavior.

## Keep ordinary Cargo builds predictable

The calculator's
[prepared build-script consumer](https://github.com/wavect/semaprax/tree/main/examples/calculator-rust/build-script-consumer)
uses a prepared SDK in `OUT_DIR`. Its build script does not start the Semaprax
compiler or nested Cargo. Prepare the artifact explicitly, then let the
consumer build use it.

This is helpful in CI: preparation failures, source changes, and consumer
compilation are visible as separate steps.

## Inspect a C boundary before linking it

From the repository root:

```sh
semaprax c-header examples/meaning.spx --function math.add --emit-header
semaprax abi-report examples/meaning.spx --function math.add
```

Read the generated signature and failure/ownership facts before writing a
consumer. A header describes an interface; the build and link steps supply
its implementation. Resource-bearing native-callable bundles have their own
ABI and cleanup contract.

## Test both sides

Call the generated interface with an ordinary value, a boundary value, and an
input that exercises its documented error path. For owned data, also test an
empty value and the transfer/cleanup behavior. Keep the consumer in the test:
a source-only check cannot catch a consumer using the wrong generated package.

**Next:** [Review and ship the selected package](shipping.md).
References: [Native Rust Interop v1](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-RUST-INTEROP-V1.md),
[Public Owned Data API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-OWNED-DATA-API-V1.md),
and [Native Callable ABI v3](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-CALLABLE-ABI-V3.md).
