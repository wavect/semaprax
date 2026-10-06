# SEMAPRAX {{TAG}}

This is the SEMAPRAX {{VERSION}} command-line toolchain for `{{TARGET}}`.
SEMAPRAX is beta software: see the [release page](https://github.com/wavect/semaprax/releases/tag/{{TAG}}) for what is and is not claimed.

## What is in this folder

| File | What it is |
| --- | --- |
| `semaprax.exe` | The full `semaprax` command-line tool |
| `semapraxd.exe` | The SEMAPRAX daemon used by editors and agents |
| `LICENSE` | License terms ([online copy](https://github.com/wavect/semaprax/blob/{{TAG}}/LICENSE)) |
| `release-manifest.json` | Version, commit, target and runtime requirements of this build |
| `smoke\meaning.spx` | A one-function sample used to smoke-test this archive |
| `README.md` | This file |

## Runtime requirements

- Windows 10 version 1809 (build 17763) or newer, or Windows Server 2019 or newer, on x64.
- Exact requirements of this archive are recorded under `runtime` in `release-manifest.json`.

## Run it from this folder

Open PowerShell in this folder and run:

```powershell
.\semaprax.exe --version
```

If Windows SmartScreen warns about a downloaded copy, these programs are not code-signed in this release; verify the download first as described in the [release process](https://github.com/wavect/semaprax/blob/{{TAG}}/docs/RELEASE-PROCESS.md).

## Put it on your PATH

For the current PowerShell window only:

```powershell
$env:Path = "$PWD;$env:Path"
semaprax --version
```

To keep it, move this folder somewhere permanent and add its path to your user `Path` in Windows Settings. The installer script does this for you; see the [install guide](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/install.md).

## Your first project

From any empty working directory, with `semaprax` on your `Path`:

```powershell
semaprax --version
semaprax new first-semaprax
semaprax check first-semaprax\semaprax.toml
semaprax test first-semaprax\semaprax.toml
semaprax run first-semaprax\semaprax.toml
```

The last command prints `42`.

To try the bundled sample without creating a project, run these from this folder:

```powershell
.\semaprax.exe check smoke\meaning.spx
.\semaprax.exe run smoke\meaning.spx
```

## Learn more

- [Install guide](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/install.md)
- [Your first project](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/getting-started/first-project.md)
- [Handbook](https://github.com/wavect/semaprax/blob/{{TAG}}/handbook/README.md)
- [Quick reference for writing `.spx` source](https://github.com/wavect/semaprax/blob/{{TAG}}/docs/AGENT-QUICK-REFERENCE.md)
- [Verifying a release](https://github.com/wavect/semaprax/blob/{{TAG}}/docs/RELEASE-PROCESS.md)
- [Source repository](https://github.com/wavect/semaprax/tree/{{TAG}})
