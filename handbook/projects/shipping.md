# Shipping: lock, resolve, review

You can pin a project's interface, declare and resolve dependencies, preview a
change before it touches source, and verify what you ship. After this page you
know which command to run at each step. Every command here reads files you name
and makes no network call.

## Lock the interface

```sh
semaprax lock . --write                      # create semaprax.lock
semaprax lock . --verify                     # re-check it
semaprax lock . --compare base.lock          # breaking or not? (CI gate)
semaprax lock . --emit-interface > iface.json
semaprax lock . --compare-interface iface.json
```

The lock records identity, source digests, interface digest, targets and
capabilities. `--compare` prints a verdict and exits `1` when the change is
breaking:

```json
{"changes":[{"classification":"breaking","detail":"a retained export changed its types, ownership, or contracts","kind":"interface-digest"}],"schema":"semaprax.project-lock-compatibility.v1","verdict":"breaking"}
```

A stale lock fails `--verify` with `SPX-J123`; run `--write` again or restore
the sources. `--compare-interface` takes an interface file made by
`--emit-interface` (`SPX-J124` for anything else).

## Resolve dependencies

```sh
semaprax add . std.num "^0.1.0"                       # declare
mkdir cache
semaprax fetch cache vendor/pkg.subject.json          # verify and file by digest
semaprax resolve . --target wasm32 --cache cache --write
semaprax resolve . --target wasm32 --cache cache --verify
```

| Step | What it does |
| --- | --- |
| `add` | Adds one `[dependencies]` row. Touches nothing else. |
| `fetch` | Replays each Subject-v3 file and stores it as `<digest>.json`. `--lock <lock.json>` also checks the files against that lock. Up to 64 subjects. |
| `resolve` | Selects versions only from that cache and pins the result per target (`native64` or `wasm32`). The cache directory must exist (`SPX-J126`). |

Bundled `std.*` packages need no fetch. Other packages need a
`[dependency-sources]` row. Nothing discovers a registry or reads the network.

For work outside a project, the `package` commands take subject files directly:

```sh
semaprax package report <file> [--max-bytes N]
semaprax package lock <subject.json>...
semaprax package resolve <subject.json>... --require <pkg>:<range> --target native64|wasm32 [--allow-capability <cap>]...
```

`package report` describes one checked file. `lock` and `resolve` pin a
dependency closure; capabilities are granted only with `--allow-capability`.

## Use a package registry file

A registry is one JSON document
(`semaprax.package-registry-document.v1`). The commands read it and never
write, sign or publish.

```sh
semaprax registry search registry.json num
semaprax registry add registry.json std.num "^0.1.0"   # highest matching version
semaprax registry lock registry.json template.json --raw > registry.lock.json
semaprax registry fetch registry.json my.pkg 1.2.0 --raw > my.pkg.subject.json
semaprax registry verify registry.json snapshot-evidence.json
semaprax registry verify registry.json template.json lock-evidence.json
semaprax registry publish registry.json entry.json     # decide and print only
```

`publish` shows the registry document that would result. It publishes nothing.
There is no default registry and no search path. A missing coordinate is
`SPX-Z927`; an unreadable file is `SPX-Z926`; registry rules keep their
`SPX-PKR6xx` codes. Specs:
[Registry Snapshot v1](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-REGISTRY-SNAPSHOT-V1.md),
[Registry-Bound Resolution v1](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-REGISTRY-BOUND-RESOLUTION-V1.md).

## Change with review

Look first, then preview, then review. Impact, preview and review are read-only
and bound to the exact source bytes: drift fails closed.

```sh
semaprax query . impact declaration calculator.add --depth 1 --max-bytes 4096
semaprax query . available-operations calculator.add
semaprax change preview . rename-display-name calculator.add sum
semaprax change preview . add-contract calculator.add ensures predicate.json
semaprax change preview . replace-expression <fn-id> <expression-id> replacement.json
semaprax change preview . add-declaration <anchor-id> declaration.json
semaprax review . transaction.json [--evidence]
```

