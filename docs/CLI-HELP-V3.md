# Capability-Aware CLI Recovery v3

Status: authored and locally exercised; unpublished and unpromoted.

Audience: CLI users, release engineers, and compiler contributors.

This additive revision gives a user whose known command invocation is rejected
a direct route to that command's scoped usage. It preserves the v1 catalog and
help bytes and the v2 typo behavior.

## Recovery hint

When a capability-visible known command returns invocation status 2, the CLI
appends this exact stderr line after the command's diagnostic:

```text
hint: run `semaprax check --help` for usage
```

The entered command name or alias is reproduced exactly. The hint is omitted
for successful commands, compiler or execution failures with status 1, empty
invocations, unknown commands, commands hidden by the executable's capability
boundary, the `help` command, and invocations containing `--help` or `-h` in a
malformed position. Those exclusions preserve existing exact output and avoid
revealing private commands.

The hint is derived only from the executable's static capability-visible help
catalog and the final exit status. It performs no source, filesystem,
environment, target, plugin, process, or network inspection and grants no
command authority.

## Preservation and evidence

Global and scoped help stdout, command diagnostics, statuses, and side effects
remain unchanged; v3 adds only the recovery line to admitted status-2 cases.
The machine-readable diagnostic formats of commands that support JSON are
unchanged.

Executable evidence covers public standalone and private full-toolchain
commands, exact stderr and status, empty working directories, and preservation
of unknown-command, hidden-command, malformed-help-position, and successful
help output.
