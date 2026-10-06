# Set up VS Code

Use the editor to see diagnostics where you are working, follow stable IDs,
and review candidate changes. Start with checking on save; add a saved-source
session once you understand the basic project workflow.

The repository's extension is **`wavect.semaprax`**. Its
[setup guide](https://github.com/wavect/semaprax/blob/main/editors/vscode/README.md)
contains installation details and the full command list.

## Connect the compiler you installed

Install a compiler first (see the [installation guide](install.md)). Then click
the **SEMAPRAX** item in the status bar, or run **SEMAPRAX: Configure Compiler**
from the command palette, and choose **Select installed compiler...** to pick
the `semaprax` executable with a file picker. The list also offers executables
it found in the usual per-user install locations and on your `PATH`; those are
only listed, and nothing runs until you select one.

After you choose, the extension runs `version --json` and `help all` once on
that file to confirm it is a Semaprax compiler, then saves its absolute path
in your VS Code **user settings**. If the check fails, or you cancel, nothing
changes. The status bar then shows the compiler version, or one of: select
compiler, compiler unavailable (the file moved or was removed; select it
again), incompatible compiler, or untrusted workspace. No dialog appears on
every save.

You can still set the path yourself in user settings, replacing the example
value below:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax"
}
```

An absolute path starts at the file system root, rather than being relative to
the current project. The extension deliberately uses machine/user settings
for tool selection; a repository's workspace settings cannot select a different
compiler for you.

## Check your first file

Open an `.spx` file, save it, and inspect the Problems panel. You can also open
the command palette and run **SEMAPRAX: Check Project**.

Check-on-save is enabled by default and needs only the compiler: no manifest,
host policy, or session. The extension does not download a compiler for you.
A missing selection, a moved executable, or an older compiler that does not
advertise a command can explain missing features even when syntax highlighting
works; the status bar tooltip says which.

## Navigate by meaning

Use the command palette to find the extension's declaration, caller, and
ownership/contract/effect commands. A stable ID selects a declaration even when
its display name has changed. Code lenses can show that identity and related
metadata above source declarations.

Save relevant files before using revision-sensitive results. When source is
dirty or has changed, refresh the session rather than trusting an old location
or candidate diff.

## Add a saved-source session

A saved-source session binds the editor to a particular manifest and host policy.
The policy is the configuration that states what the local host permits.
Configure existing, compatible files in user settings:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax",
  "semaprax.manifestPath": "/absolute/project/semaprax.toml",
  "semaprax.hostPolicyPath": "/absolute/path/to/host-policy.json"
}
```

Use the policy format in the extension's
[technical guide](https://github.com/wavect/semaprax/blob/main/editors/vscode/TECHNICAL.md).
An empty `{}` file is not a substitute for that policy.

A typical session follows these steps:

1. Start a saved-source session and open a candidate.
2. Select the stable target ID and inspect its change catalog.
3. Apply the selected typed intent and preview the candidate source diff.
4. Run the available candidate test task, then review its result.

A **candidate** is a proposed revision. A **typed intent** is a structured edit
with the fields expected for that operation. Neither is a text instruction to
rewrite arbitrary files.

## Use typed holes for unfinished work

A typed hole marks an incomplete part of a candidate. The editor can show its
context, request checked suggestions, submit an exact fill, and complete the
draft through the supported workflow. Keep the candidate and target revision
together throughout that process.

This helps when you know the type and purpose of the missing part but still
need to choose its implementation.

## Inspect token reports

**SEMAPRAX: Show Token Report** opens a local report snapshot you select.
Create one using [Token reports and caches](../practices/context-performance.md).
Opening it does not start a session or make a model call.

## Keep review and publication separate

The extension displays source diffs through read-only views and keeps candidate
operations tied to revisions. Build, commit, approval, and publication are
separate operations with their own authority. Complete that step through your
chosen host workflow after reviewing the candidate.

**Next:** [Inspect a project visually](../practices/explorer.md), or
[guide a coding agent](../practices/agents.md).