| Command | Use it to |
| --- | --- |
| `query ... available-operations <id>` | See which typed changes the project allows for a declaration. |
| `change preview` | Validate a change and print its candidate, impact and review. Writes nothing. `--evidence` or `--structural-diff` print those artifacts instead. Pass `--revision <digest>` to bind a revision. |
| `change rebase <base> rename-display-name <id> <name> --onto <project>` | Replay a change on another project. |
| `change merge <project> rename-display-name <a> <x> --with rename-display-name <b> <y> --order left-then-right` | Combine two changes in an explicit order. |
| `review <project> <transaction.json>` | Review a canonical Universal Semantic Transaction. `--evidence` binds intent, impact and review. |
| `verify <subject> <change> <evidence.json>` | Replay an evidence capsule. Its `schema` selects the verifier (`SPX-V201`, `SPX-V202` if none). |

A preview is not a transaction. Do not pass `change preview` output to
`review`: `review` wants the closed transaction envelope
(`SPX-G525` otherwise). Evidence carries no authority; a passing `verify` is
proof data, not permission to write.

### Single-file patches

For one `.spx` file, a semantic patch (`.spatch`) works the same way:

```sh
semaprax impact app.spx change.spatch            # read-only blast radius
semaprax review app.spx change.spatch
semaprax patch-evidence app.spx change.spatch > evidence.json
semaprax verify app.spx change.spatch evidence.json
semaprax patch-with-evidence app.spx change.spatch evidence.json   # writes
semaprax patch app.spx change.spatch                               # writes
```

`patch` and `patch-with-evidence` are the only commands in this list that write
source. The `-v2` forms (`patch-evidence-v2`, `verify-patch-evidence-v2`,
`patch-with-evidence-v2`) use the second evidence schema, and `target-evidence`
adds per-target facts.

## Change a managed workspace

A managed workspace holds 2 to 32 canonical `.spx` files under
`.semaprax-workspace`. A successful change publishes one complete generation
through a single `ACTIVE` pivot. It does not rewrite your original files or
make the change atomic for Git or editors.

```sh
semaprax semantic-workspace-init ws paths.json
semaprax semantic-workspace-change-preview ws proposal.json     # read-only
semaprax semantic-workspace-change-evidence ws proposal.json > ev.json
semaprax verify-semantic-workspace-change-evidence ws proposal.json ev.json
semaprax apply-semantic-workspace-change-evidence ws proposal.json ev.json   # publishes
```

The order is always preview, evidence, verify, apply. Apply replays the
evidence under the workspace lock first; stale or failed input leaves the
workspace unchanged. The same four steps exist for structural changes
(`semantic-workspace-structural-change-*`) and for operations derived from a
declaration-level change (`semantic-workspace-operations-derive`,
`-change-proposal`, `-evidence`). For read-only questions use
`workspace-snapshot`, `workspace-graph`, `workspace-context`,
`workspace-impact` and `workspace-review`. The older `.wspatch` route
(`workspace-init`, `workspace-preview`, `workspace-apply`,
`workspace-patch-evidence`) also remains. Specs:
[Semantic Workspace v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-WORKSPACE-V1.md),
[Workspace Change v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-WORKSPACE-CHANGE-V1.md),
[Operations v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-WORKSPACE-OPERATIONS-V1.md).

## Keep candidates and images

For tool builders. A **candidate** is a proposed project revision kept as data.

| Command | Use it to |
| --- | --- |
| `project-image <manifest>` | Print the project's semantic image. |
| `project-image-store` / `-load` / `-verify` | Keep an image in a store and re-check it. |
| `project-symbol <manifest> <id>` | Read one symbol from the image. |
| `project-candidate-preview` / `-export` / `-restore` | Preview a change, export it as a capsule, restore it later. |
| `project-candidate-persist` / `-load`, `project-draft-persist` / `-load` | Store candidate and draft archives by digest. |
| `project-candidate-git-publish <manifest> <capsule> <approved-digest> <host-policy.json>` | Commit an approved candidate to Git under a host policy. This is the one publishing step. |
| `patch-receipt <project> render\|verify\|compare\|...` | Render and check receipts for a transaction and candidate digest. |

Run `semaprax help <command>` for each exact shape.

