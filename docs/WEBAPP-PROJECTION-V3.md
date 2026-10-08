# Web Application Projection v3

Audience: language users and compiler contributors.

Status: implementation authored; focused verification pending the complete OPT
implementation batch. Additive source conventions over v2; the application
platform completion row remains Partial. No hosted or production evidence.

## Sign-in and CSRF

The generated server accepts at most five sign-in attempts per login and twenty
per peer IP per sixty-second window, including successes and unknown logins.
It hashes login names in its process-local budget. It bounds the table to 4096
entries and refuses new entries while full instead of evicting active budgets.
A refused attempt returns 429 with `Retry-After: 60` before scrypt. Restart resets
budgets; distributed limits and proxy-address inference are outside this profile.
Only the actual socket peer identifies an IP; forwarded headers have no authority.

`GET /api/session/csrf` issues an HttpOnly, SameSite=Strict `csrfSeed` cookie and
JSON `{token}`. Every API mutation, including sign-in and sign-out, requires that
cookie and `X-CSRF-Token`. The HMAC token binds the random seed to the current
session token; signing in invalidates a pre-login token. Sign-in also returns the
new token in `X-CSRF-Token`. A server restart invalidates CSRF tokens; clients
refresh them through the GET route. The generated UI and self-test obtain a fresh
token before each mutation. Unauthenticated entity requests retain their 401.
Routes that require an existing session check identity before CSRF, so anonymous
sign-out and entity mutations return 401; a signed-in request with a missing or
mismatched token returns 403. These tokens supplement the existing SameSite cookie and do not grant permissions.
Existing HTTP clients must adopt this explicit mutation protocol.

## Pairwise cross-row rules

`<entity>_constraint` and `<entity>_constraint_<name>` are pure bool functions.
Parameters bind to this entity's fields, `self_id: i64`, and
`other_<entity>_<field>` or `other_<entity>_id: i64` from exactly one other entity.
All names and types must match; invalid conventions are `SPX-WA102`. For example:

```semaprax
record Booking { room: i64, start: i64, end: i64, }
fn booking_constraint_overlap(room: i64, start: i64, end: i64,
    other_booking_room: i64, other_booking_start: i64,
    other_booking_end: i64) -> bool
{
    room != other_booking_room || end <= other_booking_start || start >= other_booking_end
}
```

The server checks every ordered pair in the complete proposed state, skipping a
row's own identity for same-entity comparisons. A false result, precondition
failure, or trap rejects the mutation with 400 before state, passwords, ids, or
audit publication. Updates also recheck incoming constraints on other entities.
Deletion uses the same candidate check and retains existing reference restrictions.
Rules see all rows regardless of read permissions; responses name the entity and
constraint without exposing the other row identity or values. The generated
UI leaves this enforcement authoritative on the server. Empty other tables satisfy
pairwise rules vacuously; existence/count constraints and multi-row transactions
are not added by this contract. Checking is quadratic for same-table constraints.

## Explicit field migration

New databases save envelope version 2 with stored field descriptors and enums.
A changed schema refuses startup until the operator supplies `--migrate`.
A missing, renamed, or retyped field requires a pure
`<entity>_migrate_<destination>` function returning that field's exact type.
Parameters `old_<field>` declare the previous scalar types. Zero parameters
supply a newly added field's default. Functions use the existing checked webapp
expression subset; preconditions and arithmetic traps fail migration.

```semaprax
fn booking_migrate_start(old_begin: i64) -> i64 { old_begin }
fn booking_migrate_label() -> string { "untitled" }
```

Migration decodes old inputs with their historical enum definitions, verifies
parameter type descriptors, and decodes produced values with the current schema.
Unchanged fields copy unchanged. Explicit `--migrate` permits removed fields;
removing a populated entity refuses instead of silently discarding its rows.
Legacy envelope v1 is also admitted; its absent historical descriptors cannot
prove a previous field type, so explicit migration parameters and ordinary value
decoding govern it. Legacy missing/extra fields refuse without `--migrate`.
All ids remain positive and unique, next ids remain monotonic, and every new
row, reference, key, ordinary rule and pairwise constraint must pass before the
in-memory state is published. Failed migration leaves `db.json` unchanged.
Before a successful atomic database rewrite the server saves the exact previous
bytes as `db.before-<sha256>.json` in its data directory. The new schema prevents
reapplying migration functions on later restarts. Account hashes and session ids
remain separate in `auth.json`; account permission checks remain live.

## Ownership and verification

`webapp/model/v3` binds the two conventions, ordinary `webapp/translate` owns
expression semantics, and `webapp/emit` publishes deterministic schema objects.
`runtime/security.mjs` owns the bounded process-local protection;
`runtime/state.mjs` owns staging and full candidate constraints. The compiler
adds no authority. The generated server uses its existing socket, crypto, clock,
and explicitly selected data directory. No outbound connection is introduced.

Focused selectors, to run after the complete implementation batch:
`webapp::tests::v3_` plus the existing generated server self-test. The authored
runtime fixture checks CSRF session binding, expiry and both rate budgets, migration
failure atomicity, preserved ids, duplicate ids, and incoming constraint changes.
The HTTP fixture exercises rejected and accepted writes, sign-in, sign-out, CSRF,
rate limits, and unchanged disk bytes after rejected pairwise constraints.
