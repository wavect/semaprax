# SEMAPRAX {{TAG}}

This is the SEMAPRAX {{VERSION}} command-line toolchain for `{{TARGET}}`.
SEMAPRAX is beta software: see the [release page](https://github.com/wavect/semaprax/releases/tag/{{TAG}}) for what is and is not claimed.

## What is in this folder

| File | What it is |
| --- | --- |
| `semaprax` | The full `semaprax` command-line tool |
| `semapraxd` | The SEMAPRAX daemon used by editors and agents |
| `LICENSE` | License terms ([online copy](https://github.com/wavect/semaprax/blob/{{TAG}}/LICENSE)) |
| `release-manifest.json` | Version, commit, target and runtime requirements of this build |
| `smoke/meaning.spx` | A one-function sample used to smoke-test this archive |
| `README.md` | This file |

## Runtime requirements

- Linux: a GNU/Linux system with glibc 2.35 or newer (Ubuntu 22.04 or newer, Debian 12 or newer). musl-based systems such as Alpine are not supported.
- macOS: macOS 11 or newer on Apple Silicon, macOS 10.12 or newer on Intel.
- Exact requirements of this archive are recorded under `runtime` in `release-manifest.json`.

## Run it from this folder

Open a terminal in this folder and run:

```sh
./semaprax --version
```

If macOS refuses to open the program after a browser download, remove the download quarantine mark once:

```sh
xattr -dr com.apple.quarantine .
```

## Put it on your PATH

For the current terminal only:

```sh
export PATH="$PWD:$PATH"
semaprax --version
```

To keep it, move this folder somewhere permanent and add its path to your shell profile. The installer script does this for you; see the [install guide](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/install.md).

## Your first project

From any empty working directory, with `semaprax` on your PATH:

```sh
semaprax --version
semaprax new first-semaprax
semaprax check first-semaprax/semaprax.toml
semaprax test first-semaprax/semaprax.toml
semaprax run first-semaprax/semaprax.toml
```

The last command prints `42`.

To try the bundled sample without creating a project:

```sh
./semaprax check smoke/meaning.spx
./semaprax run smoke/meaning.spx
```

## Learn more

- [Install guide](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/install.md)
- [Your first project](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/first-project.md)
- [Handbook](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/README.md)
- [Quick reference for writing `.spx` source](https://github.com/wavect/semaprax/blob/{{TAG}}/docs/AGENT-QUICK-REFERENCE.md)
- [Verifying a release](https://github.com/wavect/semaprax/blob/{{TAG}}/docs/RELEASE-PROCESS.md)
- [Source repository](https://github.com/wavect/semaprax/tree/{{TAG}})
