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
multi-file line-filter product. The release exit remains open because the
complete promotion matrix has not passed at one exact candidate commit.

Exit outcomes:

1. Make the blocking baseline, Product, browser, Project, public SDK, mobile,
   sanitizer, MSRV, and dependency-policy jobs green at one exact head.
2. Remove diagnostic masks from every claim required for release.
3. Re-run baseline and display-renamed Project consumers against the exact
   generated artifacts.
4. Confirm the line-filter product on hosted native and WebAssembly lanes and
   add the browser/runtime breadth claimed by the release.
5. Publish release notes that cite the exact commit and preserve all bounded
   non-claims.

The [v0.2 audit](COMPLETION-MATRIX.md#v02-product-exit-audit) is the acceptance
checklist.

## 0.3: ownership and fast development

Goal: make the language safer and faster to iterate on without widening public
ABIs prematurely.

### Language and ownership outcomes

- generalize unique ownership beyond the current bounded Copy, string, byte,
  and resource slices;
- implement borrow/lifetime inference, reborrowing, and escape analysis;
- make cleanup plans cover general control flow, nested aggregates, calls, and
  FFI with independently replayed exactly-once behavior;
- integrate regions/arenas and opt-in shared immutable ARC only after their
  proof models have executable language and runtime counterparts;
- define restricted raw-memory operations and an auditable `unsafe` policy;
- complete aggregate, `Option`, `Result`, matching, mutation, and generic
  interactions across interpreter, native, and Wasm lanes.

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