## Serve a project to tools

```sh
semaprax service . [--mcp]                          # JSON-RPC (or MCP) on stdin/stdout
semaprax serve-workspace semaprax.toml host-policy.json
semaprax serve-workspace-mcp semaprax.toml host-policy.json
semaprax serve <file> [--max-request-bytes N]
```

`service` authenticates one project at startup and answers queries and
transaction validation over line-delimited JSON-RPC 2.0, single client, local
only. `serve-workspace` speaks the image-agent protocol; its closed
host-policy file (`semaprax.workspace-host-policy.v1`) decides whether
candidates, diagnostics, builds, tests and Git commits are allowed. The client
cannot widen it. `serve-image`, `serve-candidates`, `serve-test-candidates`,
`serve-diagnostics` and `serve-diagnostics-tested` are narrower variants of the
same protocol. Specs:
[Service Transport v1](https://github.com/wavect/semaprax/blob/main/docs/PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md),
[Workspace session CLI](https://github.com/wavect/semaprax/blob/main/docs/WORKSPACE-SESSION-CLI-V1.md).

## Check assurance and proofs

```sh
semaprax assurance-policy app.spx --profile require-static
semaprax assurance-diff base.spx candidate.spx --profile require-static
semaprax assurance-manifest app.spx
semaprax project-assurance-manifest semaprax.toml
semaprax properties app.spx --max-cases 64 --seed 11
semaprax region-report app.spx
```

`assurance-policy` checks each obligation against a profile
(`require-static`, `allow-runtime-guard`, `allow-test-evidence`,
`report-only`). `assurance-diff` shows what a candidate changes.
`properties` generates bounded inputs from contracts and evaluates them
(scalar, effect-free functions only). `project-proof-check` runs an external
Lean or Z3 you name by absolute path against a project law; it needs
`--tool`, `--executable`, `--version-line` and `--host-profile`. See
[Laws and proofs](../language/laws.md).

## Verify a release you downloaded

```sh
semaprax release verify <release-dir>
semaprax doctor verify-release <release-dir> --trusted-root-sha256 <64-hex>
```

`release verify` reads `release-manifest.json`, `release-provenance.json` and,
if present, `release-signature-claim.json`. It recomputes the manifest digest
and re-hashes every archive the manifest names; nothing a document says about
itself is trusted. `doctor verify-release` requires complete signed material and
checks it against the root digest you pass (`SPX-Z707` on mismatch). Get that
digest from a channel you trust, not from the release directory. Missing
manifest: `SPX-Z705`. Verifying does not install anything; see
[Install](../getting-started/install.md).

## Check an audit capsule or a workflow

```sh
semaprax audit inspect capsule.json
semaprax audit verify capsule.json objects/ --require-role reviewer
semaprax audit diff a.json b.json
semaprax workflow validate workflow.json
semaprax workflow inspect workflow.json
semaprax workflow checkpoint checkpoint.json
semaprax workflow dispatch policy.json request.json
```

`audit` inspects, verifies and diffs evidence capsules offline. Verification
can check Ed25519 signatures against a trust roster you supply
(`--trust-roster`); nothing here signs or submits to a log. `workflow validate`
and `inspect` check a typed workflow graph. `workflow checkpoint` decodes a
checkpoint and prints its state; it never resumes anything. `workflow dispatch`
decides whether a request target is in a declared policy and records the
decision. All are read-only. See
[Audit Capsule v1](https://github.com/wavect/semaprax/blob/main/docs/AUDIT-CAPSULE-V1.md).

## Checklist

1. Commit `semaprax.lock` beside `semaprax.toml`.
2. Gate CI on `lock --compare <base.lock>`.
3. Run `query impact`, then `change preview`, then `review` before you apply.

Exact rules: [Project Lock v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-LOCK-V1.md),
[Project Dependency Resolution v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-DEPENDENCY-RESOLUTION-V1.md),
[Semantic Impact v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-IMPACT-V1.md),
[Semantic Review v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-REVIEW-V1.md),
[Unified CLI v1](https://github.com/wavect/semaprax/blob/main/docs/UNIFIED-CLI-V1.md).
