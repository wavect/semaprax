# Audit Capsule v1

Audience: implementers wiring a capsule producer, reviewers auditing a
capsule, and anyone extending `src/audit_capsule.rs`.

Status: versioned manifest schema and registries, a canonical builder, a
structural verifier, a structural diff, machine-readable `nonclaims`,
independent replay of a `change` capsule against source, opt-in Ed25519
signature verification against a caller-supplied trust roster
(`signature_verification`), and a read-only `semaprax audit inspect|verify|diff`
CLI front whose `verify` verb now exposes that roster as `--trust-roster
<path.json>`. Cryptographic *signing* and real transparency-log submission
are `HUMAN_BLOCKED` -- neither exists here, and this document must not be
read as claiming otherwise.


## Scope and current state

[Issue #209](https://github.com/wavect/semaprax/issues/209) asks for one
independently verifiable capsule that unifies SEMAPRAX's source, semantic,
transaction, assurance, execution, model/tool, artifact, package, decision,
and publication evidence. This document and `src/audit_capsule.rs` provide
the **envelope**: a `semaprax.audit-capsule.v1` manifest, a content-addressed
object set referenced by plain SHA-256 digest, an association graph, a
redaction mechanism, a signature policy that verifies `ed25519-entry-v2`
cryptographically once a caller supplies a trust roster (opaque otherwise,
and always opaque for `sigstore-cosign-bundle-v0.3`), and a
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
- [`render_capsule`]: a canonical builder that assembles the same typed
  pieces [`ParsedCapsule`] carries into well-formed manifest bytes, sorting
  objects into canonical order itself. Before this, the module had a
  decoder and a verifier but no producer at all -- a real emitter had no
  in-crate way to assemble a capsule short of hand-writing JSON. Every call
  round-trips its own output through [`parse_capsule`] before returning, so
  it can never hand back bytes this module's own decoder would reject.
- [`diff_capsules`] and [`CapsuleDiff`]: a pure, read-only structural diff
  between two already-parsed capsules (added/removed/changed objects,
  associations, and signature roles, plus subject and profile changes) --
  the library half of issue #209's `audit diff`.

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
- **CLI surface: now wired, thinly.** `src/cli/audit.rs` adds
  `semaprax audit inspect|verify|diff` (`CommandId::Audit` in
  `src/cli/help.rs`, dispatched from `src/cli_driver.rs`). It is a pure
  adapter: `inspect` calls [`parse_capsule`] and prints the decoded
  structure; `verify` reads each non-redacted object's bytes from a
  caller-supplied `<objects-dir>/<object-id>` file (rejecting a
  traversal-shaped id before ever joining it into a path) and calls
  [`verify_capsule`];
  `diff` calls [`parse_capsule`] on two manifests and [`diff_capsules`] on
  the results. No rule lives in the CLI front -- every check is
  `audit_capsule`'s own, and the front's own diagnostic code (`SPX-Z920`) is
  used only for "this document could not be read at all," never for a
  decode or verification failure. `verify` also accepts an optional
  `--trust-roster <path.json>`: a JSON object mapping signer identity to a
  64-lowercase-hex-character Ed25519 public key, loaded into
  [`SignaturePolicyContext::identity_public_keys`] so `check_signature_policy`
  cryptographically verifies every signature in the capsule against it (see
  `signature_verification`). **`audit verify`'s report always states, in one
  of three plainly distinguishable ways, whether that cryptographic check
  actually happened: verified against a supplied roster, present but
  unverified (no roster, or a roster naming zero identities), or rejected**
  -- together with every capsule's own required `nonclaims`, printed in
  full, so a green `audit verify` run can never be mistaken for a stronger
  guarantee than it actually establishes. Profile composition (below)
  remains unimplemented, so there is nothing for a composed-capsule CLI verb
  to do yet.
- **`diff_capsules` compares parsed capsules, not files.** It never reads or
  re-verifies either side; `semaprax audit diff` performs those steps before
  calling it.

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
   no expired signature; then, only when the caller opts in by populating
   [`SignaturePolicyContext::identity_public_keys`] with a roster of
   already-trusted Ed25519 verifying keys, cryptographically checks every
   `ed25519-entry-v2` signature against it (`signature_verification`; see
   "Signatures" below). Leaving that roster empty -- the default -- keeps
   `signature` bytes opaque exactly as before.
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

### Building a capsule: `render_capsule`

[`render_capsule`] takes a profile, a subject map, and slices of
[`ObjectRef`]/[`AssociationEdge`]/[`SignatureEntry`] plus an optional
[`TransparencyEntry`], and returns canonical manifest bytes. It sorts
`objects` into ascending order by `id` itself -- a builder's job is to
*produce* canonical bytes, not merely to demand the caller already supplied
them in order -- and rejects a `subject` whose key set does not exactly
match the profile's [`Profile::subject_keys`]. Before returning, it
round-trips its own output through [`parse_capsule`], so a caller can never
receive rendered bytes that this module's own decoder would refuse to
accept back.

### Diffing two capsules: `diff_capsules`

[`diff_capsules`] takes two already-`parse_capsule`d [`ParsedCapsule`]
values and returns a [`CapsuleDiff`]: profile/subject changes, added and
removed object ids, per-id [`ObjectChange`] (digest/type/schema change vs.
a redaction-only change), and added/removed associations and signature
roles. It is pure data comparison -- it never re-verifies either capsule
and never reads a file. `semaprax audit diff` (`src/cli/audit.rs`) is that
wrapper: it independently `parse_capsule`s each manifest first and never
calls `verify_capsule` on either side, so a clean diff is not evidence that
either capsule is itself valid -- run `audit verify` on each side for that.

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

**Does prove, when the caller opts in:** for an `ed25519-entry-v2` signature
whose identity appears in a caller-supplied
[`SignaturePolicyContext::identity_public_keys`] roster,
`signature_verification` cryptographically checks the `signature` bytes
against that already-trusted public key -- a real Ed25519 verification, not
a policy heuristic. This needs no signing key, only a verifying key the
caller already holds, which is why it is not `HUMAN_BLOCKED` the way
producing a signature is (see `src/release_provenance.rs` #168). Leaving
the roster empty -- the default, and the only behavior before this
capability existed -- keeps `signature` bytes fully opaque.
`semaprax audit verify --trust-roster <path.json>` wires a roster in from
the command line; omitting the flag, or pointing it at a roster naming zero
identities, preserves the empty-roster default exactly, and the CLI's own
report says explicitly which of the two happened (see "CLI surface" above).

**Signed entry metadata (issue #577).** Role, identity, algorithm, and
expiry are policy inputs, so a strict roster accepts them only when the
signature covers them. An `ed25519-entry-v2` signature is an Ed25519
signature over a per-entry, domain-separated preimage
(`signature_verification::entry_signable_bytes`):

```text
semaprax.audit-capsule.v1/signature-entry/ed25519-entry-v2 LF
{"algorithm":…,"identity":…,"not_valid_after_unix_seconds":…,"role":…} LF
<capsule payload: canonical manifest with `signatures` set to [] , LF-terminated>
```

The metadata line is sorted, compact JSON for this one entry; only the
entry's own `signature` bytes are left out. Each entry is signed
independently, so an early signer never needs to know later signatures,
and a genuine multi-role decision needs one signature per role. Relabelling
a role, extending an expiry, moving a signature to another identity (even
one mapped to the same key), copying it into another role, or changing any
covered payload field all fail verification. The legacy `ed25519-raw-v1`
preimage was the payload alone and authenticated none of that metadata, so
under a non-empty roster it is refused with an instruction to re-sign as
`ed25519-entry-v2` rather than reinterpreted; with no roster it remains a
recognized, opaque, policy-only entry exactly as before.

**Does not prove, ever:** that a `sigstore-cosign-bundle-v0.3` signature is
genuine -- checking a Sigstore bundle needs Rekor, which needs network
access this module must never use. **`HUMAN_BLOCKED: a real Sigstore/cosign
bundle verifier (the same gap #168 documents) is required before that
algorithm can be trusted, and producing any signature at all still needs a
signing key, keyless-signing identity, or publish authority this repository
does not have.`**

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
  identity?) is established only for an `ed25519-entry-v2` signature whose
  identity a caller-supplied trust roster covers -- see "Signatures" above.
  With no roster (the default) or for `sigstore-cosign-bundle-v0.3`, it is
  explicitly **not** established. No capsule is produced by this repository
  today either way: signing itself remains `HUMAN_BLOCKED`.
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


## Machine-readable `nonclaims` (required)

Every `semaprax.audit-capsule.v1` manifest carries a required, non-empty,
canonically ordered `nonclaims` array drawn from the closed vocabulary
`src/audit_capsule/nonclaims.rs` defines in `KNOWN_NONCLAIMS`. It states, in
a form a downstream tool can act on, exactly what the capsule does **not**
establish.

The field exists because a capsule travels. Its whole purpose is that
someone who was not present can verify it later, elsewhere, with no access
to this repository -- and a disclaimer recorded only in this document does
not travel with the bytes. Without the field, a downstream reader who never
reads this page would see a fully green `verify_capsule` report and
reasonably, and wrongly, conclude the capsule's signatures had been
cryptographically checked.

`ALWAYS_REQUIRED_NONCLAIMS` applies to every capsule regardless of profile
or contents:

| Nonclaim | What it denies |
| --- | --- |
| `signatures-not-cryptographically-verified` | Present on every capsule unconditionally, even one whose signatures a caller did cryptographically check: this repository still has no signing key, keyless-signing identity, or Sigstore bundle verifier (issue #168 remains open), and a `sigstore-cosign-bundle-v0.3` signature is never verifiable regardless of caller configuration. With no trust roster supplied to [`check_signature_policy`] (the default), a forged signature naming an approved identity is not detected at all; see "Signatures" above for what a caller-supplied `ed25519-raw-v1` roster now catches. |
| `transparency-inclusion-not-independently-confirmed` | No transparency log was contacted. Only a caller-supplied entry's internal consistency was checked. |
| `evidence-is-not-authorization` | Verification authorizes nothing: not publication, execution, signing, tagging, or deployment. |
| `local-evidence-only` | Everything referenced was produced on a private developer machine. This is not evidence of a hosted CI run, a physical-device run, or a production deployment. |

One further nonclaim is derived from the capsule's own structure: a capsule
with any redacted object must also declare
`redacted-objects-withhold-facts`.

### Why a reader cannot lose them

`check_nonclaims` does not trust the declared list. It re-derives, from the
capsule's own structural facts and sharing no data with the declared array,
the set this exact capsule is obliged to carry, and fails closed with
`SPX-Z908` when any is missing. Deleting
`signatures-not-cryptographically-verified` to make a capsule look stronger
does not yield a weaker-but-valid capsule; it yields one that no longer
verifies at all. A capsule may declare *more* nonclaims than required --
being more modest is always admitted -- but never fewer, and never one
outside the closed vocabulary. `CapsuleVerificationReport::nonclaims`
carries them back out of verification as data, so a caller rendering a
report cannot present a green result without them.

Unsorted, duplicated, empty, and invented `nonclaims` are all rejected at
parse time (`SPX-Z908`); the list is never silently repaired, because two
orderings of one set would otherwise give one capsule two digests.

## Independent replay against source (`change` profile)

Every other check in this module is *structural*: it proves a capsule is
internally consistent and that its retained objects still hash to the
digests it records. That is necessary and insufficient. A producer can
hand-write a flawless capsule whose subject names a `source_digest`,
`root_digest`, and `revision` that no source tree ever produced, and every
structural check passes -- because every structural check only ever compares
the capsule against itself.

`src/audit_capsule/change_replay.rs` closes that gap for the `change`
profile:

- `derive_change_identities` recomputes every identity from source text
  alone, via `crate::parse`, `crate::verify::verify`,
  `crate::format::canonical`, `crate::graph::revision`, and
  `crate::graph::to_json`. It refuses source that does not parse or no
  longer passes verification.
- `emit_change_capsule` builds a capsule whose subject was re-derived rather
  than asserted. The caller supplies only the evidence objects the change
  itself owns (its semantic transaction, its assurance manifest); the
  emitter derives the canonical source projection and the semantic graph
  document, their digests, and the `derived_from` edge between them. Object
  ids `derived-a-source-projection` and `derived-b-program-root` are
  reserved, so a supplied object cannot shadow a derived one.
- `verify_change_capsule_against_source` runs `verify_capsule` first, then
  recomputes every identity from the source under test and refuses on any
  disagreement with `SPX-Z909`. Toolchain drift is checked before the
  identities, since a different compiler can legitimately derive a different
  revision, and reporting that as source drift would mislead.
- Replay selects the retained bytes to compare by **object type**, not by
  the emitter's conventional ids (issue #583). The unique `source-projection`
  and `program-root` objects the structural check already requires are
  resolved from the parsed capsule's type inventory, their declared schema
  must match the one the current source renders, and their retained bytes
  must equal the fresh canonical projection and graph document under
  whatever id each actually carries. Renaming an object therefore never
  moves it outside replay, an unrelated object occupying a conventional id
  is never compared as a source-derived one, and a redacted source-derived
  object stays reported as unavailable rather than replayed.

This mirrors `verify_certificate_against_source` in
`src/assurance_manifest/proof_certificate/verify.rs`, for the same reason: a
document's self-reported fields are the claim under test, never the evidence
for it.

`change` is the only profile with such a replay, and deliberately so. Its
subject is the only one whose every identity is a pure, deterministic
function of bytes already in hand. An `agent-run` subject names a
`session_id` only a live invocation can attest; a `release` subject names a
`release_tag` and `commit` whose authority lives in Git and in a signing
identity this repository does not have. Replaying those would mean inventing
evidence.

### Still no authority, and no new reach

A successful replay proves the capsule's identities match that exact source.
It is not permission to publish, tag, deploy, or sign anything, and nothing
in this module performs such an action. `current_source` arrives as a string
the caller already read: replay takes no `Path`, opens no file, spawns no
process, and reaches no socket, so it cannot become a route to reading
something the caller was never authorized to read. A test asserts this
against the module's own source text rather than claiming it by inspection.

### Subject key change

`SUBJECT_KEYS_CHANGE` binds `compiler_version` alongside `source_digest`,
`root_digest`, and `revision`. Both the root digest and the revision are
deterministic functions of the source *and* the toolchain, so a capsule that
recorded them without naming the compiler could not be replayed without
silently assuming one.

## What is still not implemented

Repeated here so a reader of this section alone is not misled: there is no
cryptographic signing, no transparency-log submission or verification, no
profile composition, and no replay for the `agent-run` or `release`
profiles. `semaprax audit inspect|verify|diff` (`src/cli/audit.rs`) is now
wired, but it is a thin, purely structural front over the checks above --
running it changes none of these nonclaims.
