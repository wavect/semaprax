# Authentication host v1

Status: private Rust embedding profile for password hashing and session service; local evidence, no network or production deployment claim.
Audience: host integrators and compiler contributors implementing authenticated embedding.

This Rust embedding adds password hashing and authenticated sessions beside the
pure `std.auth` predicates in [Authentication and Sessions v1](AUTHENTICATION-SESSIONS-V1.md).
It adds no source operation or ambient compiler authority.

The host explicitly supplies entropy, signing key, clock ticks and storage.
`SecretBytes` has redacted debug output, no serialization or clone traits,
and zeroizes owned storage on drop. This does not erase caller-owned copies.
Errors are closed tags and carry no credential or bearer-token contents.
`authentication::tests::secret_bytes_debug_never_prints_its_content` pairs a
non-vacuity control (the marker bytes are really present in the owned
storage) with the redaction assertion (the marker is absent from every byte
window of the rendered `Debug` output), rather than checking only a fixed
positive string.

Reference stores are bounded and process-local; they do not claim database
durability, distributed transactions, HTTP middleware,
rate limiting, or production deployment support.

## Signed session service

`authentication::session` provides a host-owned session flow with explicit
entropy, signing key, clock tick, and `SessionStore`. `SessionService` fixes
HS256 at construction. No bearer token selects an algorithm, key, issuer,
audience, or policy identity. `SessionPolicy` binds stable exact policy,
issuer, and audience strings and positive bounded TTLs.

The bearer value is a `semaprax-session-token.v1`, not a JWT: canonical fixed
binary claims and a fixed HMAC-SHA-256 tag are independently base64url-no-pad
encoded and separated by one dot. The authenticated claims carry the schema
magic/version, 32-byte session id, generation, issue and expiry ticks, and the
exact policy, issuer, audience, and subject strings. Parsing is bounded and
canonical; MAC verification uses `hmac`'s `verify_slice` before claims are
accepted. The tag includes a fixed domain separator. Verification rejects a
clock tick before issue time and rechecks the claimed lifetime against the
current server policy.

`SessionToken` owns redacted, zeroized bytes and exposes its value only through
`bearer()` for an explicitly selected transport boundary. It has no `Clone`,
`Debug`, or serialization implementation. `SecretBytes` likewise never exposes
the server signing key outside the authentication module.

`SessionStore` requires atomic `issue_if_absent`,
`compare_and_rotate`, and `compare_and_revoke` operations. Rotation retires the
presented record and creates a fresh random 32-byte id with an incremented
generation in one store operation; revocation and rotation refuse stale,
revoked, and generation-mismatched records. `InMemorySessionStore` is a capped
process-local reference implementation. It is neither a durable database nor a
distributed transaction implementation.

The host can call `AuthService::purge_expired_sessions(&mut store, now)` before
login to reclaim records with `expires_at <= now`, including retired records.
This keeps the same capped store usable across repeated issue/expiry cycles;
unexpired active and retired records remain until their expiry. `now` is a
trusted host-supplied tick, never read from an ambient clock. The store
rejects maintenance time regression and subsequent issuance using a tick
earlier than its last purge. This preserves refusal of old bearers if a
random session id is ever reused after cleanup. For example, a host with a
capacity-two store can issue at tick 10 with TTL 1, purge at tick 11, then
issue again at tick 11 without reconstructing the store.

The service has no source-language operation, ambient clock, entropy, key,
filesystem, network, HTTP middleware, authorization policy, rate limit, or
production external-store implementation. A successful authentication result
returns only the subject and public issue/expiry/generation claims; it never
returns a bearer token or signing key.

### Signing key rotation

`SessionService::with_key_rotation` accepts one optional retired key wrapped
in `SigningKeyRotation::new(key, grace_until)`. New tokens are always signed
with the current key; a token signed with the retired key still verifies
while the caller's `now` has not yet passed `grace_until`, after which it is
refused identically to an unknown key. This mirrors `std.auth`'s pure
`token_key_is_current_or_in_grace` policy at the host layer: rotating the
server signing key does not need to invalidate every live session
immediately, and the bound is explicit and caller-supplied, never open-ended.
A rotation cannot name the same key as both current and previous.
`authentication::session::tests::
signing_key_rotation_accepts_the_old_key_only_within_its_grace_window` proves
a pre-rotation token verifies within the grace window and is refused past it,
that a party holding only the new key cannot verify a pre-rotation token,
and that a party holding only the retired key (with no configured grace)
cannot verify a post-rotation token.

### Hostile input and concurrent sessions

