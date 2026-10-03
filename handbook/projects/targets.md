# Targets: interpreter, native, web

One checked meaning, three engines. They share HIR and the cleanup plan, so a
safe program behaves equivalently on every backend that admits the features
it uses.

A **target** is the form you build or run: interpreted execution, a native
program, or a WebAssembly package. Start with the interpreter, then choose
the form your application needs.

## Choose the output you need

| Route | What it gives you | Extra tools |
| --- | --- | --- |
| `run` | A result from the interpreter. | None for the introductory pure examples. |
| `run --native` | A native execution of a supported source program. | Clang. |
| Single-file `build --target native` | A native executable at the output path. | Clang. |
| Project `build --target native` | The selected project's native output package. | The tools required by that project profile. |
| `build --target web` | A WebAssembly package and its consumer interface. | Node.js for the repository's package verification scripts. |

**Single-file native output is a file; project output is a package directory.**
Do not pass a directory where an executable filename is expected.

## Start with interpreter execution

From the repository root:

```sh
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

The result is `42`. The single-file runner selects `app.main`. It also has a
specific stdout-transcript route, used by the printing lesson in
[First program](../getting-started/first-program.md).

For single-file execution, `--max-steps` limits the work performed by the
interpreter. `--max-bytes` limits the output envelope, not all process memory.
The implementation also limits call depth to 256 frames. Read
`semaprax help run` before combining execution options.

## Try native execution

The following commands use the same source file:

```sh
semaprax run examples/meaning.spx --native
semaprax build examples/meaning.spx --target native -o meaning-native
```

On macOS or Linux, run the created executable with `./meaning-native`.
On Windows, choose an output name such as `meaning-native.exe` and run it
with `./meaning-native.exe` in PowerShell.

The native route generates C11 and compiles it with Clang. Keep compiler and
runtime failures separate: a missing Clang executable is an environment issue,
while an unsupported signature is a source/profile issue.

## Build a web package and check its boundary

From the repository root:

```sh
semaprax test examples/calculator-project/semaprax.toml
semaprax build examples/calculator-project/semaprax.toml --target web -o dist/calculator-web
node scripts/verify-wasm-scalar-exports.mjs dist/calculator-web
```

The manifest selects the exported functions by stable ID. The scalar route
emits `app.wasm` and `semaprax.scalar-exports.json`; the latter describes the
consumer boundary. The Node script checks the generated calculator exports.

To call the package from a page, follow the complete
[calculator browser consumer](https://github.com/wavect/semaprax/blob/main/examples/calculator-web/README.md).
A build creates files. Serving those files and loading the package are separate
steps in the consumer application.

## Choose the route for richer data

Command I/O, resources, owned-data APIs, and host callbacks each use their
specified execution profile. See [Profiles](profiles.md) before selecting a
backend for them. A `web` scalar package and an owned-data `npm` package are
different interfaces, even though both involve WebAssembly.

For a Project Rust SDK, the documented `--target rust` route uses
`semaprax-full`. The [integration guide](integrations.md) also shows the
standalone Cargo setup examples and the C-header inspection route.

## Diagnose the environment before changing source

```sh
semaprax doctor --target web
semaprax doctor --target native
```

Read the reported checks and their required/optional status. Use the installed
command's help for additional doctor profiles. A successful prerequisite check
helps establish the environment; still run the project's tests and the chosen
consumer after building.

**Next:** [Integrate with another language](integrations.md), or
[prepare the package for review](shipping.md).
References: [Interpreter v1](https://github.com/wavect/semaprax/blob/main/docs/INTERPRETER-V1.md),
[Wasm Scalar Exports v1](https://github.com/wavect/semaprax/blob/main/docs/WASM-SCALAR-EXPORTS-V1.md),
and [Native Callable ABI v3](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-CALLABLE-ABI-V3.md).
