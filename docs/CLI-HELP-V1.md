# Capability-Aware CLI Help v1

Status: authored, locally exercised, unpublished, and unpromoted. The
completion matrix and release evidence own product status.

Audience: CLI users, release engineers, and compiler contributors.

This additive command-help surface makes the closed CLI grammar inspectable
without acquiring command authority. The help mechanism itself does not add an
option, alias, target, plugin, or host capability. The later public
`project-scaffold` command is an additive catalog entry owned by [Public
Project Scaffold Capsule v1](PROJECT-SCAFFOLD-V1.md).

## Capability boundary

The standalone `semaprax` executable has no private host. Its help omits the
private `doctor` and `new` commands and the private Rust-package build target.
It does include the public stdout-only `project-scaffold --name project-name
[--template calculator]` route, which has no private host hook.
The unpublished `semaprax-full` executable receives one explicit
`PrivateHost`; only that presence bit selects the fuller catalog. Help must not
call a host hook, read a path, inspect the environment, search `PATH`, discover
plugins, or probe a target.

An unavailable private command is indistinguishable from an unknown command at
the scoped-help boundary. Help text is documentation, not authority: ordinary
dispatch still performs its existing parsing, authentication, and capability
checks.

## Closed forms

The existing no-argument and exact one-token global `help`, `--help`, and `-h`
forms retain their exact stdout bytes and exit status. Global output is rendered
from one closed catalog whose ordered usage lines are also the scoped-help
source.

Two scoped forms are admitted:

```text
semaprax help <command>
semaprax <command> --help
semaprax <command> -h
```

For an available command they return status zero, empty stderr, and exactly:

```text
Usage:
  <canonical global usage line>
```

Every global line for that command is included in its existing order. Thus a
command with multiple canonical invocation shapes exposes multiple indented
lines. Canonical names and already-dispatched aliases select the same entry;
an alias does not invent another usage grammar.

No other placement is help. Extra operands, extra options, and embedded
`--help` or `-h` reject with status two before command effects and the fixed
stderr line `help flags are admitted only as the sole operand of a command`.
They emit no stdout. This prevents help recognition from masking malformed
input or bypassing validation. The rejection does not convert an
otherwise-invalid invocation into help.

Unknown and capability-hidden scoped selections, plus malformed `help ...`
selections without an embedded help flag, use the existing unknown-command
surface: status two, the exact bounded diagnostic on stderr, and the unchanged
capability-appropriate global help on stdout. The selected name is never used
to discover code or authority.

## Catalog invariants

The catalog is static and source-owned. Each entry contains only its canonical
dispatch name, already-supported aliases, exact ordered usage lines, and a
public/private availability class. Global and scoped renderers consume those
same bytes; they do not maintain parallel usage strings.

Executable evidence must enumerate every top-level dispatcher arm and every
catalog entry in both directions, including aliases. It must also prove exact
global-byte preservation, standalone/full capability separation, all help
aliases, multi-line commands, malformed positions, unknown/private selection,
status/stdout/stderr, and an empty working directory with no created entries.

## Nonclaims

This surface is not shell completion, dynamic discovery, a plugin registry, a
machine-readable command schema, or proof that a documented command is
published. It grants no filesystem, process, network, environment, clock,
compiler, target, publication, doctor, or project-creation authority. Local
tests are not hosted, cross-platform, release, or support evidence.
