# Project v8 Promotion Receipt v1

Status: authority-free local evidence model; not promotion or hosted evidence.

Audience: release-gate implementers, evidence producers, and reviewers.

Project v8 Promotion Receipt v1 records one closed set of explicit,
caller-owned observations for the bounded `owned-data-api.v1` programme. It
does not execute a gate, inspect CI, discover a tool, attest a host, decide
support, publish a package, or promote a completion-matrix row.

The schema is `semaprax.project-v8-promotion-receipt.v1`. A receipt is one
canonical JSON line with one terminal LF and at most 1 MiB. Unknown, duplicate,
missing, surplus, reordered, noncanonical, nested-over-depth, or oversized data
rejects. Its digest is SHA-256 over the domain
`semaprax.project-v8-promotion-receipt.v1` plus a NUL byte and the complete
canonical receipt bytes. The digest is integrity data, not a signature,
provenance, or host attestation.

## Exact subject and artifact binding

The receipt binds one exact 40-byte lowercase hexadecimal commit and only the
Project schema/profile pair `semaprax.project.v8` / `owned-data-api.v1`. It
contains baseline and `display-rename` subjects. Each subject binds exact
Project revision, Workspace revision, Project graph, descriptor, npm-carrier,
and Rust-package SHA-256 digests plus this stable-ID inventory in declaration
order:

1. `frame.payload`
2. `frame.payload-maybe`
3. `frame.payload-result`

A display rename changes authenticated Project, Workspace, and graph identity.
Descriptor and package digests may also change because those artifacts embed
revision bindings. The receipt proves only the exact paired facts and preserved
stable-ID inventory; it does not by itself prove byte equality or behavioral
equivalence.

The artifact inventory is closed and ordered:

1. baseline descriptor
2. baseline npm carrier
3. baseline Rust package
4. display-rename descriptor
5. display-rename npm carrier
6. display-rename Rust package
7. browser-toolchain lock inventory
8. compatibility known-answer inventory

The first six digests must equal their subject bindings. The final two are
opaque digests of caller-owned canonical inventory bytes. They do not prove
tool provenance, installation, execution, or compatibility merely by being
present. Every gate row carries one domain-separated digest of the complete
artifact inventory to prevent cross-receipt splicing.

## Closed gate inventory

The canonical order contains exactly fifteen rows:

- manifest/descriptor/compatibility on Linux x86-64, macOS AArch64, and Windows
  x86-64 under Rust 1.88;
- installed npm consumption on those three platforms under Node 22 and
  TypeScript 5.8.3;
- external Rust SDK consumption on those three platforms under Rust 1.88;
- Linux interpreter/native-O0/O2/Core-Wasm equivalence;
- Linux Playwright 1.55 Chromium, Firefox, and WebKit consumption;
- Linux Clang ASan/UBSan; and
- Linux hostile carrier and settlement evidence.

Each row repeats the exact commit, Project schema, profile, platform, tool
profile, and artifact-inventory digest. The only admitted outcome is `passed`.
Failed, skipped, masked, cancelled, duplicated, missing, foreign-head,
cross-profile, foreign-tool, or reordered observations reject rather than being
normalized or sorted.

## Derivation and replay

`derive_project_v8_promotion_receipt` accepts only typed observation, subject,
and artifact values already owned by its caller. It validates the fixed
inventory before encoding. `parse_project_v8_promotion_receipt` validates the
bounded closed wire form and reconstructs its exact canonical bytes.
`replay_project_v8_promotion_receipt` independently derives from a fresh copy
of the explicit inputs and requires byte-identical receipt meaning. A changed
or self-consistently reminted receipt does not replay against the original
observations.

No local runner is supplied because existing test processes do not emit a
common authenticated observation format. Treating command output or a green
exit status as an observation would weaken this contract. A future evidence
producer must own its tools and artifacts, emit these exact facts, and remain
separately accountable for their provenance.

## Nonclaims

The receipt grants no filesystem, process, environment, network, registry,
release, signing, support, or publication authority. It is not proof that a
gate ran, that a host was genuine, that the named tool version produced an
artifact, that browser or sanitizer evidence is complete, that Project v8 is
supported, or that WP-15 is complete. Hosted exact-head execution and an
explicit support decision remain separate prerequisites.
