# Packaged TypeScript workflow SDK v1

Status: the explicitly provisioned local Unix package installation and workflow
gate passed 1/1 on 2026-09-03. This is focused current-tree evidence, not hosted,
cross-platform, release, registry-publication, or full-quality evidence.

Audience: generated-client consumers, embedding hosts, package reviewers, and
evidence-runner authors.

## Purpose

The `@semaprax/agent-workflow` package supplies the bounded
`function_signature_review_publish_v1` driver for Node.js 22 or later. It
composes a generated v5 TypeScript codec through one caller-supplied transport;
it does not contain a compiler, select a workspace, open a process, or acquire
source-publication authority.

The driver preserves the workflow's two authority domains. `runReview` accepts
a review codec, one review-session transport, and a typed target, ordered scalar
signature parameters, and host failure classifier. It executes the closed
thirteen read, candidate, validation, test, review, coverage, and recovery
steps, then returns a closed handoff without approval. `runPublish` accepts
that handoff, a publish codec, a distinct publish-session transport, and an
independent publication-inspection callback. Only the host that created the
second session may attach a fixed Git target and approve the exact reviewed
candidate.

## Package contract

The package identity is `@semaprax/agent-workflow` version `0.1.0`. It is an
ES module for Node.js 22 or later, has no runtime dependency or lifecycle
script, and exposes only its closed root entry. Its repository inventory is
exactly `README.md`, `package.json`, `src/index.ts`, `tsconfig.json`,
`dist/index.js`, and `dist/index.d.ts`. A fresh strict TypeScript 5.8.3 compile
of the source must reproduce both checked distribution files byte for byte.
The packed inventory is exactly the README, manifest, and two distribution
files. The manifest maps both `import` and `types` to those distribution files
and rejects ambient fallback entry points.

The public driver consumes a generated codec with these immutable bindings:

- `PROTOCOL` is `semaprax.image-agent-protocol.v5`;
- `CLIENT_CONTRACT_REVISION` is the producer-emitted client revision;
- `WORKFLOWS` contains the selected workflow and selected-profile revision;
- `request` creates one method-bound JSON-RPC request; and
- `decodeTyped` authenticates and decodes the response for the same request.

Before either workflow phase, the installed consumer fetches
`protocol/capabilities` and `protocol/client` from that exact live stdio
session. Candidate diagnostics and the fixed test policy must match the
host-policy file. The generated `CLIENT_CONTRACT_REVISION` and selected-profile
revision must equal the constants in the live client response. Review and
publish revisions are bound separately; equality between the two profiles is
not assumed.

The transport has one nonempty `sessionId` and one `exchange(frame)` operation.
The frame is the complete request string and the result is the complete
response string. A transport grants no authority merely by implementing this
shape. The embedding host remains responsible for framing, session lifetime,
tool selection, and every filesystem, process, network, and publication
capability.

The driver fails closed on a missing or foreign workflow, protocol or client
revision mismatch, wrong phase/method order, malformed response, stale binding,
nonpassing validation or test, incomplete source review or analysis coverage,
tampered handoff, publication precondition mismatch, and incomplete or
contradictory receipt. It does not retry a commit. A result lost after a ref
update remains publication-uncertain and requires host inspection.
Compiler repair options remain empty throughout this scalar workflow. A
semantic-review rejection alone carries the typed transition option
`start_new_review_with_different_intention`; malformed transport and uncertain
publication failures carry no transition option. That option is guidance for a
new host-authorized review session, not an automatic repair or retry.

## Focused installed-package gate

The ignored integration test
`installed_typescript_sdk_drives_review_and_separately_approved_publish` in
`tests/image_packaged_typescript_workflow_v1.rs` is the owning executable gate.
It requires absolute `NODE`, `NPM_CLI`, `TSC_CLI`, and `SEMAPRAX_TEST_GIT`
paths. The TypeScript compiler must report exactly 5.8.3 and Node.js must be at
least version 22.

The gate invokes both JavaScript CLIs through the selected Node executable. It
clears Node and npm configuration, supplies empty user and global npmrc files,
uses a private cache, disables lifecycle scripts, audit, funding, and workspace
discovery, and selects npm offline mode. It then:

1. checks the production package's exact regular-file inventory and manifest;
2. compiles the generated review and publish codecs and a strict consumer;
3. runs `npm pack --json`, checks its one-row report and independently hashes
   the tarball;
4. installs only that local tarball through a closed lockfile and `npm ci`;
5. proves package-name resolution selects the installed regular files and that
   their bytes equal the packed inputs;
6. drives review through one real v5 stdio session and publication through a
   distinct explicitly approved v5 stdio session backed by an isolated local
   bare SHA-256 Git repository;
7. authenticates the handoff, source preservation, commit receipt and Git
   old/new/parent/tree/manifest/source objects plus the unrelated executable
   entry and mode;
8. rejects duplicate publication and malformed or structured failure input,
   including the semantic-rejection transition option without compiler repair;
   and
9. drops a real commit response after the fixed ref moves and requires the
   driver to return terminal `publication_uncertain` with no blind retry.

```sh
NODE=/absolute/node \
NPM_CLI=/absolute/npm-cli.js \
TSC_CLI=/absolute/typescript/lib/tsc.js \
SEMAPRAX_TEST_GIT=/absolute/git \
cargo test --locked --offline -p semaprax \
  --test image_packaged_typescript_workflow_v1 \
  installed_typescript_sdk_drives_review_and_separately_approved_publish \
  -- --ignored --exact --nocapture --test-threads=1
```

Missing or mismatched provisioned tools fail the selected gate; they do not
turn it into a skip. Packing, installing, resolving, and loading the package do
not invoke the SEMAPRAX or TypeScript compiler. The end-to-end gate separately
spawns the provisioned SEMAPRAX binary as two `serve-workspace` hosts, and the
strict source-to-distribution and consumer checks invoke the provisioned
TypeScript compiler.

The focused gate passed on the working tree on macOS arm64 with Node 24.3.0,
npm 11.4.2, TypeScript 5.8.3, and Apple Git 2.50.1. This is a local development
result rather than a clean exact-commit evidence bundle, release result, or
transferable later-head claim.

## Authority and blind spots

The package and its handoff carry no authority. npm filesystem/process access,
the stdio compiler process, the review policy, the publish policy, the Git
executable and repository, and approval all belong to the test host. Review has
no source-commit grant. Publish can reach only the fixed startup-selected Git
ref and exact approved candidate through the separately configured host.

Installed-package evidence may describe `generated_artifacts` as partial for
this exact package and `external_consumers` as partial for this exact local
consumer. It does not rewrite the workflow's embedded analysis-coverage
reports, where those areas remain `not_inspected`. Deployment configuration,
generated-file provenance beyond the bound package inputs, external API and
provider behavior, broader runtime environments, native/Wasm equivalence, and
other consumers remain uninspected.

Offline npm flags, an empty npm configuration, and disabled lifecycle scripts
are not operating-system network confinement. This gate does not establish npm
registry publication, signing, release provenance, browser support, hosted or
cross-platform support, remote Git publication, physical durability,
multi-writer atomicity, automatic repair, cancellation, deduplication, retry,
exactly-once delivery, full quality, programme completion, or support for a
workflow other than the named scalar signature workflow.
