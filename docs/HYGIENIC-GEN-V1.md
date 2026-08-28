# Typed Hygienic Generation v1

Audience: agent and tool authors, plus compiler contributors.

Status: locally evidenced, read-only, single-file. Schema
`semaprax.hygienic-gen.v1`. Diagnostic family `SPX-Y`.

## Command

```text
semaprax hygienic-gen <file> [--templates default-constructor,field-accessors] [--max-bytes N]
```

The library entry point is `hygienic::generate(&path, &options)`. Options are
validated before any work: the template selection must be non-empty,
duplicate-free, and drawn only from the closed registry; `max_bytes` must lie
in the shared agent-context bounds (`SPX-Y100` otherwise). The default
selection is the full registry and the default budget is 65536 bytes.

## Model

Generation is a typed AST-to-AST transform followed by a real verifier pass —
never text templating:

1. Parse and verify the base program with the ordinary pipeline. Any parse or
   verification diagnostic surfaces unchanged (no wrapper code); a failed base
   program generates nothing.
2. Build the admission inventory. Records admit only when they are
   non-generic, non-empty, and every field is `i64` or `bool`; resources,
   variants, interfaces, generic records, empty records, and non-scalar
   records exclude with stable reasons. Function bodies are scanned under one
   step budget for unsupported shapes (floats, record construction/update/
   projection, variant construction, match, `try`, generic calls, unsupported
   callees); excluded functions are reported but remain usable as admitted
   callee context for scans of other bodies.
3. Enforce hygiene before synthesis (`SPX-Y102`/`SPX-Y103` fail closed).
4. Synthesize typed `ast::Function` nodes whose tails are real
   `ConstructRecord` / `Project` expressions over zero literals.
5. Verify the combined program with the same verifier (`SPX-Y104` on any
   rejection) and project both programs through the real Graph module.
   Every generated function identity must resolve in the combined projection
   (`SPX-Y106`), proving graph visibility.

## Hygiene and derived names

The reserved prefix is `__gen_`. Existing program symbols may not use it
(`SPX-Y103`). Derived names are a pure function of the owning record's
persistent stable ID: `__gen_<h8>_default` and `__gen_<h8>_get_<field>` where
`h8` is the first four bytes of
`SHA256("semaprax.hygienic-gen.v1:name-digest.v1\0" || stable_id)` in
lowercase hex. Renaming a record while keeping its `@id` therefore keeps every
derived name, moving code changes nothing, and accessor names additionally
bind the field identifier they project. A derived name that collides with an
existing symbol or another derived name fails closed (`SPX-Y102`) instead of
mangling. Generated declarations carry auto identities
(`auto:<module>.<name>`, `explicit_id = false`) so no persistent namespace is
claimed.

Generated functions have no effects, no contracts, value-mode scalar
parameters, and canonical synthesized spans.

## Output

Canonical compact JSON with fixed key order, ending in an authenticated
envelope: the payload's final brace is trimmed and
`,"outer_sha256":"sha256:<64 hex>"}` is appended, where `outer_sha256` is
`SHA256("semaprax.hygienic-gen.v1:outer-digest.v1\0" || payload_bytes)` over
the full payload including its final brace. Key groups: `source`
(path, base revision, domain-separated source digest), `registry`,
`templates`, `limits`, `types` (total/admitted/excluded with closed reasons),
`functions` (total/admitted/excluded), `generated` entries (template, record,
record stable id, field, name, resolved id, `formatted_sha256` of the
canonical formatter output of the generated function, and an `ast` summary),
`budget` (`generated_total`/`generated_emitted`), `combined`
(base/combined Graph schema and revision, function-node delta), `truncation`,
and fixed-order `nonclaims`.

Byte-budget truncation drops generated entries from the canonical tail while
preserving prefix order, records `byte_budget` in `truncation.reasons`, and
keeps the JSON valid. If even the zero-entry envelope cannot fit, generation
fails closed with `SPX-Y105`. The bounded-output accounting charges nested
strings conservatively, so the emitted envelope is always within the
requested budget.

## Nonclaims

Fixed order: `no_unrestricted_textual_rewriting`, `no_macro_system`,
`no_cross_file_scope`, `read_only_no_source_mutation`,
`no_persistent_artifacts`, `no_target_execution`. Generation reads exactly
one source snapshot, writes nothing, executes nothing, and grants no ambient
authority.

## Evidence

`tests/hygienic_gen_v1.rs` pins the known-answer identities and formatted
digests, cross-run byte determinism, rename-with-same-id name stability,
move/comment stability, semantic field-order sensitivity, hygiene collisions
(`SPX-Y102`), reserved-prefix preemption (`SPX-Y103`), verifier passthrough,
closed exclusion reasons, template selection, budget truncation and the
fail-closed floor (`SPX-Y105`), source immutability, nonclaim surface, and
the CLI grammar. Unit tests in `src/hygienic.rs` cover registry closure,
option validation, digest derivation, envelope exactness, and domain
separation.
