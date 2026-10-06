# Set up VS Code

You will connect VS Code to your compiler, see diagnostics when you save a
`.spx` file, and find the commands for navigation and review. The extension is
**`wavect.semaprax`**. Its [README](https://github.com/wavect/semaprax/blob/main/editors/vscode/README.md)
has the full command list.

## 1. Install a compiler

[Install Semaprax](install.md) first. The extension never downloads or runs a
compiler by itself.

## 2. Select the compiler

1. Open the command palette and run **SEMAPRAX: Configure Compiler**. Clicking
   the **SEMAPRAX** status bar item does the same.
2. Choose **Select installed compiler...** and pick the `semaprax` executable.

The list also shows compilers it found in per-user install folders, Homebrew's
folders, and on your `PATH`. Listing one does not run it. Other choices:
**Re-check current compiler** and **Open installation guide**.

After you pick a file, the extension runs `version --json` and `help all`
once to confirm it is Semaprax, then saves the absolute path in your **user**
settings. A failed check or a cancel changes nothing.

The status bar item shows the state:

| Status | What to do |
| --- | --- |
| select compiler | Run **Configure Compiler**. Diagnostics stay off until you do. |
| compiler *x.y.z* | Ready. Hover for missing prerequisites. |
| compiler unavailable | The file moved or was removed. Select it again. |
| incompatible compiler | The file is not a compatible Semaprax. Select another. |
| untrusted workspace | Trust the workspace. Nothing runs until you do. |

To set the path by hand, use user settings (not workspace settings, which
cannot choose the compiler):

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax"
}
```

## 3. Check a file

Save an `.spx` file and read the Problems panel. Or run **SEMAPRAX: Check
Project**. This needs only the compiler: no manifest, policy, or session.

If a feature is missing, hover the status item. Features follow your compiler's
`help all` list, so an older compiler offers fewer commands.

## 4. Navigate by meaning

| Command | Use it to |
| --- | --- |
| **Go to Declaration by Stable ID** | Jump to a declaration even after a rename. |
| **Show Callers of a Declaration** | See who calls it. |
| **Show Ownership, Contracts, and Effects** | Inspect what the compiler knows. |
| **Safe Rename by Stable ID** | Rename across the project. |
| **Open Semantic Explorer** | Browse the project visually. See [Explorer](../practices/explorer.md). |
| **Show Token Report** | Open a report snapshot you pick. See [Token reports](../practices/context-performance.md). |
| **Inspect Agent Definition** | Read an agent's AgentGraph. |

Save files before you use results that depend on a revision. If the source
changed, refresh the session instead of trusting an old location.

## 5. Review changes in a saved-source session

A session ties the editor to a manifest and a host policy (the file that says
what your machine permits). Set both in user settings:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax",
  "semaprax.manifestPath": "/absolute/project/semaprax.toml",
  "semaprax.hostPolicyPath": "/absolute/path/to/host-policy.json"
}
```

An empty `{}` is not a valid policy. The format is in the
[technical guide](https://github.com/wavect/semaprax/blob/main/editors/vscode/TECHNICAL.md).
Sessions need a compiler that advertises `serve-workspace-mcp`.

Then:

1. **Start Saved-Source Session**, then **Open Candidate**. A candidate is a
   proposed revision, held in memory.
2. **Select Stable Target ID**, then **Show Target Change Catalog**.
3. **Apply Active Typed Intent**. A typed intent is a structured edit, not
   free text.
4. **Preview Candidate Source Diff**, then **Run Candidate Interpreter Tests**.

For unfinished code, use the typed-hole commands (**Open Typed Hole**, **New
Hole Fill Scratch**, **Fill Selected Hole from Active Scratch**). They show the
hole's context and checked fills for its type.

For a rejected change, **Show Compiler-Admitted Repair Catalog** lists exact
fixes.

## Hot reload (opt-in)

**Start Hot Reload** runs `semaprax dev` for the interpreter in a trusted
local workspace. It checks each change, activates valid code between
invocations, and keeps the last good version when a change is invalid. Native
and Wasm swapping are not supported.

## What the extension does not do

Build, commit, approval, publication, package installation, and native
execution stay outside the extension. Finish those with your normal workflow
after you review the diff.

**Next:** [Inspect a project visually](../practices/explorer.md), or
[guide a coding agent](../practices/agents.md).
