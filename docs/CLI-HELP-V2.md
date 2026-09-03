# Capability-Aware CLI Help v2

Status: authored and locally exercised; unpublished and unpromoted.

Audience: CLI users, release engineers, and compiler contributors.

This additive revision helps users recover from a misspelled command while
preserving the v1 catalog, global help bytes, scoped help bytes, exit statuses,
capability boundary, and zero-authority behavior.

## Bounded suggestion rule

When top-level dispatch or `semaprax help <command>` receives an unknown ASCII
command name of at most 64 bytes, the CLI compares it only with command names
and aliases already visible in that executable's static catalog. A unique
nearest name is suggested when its bytewise Levenshtein edit distance is at
most one for inputs up to four bytes and at most two for longer inputs.

The exact diagnostic becomes:

```text
unknown command `chek`; did you mean `check`?
```

It retains the v1 trailing blank line, status 2, capability-appropriate global
help on stdout, and empty working-directory behavior. A tie, a more distant
name, non-ASCII input, or input longer than 64 bytes keeps the exact v1
`unknown command` diagnostic without a suggestion.

Suggestions never inspect source, paths, the environment, installed tools,
plugins, targets, or network state. The standalone compiler cannot suggest
`doctor`, `new`, or another private-only entry because those names are absent
from its visible catalog. The full toolchain may suggest them because it
already exposes them in global help.

## Preservation and evidence

All exact global and successful scoped help output remains v1. Known commands,
help aliases, malformed help-flag placement, unavailable exact private command
names, and unrelated unknown names retain their prior behavior. Suggestions
grant no command, filesystem, process, network, compiler, target, or private
host authority.

Executable evidence covers insertion, deletion, substitution, transposition,
unrelated, non-ASCII and over-limit inputs; exact stdout/stderr/status; direct
and scoped-help selection; and standalone/full private-command separation.
