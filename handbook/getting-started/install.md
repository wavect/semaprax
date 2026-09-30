# Install

Get a working `semaprax` in about five minutes. You need **Git** and
**Rust/Cargo 1.88+**. Add **Clang** for native builds and **Node.js 22+** for
web-package verification.

## Install from source (recommended)

```sh
git clone https://github.com/wavect/semaprax.git
cd semaprax
cargo install --locked --path .
```

`--locked` pins the recorded dependencies. Cargo may download them during
install; later Semaprax builds fetch nothing.

### Fix `command not found`

`cargo install` writes to `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on
Windows). If your shell can't find `semaprax`, add that directory to `PATH`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # bash/zsh, then open a new shell
command -v semaprax                    # should print the binary path
```

## Verify the install

```sh
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx     # prints 42
```

The first command verifies a program; the second runs it. No Clang, Node, or
model provider needed for this — just the checker and interpreter.

## Which binary do I have?

| Binary | Source | Commands |
| --- | --- | --- |
| `semaprax` | Source install, or release archive | Everyday work: `new`, `fmt`, `check`, `run`, `test`, `graph`, `build`, `doctor` |
| `semaprax-full` | `cargo install --locked --path crates/semaprax-toolchain` | Same, plus private host and publication operations |

If `semaprax help all` doesn't show a command, you almost certainly have the
standalone binary and the command is private — that mismatch, not a broken
install, is the usual cause.

## Alternative: release archive

This handbook describes **v0.6.0**, which is currently available **from source
only** — its release gate is not green and no signed v0.6.0 archive is
published. The last downloadable archives are the
[v0.5.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.5.0)
(one per host: Linux x86-64, Apple Silicon macOS, Windows x86-64, plus
`SHA256SUMS`):

```sh
shasum -a 256 -c SHA256SUMS
```

Each archive contains `semaprax`, a smoke program, and a release manifest.
Prefer the source install above when following this handbook: v0.5.0 predates
several v0.6.0 commands and templates. Check the
[releases page](https://github.com/wavect/semaprax/releases) for the newest
version and the completion matrix in
[`docs/`](https://github.com/wavect/semaprax/tree/main/docs) for what each
release actually implements — a specification existing is not proof it ships.

## Next step

Write and run your first file: [First program](first-program.md).
