# Shipping: lock, resolve, review

Deterministic pins, explicit dependencies, and checked changes — the
mechanics of taking a project from laptop to CI to consumers.

## Lock the interface

```sh
semaprax lock semaprax.toml --write            # pin to semaprax.lock
semaprax lock semaprax.toml --verify           # re-check the pin
semaprax lock semaprax.toml --compare base.lock  # breaking or not? (CI gate)
```

The lock records identity, source digests, interface digest, targets, and
capabilities. `--compare` reports whether the interface change since the
baseline is breaking and exits nonzero for CI. `--emit-interface` and
`--compare-interface` operate on the interface projection alone.

## Resolve dependencies

```sh
semaprax add semaprax.toml std.num "^0.1.0"     # add a dependency row
semaprax resolve semaprax.toml --target native64 --cache <dir> --write
semaprax resolve semaprax.toml --target wasm32 --cache <dir> --verify
semaprax fetch <cache-dir> <subject.json>...    # populate the local cache
```

Resolution selects `[dependencies]` ranges against a local content-addressed
cache and pins the per-target result. A build does not yet link resolved
dependencies — resolution and linking are separate, explicit steps.

## Change with review

Preview semantic edits before applying them; inspect the blast radius first:

```sh
semaprax query <project> impact declaration <stable-id> --depth 1 --max-bytes 4096
semaprax change preview <project> rename-display-name <stable-id> <new-name>
semaprax change preview <project> add-contract <stable-id> ensures <predicate.json>
semaprax review <project> <transaction.json> [--evidence]
```

Impact and review are read-only and bound to exact source bytes — drift fails
closed. `change rebase` and `change merge` compose transactions explicitly.
Evidence capsules replay through `verify` but never grant write authority by
themselves.

## Package reporting

```sh
semaprax package report <file> [--max-bytes N]
semaprax package lock <subject.json>... [--max-bytes N]
semaprax package resolve <subject.json>... --require <pkg>:<range> --target native64|wasm32
```

Reports describe the checked package; lock/resolve pin its dependency
closure per target with explicit capability grants (`--allow-capability`).

## Best practices

1. **Commit the lockfile.** `semaprax.lock` beside `semaprax.toml` makes
   every checkout, CI run, and consumer resolve identically.
2. **Gate CI on `--compare`.** Breaking interface changes should fail loudly
   at the PR, not surface in a downstream build.
3. **Preview before applying.** `impact` → `preview` → `review` → apply is
   the change pipeline; skipping to apply discards the evidence trail.

Exact rules: [Project Lock v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-LOCK-V1.md),
[Project Dependency Resolution v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-DEPENDENCY-RESOLUTION-V1.md),
[Semantic Impact v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-IMPACT-V1.md),
[Semantic Review v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-REVIEW-V1.md).