`authentication::session::tests::
oversized_truncated_and_control_byte_bearer_tokens_are_refused` and
`authentication::session::tests::
control_bytes_and_unicode_confusable_subjects_are_refused_at_issue` cover
oversized bearer strings, truncated tokens missing their tag, an empty
bearer, embedded NUL and control bytes, and a Unicode-confusable subject
(Cyrillic "а" for Latin "a") that is refused when malformed and kept
byte-distinct rather than normalized when well-formed.
`authentication::session::tests::
concurrent_sessions_for_the_same_subject_are_independent` proves the store
admits more than one simultaneously active session per subject and that
revocation is per-session, not per-subject.

## Password records

`authentication::password` is a native host service. It admits only RustCrypto
Argon2id version 19 records, gives the KDF a 16-byte host-supplied salt, and
stores a 32-byte output. A password input is at most 1,024 bytes and a PHC
record is at most 512 bytes.

The default policy is 19,456 KiB, two iterations, and parallelism one. The
only accepted resource envelope is 19,456--65,536 KiB, two--four iterations,
and parallelism one. A `PasswordHasherHost` creates hashes with its current
policy and verifies only that policy plus at most four explicit, bounded
migration policies.

`StoredPasswordHash::parse_for_storage` parses the entire PHC envelope before
any KDF work: its algorithm and version are fixed, parameter keys are closed,
salt and digest encodings are canonical and fixed-size, and all resource bounds
are checked. Its debug representation is redacted; storage adapters receive a
record only through `expose_for_storage`. Incorrect passwords and malformed
stored credentials are returned as closed `AuthError` tags. This host API does
not add a source-language secret type, password operation, or authority.

`PasswordHasherHost::with_approved_migrations` lets a deployment verify a
record hashed under a previously current policy while every new hash uses the
active policy: `authentication::password::tests::
approved_migration_verifies_old_hash_but_new_hashes_always_use_current_policy`
hashes under an old policy, shows an unapproved-migration hasher refuses it,
then shows an approving hasher verifies the legacy record, still rejects a
wrong password against it, and rehashes under the current policy rather than
the migration policy. The migration set itself is bounded (`InvalidPolicy` at
five entries or when it names the active policy).
`authentication::password::tests::
policy_bounds_reject_out_of_range_iterations_and_parallelism` asserts the
floor and ceiling on iterations and refuses any parallelism other than one,
each individually, as a closed-form resource-exhaustion bound rather than a
silent clamp.


## Account and session composition

`service::AuthService` performs signup by hashing a supplied secret before an
atomic `AccountStore::insert_if_absent`; storage failure never issues a token.
Login verifies the actual stored hash before asking `SessionService` to issue
a token. `protected`, `rotate` and `logout` delegate authenticated session
validation and atomic session transitions. Account identifiers are bounded
ASCII identifiers (letters, digits, `.`, `_`, `-`, `@`), at most 128 bytes.
The reference account store holds at most 4096 entries under a lower explicit
host capacity. The host supplies transaction semantics for its own store.

`OsAuthEntropy` is an explicitly instantiated native capability. Tests inject
deterministic entropy; that fixture is not a deployment random generator.
Clock values are explicit host ticks and must use one consistent trusted clock.
Hosts must bound simultaneous KDF calls and apply login/signup rate limits.
Unknown accounts and wrong passwords share an error tag, but this profile does
not promise indistinguishable response timing or conceal signup conflicts.

The executable composition gate is
`authentication::service::tests::actual_signup_login_rotation_protected_logout`.
It performs real hashing, wrong-password rejection, login, protected access,
rotation, old-token refusal and logout. It is a Rust embedding test, not an
HTTP deployment or a source-language/backend conformance claim.

## Audit events

`authentication::audit::AuthAuditEvent` is a closed, redaction-safe record: a
closed `AuthAuditKind` (`Signup`, `Login`, `SessionVerify`, `SessionRotate`,
`Logout`), a closed `AuthAuditOutcome` (`Allowed` or `Denied(AuthError)`), a
bounded pseudonymous subject identifier re-validated at construction, and a
host-supplied timestamp tick. `AuthAuditOutcome::from_result` derives the
outcome directly from an existing `AuthService` call's `Result` without a
host needing to re-derive the allow/deny decision. No constructor on this
type accepts a `SecretBytes`, `SessionToken`, or `StoredPasswordHash`; its
`Display` and derived `Debug` need no redaction because every field was
already established as non-secret when the event was built, not because a
value was hidden after being stored.
`authentication::audit::tests::audit_event_is_complete_and_never_carries_secret_material`
pairs a non-vacuity control (a real Argon2id hash and a real wrong-password
verification failure both actually happen, using a bytes marker) with a
negative assertion that neither the marker, the stored PHC record, nor the
rejected password appears in the event's rendered `Display` or `Debug`
output. This is a host-layer building block a caller composes after calling
`AuthService`; it does not itself write to any sink, matching `std.log`'s
existing, unmodified role for that.
