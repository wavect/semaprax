# Install

Install a published Semaprax release, then run a first project. You do not need
to clone the repository, install Rust, or compile anything for this route, and
you do not need a model account, API key, or editor extension.

## Pick your download

The current published release is listed at
[github.com/wavect/semaprax/releases/latest](https://github.com/wavect/semaprax/releases/latest).
That address always moves to the newest release, so use it only to find out
which version is current. The commands below name one exact tag,
[v0.8.0](https://github.com/wavect/semaprax/releases/tag/v0.8.0), so every
download in a session comes from the same release.

| Your computer | Archive to download | Runtime requirement |
| --- | --- | --- |
| macOS on Apple Silicon (M1 or newer) | `semaprax-v0.8.0-aarch64-apple-darwin.tar.gz` | macOS 11.0 or newer (the binary's recorded minimum). |
| Linux on x86-64 | `semaprax-v0.8.0-x86_64-unknown-linux-gnu.tar.gz` | GNU/Linux with glibc 2.39 or newer. The v0.8.0 binary needs `GLIBC_2.39`, so it does not start on older distributions or on musl systems such as Alpine. |
| Windows on x86-64 | `semaprax-v0.8.0-x86_64-pc-windows-msvc.zip` | 64-bit Windows. |

Other hosts (Intel macOS, Linux on ARM, older Linux) have no v0.8.0 archive.
Use [Install from source](#install-from-source) there. Later releases are
planned to target glibc 2.35, but v0.8.0 does not, so check the release notes of
the version you pick.

GitHub also shows **Source code (zip)** and **Source code (tar.gz)** under every
release. Those are automatically generated snapshots of the repository, not
Semaprax programs you can run. Download the archive named in the table.

Each archive unpacks to a directory named after itself. It contains `semaprax`
(the full build of the command-line tool), `semapraxd` (the daemon), `LICENSE`,
`README.md`, a per-archive `release-manifest.json`, and a `smoke/` program. On
Windows the two programs are `semaprax.exe` and `semapraxd.exe`.

## macOS and Linux

The steps below use the Apple Silicon archive. For Linux, replace the target
with `x86_64-unknown-linux-gnu` and `shasum -a 256 -c -` with
`sha256sum -c -`.

1. Download the one archive for your computer and the checksum list. Do not
   download the other platforms' archives.

   ```sh
   TAG=v0.8.0
   TARGET=aarch64-apple-darwin
   BASE=https://github.com/wavect/semaprax/releases/download/$TAG
   curl -fLO "$BASE/semaprax-$TAG-$TARGET.tar.gz"
   curl -fLO "$BASE/SHA256SUMS"
   ```

2. Verify the archive you downloaded. `SHA256SUMS` lists every platform, so
   select only your archive's line; checking the whole file would report the
   archives you did not download as missing.

   ```sh
   grep " semaprax-$TAG-$TARGET.tar.gz$" SHA256SUMS | shasum -a 256 -c -
   ```

   It must print `semaprax-v0.8.0-aarch64-apple-darwin.tar.gz: OK`. If it
   prints `FAILED` or nothing at all, delete the download and start again; do
   not unpack it.

3. Optional: verify who built it. A checksum only shows that the file matches
   the list published beside it. If you have the [GitHub CLI](https://cli.github.com/),
   this also checks the publisher's attestation:

   ```sh
   curl -fLO "$BASE/release-attestation-$TARGET.json"
   gh attestation verify "semaprax-$TAG-$TARGET.tar.gz" \
     --bundle "release-attestation-$TARGET.json" --repo wavect/semaprax
   ```

4. Unpack it somewhere permanent. This example uses `~/.local/opt`.

   ```sh
   mkdir -p "$HOME/.local/opt"
   tar -xzf "semaprax-$TAG-$TARGET.tar.gz" -C "$HOME/.local/opt"
   ls "$HOME/.local/opt/semaprax-$TAG-$TARGET"
   ```

   You should see `LICENSE`, `README.md`, `release-manifest.json`, `semaprax`,
   `semapraxd`, and `smoke`. If you downloaded the archive in a web browser
   instead of with `curl`, macOS may refuse to open the unsigned program; the
   archives are not notarized, so verify the checksum first and then clear the
   download flag with
   `xattr -dr com.apple.quarantine "$HOME/.local/opt/semaprax-$TAG-$TARGET"`.

5. Put that directory on your `PATH`. For the **current terminal** only:

   ```sh
   export PATH="$HOME/.local/opt/semaprax-$TAG-$TARGET:$PATH"
   ```

   To keep it for **every new terminal**, add the same line, with the tag and
   target written out, to your shell's startup file, then open a new terminal:

   | Shell | Add this line | To this file |
   | --- | --- | --- |
   | zsh (the macOS default) | `export PATH="$HOME/.local/opt/semaprax-v0.8.0-aarch64-apple-darwin:$PATH"` | `~/.zshrc` |
   | bash | the same `export` line | `~/.bashrc` (and `~/.bash_profile` on macOS) |
   | fish | `fish_add_path $HOME/.local/opt/semaprax-v0.8.0-aarch64-apple-darwin` | run once; fish remembers it |

   A new version unpacks to a new directory, so update this line when you
   upgrade.

## Windows (PowerShell)

1. Download the archive and the checksum list, then verify only that archive.
   `SHA256SUMS` lists every platform, so select your archive's line.

   ```powershell
   $Tag = "v0.8.0"
   $Target = "x86_64-pc-windows-msvc"
   $Name = "semaprax-$Tag-$Target.zip"
   $Base = "https://github.com/wavect/semaprax/releases/download/$Tag"
   Invoke-WebRequest "$Base/$Name" -OutFile $Name
   Invoke-WebRequest "$Base/SHA256SUMS" -OutFile SHA256SUMS

   $Line = Get-Content SHA256SUMS | Where-Object { $_ -match "  $([regex]::Escape($Name))$" }
   $Expected = ($Line -split "\s+")[0]
   $Actual = (Get-FileHash $Name -Algorithm SHA256).Hash.ToLower()
   if (-not $Expected -or $Actual -ne $Expected) { throw "Checksum mismatch for $Name - do not unpack it" }
   "$Name : OK"
   ```

2. Optional publisher verification, if the [GitHub CLI](https://cli.github.com/)
   is installed:

   ```powershell
   Invoke-WebRequest "$Base/release-attestation-$Target.json" -OutFile "release-attestation-$Target.json"
   gh attestation verify $Name --bundle "release-attestation-$Target.json" --repo wavect/semaprax
   ```

3. Unpack it somewhere permanent and add it to `PATH`:

   ```powershell
   $Dest = "$env:LOCALAPPDATA\Programs\semaprax-manual"
   Expand-Archive $Name -DestinationPath $Dest
   $Bin = "$Dest\semaprax-$Tag-$Target"
   Get-ChildItem $Bin   # semaprax.exe and semapraxd.exe are here

   # This terminal:
   $env:Path = "$Bin;$env:Path"
   # Every new terminal (your user account only):
   $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
   if (($UserPath -split ";") -notcontains $Bin) {
     [Environment]::SetEnvironmentVariable("Path", "$UserPath;$Bin".TrimStart(";"), "User")
   }
   ```

   Windows only reads the saved `Path` when a terminal starts, so open a new
   PowerShell window to see it there.

## Run your first project

From any directory, with nothing checked out:

```sh
semaprax --version
semaprax new first-semaprax
semaprax check first-semaprax/semaprax.toml
semaprax test first-semaprax/semaprax.toml
semaprax run first-semaprax/semaprax.toml
```

With the v0.8.0 macOS archive these print the version and commit,
`created calculator project first-semaprax`, `verified project first-semaprax
(sha256:...)`, `project tests passed`, and finally **`42`**. `new` needs a
destination that does not exist yet. Continue with
[your first program](first-program.md) or the
[first project](first-project.md).

`semapraxd` is the same release's daemon. You do not start it for the steps in
this handbook; keep it beside `semaprax`.

## Fix “command not found”

* Open a new terminal, or run the current-terminal `export` or `$env:Path`
  line from the step above.
* Check which executable your shell finds: `command -v semaprax` on macOS and
  Linux, `Get-Command semaprax` in PowerShell. If it is not the one you
  unpacked, an earlier directory on `PATH` is shadowing it.
* `version 'GLIBC_2.39' not found` on Linux means the host's C library is older
  than the v0.8.0 build requires; use the source route below.

## Install from source

Choose this to follow the latest `main`, to contribute, or when no archive
fits your computer. It needs **Git** and **Rust/Cargo 1.88 or newer**. Clang is
needed only when you build native executables, and Node.js 22 or newer only for
the web-package tooling. The checker and interpreter path needs neither Clang
nor Node.js, and nothing here needs an API key.

```sh
git --version
rustc --version
cargo --version
```

Clone the repository and run the starter example without installing anything:

```sh
git clone --branch main https://github.com/wavect/semaprax.git
cd semaprax
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- run examples/meaning.spx
```

`check` reports a verified file and `run` prints `42`. The `--` separates
Cargo's options from Semaprax's. Cargo downloads dependencies and compiles on
the first run.

To get the short `semaprax` command, install from the checkout. Each command
installs different programs:

| Command, run in the repository root | Installs |
| --- | --- |
| `cargo install --locked --path .` | `semaprax` (the standalone build) **and** `semapraxd`. |
| `cargo install --locked --path . --bin semaprax` | Only the standalone `semaprax`. |
| `cargo install --locked --path crates/semaprax-toolchain` | Only `semaprax-full`, the full build, under that name. |
| `cargo install --locked --git https://github.com/wavect/semaprax --tag v0.8.0 semaprax` | The v0.8.0 standalone `semaprax` and `semapraxd`, without a checkout. |

A release archive's `semaprax` is the **full** build, the same code as
`semaprax-full`, renamed when packaged. A source-installed `semaprax` is the
standalone build, which has fewer commands; the
[technical install reference](../../docs/INSTALL.md) lists the difference. Use
`semaprax-full` from source when a chapter needs a command the standalone build
lacks.

Cargo installs into `~/.cargo/bin`. If `semaprax` is not found afterwards, add
it for the current terminal:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

To reproduce the exact source this edition of the handbook was reviewed against,
run the following in a clean clone **before** installing, and create a branch
before you develop:

```sh
git switch --detach 508b851a5fda25002ec27453bb559755a6a0d930
```

Latest `main` can contain features that no published release has yet. A command
absent from `semaprax help all` may need a newer build or the full toolchain.

## Find help for your build

```sh
semaprax help run
semaprax help build
semaprax help diagnostic SPX-T208
```

Use the help of the executable you installed rather than guessing a flag.

**Next:** [Write your first program](first-program.md), or
[configure VS Code](editor.md).
