# Harness setup and distribution v1 (HN-07)

Status: additive development-harness specification; local macOS aarch64 evidence only (Linux untested).

Audience: harness users, packagers and toolchain contributors.

Extends `docs/HARNESS-PROVIDER-V1.md` (profile, adoption, trust) with one reviewed
setup action, a single runtime resolver and a distributable tarball. It adds no
installation database: `setup` composes `adopt`, `trust` and the existing
machine-local state.

## Embedded adapters

The shipped adapters (`graft`, `graphify`, `rtk`, `systemone`,
`examples/source-index-python`, `sdk/python`, `sdk/node` under
`packages/semaprax-harness-adapters/`) are compiled into the binary
(`src/assets.rs`, list generated into `src/assets_list.rs` by
`scripts/gen_harness_assets.sh`; tests, evidence, research, fixtures and caches
are excluded; a unit test fails on drift). Setup materializes the bundle into
the content-addressed artifact store (`<home>/artifacts/<hex>/files/`, artifact-v2
identity, idempotent) and adopts descriptors from there, so adapters work when
no checkout exists. Official skills were already embedded (HN-04).

## `semaprax harness setup`

```
setup [--project <dir>] [--path-dirs <abs1:abs2>] [--tool rtk|graft|graphify|node|python=<abs>]...
      [--preset native|local-efficient] [--provider graft|graphify|none] [--require graft|graphify|rtk]...
      [--dry-run | --yes] [--json]
```

- Discovery looks only in `--path-dirs` and explicit `--tool` paths. PATH, HOME
  and the project are never scanned; an executable inside the project (even via
  symlink) is refused and noted, never adopted, never trusted. The only process
  started is the descriptor's `identity_probe` (or `--version` for node/python)
  through the bounded `adopt` probe (absolute path, scrubbed environment, 5 s, 64 KiB).
- Plan, no flag: prints the plan and changes nothing. `--dry-run`: same, explicit.
  `--yes`: apply. No interactive prompt exists.
- Presets: `native` (builtin context and command view, official skills) and
  `local-efficient` (adds RTK and one repository provider, Graft preferred, when
  usable). Exactly one repository provider is ever adopted; Graft and Graphify are
  never both chosen. A version outside a descriptor's tested list is reported as
  "installed but untested" and never substituted; a platform the descriptor does
  not list is not chosen.
- Apply: materialize adapters, `adopt` with the chosen upstream, record the
  interpreter (`--runtime` equivalent), `trust`, set the user preference, and write
  `semaprax.harness.toml` (provider ids only; no paths, no trust) when absent. An
  existing project file or an existing adoption of the same provider id from another
  readable descriptor is kept and reported, never overwritten.
- Idempotent: a repeat finds adoption, digests, trust, preference and profile
  current and reports `noop`.
- Optional tools missing: builtin fallback, exit 0. `--require`/`--provider`
  naming an unusable provider: `SPX-HPB060`, actionable, nothing changed. Pinned
  managed installs are `harness updates` (HN-05); setup downloads and installs nothing.
- Output: human text or canonical JSON `semaprax.harness-setup.v1` with a
  reproducible teammate command (no machine paths).

Diagnostics: `SPX-HPB050` usage, `060` required provider unavailable, `061`
conflicting provider selection, `062` project path problem, `063` bundled
adapter store problem.

## One runtime resolver

`profile::runtime` is used by `run`, `exec`, `conformance` and the context broker
(through `launch.runtime`): explicit flag, then the runtime adopted for that
installation, then a policy file, then `HARNESS_NODE` / `HARNESS_PYTHON`. Every
resolved external launch carries `launch.runtime`. `conformance` defaults an omitted
`--runtime`/`--upstream` to the adopted installation of the same descriptor.
Bridges start `run`/`context` and inherit the same resolution.

## Distribution

`scripts/harness_dist.sh build --binary <semaprax-harness> --out <dir> [--full <semaprax>]`
packs the binary, optional prebuilt `semaprax-full`, `LICENSE`, `docs/HARNESS-*.md`,
`share/support-catalog.json` (providers, tested platforms and versions, official
skills) and smoke fixtures. `smoke` extracts it outside the checkout with an empty
HOME and a scrubbed environment, discovers skills, runs setup against preinstalled
RTK and Graft, checks the repeat is a no-op and runs a native-only and a
Graft-backed task. The provisioned test is
`tests/real_tools_v1/setup_dist.rs` (`--ignored`). The standalone compiler install is unaffected.
