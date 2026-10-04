# Harness artifact identity v2 (HN-19)

Status: additive development-harness specification (HN-19); local macOS aarch64 evidence only.

Audience: toolchain contributors, adapter authors and anyone binding trust, locks or caches to content.

Identity is integrity evidence, never permission or authorship proof. Diagnostics use letter `M`
(`SPX-HPM034`, `SPX-HPM035`) plus the existing trust codes `SPX-HPB031`/`034` and launch code `SPX-HPC002`.
Implementation: `crates/semaprax-harness/src/skills/{inventory,snapshot}.rs`.

## Inventory and digest

An inventory is a set of entries `{path, kind, bytes, sha256}` over every admitted file. `path` is a
normalized relative path (`/` separators; no root, empty, `.`/`..`, backslash or control character).
`kind` is one of `passive-text` (SKILL.md, manifests, prose), `reference-asset` (`references/`, `assets/`),
`executable-script` (`scripts/`), `adapter-code` (adapter entry, descriptor and helpers). The digest is
`sha256` of domain `semaprax.artifact-inventory.v2` plus the canonical JSON of the path-sorted entries, so
entry order never matters and any path, kind or byte change does. Duplicate or case-folding-colliding
paths cannot form an inventory (`SPX-HPM034`).

Walk rules (`inventory::walk`): symlinks anywhere, hardlink aliases (link count above 1), special files,
non-UTF-8 names, and bound violations (skills: 256 files, 16 MiB, 4 MiB per file, depth 8; adapters: 2048
files, 64 MiB, 16 MiB per file, depth 12) are refused. Caches (`.git`, `__pycache__`, `*.pyc`, `.DS_Store`
...) are excluded. Files are opened and re-stat'ed (device and inode) to narrow swap races.

## Labels and the legacy digest

| Where | Value | Label |
|---|---|---|
| Skill `digest` (list, load, contract) | `sha256:<hex>` over the v2 inventory | `artifact-v2` |
| Skill `legacy_digest` | pre-v2 digest: SKILL.md or manifest plus script names with empty bytes | `legacy-v1` |
| Adapter `entry_digest` in installations, trust records and grants | `artifact-v2:sha256:<hex>` | `artifact-v2` |
| Adapter `entry_digest` recorded by earlier builds | plain `sha256:<hex>` of the entry file only | `legacy-v1` |

Old records stay readable and are interpreted exactly as before (entry file only). `inspect` recomputes
whatever kind the installation recorded, so a legacy installation stays legacy until `adopt` runs again.
A legacy-v1 skill digest used as `approved_digest` is accepted only when it covers every file of the bundle.

## Adapter closure

The closure of an adapter is every regular file under the descriptor directory (entry, descriptor, helper
modules, data files) except caches. Explicit rules live in the optional `harness-closure.json`:
`{"schema":"semaprax.harness-closure.v1","exclude":["notes/","EVIDENCE.md"]}` (trailing `/` is a
directory). The rules file is part of the closure, and the entry cannot be excluded. Adoption records the
closure label; trust binds it (`SPX-HPB031` when it changes, naming the closure); `check_grant_current`
revalidates; launch (`host::launch`) recomputes the closure over the descriptor directory and refuses a
changed one (`SPX-HPC002`). New code or a changed helper therefore cannot ride along under an old grant,
and permissions never widen from identity alone (`SPX-HPB032` is unchanged).

## Snapshot store

`<harness_home>/artifacts/<hex>/{inventory.json,files/..}` is content addressed by the v2 digest. Publication
extracts into `.tmp-*` through the same walk (symlink, hardlink, duplicate and bound checks), writes the
inventory and renames atomically; a failed extraction leaves nothing and `.tmp-*` is never addressable.
`snapshot::open` validates the address, every file hash and the absence of extra files, and runs at
activation. A session (`SkillService::with_snapshot_store` + `activate`) loads bodies and resources only from
its snapshot; editing the source creates a new candidate digest and `activate`/`drift` report the divergence.

## Not covered (unavoidable trust)

The identity covers files inside the directory only. It does not authenticate the node, python or native
runtime, the standard library or `node_modules` outside the closure, system shared libraries, imports
resolved from outside the directory, the upstream tool the adapter drives (bound separately by its own
executable digest), or the operating system. Hashing `entry.py` never authenticated its imports; the
closure now does for files in the directory. Users who need more should install helpers inside the
adapter directory or adopt a managed package identity. Case folding uses Unicode lowercase, not full
Unicode normalization (NFC/NFD collisions on normalizing file systems are not detected).
