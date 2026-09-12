# Audit Capsule v1

Status: versioned manifest schema, object-type/relation/role/algorithm
registries, and an independent structural verifier. Cryptographic signing and
real transparency-log submission/verification are `HUMAN_BLOCKED` -- neither
exists in this repository, and this document must not be read as claiming
otherwise.

Audience: implementers wiring a capsule producer, reviewers auditing a
capsule, and anyone extending `src/audit_capsule.rs`.

## Scope and current state

[Issue #209](https://github.com/wavect/semaprax/issues/209) asks for one
independently verifiable capsule that unifies SEMAPRAX's source, semantic,
transaction, assurance, execution, model/tool, artifact, package, decision,
and publication evidence. This document and `src/audit_capsule.rs` implement
the **envelope**: a `semaprax.audit-capsule.v1` manifest, a content-addressed
object set referenced by plain SHA-256 digest, an association graph, a
redaction mechanism, a non-cryptographic signature policy, and a
non-cryptographic transparency-inclusion check -- plus the hostile-input
tests issue #209 names.

**What is genuinely new here** (nothing on `main` at the audit baseline
`ae25c6a49dc09ec4613c7a6f52a27daa69dcd3f3` implements any of this):

- The `semaprax.audit-capsule.v1` schema itself: profile, subject, objects,
  associations, signatures, transparency.
- [`KNOWN_OBJECT_TYPES`], [`KNOWN_RELATIONS`], [`KNOWN_SIGNATURE_ROLES`],
  [`KNOWN_SIGNATURE_ALGORITHMS`] -- the closed vocabularies issue #209 asks
  for as an "object-type registry."
- Per-profile required-object-type sets and subject-key sets for the three
  named profiles (change, Agent run, release).
- Independent parse/verify: [`parse_capsule`], [`check_required_object_types`],
  [`check_associations`], [`check_subject_bindings`], [`check_object_bytes`],
  [`check_signature_policy`], [`check_transparency`], and the [`verify_capsule`]
  entry point composing all six.
- The `transparency_leaf_digest` / `capsule_digest` split (see below) -- a
  genuinely new design decision this work had to make, not copied from any
  existing schema.

**What existing modules already deliver, reused here only by reference (all
four are read-only from this module's perspective; none of their files are
modified by this change):**

| Existing module | What it already gives a capsule |
| --- | --- |
| `src/release_provenance.rs` (#168) | `semaprax.release-manifest.v1` / `semaprax.release-provenance.v1` / `semaprax.release-signature-claim.v1`, byte-exact digest binding, and the exact "opaque signature, structurally validated, never cryptographically verified" pattern this module's [`SignatureEntry`] copies. |
| `src/model_call_receipt/` (#180) | Per-call receipts, domain-separated commitment digests, redaction (`audit_view`), replay, and reconciliation -- a capsule references a receipt's `RECEIPT_SCHEMA` document by digest and never reinterprets its fields. |
| `src/job_evidence.rs` (#192) | An append-only, hash-chained, replayable job lifecycle log -- referenceable as a `job-evidence-log` object. |
| `src/live_invocation/` (#108/#177) | The causal journal a receipt is itself projected from; out of this module's scope entirely, referenced only transitively through a model-call-receipt object if a real integration chooses to. |
| `src/assurance_manifest.rs` and `src/assurance_manifest/` | The per-obligation assurance lattice; referenced as an `assurance-manifest` object. |

None of these are re-hashed with a capsule-specific domain tag, re-parsed for
their own internal meaning, or otherwise turned into a second, competing
interpretation. A capsule's [`ObjectRef::digest`] is the *plain* SHA-256 of
the referenced object's exact bytes -- the same digest an independent
`sha256sum` invocation over those same bytes would produce.

## What this implementation does not do (by design, not by oversight)

- **No profile composition.** Issue #209 asks that capsule profiles "allow
  composition." This implementation defines three profiles
  ([`Profile::Change`], [`Profile::AgentRun`], [`Profile::Release`]) but does
  not implement embedding one capsule's objects inside another (e.g. a
  release capsule that composes several prior change capsules). This is a
  scope decision for a follow-up change, not a blocked dependency: it needs a
  concrete design for how a composed capsule's own required-object-type
  checking and association graph behave across the boundary, which issue
  #209 leaves unspecified.
- **No CLI surface.** Issue #209's implementation sequence asks for
  `semaprax audit verify`, `audit inspect`, and `audit diff` subcommands.
  Wiring those touches `src/cli_driver.rs`, a large, shared, actively-worked
  file outside this change's file lease (`src/audit_capsule.rs`, this
  document, and its tests only). The verifier functions
  ([`verify_capsule`], [`parse_capsule`], and the individual `check_*`
  functions) are the library surface a thin CLI wrapper would call; adding
  that wrapper is follow-up work for whoever owns `cli_driver.rs` next.
- **No `audit diff`.** Comparing two capsules structurally (which objects
  were added/removed/changed) is not implemented; [`ParsedCapsule`]'s public
  fields are enough to build one, but no diff algorithm exists here yet.

## Schema: `semaprax.audit-capsule.v1`

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Always `semaprax.audit-capsule.v1`. |
| `profile` | string | One of `"change"`, `"agent-run"`, `"release"`. |
| `subject` | object | Exactly the keys [`Profile::subject_keys`] names for this profile -- the change/run/release this capsule is about. |
| `objects` | array of object envelopes | See below. Must be in ascending order by `id` (canonical ordering is part of this schema, not a rendering nicety) with no duplicate `id`. |
| `associations` | array of `{from_id, relation, to_id}` | `relation` is one of [`KNOWN_RELATIONS`]. Every id must name an object actually present; the graph must be acyclic. |
| `signatures` | array of role-tagged entries | At most one entry per role; `role` is one of [`KNOWN_SIGNATURE_ROLES`], matching issue #209's "Proposer/reviewer/validator/approver/publisher decisions" list exactly. |
| `transparency` | object or `null` | Optional; see "Transparency" below. |

### Object envelope

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | Unique within the capsule. |
| `object_type` | string | One of [`KNOWN_OBJECT_TYPES`]. Deliberately excludes `"audit-capsule"` itself, so a capsule can never embed another capsule as one of its own objects. |
| `schema` | string | The exact schema id the referenced object's own owning module defines (e.g. `"semaprax.model-call-receipt.v1"`). Opaque and unvalidated here -- existing object semantics remain owned by their original schemas. |
| `digest` | `sha256:<64 lowercase hex>` | The plain SHA-256 of the referenced object's exact bytes. |
| `redacted` | boolean | See "Redaction" below. |
| `redaction_reason` | string or `null` | Required (and non-empty) iff `redacted == true`; must be `null` iff `redacted == false`. |
| `binds` | object of string→string | A subset of the capsule's `subject` keys this object claims to be bound to, with its own claimed value for each. |

### Profiles

| Profile | Required object types | Subject keys |
| --- | --- | --- |
| `change` | `source-projection`, `program-root`, `semantic-transaction`, `assurance-manifest` | `source_digest`, `root_digest`, `revision` |
| `agent-run` | `agent-definition`, `agent-deployment`, `agent-invocation`, `model-call-receipt` | `session_id`, `deployment_digest`, `target_digest` |
| `release` | `release-provenance`, `release-signature-claim`, `artifact`, `package-manifest` | `release_tag`, `commit`, `artifact_digest` |

Each required type must appear **exactly once**; a capsule may still carry
additional objects of other [`KNOWN_OBJECT_TYPES`] types beyond its profile's
required set.

## Verification

`src/audit_capsule.rs` provides, in the order [`verify_capsule`] runs them:

1. [`parse_capsule`]: independent, from-bytes structural validation (exact
   key sets, closed vocabularies, `sha256:` wire forms, ascending object
   order, no duplicate ids, at most one signature per role). Also enforces
   [`MAX_MANIFEST_BYTES`], [`MAX_OBJECTS`], and [`MAX_ASSOCIATIONS`] before
   any heavier work, so an oversized capsule is rejected at the cheapest
   possible check.
2. [`check_required_object_types`]: the profile's required object-type set is
   present exactly once each (missing / extra).
3. [`check_associations`]: every edge names a real object (no dangling
   reference) and the graph is acyclic, including a one-node self-loop
   (cyclic).
4. [`check_subject_bindings`]: every object's `binds` agrees with the
   capsule's own `subject` (stale).
5. [`check_object_bytes`]: every retained object's plain SHA-256, recomputed
   from caller-supplied bytes, matches its declared `digest` (substituted);
   every redacted object has no bytes supplied (a redaction leak, if it
   does).
6. [`check_signature_policy`]: required roles present, no revoked identity,
   no expired signature. Never decodes or verifies `signature` bytes
   themselves.
7. [`check_transparency`]: if a `transparency` entry is present, its
   `leaf_digest` matches [`transparency_leaf_digest`] recomputed from the
   manifest under test, its `log_id` is one of the caller's trusted logs, and
   its `observed_checkpoint_size` is not older than the caller's trusted
   minimum.

### The `capsule_digest` / `transparency_leaf_digest` split

A naive design would check `transparency.leaf_digest == sha256(the exact
manifest bytes)`. That is unsatisfiable by construction whenever the
manifest itself carries the `transparency` field: the leaf digest would have
to commit to bytes that include itself, a fixed-point requirement no hash
function lets you satisfy honestly. [`transparency_leaf_digest`] instead
hashes the manifest with `transparency` replaced by `null` (and object keys
canonically sorted) -- exactly the payload a real transparency log would
have received at submission time, before its inclusion proof was appended
back onto the capsule. [`capsule_digest`] remains the plain SHA-256 of the
literal bytes handed to it, an identity for one exact byte-for-byte capsule
version, used for nothing else in this module.

### Portability

Every function in `src/audit_capsule.rs` takes only in-memory byte slices,
string maps, and closed-vocabulary values -- never a `Path`, a socket, or a
subprocess handle. `src/audit_capsule/tests.rs` verifies every fixture
(well-formed and hostile) purely from `&[u8]`/`BTreeMap<String, Vec<u8>>`
values constructed in the test itself; verifying a capsule needs no compiler
invocation, no original build machine, and no network access -- only
`serde_json` and `sha2`, ordinary library dependencies with no ambient
authority.

## Redaction

An object with `redacted: true` carries no bytes in the content-addressed
object set; `redaction_reason` is a mandatory, non-empty, human-readable
explanation. [`verify_capsule`]'s [`CapsuleVerificationReport::unavailable_claims`]
lists `(object_id, object_type, redaction_reason)` for every redacted object
explicitly, so a verifier can see exactly which required facts are
unavailable rather than a green summary that silently omits them. Supplying
retained bytes for an object marked redacted is rejected
([`check_object_bytes`]) rather than silently accepted, because that would
leak exactly what the redaction was meant to withhold.

## Signatures: what is and is not proved

**Does prove (once a real signer exists):** which role-tagged identity
claims to have produced this exact capsule manifest, whether that claim is
expired or revoked per caller-supplied policy, and that at most one identity
claims each role (so a proposer's signature can never be mistaken for an
approver's).

**Does not prove:** that `signature` bytes are a real cryptographic
signature produced by the claimed identity's private key. Exactly as
`src/release_provenance.rs` (#168) documents for release signing, verifying
that honestly needs a signature-verification implementation (Sigstore/cosign
bundle verification, or raw Ed25519/ECDSA point arithmetic) that needs a
cryptography dependency this change is not permitted to add, and no signing
key, keyless-signing identity, or publish authority exists in this
repository or session. **`HUMAN_BLOCKED: pairing [`check_signature_policy`]
with a real external verifier (the same `cosign verify-blob`-shaped gap
#168 documents) is required before "signed" can be claimed for a capsule.**

## Transparency: what is and is not proved

**Does prove:** that a caller-supplied inclusion record is internally
consistent with the exact manifest under test, and not older than a
caller-supplied trusted checkpoint size.

**Does not prove:** that any real, independently operated transparency log
(e.g. Sigstore's Rekor) actually accepted this capsule's digest.
[`check_transparency`] never contacts a network-reachable log.
**`HUMAN_BLOCKED: submitting a capsule digest to a real transparency log and
verifying its returned signed checkpoint requires network access this
module does not have and does not implement.`**

## Nonclaims

Integrity, authenticity, provenance, and authority remain four separate
claims, never collapsed into one green summary:

- **Integrity** (do these bytes match what was recorded?) is what
  [`check_object_bytes`] and [`check_subject_bindings`] give: a retained
  object's bytes are independently recomputed and compared, never trusted
  from an embedded field.
- **Authenticity** (were these bytes produced by the claimed signing
  identity?) is explicitly **not** established by anything in this module --
  see "Signatures" above. No capsule is cryptographically signed today.
- **Provenance** (what exactly is bound: which objects, which subject, which
  associations) is what a successfully parsed and structurally verified
  capsule records.
- **Authority**: a successfully verified capsule proves exactly what its
  retained objects' own schemas already prove, bound together and
  unmodified since assembly. It never authorizes a release, widens a
  budget, replays an effect, or approves itself -- nothing in
  `src/audit_capsule.rs` executes, publishes, or spawns anything, and
  possessing (or fully verifying) a capsule is not the same fact as a human
  or policy having granted permission for whatever the capsule describes.
