# Human Diagnostic Locations v1

Status: authored and locally exercised; unpublished and unpromoted.

Audience: compiler users, editor authors, and compiler contributors.

This additive rendering contract makes the source path already carried by a
SEMAPRAX diagnostic visible in ordinary terminal output. It changes no
diagnostic selection, severity, code, message, help, source span, exit status,
or machine-readable JSON field.

## Human rendering

A diagnostic with both a path and span renders its location after the message:

```text
error[SPX-P104]: expected `module` at src/main.spx:1:1
```

The location spelling is `path:line:column`. A path without a span renders as
`at path`. A span without a path retains the prior `at line:column` spelling,
and a diagnostic with neither retains its prior output. Help remains on the
following indented line.

Control characters in a human-rendered path use deterministic Rust-style
escapes, preventing a source filename from injecting terminal control
sequences or extra diagnostic lines. Ordinary ASCII and Unicode path
characters remain readable.

## Preservation and authority

`Diagnostic::json()` is unchanged and remains the automation interface. Its
`path`, `location`, and `help` fields retain their existing values and null
behavior. Human locations are descriptive only: rendering a path does not
read it, navigate to it, grant filesystem authority, or authenticate current
source bytes.

Executable evidence covers path plus span, path only, span only, absent
locations, help placement, control-character escaping, unchanged JSON, and an
actual `semaprax check` failure.
