# Use Semaprax from another language

You can call Semaprax from a browser, Rust, C or C++, and you can let Semaprax
call Rust. After this page you can choose a route and run its first command.
Start with one function (a price calculation, a parser, a validator), not a
whole application.

## Choose a route

| Your goal | Route | Start here |
| --- | --- | --- |
| Call scalar functions from a browser | `build --target web` | [calculator-web](https://github.com/wavect/semaprax/blob/main/examples/calculator-web/README.md) |
| Call Semaprax from Rust | Generated safe-Rust SDK | [calculator-rust](https://github.com/wavect/semaprax/blob/main/examples/calculator-rust/README.md) |
| Exchange owned bytes with Rust or JavaScript | Owned-data SDK | [owned-data-rust](https://github.com/wavect/semaprax/tree/main/examples/owned-data-rust), [frame-payload-web](https://github.com/wavect/semaprax/blob/main/examples/frame-payload-web/README.md) |
| Call a Rust host operation from Semaprax | Checked native Rust import | [Native Rust Interop v1](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-RUST-INTEROP-V1.md) |
| Embed checking in a Rust tool | Embedding API | [embedding-api](https://github.com/wavect/semaprax/blob/main/examples/embedding-api/README.md) |
| Call from C or C++ | `c-header`, `cxx-shim`, `cxx-package` | below |
| Describe functions as HTTP | `openapi` | below |

## Inspect a C boundary

Pick functions by name or stable ID. Each command is read-only and prints
deterministic output.

```sh
semaprax c-header examples/meaning.spx --function math.add --emit-header
semaprax abi-report examples/meaning.spx --function math.add
semaprax cxx-shim examples/meaning.spx --function math.add --emit-fragment
semaprax cxx-package examples/meaning.spx --function math.add --max-bytes 1000000
semaprax freestanding-object examples/meaning.spx
```

| Command | Prints |
| --- | --- |
| `c-header` | A C signature report; `--emit-header` prints the header text. |
| `abi-report` | Argument, result, failure and ownership facts per function. |
| `cxx-shim` | A C++17 header fragment of `extern "C"` declarations for scalar functions (`--emit-fragment` prints it). No wrappers. |
| `cxx-package` | The header and shim as one package. `SPX-X103` means raise `--max-bytes`. |
| `freestanding-object` | One freestanding C11 translation unit for a whole effect-free scalar module, with profile assertions. |

A header describes an interface; the link step supplies the implementation.
Read the ownership facts before you write a consumer.

## Describe functions as OpenAPI

```sh
semaprax openapi examples/meaning.spx --function math.add
semaprax openapi-compat base.json candidate.json     # breaking change check
```

`openapi` prints an OpenAPI document for the named functions, including the
shared failure-status schema. `openapi-compat` compares two documents and
reports whether the candidate breaks the base. Spec: [OpenAPI v1](https://github.com/wavect/semaprax/blob/main/docs/OPENAPI-V1.md).

## Call Semaprax from Rust

The calculator example has two packages. The **setup package** reads Semaprax
source and builds the SDK. The **consumer package** depends on the generated
crate as an ordinary Cargo dependency and does not compile the compiler.

Follow the [calculator README](https://github.com/wavect/semaprax/blob/main/examples/calculator-rust/README.md)
for tool paths. It names Clang, the archiver (on macOS `/usr/bin/libtool`),
the manifest and output paths. Build the SDK first, then the matching
consumer. Keep generated output out of Git.

`semaprax build <project> --target rust` is a different route. It exists only
in the full toolchain built from source. Run `semaprax help build` and read the
matching example first.

In CI, prepare the SDK in its own step so failures separate cleanly: the
[build-script consumer](https://github.com/wavect/semaprax/tree/main/examples/calculator-rust/build-script-consumer)
uses a prepared SDK and never starts the compiler from `build.rs`.

## Call Rust from Semaprax

A Rust import declares one operation Semaprax may call. The generated adapter
connects it to your Rust code. Keep input types, result type, effects and
failure behavior explicit.

- **Indexed route.** A prepared Rust API index lists items and signatures.
  Binding checks package, version, source digest, target, features, path and
  signature. `semaprax context <file.spx> <rust-path> --rust-index <index.json>`
  answers questions about one imported item; add `--candidates` with a path
  prefix to list matches.
- **Selected bindings.** Owned Regex and Url bindings and generated callback
  adapters exist as starting points.
- **Receiver-tied views.** A returned view borrows from its owner. Keep the owner
  alive while you use the view.

Sources: [checked Rust bindings](https://github.com/wavect/semaprax/blob/main/src/native_rust_binding.rs),
[native Rust builder](https://github.com/wavect/semaprax/tree/main/crates/semaprax-native-rust-interop-builder).

## Test both sides

Call the generated interface with an ordinary value, a boundary value and an
input that hits the documented error path. For owned data, also test an empty
value and transfer and cleanup. Keep the consumer in the test: a source-only
check cannot catch a consumer wired to the wrong package.

**Next:** [Review and ship the selected package](shipping.md).
References: [C Header v1](https://github.com/wavect/semaprax/blob/main/docs/C-HEADER-V1.md),
[C++ Shim v1](https://github.com/wavect/semaprax/blob/main/docs/CXX-SHIM-V1.md),
[Public Owned Data API v1](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-OWNED-DATA-API-V1.md),
[Native Callable ABI v3](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-CALLABLE-ABI-V3.md).
