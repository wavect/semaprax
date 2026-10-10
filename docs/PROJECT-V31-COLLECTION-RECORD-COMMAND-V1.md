# Project v31 Collection-Record Command v1

Status: Project routing, compiler carriers and owning regression source integrated; current-head execution qualification is pending.

Audience: Project profile authors, compiler contributors, and reviewers.

This document defines the additive table profile `language-command-io.collection-record.v1` and its semantic schema `semaprax.project.v31`. Recognizing the manifest profile does not establish that ordinary Project linking, backend execution, or a host route admits the new carriers.

## Manifest contract

The profile uses the v30 command input `argv-utf8+stdin-stream.v1` and the same exact sorted capability inventory: `process.args.read`, `process.stderr.write`, `process.stdin.read`, and `process.stdout.write`. The authored entry and selected command remain separate explicit stable-ID roots with `fn() -> i64` ABIs. The route adds no authority and retains the existing native64 target and stream limits.

Project v30, `language-command-io.owned-data.v1`, remains frozen to its owned-leaf contract. This successor selects a separate schema so nested-record support cannot widen v30.

## Intended carrier boundary

The proposed internal carrier is an explicitly identified, monomorphic, acyclic record tree without invariants. Records may contain Copy scalars, `string`, `Bytes`, nested records, and only the existing admitted `Vec<T>` element shapes; at least one vector is required. The existing bounds are at most 64 record levels, 4096 visited fields, and 256 owned leaves. Existing vector element/capacity and byte/work limits remain in force.

Names, generated identities, schema-only declarations, and source provenance grant no runtime authority. Arbitrary recursive variants, dynamic-origin values, immutable record updates, and a wrapped `Ready<Report>` outcome are outside this tranche. Public projections and native providers do not gain a new ABI.

## Evidence status

This source batch adds profile/schema metadata, separate command admission and backend/interpreter routes, and owning Project cases for execution plus older-profile refusal before source cropping. The ordinary compiler carrier contract is [Nested owned collection records v1](NESTED-OWNED-COLLECTION-RECORDS-V1.md). Nested carrier execution and native settlement remain pending focused current-head qualification. Source presence alone is not a completion claim.

Use `semaprax help language author:collection-records` for bounded profile guidance. The response-codec selector has its own card, `author:json-collection-response`; neither a schema nor a generator selection grants runtime admission.
