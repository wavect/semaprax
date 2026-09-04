# Standalone project creation v1

Status: authored with local evidence; unpublished and unpromoted. The
completion matrix and release evidence own product status.

Audience: new SEMAPRAX users, toolchain contributors, and reviewers.

## Purpose

`semaprax new <destination>` used to exist only in the unpublished full
toolchain, because its publication went through a held-parent staged authority
that lives in a private platform crate. A developer or agent who installed the
published compiler had to install a second binary from a source checkout to
create a first project. This version gives the standalone compiler its own
`new`, with the same grammar, template, file bytes, and success line, through
a bounded route the compiler library can implement with the standard library.

## Grammar and template

```text
semaprax new <destination> [--name project-name] [--template calculator]
```

The grammar and every rejection message are those of the full toolchain's
`new`. The project name defaults to the destination's final component and
must match lowercase `[a-z][a-z0-9-]*` within 64 bytes. The only template is
`calculator`. The files are exactly the [Public Project Scaffold Capsule
v1](PROJECT-SCAFFOLD-V1.md) files for that name, in that order; this route adds
no bytes and no file.

## Route

On success the standalone compiler has performed exactly these steps:

1. Derive the scaffold in memory; a derivation failure is reported before the
   filesystem is touched.
2. Resolve the destination to an absolute path. Its parent must exist and be a
   directory when inspected without following a final symbolic link; the
   destination itself must not exist as any kind of entry.
3. Create the destination directory with create-new semantics, then its `src`
   directory, then each file with create-new semantics, in scaffold order.
4. Read every file back and require byte equality with the scaffold.
5. Authenticate the written project through the ordinary Project v1 snapshot
   path and run its `check`.
6. Print `created calculator project <destination>` with the destination as
   the caller spelled it, and exit zero.

Invocation errors exit two with `new: <message>` and the scoped-help recovery
hint; they create nothing. A creation failure exits one with `new: <message>`.
A failure before step 3 creates nothing. A failure during or after step 3
leaves whatever was written in place and reports it; `new` never deletes,
replaces, or writes into an entry it did not create in the same invocation.

## Non-claims

This route has no staging directory, no atomic rename, and no re-verification
that the parent or destination identity was unchanged between steps. A
concurrent writer that creates the destination between step 2 and step 3 is
detected by the create-new directory creation; one that substitutes the parent
after step 2 is not. The full toolchain's `new`, owned by [calculator project
publication](NEW-PROJECT-PUBLICATION-V1.md), remains the hardened route and is
what the tag archives ship. This is not template discovery, a package manager,
Git initialization, dependency installation, or a network operation. Local
evidence is not hosted, cross-platform, release, or support evidence.

## Evidence

`tests/project/new_cli.rs` pins, against the standalone binary: the exact
template bytes for a default and an explicit name, nested destinations under an
existing parent, the success line, that the created project checks, tests, and
runs, refusal of existing directories and files without touching them, the
exit-two invocation rejections and their messages, the missing-parent failure,
and the guided and scoped help entries. The standalone help harness pins the
public catalog entry; the quickstart harness executes the documented flow with
the standalone binary alone.
