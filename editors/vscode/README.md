# Semaprax for Visual Studio Code

Official Visual Studio Code support for the Semaprax programming language, published by [Wavect GmbH](https://wavect.io).

**Marketplace publisher:** Wavect  
**Extension ID:** `wavect.semaprax`

Semaprax is an experimental AI-agent-native systems programming language built around stable semantic identity, compiler-checked meaning, explicit effects, ownership, contracts, typed transformations, and reproducible source review.

> **Preview software**
> Semaprax is currently alpha research software. The language, graph schemas, diagnostics, ABIs, and editor workflows may change. Do not use it for production or safety-critical workloads yet.

## What you get

- `.spx` syntax highlighting, bracket matching, comment toggling, and auto-closing pairs
- Compiler diagnostics on save and explicit project checks
- Semantic navigation by stable declaration identity instead of text matching
- Cross-project caller discovery and code lenses
- Ownership, contract, and effect inspection
- Safe semantic rename workflows
- Saved-source candidate sessions with immutable source-diff previews
- Compiler-admitted repair proposals for rejected transformations
- Typed-hole workflows with checked fill suggestions
- Candidate interpreter-test tasks with explicit cancellation and bounded authority
- Agent-definition inspection and trace/evidence transcript viewing
- Read-only local token-report snapshots

The extension does not silently discover, run, or download a compiler (discovered candidates are only offered for you to select), and it does not grant itself build, commit, approval, publication, or network authority.

## Requirements

Install a Semaprax compiler first; the [installation guide](../../handbook/getting-started/install.md) ([online](https://github.com/wavect/semaprax/blob/main/handbook/getting-started/install.md)) covers the options. The extension never downloads or runs a compiler on its own.

## Select your compiler

A status bar item shows the compiler state. Click it, or run **SEMAPRAX: Configure Compiler** from the command palette, and choose:

- **Select installed compiler...** opens a file picker for the `semaprax` executable.
- A listed candidate: known per-user install locations (`$HOME/.semaprax/bin/semaprax`, `%LOCALAPPDATA%\Programs\Semaprax\bin\semaprax.exe`, Homebrew's `/opt/homebrew/bin` and `/usr/local/bin`) and `semaprax` entries on your `PATH` that exist on disk. Candidates are only listed; none is run until you select it.
- **Re-check current compiler** and **Open installation guide**.

After you select a file, the extension runs `version --json` and `help all` once on that executable (directly, without a shell, with a 10 second and 256 KiB bound) to confirm it is a Semaprax compiler, and only then saves the absolute path to your **user** settings. Cancelling, or selecting something that fails the check, changes nothing and keeps any working selection. Saving an `.spx` file then publishes compiler diagnostics in the Problems panel; no manifest, host policy, or session is needed for that.

Status bar states:

| State | Meaning |
| --- | --- |
| select compiler | No compiler is selected. Diagnostics are off; no prompt appears on save. |
| compiler *x.y.z* | Ready for diagnostics on save. The tooltip lists any missing prerequisite for saved-source sessions and warns when `semaprax` on `PATH` is a different installation. |
| compiler unavailable | The selected file was moved, removed, or cannot start. Select it again to repair. |
| incompatible compiler | The executable did not answer the version contract or does not advertise `check`. |
| untrusted workspace | Nothing is executed until the workspace is trusted. |

Which advanced features are offered depends on the selected executable's own `help all` catalog, not on its file name.

The selection is stored in the machine-scoped setting `semaprax.compilerPath`; workspace and folder values are never used to choose the executable. You may still set it by hand:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax"
}
```

Saved-source sessions additionally need the project manifest and host policy, and a compiler that advertises `serve-workspace-mcp`:

```json
{
  "semaprax.manifestPath": "/absolute/project/semaprax.toml",
  "semaprax.hostPolicyPath": "/absolute/path/to/host-policy.json"
}
```

These are machine-scoped settings. Workspace and folder overrides are intentionally not trusted for compiler or host-policy selection.

## Quick start

1. Install Semaprax (see the installation guide above).
2. Open a project containing `.spx` files.
3. Run **SEMAPRAX: Configure Compiler** and select your `semaprax` executable.
4. Save an `.spx` file to get compiler diagnostics, or run **SEMAPRAX: Check Project** from the command palette.
5. Use semantic commands such as **Go to Declaration by Stable ID**, **Show Callers of a Declaration**, and **Show Ownership, Contracts, and Effects**.

To try Semaprax without installing it globally:

```sh
git clone https://github.com/wavect/semaprax.git
cd semaprax
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- run examples/meaning.spx
```

See the [installation guide](../../handbook/getting-started/install.md) for prerequisites and installation options.

## Saved-source and typed-intent workflows

The advanced editor workflow is intentionally explicit. A session is bound to saved source, a selected manifest, and a selected host policy. Candidate changes are prepared in memory, tied to exact revisions, and reviewed before any separately-authorized publication step.

A typical flow is:

1. **SEMAPRAX: Start Saved-Source Session**
2. **SEMAPRAX: Open Candidate**
3. **SEMAPRAX: Select Stable Target ID**
4. **SEMAPRAX: Show Target Change Catalog** or **New Typed Intent Scratch**
5. **SEMAPRAX: Apply Active Typed Intent**
6. **SEMAPRAX: Preview Candidate Source Diff**

For incomplete work, the extension also exposes typed-hole planning, contextual inspection, checked suggestions, exact fill submission, and explicit draft completion.

## Safety and authority boundaries

The extension is designed around narrow local authority:

- compiler paths come from user/machine settings, never workspace settings
- compiler processes are invoked directly, never through a shell
- check/navigation runs are bounded by time and output limits
- dirty or changed source invalidates revision-sensitive results instead of reusing stale positions
- source review is shown through read-only virtual documents
- candidate operations remain revision-bound
- build, commit, approval, publication, arbitrary package installation, and direct native execution are intentionally outside the extension command surface

The compiler remains the semantic verifier. The extension does not infer semantic correctness from editor text.

## Commands

Open the VS Code command palette and search for `SEMAPRAX:`. The extension currently contributes commands for:

- project checking
- saved-source session start/stop/refresh
- candidate creation and target selection
- typed intents and diagnostic recovery
- compiler-admitted repairs
- source-diff preview
- candidate interpreter-test tasks and cancellation
- typed-hole planning and filling
- declaration navigation and caller discovery
- documentation, ownership, contracts, effects, and cleanup-plan inspection
- safe rename
- agent inspection and trace/evidence transcripts
- **SEMAPRAX: Show Token Report**, which opens one user-selected local report
  snapshot without starting a session, calling the compiler, or refreshing source

## Settings

| Setting | Default | Purpose |
| --- | --- | --- |
| `semaprax.compilerPath` | empty | Absolute Semaprax compiler path |
| `semaprax.checkOnSave` | `true` | Run read-only project/file checks when saved |
| `semaprax.codeLens` | `true` | Show stable identity, effects, and contract metadata above declarations |
| `semaprax.manifestPath` | empty | Absolute `semaprax.toml` path for saved-source sessions |
| `semaprax.hostPolicyPath` | empty | Absolute existing host-policy JSON path |

## Project links

- [Semaprax repository](https://github.com/wavect/semaprax)
- [Getting started](https://github.com/wavect/semaprax#readme)
- [Installation guide](https://github.com/wavect/semaprax/blob/main/docs/INSTALL.md)
- [Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md)
- [Issue tracker](https://github.com/wavect/semaprax/issues)
- [Wavect GmbH](https://wavect.io)

## Deep technical notes and verification evidence

The previous editor-adapter README contained detailed protocol, evidence, task-controller, typed-hole, repair, navigation, and authority-boundary documentation. It is preserved verbatim in [TECHNICAL.md](TECHNICAL.md) so Marketplace users get a concise landing page without losing the implementation evidence and exact behavioral boundaries.

## Development

This extension is intentionally zero-build CommonJS and uses VS Code APIs plus Node built-ins.

Run the authored Node tests with:

```sh
node --test test/*.test.js
```

The repository also contains a real VS Code Extension Host evidence runner for integration-level verification against exact source subjects.

## License

Apache License 2.0. See [LICENSE](LICENSE).
