# Roadmap

Status: living future-sequencing document; not implementation evidence.

Audience: contributors, maintainers, and project evaluators.

The roadmap orders future outcomes. It is not implementation status and does
not repeat completed milestone history. Use the [completion
matrix](COMPLETION-MATRIX.md) for current claims and the
[changelog](../CHANGELOG.md) for what changed.

SEMAPRAX follows risk rather than feature count. Stable semantic identity,
sound ownership, replayable change authority, and honest target boundaries take
priority over broad syntax or generated artifact volume.

## Current priority: finish the v0.2 product exit

The current codebase has a bounded multi-file calculator, Project agent
workflow, stable-ID JavaScript/TypeScript and unpublished Rust consumers, and a
multi-file line-filter product. The blocking promotion matrix is exact-head
green at `4cc03820c86e70527cb65c4b10ee3841c7af167d` in
[run 33259787886](https://github.com/wavect/semaprax/actions/runs/33259787886).
The release exit remains open on line-filter browser/runtime breadth,
intentional Rust publication, and final release notes.

Exit outcomes:

1. Confirm the line-filter product on hosted native and WebAssembly lanes and
   add the browser/runtime breadth claimed by the release.
2. Publish the Rust builder only through an intentionally supported entry
   point, or keep the release claim explicitly unpublished.
3. Publish release notes that cite the exact commit and preserve all bounded
   non-claims.

The [v0.2 audit](COMPLETION-MATRIX.md#v02-product-exit-audit) is the acceptance
checklist.

## Developer preview: public owned-data integrations

After the v0.2 release gate is closed, the next bounded product tranche is the
additive [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md). Its proposed
Project Manifest v8 profile keeps v1–v7 frozen and admits only scalar and
borrowed text/byte parameters with scalar or copied owned-byte results.

Sequence this tranche narrowly:

1. freeze and independently replay one target-neutral public API descriptor;
2. generate JavaScript/TypeScript and safe Rust consumers from that descriptor;
3. prove direct `Bytes`, then `Option<Bytes>` and `Result<Bytes, i64>`, with
   exact copy-out and settlement before host publication;
4. activate Project v8 only after both target consumers and all legacy
   preservation gates are independently green; and
5. validate one realistic multi-module project across interpreter, native,
   Wasm, installed npm, and compiler-free Rust consumption before any hosted
   or completion-matrix promotion.

Records, authored variants, nested algebraic data, owned UTF-8 strings,
allocator transfer, callbacks, async work, and general public aggregate ABIs
remain outside this preview. The versioned specification owns its exact
identifiers, admission, lifetime, compatibility, and promotion gates.

## 0.3: ownership and fast development

Goal: make the language safer and faster to iterate on without widening public
ABIs prematurely.

### Language and ownership outcomes

- generalize unique ownership beyond the current bounded Copy, string, byte,
  resource, flat owned-byte record, and flat owned-byte variant slices;
- use the bounded [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md) as the
  independently replayed proof foundation, then admit nested owned aggregate
  borrowing before general lifetime inference, mutable borrowing, and escape
  analysis;
- make cleanup plans cover general control flow, nested aggregates, calls, and
  FFI with independently replayed exactly-once behavior;
- integrate regions/arenas and opt-in shared immutable ARC only after their
  proof models have executable language and runtime counterparts;
- define restricted raw-memory operations and an auditable `unsafe` policy;
- extend aggregate, `Option`, `Result`, and matching beyond the exact
  [Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) profiles,
  including nesting, authored generics, non-Copy propagation, and public ABIs;
- complete mutation and generic interactions across interpreter, native, and
  Wasm lanes.

### Development-loop outcomes

- evolve the interpreter into a fast incremental execution lane;
- add source-level trace and diagnostic mapping;
- persist authenticated Project revisions without granting ambient filesystem
  authority;
- broaden context and impact edges beyond the current bounded call and
  workspace families;
- measure semantic-context cost and usefulness on representative repositories
  and actual model tokenizers.

Exit condition: representative owned applications pass the same success,
failure, cleanup, and contract corpus through the development, native, and
WebAssembly lanes, with stable source/graph migrations.

## 0.4: components, packages, and interoperability

Goal: turn bounded reports and private host evidence into a supported,
versioned ecosystem surface.

### Package outcomes

- interface-first manifests with resolver, lockfile, target matrix, capability
  closure, provenance, licenses, and reproducible artifact records;
- compatibility analysis over types, effects, contracts, ownership, and target
  availability;
- a package registry and offline cache model with explicit least authority;
- stable migration rules for language, graph, patch, package, and ABI schemas.

### ABI and host outcomes

- stable canonical and native ABIs for aggregates, resources, borrowed views,
  strings, errors, callbacks, and async work;
- supported C/C++, Rust, Java/Kotlin, Swift/Objective-C, JavaScript/TypeScript,
  and WIT consumers with conformance suites;
- WebAssembly Component Model publication and multi-runtime execution;
- replacement of private loader/host fixtures with intentionally public,
  reviewed APIs where appropriate;
- capability-limited plugin loading and hostile-plugin tests.

Exit condition: one versioned package is consumed from every supported host
language and target lane with reproducible builds, compatibility checks, and
no undocumented ambient authority.

## 0.5: concurrency and applications

Goal: demonstrate that verified shared meaning can support real applications
without pretending every platform is identical.

### Concurrency and services

- structured tasks, cancellation, cleanup, `Sendable`, and `Shareable` checks;
- deterministic effect handlers and test schedule replay;
- general command, filesystem, network, clock, and service I/O through explicit
  capabilities;
- server/edge packaging, observability, deployment diagnostics, and load tests.

### Application model

- typed state, actions, update functions, semantic view trees, navigation,
  localization, assets, accessibility, and lifecycle;
- accessible DOM/CSS and hydration for the web;
- supported Swift/Apple, Kotlin/Android, Windows, Linux, and desktop adapters;
- explicit platform blocks and custom accelerated rendering escape hatches;
- distributable artifacts with permissions, entitlements, manifests, and
  signing metadata while credentials remain outside compiler authority.

Exit condition: one shared application has maintained web, iOS, Android,
macOS, Windows, and Linux clients with declared platform differences and
representative hosted or device evidence.

## 1.0: validate the complete programming system

The 1.0 gate is the final product in the
[completion matrix](COMPLETION-MATRIX.md#final-validation-product), not a
version-number aspiration.

It requires one maintained offline-first product with:

- all six client platforms from the shared SEMAPRAX program;
- native notifications, secure storage, local databases, authentication, and
  background synchronization;
- native or WASI server execution;
- a custom accelerated visual;
- one C library, one JavaScript package, and one WebAssembly component;
- reproducible builds, compatibility and migration evidence, and representative
  CI/simulator/device execution;
- complete language safety, debugger/diagnostic, package, capability, and
  operations gates for the features the product uses.

No narrow report, generated fixture, or private platform adapter substitutes
for this maintained end-to-end proof.

## Research profiles after the core product

Economic-agent work remains optional and subordinate to the language's
authority model. The current injected-host policy and evidence core grants no
built-in provider transport, wallet, key, mainnet, or signing authority. Any
future profile must preserve explicit capabilities, approvals, custody
separation, idempotent settlement, private-data boundaries, and complete audit
traces without weakening the core product gates.
