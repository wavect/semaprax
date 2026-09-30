# Targets: interpreter, native, web

One checked meaning, three engines. They share HIR and the cleanup plan, so a
safe program behaves equivalently on every backend that admits the features
it uses.

## The three engines

| Engine | Command | Needs | Good for |
| --- | --- | --- | --- |
| Interpreter | `semaprax run <target>` | Nothing | Edit loop, tests, learning |
| Native (C11) | `semaprax run --native` / `build --target native` | Clang | CLIs, command I/O, resources, speed |
| Web (Wasm) | `semaprax build --target web -o dist/` | Node 22 for verification | Browser/npm packages, scalar exports |

```sh
semaprax run semaprax.toml                              # interpreter
semaprax run examples/meaning.spx --native              # one file, native lane
semaprax build semaprax.toml --target web -o dist/web   # web package
semaprax build semaprax.toml --target native -o dist/native
```

The interpreter is bounded: `--max-steps` and `--max-bytes` cap execution,
recursion is limited to 256 frames (exceeding it is a reported
runtime-capacity failure, not a crash), and single-file `run` evaluates
`app.main` with exactly the `process.stdout.write` transcript profile.

## Choosing per feature

- **Command I/O** (`args`, `stdin`, `stderr`) and **resources** need a
  project built for **native**. Single-file `run` rejects resource modules
  (`SPX-B104`).
- **Web exports** are admitted scalar functions listed in `[exports] web`.
  The build emits `app.wasm` plus `semaprax.scalar-exports.json` describing
  the JavaScript/TypeScript boundary.
- **Effects** must be provided by the target's host: a declared effect with
  no injected provider fails at the boundary, never silently.

## The build loop

```sh
semaprax check semaprax.toml                 # verify first, always
semaprax test semaprax.toml                  # executable checks
semaprax build semaprax.toml --target web -o dist/web
semaprax doctor --target web                 # environment + target readiness
```

`build` creates the output directory if needed. `doctor` checks the
toolchain and target prerequisites (`--profile`, `--target native|web|all`,
`--json`) — run it when a build fails for environmental reasons before
debugging the code.

Exact rules: [Wasm Scalar Exports v1](https://github.com/wavect/semaprax/blob/main/docs/WASM-SCALAR-EXPORTS-V1.md),
[Native Callable ABI v3](https://github.com/wavect/semaprax/blob/main/docs/NATIVE-CALLABLE-ABI-V3.md),
[Interpreter v1](https://github.com/wavect/semaprax/blob/main/docs/INTERPRETER-V1.md).
