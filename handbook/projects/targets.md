# Targets: interpreter, native, web

You can run a program in the interpreter, build it to a native executable, or
build it to a WebAssembly package. After this page you can pick one and fix
the usual failures.

One checked meaning feeds every target. A safe program behaves the same on each
backend that admits the features it uses.

## Run it

```sh
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx          # interpreter, prints 42
semaprax run examples/meaning.spx --native # generated C11, needs Clang
semaprax run .                             # a project: runs its entry
```

The interpreter is bounded: `--max-steps N` limits work, `--max-bytes N` limits
the output envelope, and call depth stops at 256 frames. `--json` gives a
machine-readable result. A single file needs a `fn main() -> i64`
(`SPX-T105` otherwise). If the interpreter refuses a program (`SPX-F102`), try
`--native`.

## Build it

```sh
semaprax build examples/meaning.spx --target native -o meaning-native
semaprax build . --target web -o dist/web
```

| Input | `--target` | You get |
| --- | --- | --- |
| File | `native` (default) | Executable file. Needs Clang. |
| File | `native-callable` | Bundle for a function with a direct `own` resource parameter (`SPX-B105` otherwise). Add `--function <id>`. |
| File | `web`, `wasm` | Package directory with `app.wasm`. `--export <id>` picks exported functions. |
| Project | `web` (default), `wasm` | Package directory: `app.wasm`, `index.html`, `package.json`, `semaprax.js`, bindings and a boundary description. `wasm` is an alias for `web`. |
| Project | `native` | Native executable of the project. Needs Clang. |
| Project | `npm` | Owned-data npm package. Needs a profile that admits it (`SPX-W120` for the scalar profile). |
| Project | `oci` | Offline OCI Image Layout (`oci-layout`, `index.json`, `blobs/`). Scalar and Useful Data profiles only. Not signed, not pushed anywhere. |
| Project | `rust` | Generated Rust SDK. Only in the full toolchain built from source. Read `semaprax help build` first. |

Rules that save time:

- `-o` and `--output` are the same. The path must be new: an existing one fails
  with `SPX-I307`, a bad parent with `SPX-I301`.
- `--json` reports `status`, `target`, `product` and `output`.
- `[targets] matrix` in the manifest can forbid a target (`SPX-J122`). `web`,
  `wasm` and `npm` need `wasm32`; the rest need `native64`.
- Run `semaprax help build` for the exact list on your binary.

## Check a web package

```sh
semaprax test examples/calculator-project
semaprax build examples/calculator-project --target web -o dist/calculator-web
node scripts/verify-wasm-scalar-exports.mjs dist/calculator-web   # source checkout
```

`semaprax.scalar-exports.json` lists the exported functions. A build only
writes files. Serving them and loading the module is your app's job; follow the
[calculator browser consumer](https://github.com/wavect/semaprax/blob/main/examples/calculator-web/README.md).

## Edit and re-run (hot reload)

```sh
semaprax dev semaprax.toml --human
```

`dev` keeps one checked interpreter session open. It starts only after you send
a `start` frame on stdin, then reads one JSON control frame per line
(`semaprax.hot-reload-control.v1`). Operations are `start`, `status`, `plan`,
`activate`, `invoke` and `stop`. Saving a file never runs code; `invoke` does.
A broken revision is rejected and the previous one stays usable.

```sh
printf '%s\n' \
  '{"schema":"semaprax.hot-reload-control.v1","id":1,"op":"start"}' \
  '{"schema":"semaprax.hot-reload-control.v1","id":2,"op":"invoke"}' \
  '{"schema":"semaprax.hot-reload-control.v1","id":3,"op":"stop"}' |
  semaprax dev semaprax.toml --human
```

Use `--jsonl` for tools. The VS Code extension drives this for you
([editor setup](../getting-started/editor.md)). Native and Wasm swapping are
not supported; `--source-agent` is refused by the public binary.
Spec: [Hot Reload Watcher v1](https://github.com/wavect/semaprax/blob/main/docs/HOT-RELOAD-WATCHER-V1.md).

## Check the environment

```sh
semaprax doctor                      # versions and OS
semaprax doctor --profile <id>       # probe tools through an admitted offline profile
semaprax doctor --target native|web|all --json
```

`doctor` never discovers tools on `PATH`. Without `--profile` it reports
`failed profile: an explicit offline profile is required` and lists tools
(`clang`, `node`, `rust`) as not probed. That is expected. If a native build
fails with `SPX-B101 failed to start clang`, install Clang and put it on `PATH`.

## Richer data, commands, resources

These use the execution route of their [profile](profiles.md). A `web` scalar
package and an owned-data `npm` package are different interfaces, even though
both are WebAssembly.

**Next:** [Integrate with another language](integrations.md), or
[prepare the package for review](shipping.md).
References: [Interpreter v1](https://github.com/wavect/semaprax/blob/main/docs/INTERPRETER-V1.md),
[Wasm Scalar Exports v1](https://github.com/wavect/semaprax/blob/main/docs/WASM-SCALAR-EXPORTS-V1.md),
[OCI Deployable Artifact v1](https://github.com/wavect/semaprax/blob/main/docs/OCI-DEPLOYABLE-ARTIFACT-V1.md),
[Native Callable ABI v3](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-CALLABLE-ABI-V3.md).
