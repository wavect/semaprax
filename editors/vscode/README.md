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

The extension does not silently discover or download a compiler, and it does not grant itself build, commit, approval, publication, or network authority.

## Requirements

Install a compatible Semaprax compiler and configure its absolute path in VS Code user settings:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax"
}
```

For saved-source sessions, also configure the project manifest and host policy:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax",
  "semaprax.manifestPath": "/absolute/project/semaprax.toml",
  "semaprax.hostPolicyPath": "/absolute/path/to/host-policy.json"
}
```

These are machine-scoped settings. Workspace and folder overrides are intentionally not trusted for compiler or host-policy selection.

## Quick start

1. Install or build Semaprax from the main repository.
2. Open a project containing `.spx` files.
3. Set `semaprax.compilerPath` to your Semaprax binary.
4. Save an `.spx` file to get compiler diagnostics, or run **SEMAPRAX: Check Project** from the command palette.
5. Use semantic commands such as **Go to Declaration by Stable ID**, **Show Callers of a Declaration**, and **Show Ownership, Contracts, and Effects**.

To try Semaprax without installing it globally:

```sh
git clone https://github.com/wavect/semaprax.git
cd semaprax
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- run examples/meaning.spx
```

See the repository's [installation guide](../../docs/INSTALL.md) for prerequisites and installation options.

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
