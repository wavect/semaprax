# Authentication and Sessions v1

Audience: language users, tool authors, and compiler contributors.

Status: first bounded slice of issue #191's authentication/session profile,
plus a second tranche adding the OAuth/OIDC authorization-code callback
policy. `std.auth` ships the pure, effect-free decision procedures for
session lifecycle legality, CSRF and cookie policy, constant-time secret
comparison, closed-algorithm token verification, password-hash *policy*
bounds, audit-event safety, and an OAuth/OIDC authorization-code callback
policy (state binding, redirect-uri matching, PKCE challenge validity, and
single-use code freshness — see
[OAuth/OIDC authorization-code callback policy](#oauthoidc-authorization-code-callback-policy)).
It does **not** ship a password hash function, a real signature/MAC
implementation, an `Secret<T>` wrapper type, a real PKCE `S256` hash
computation, an OAuth token-endpoint or discovery-document client, or any new
host operation, `permit`, or dependency. See
[Non-claims and remaining work](#non-claims-and-remaining-work) for exactly
why and what would be required to lift each one, and
[Acceptance-criteria mapping](#acceptance-criteria-mapping) for a line-by-line
status against issue #191.

## Objective

[Database Access v1](DATABASE-ACCESS-V1.md) and [Durable Jobs
v1](DURABLE-JOBS-V1.md) established the pattern this tranche follows:
specify the pure, checked decision procedures a domain needs, prove them
correct on every backend, and be explicit about the host authority a real
deployment would still need to supply. Authentication is the domain where
that discipline matters most, because AGENTS.md's non-negotiable invariant
binds it directly:

> Capabilities are explicit. Compiler and generated code gain no ambient
> filesystem, process, network, home, secret, key, wallet, or signing
> authority.

A "secret" in `std.auth` is never anything other than a `borrow Slice<u8>` a
caller already held and passed explicitly into one function call. The package
performs no I/O, declares no `permit`, calls no `uses`-gated operation, stores
nothing between calls, and returns nothing it was not handed. It cannot leak
ambient authority because it has none to leak.

## Threat model

Each named threat class below states what `std.auth` provides and, just as
importantly, what it explicitly does not.

| Threat | What `std.auth` provides | What it does not provide |
| --- | --- | --- |
| Credential stuffing / brute force | `password_policy_within_bounds` bounds the memory/time/parallelism cost of a real hash on both ends, so a deployment cannot configure a hash cheap enough to brute-force at scale | rate limiting, account lockout, and the hash function itself (see [Password hashing](#password-hashing-policy-only-not-an-algorithm)) |
| Session fixation | Every session state transition is sticky: `session_next_state_on_access`/`_on_rotate`/`_on_revoke`/`_on_logout` never move a terminal state back to `active` (`session.spx`'s "Only `active` is nonterminal" invariant) | binding a session id to a TLS channel or client fingerprint |
| Session theft after rotation | `session_rotated_id_reuse_is_attack_signal` names presentation of a `rotated_out` id as a signal a caller must escalate (recommended response: revoke the whole session family) | family-revocation storage itself, which is deployment state this pure layer does not hold |
| Token theft / replay | `token_replay_is_fresh` composes into `token_claims_are_valid`; a deployment supplies the `seen_before` bit from its own nonce/jti store | the nonce store itself |
| CSRF | `csrf_token_matches` is defined in terms of `ct_bytes_equal`, never a short-circuiting comparison; `csrf_required_for_method` scopes the check to state-changing verbs | binding the token to a specific request/origin header — that is caller-side wiring this pure layer does not perform |
| Timing leaks | `ct_bytes_equal`'s loop condition is `index < limit` alone, never conditioned on whether a mismatch has already been found — see [Constant-time comparison](#constant-time-comparison) | a wall-clock measurement proving the property on any concrete backend (stated explicitly, not silently assumed) |
| Downgrade / algorithm confusion | `token_algorithm_is_allowed` is a closed two-value allow-list; `alg_id == 0` ("none") and every unrecognized id are refused identically | negotiating or accepting a caller-selected algorithm at all — the allow-list is the only mechanism, by design |
| Key confusion / rotation | `token_key_id_is_allowed` requires membership in an explicit deployment-supplied allow-list (never "trust the token's claimed key"); `token_key_is_current_or_in_grace` bounds how long a rotated-out key stays valid | key storage, distribution, or the rotation schedule itself |
| Clock skew / unbounded acceptance window | `token_leeway_is_bounded` caps leeway at 300 ticks; `token_time_is_valid` requires that bound before it runs, so no deployment configuration path produces an unbounded window | a real wall clock — every tick argument here is caller-supplied, exactly like `std.jobs`'s deterministic tick clock |
| Log/audit leakage | `audit_event_is_safe` is a closed refusal over six named secret-bearing fields (raw password, password hash, session token, bearer token, CSRF token, authorization header); it is `false` if *any* one is present, regardless of what else the event carries | a logging sink, redaction pipeline, or structured writer — `std.log` (see [Non-claims](#non-claims-and-remaining-work)) is the existing package for that, unmodified here |
| OAuth callback CSRF / authorization-code interception / PKCE downgrade | `oauth_state_matches` binds a callback to its request; `oauth_pkce_method_is_allowed` refuses "no PKCE" and any unrecognized method identically; `oauth_authorization_code_is_fresh` refuses a replayed code | opening the redirect, calling the token endpoint, or computing the `S256` digest itself (see [OAuth/OIDC authorization-code callback policy](#oauthoidc-authorization-code-callback-policy)) |

## Secret value semantics: why there is no `Secret<T>`

Issue #191 asks for "an opaque `Secret<T>` or equivalent non-printable/
non-serializable resource" whose value cannot leak "through derived traits,
panic messages, evidence, or generated clients." Two facts about the current
language change what that requirement means here, and both are worth stating
plainly rather than working around silently:

1. **There is no derive, `Debug`, `Display`, or reflection mechanism in
   SEMAPRAX today.** A grep of every `.spx` package under `std/` and of
   [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) turns up no `derive`, no format
   trait, and no generic "print this value" operation. A record cannot be
   printed, formatted, or serialized except by a function the author writes
   by hand, byte by byte (exactly how `std.log`'s `Event` renders itself: an
   explicit `append_event` function that copies named fields into a caller
   owned `Writer`, never an automatic trait). **The entire class of leak this
   requirement worries about — "leaks through derived traits" — does not
   exist as an attack surface in this language, because the mechanism that
   would produce it does not exist.** This is a real, load-bearing finding,
   not an evasion: it means a wrapper type's main job in a language like Rust
   (blocking an auto-derived `Debug`/`Display`) has no analogue to block here.
2. **A wrapper record would still buy real things** — a distinct nominal
   type so a secret cannot be passed where an ordinary `Slice<u8>` is
   expected by accident, and a single declared place to hang a
   non-serialization contract for a future `std.data.json`/`std.log`
   integration. Both require record-field projection patterns this session's
   audit confirmed the reference interpreter does not admit uniformly
   (`std.db`/`std.jobs`/`app.http_router` all represent structured state as
   ordered `usize`/`u8` tags and `Slice<u8>` views for exactly this reason,
   not preference), and extending that is downstream compiler work outside
   this package's lease (`src/hir/**`, `src/interpreter*`, `src/codegen/**`,
   `src/wasm/**` are all out of scope for this change).

Given both, `std.auth` does not define a `Secret` record. Its actual
guarantee is narrower and stated exactly: **every function in this package
takes secret-shaped bytes as an ordinary `borrow Slice<u8>` parameter, reads
them for the duration of one call, and returns only a `bool` or a closed
`usize`/`i64` state code — never the bytes themselves, a copy of them, or any
value derived from them other than a policy decision.** No function in this
package has an environment, filesystem, network, or keyring capability to
read a secret from, so there is no ambient path by which one could reach this
code without a caller choosing to pass it. A caller that wants the
"cannot be logged" property today gets it by construction: nothing here
returns a secret to be logged, and using `audit_event_is_safe` (above) is how
a caller checks its own event shape before handing it to `std.log`.

**If a future tranche adds a `Secret<T>` wrapper record**, it should be
proposed and reviewed only alongside whatever record-field-projection work
lifts admission for the reference interpreter, native, and Wasm backends
together (AGENTS.md's change protocol: parser, formatter, resolver/HIR,
verifier, semantic graph, native backend, and Wasm backend move together for
syntax with runtime meaning) — not added as a special-cased single-backend
type here.

## Constant-time comparison

```semaprax
@id("std.auth.security.ct_bytes_equal")
fn ct_bytes_equal(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
{
    let left_len = byte_len(left);
    let right_len = byte_len(right);
    let limit = if left_len > right_len { left_len } else { right_len };
    let mut index = 0usize;
    let mut mismatches = 0usize;
    while index < limit {
        let left_byte = match byte_get(left, index) { Option::Some { value } => value, Option::None {} => 0u8, };
        let right_byte = match byte_get(right, index) { Option::Some { value } => value, Option::None {} => 0u8, };
        mismatches = mismatches + if left_byte == right_byte { 0usize } else { 1usize };
        index = index + 1usize;
        index < limit
    }
    mismatches == 0usize && left_len == right_len
}
```

`std.bytes.equals` — the package's own general-purpose equality helper,
already used by `std.jobs` for non-secret descriptor comparison — is `while
index < length && same`: it stops at the first differing byte, so its total
work is proportional to the length of the shared prefix. That is exactly the
oracle a timing attack on a session id, bearer token, or signature exploits.
`ct_bytes_equal`'s loop condition is `index < limit` **alone** — the loop
never reads `mismatches` — so it performs exactly `limit` byte reads,
`Option` decodes, and comparisons every time, independent of where content
differs or whether it differs at all. Every function in this package that
compares a session id, CSRF token, bearer token, or signature/MAC calls
`ct_bytes_equal`, never `std.bytes.equals`; `token_key_id_is_allowed` is the
one deliberate exception, and its doc comment states why (key ids are public
identifiers, not secrets).

**What is tested and what is not.** `std.auth.tests.constant_time_equality`
proves *functional* correctness at every position — equal, differs at the
first byte, the middle byte, the last byte, and differs by length — showing
the function returns the right answer everywhere a short-circuiting
comparison would have exited early. It does **not**, and cannot, measure
wall-clock timing: this suite runs inside a pure interpreter and the two
generated backends (native C11, Core Wasm) with no timer access, and no gate
in this repository benchmarks generated machine code for a timing
side-channel. The constant-time property claimed here is a **structural**
one — provable by reading the source, because the loop bound is manifestly
independent of the accumulator — not an empirically measured one. Stating
that distinction is the honest alternative to a benchmark this change cannot
run.

## Session lifecycle

States are a closed `usize` tag, the same idiom `std.jobs` uses for job
state:

| Code | State | Terminal? |
| ---: | --- | --- |
| 0 | `ACTIVE` | no |
| 1 | `ROTATED_OUT` | yes |
| 2 | `REVOKED` | yes |
| 3 | `IDLE_EXPIRED` | yes |
| 4 | `ABSOLUTE_EXPIRED` | yes |
| 5 | `LOGGED_OUT` | yes |

Only `ACTIVE` is nonterminal, and every transition function is sticky: none
of `session_next_state_on_access`, `_on_rotate`, `_on_revoke`, or `_on_logout`
ever produces `ACTIVE` from a terminal input, and a malformed (out-of-range)
state code always falls to the safe default `REVOKED` rather than being
passed through or guessed at — the same "failure selection is sticky" and
"fail closed" posture `std.db`'s transaction state machine and `std.jobs`'s
lease state machine both use.

`session_is_usable` is the one question authentication middleware asks per
request — state is `ACTIVE` and neither the idle nor the absolute deadline
has passed — and it is the *only* question this package answers about a
session. It never returns, and no function in this module returns, a
capability, role, or permission; see
[Authentication is not authorization](#authentication-is-not-authorization).

Rotation (`session_next_state_on_rotate`) retires the presented id to
`ROTATED_OUT` rather than deleting it, specifically so that a later replay of
the retired id is observable: `session_rotated_id_reuse_is_attack_signal`
flags exactly that state. Concurrent-session admission
(`session_concurrent_admission_decision`) is an explicit deployment choice
(`evict_oldest_on_limit: bool`) between evicting the oldest session and
refusing the new one, never a silent default either way.

## CSRF and cookie policy

`csrf_token_matches` is `ct_bytes_equal` under a domain-specific name — never
plain equality — because a CSRF token is exactly the secret-shaped comparison
[above](#constant-time-comparison) names. `csrf_required_for_method` scopes
the check to state-changing HTTP verbs, so a caller does not have to
re-derive that policy per route.

`cookie_attributes_are_safe(http_only, secure, same_site_mode)` requires all
three: `HttpOnly`, `Secure`, and `SameSite` set to `Lax` (1) or `Strict` (2).
`SameSite=None` (0) is refused unconditionally — there is no configuration
that makes it pass — and any `same_site_mode` outside the closed `0..=2`
range is refused as malformed input rather than silently treated as one of
the three.

## Token verification

`token_algorithm_is_allowed` is a closed two-value allow-list (`1`, `2`,
standing for the deployment's own symmetric/asymmetric algorithm identities —
this package fixes no real algorithm, only the shape of a closed allow-list).
`alg_id == 0` is the reserved "none"/unsigned marker and is refused
identically to any unrecognized value: there is no code path in which an
absent or unrecognized algorithm is accepted.

`token_key_id_is_allowed` requires membership in a deployment-supplied
allow-list of key ids — the verification key is chosen by the deployment's
own lookup over that list, never by trusting whatever key id a token claims.
`token_key_is_current_or_in_grace` bounds key rotation explicitly: a previous
key verifies only until `previous_key_grace_until_tick`, after which
presenting a token signed with it is refused exactly like an unknown key.

`token_time_is_valid` requires `token_leeway_is_bounded` (`<= 300` ticks)
before it runs at all, so clock-skew leeway is capped at the type level of
its own precondition — no deployment can configure an unbounded acceptance
window through this function. `token_claims_are_valid` composes issuer match,
audience match (both via `ct_bytes_equal`), algorithm, key-and-grace, time
validity, and replay freshness into one boolean;
`token_verification_admits` additionally requires an externally supplied
`has_valid_signature: bool` — see
[Signature verification](#signature-verification-is-out-of-scope-here) for
exactly what that bit stands in for and does not provide.

### Signature verification is out of scope here

`token_verification_admits`'s `has_valid_signature` parameter is an opaque
boolean this package never computes. A real signature or MAC check needs a
cryptographic primitive (HMAC, RSA, or ECDSA verification) this pure,
effect-free package has no way to perform — the same boundary the next
section states for password hashing applies here too, and for the same
reason.

## Password hashing: policy only, not an algorithm

**This package does not, and given this change's constraints cannot, ship a
real password hash function.** It ships only the policy checks a real one
would need to be wrapped safely:

- `password_memory_cost_within_bounds`, `_time_cost_within_bounds`, and
  `_parallelism_within_bounds` bound a memory-hard hash's cost parameters on
  both ends (a floor so the hash cannot be configured too cheap to resist
  brute force, a ceiling so it cannot be configured expensive enough to deny
  service to the process hashing it) — directly answering the issue's named
  failure "password-hash parameters can cause denial of service."
- `password_needs_rehash` compares a stored policy version against the
  current one, the usual "upgrade on next successful login" trigger.

Why no hash function ships: issue #191 asks to "wrap a mature password-
hashing library through a versioned adapter," and this assignment's
constraints make that impossible to do honestly in this slice:

1. **No new Rust dependency is permitted for this change.** A real
   memory-hard hash (Argon2id, scrypt, bcrypt) is exactly the kind of
   primitive nobody should hand-roll; the honest position is to not attempt
   one rather than ship a private, unreviewed implementation under that name.
2. **A host-provided implementation — the "explicit host/runtime
   implementation" the issue itself asks for — needs a new closed host
   operation in the same family as `net_*` ([Bounded Language Network I/O
   v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md)) or `std.db`'s driver boundary.
   That means new code in `src/interpreter*`, `src/codegen/**`, and
   `src/wasm/**` so every backend admits and executes it identically — all
   three are explicitly out of this change's file lease.**
3. This repository's Rust dependency graph already links `sha2` and `hmac`
   (used elsewhere, for content-addressed package-lock hashing — unrelated to
   password storage) as of this tranche's starting commit, so a future host
   operation would not need a *new* dependency to reuse them for HMAC-based
   token signing specifically. It would still need a maintainer decision, the
   host-operation design work above, and — for password hashing specifically
   — a memory-hard KDF neither crate provides, which is a separate,
   real gap.

This is `HUMAN_BLOCKED`: shipping real password hashing needs a maintainer
decision to either add a vetted memory-hard-hash dependency or design a new
host operation, and either path requires touching modules outside this
change's lease. Nothing in `std.auth` claims to hash a password, verify one
against a real hash, or perform password-based authentication end to end.

## Audit-event safety

`audit_event_is_safe(carries_raw_password, carries_password_hash,
carries_session_token, carries_bearer_token, carries_csrf_token,
carries_authorization_header)` is a closed refusal: it is `false` if *any*
one of the six named secret-bearing fields is present, regardless of what
else the event carries. `audit_event_is_complete` requires an event kind, an
outcome, and a timestamp — the minimum shape an audit trail needs to be
useful, independent of the safety check. A caller composing a real audit
event (for example, one it intends to hand to `std.log`'s `Event`, which is
unmodified by this tranche) checks both before emitting it.

## OAuth/OIDC authorization-code callback policy

The issue's "OAuth/OIDC adapter interface as a later part of the same profile
if scope permits" is partially shipped: the same posture as
[Token verification](#token-verification) applies here, restated for the
authorization-code grant. `oauth_state_matches` and
`oauth_redirect_uri_matches` both go through `ct_bytes_equal`, exactly like
`token_issuer_matches`/`token_audience_matches` — public, request-shaped
values compared with the constant-time primitive anyway, because it costs
nothing extra at these sizes. `oauth_pkce_method_is_allowed` is a closed
two-value allow-list (`1` = `plain`, `2` = `S256`); `method_id == 0`
("no PKCE") is refused identically to any unrecognized value, the same
downgrade defense `token_algorithm_is_allowed` gives token verification.
`oauth_authorization_code_is_fresh` is `token_replay_is_fresh` under a
domain-specific name, so a single-use code cannot be redeemed twice.

**Signature computation is out of scope here, exactly like token
verification.** `oauth_pkce_challenge_is_valid`'s `has_valid_s256_hash`
parameter is the same opaque, already-decided `bool` idiom as
`token_verification_admits`'s `has_valid_signature`: this pure package cannot
compute a SHA-256 digest, so a deployment choosing the `S256` method must
supply its own comparison result. The `plain` method needs no such input —
`ct_bytes_equal` directly compares the verifier against the challenge — which
is why `std.auth.tests.oauth_pkce_policy`'s
`s256_needs_real_hash`/`plain_accepts_matching_verifier` pair exists: it
proves the `S256` branch is refused without a confirmed hash while the
`plain` branch is a real, self-contained check, not two branches that merely
look different.

`oauth_authorization_request_is_valid` and `oauth_callback_is_valid` compose
these primitives into the two calls a caller makes around a redirect: the
request-side check before minting the redirect, and the callback-side check
before exchanging the code for a token. Neither call opens a redirect,
contacts a token endpoint, or reads a discovery document — those remain
deployment-side I/O this pure layer has no capability to perform, the same
boundary [Objective](#objective) states for the rest of the package.

## Authentication is not authorization

No function in `std.auth` takes a session or token state and returns a
capability, role, or permission value. The complete function inventory in
`src/auth.spx` is: session-state predicates and transitions (return `bool` or
a closed session-state `usize`), CSRF/cookie predicates (`bool`), token
predicates and the two verification functions (`bool`), password-policy
predicates (`bool`), audit predicates (`bool`), and OAuth callback predicates
(`bool`). `session_is_usable` and `token_verification_admits` answer exactly
one question each — "is this credential currently valid" — never "what may
the caller do." A caller combines a `true` result from either with an
independently supplied authorization policy; this package supplies no such
policy and no mechanism to derive one from authentication state, by omission
rather than by a runtime check this pure layer cannot perform.

**No auth-middleware success grants unrelated effects.** Every function in
`std.auth` declares an empty `uses` clause and the module declares no
`permit` at all — there is no capability this package could hand a caller
even if it wanted to. This is not merely a design intent: it is a compiler-
checked invariant. `tests/project/standard_library.rs`'s
`every_public_declaration_has_a_std_identity_contracts_examples_and_conformance`
asserts, for every package in `std/packages.json` including `std.auth`, that
`library.program.functions[i].effects == expected_effects` and
`library.program.permits == expected_permits`; for any module other than
`std.fs`/`std.env`/`std.process` (which the same test binds to their real
effect names) both expected lists are `vec![]`. A `std.auth` function that
started declaring `uses { ... }` for any host effect, or a module-level
`permit` granting one, would fail that assertion immediately — the same test
that already runs, unmodified, for this package.

## Non-claims and remaining work

Restated plainly, matched against issue #191's "In scope" list:

| In scope (#191) | Status here |
| --- | --- |
| Opaque `Secret<T>` or equivalent | **Not shipped.** See [Secret value semantics](#secret-value-semantics-why-there-is-no-secrett) for why the language's lack of any derive/Debug/reflection mechanism changes what this requirement means, and what would be needed to add a wrapper record correctly. |
| Password hashing via a maintained memory-hard algorithm through an explicit host/runtime implementation | **Not shipped**, `HUMAN_BLOCKED`. Policy bounds only; see [Password hashing](#password-hashing-policy-only-not-an-algorithm). |
| Session IDs, storage contract, rotation, expiry, revocation, CSRF policy, secure cookies | **Shipped** as pure decision procedures (this document's session/CSRF/cookie sections). A storage contract (where session records actually live) is not shipped — like `std.db` and `std.jobs`, that is a host/driver concern this pure layer only decides over, never performs. |
| Signed token verification with algorithm/key policy and claims validation | **Policy shipped**; the signature/MAC computation itself is not (same reason as password hashing). |
| OAuth/OIDC adapter interface | **Policy shipped**; see [OAuth/OIDC authorization-code callback policy](#oauthoidc-authorization-code-callback-policy). State binding, redirect-uri matching, PKCE method allow-listing, and single-use code freshness are pure decision procedures with their own tests. The `S256` hash computation, an actual token-endpoint/discovery-document client, and the redirect itself are not shipped, for the same reason password hashing and token-signature computation are not (see those sections) — this remains a decision layer, not an HTTP client. |
| Auth middleware integration | **Not shipped** as `std.http` wiring: `examples/http_app_routing.spx` and `tests/http_app_routing.rs` are outside this change's file lease (owned by issue #189). `std.auth.examples.main` demonstrates the decision procedures composing into a signup/login/protected-route/logout sequence entirely in the abstract (synthetic ticks and byte arrays, no request parsing, no socket), proving the pure layer is internally coherent, not that it is wired into a real router. |
| Audit events without secret leakage | **Shipped** as the closed `audit_event_is_safe`/`_is_complete` predicates. |

## Acceptance-criteria mapping

Issue #191's acceptance criteria, matched exactly:

- "A reference application can implement signup/login/logout/session-protected
  routes safely" — **partially met**. `std.auth.examples.main` composes
  signup-policy, login, a protected-route access, an OAuth authorization-code
  callback, and logout using only the decision procedures in this package,
  and is executed on the interpreter, native C11, and Core Wasm (see
  [Local evidence](#local-evidence)). It is **not** a reference HTTP
  application: no request is parsed, no socket is opened, and it is not
  integrated with `std.http`/`examples/http_app_routing.spx`, both outside
  this change's lease.
- "Secrets cannot enter ordinary source, logs, diagnostics, or public
  evidence" — **met for what this package touches**: no function returns
  secret bytes it was not handed, and `audit_event_is_safe` gives a caller a
  checkable predicate before emitting an event. Not met in the sense of an
  enforced `Secret<T>` type distinct from `Slice<u8>` — see
  [Secret value semantics](#secret-value-semantics-why-there-is-no-secrett).
- "Authentication and authorization remain separate typed concepts" — **met
  at the function-signature level**: see
  [Authentication is not authorization](#authentication-is-not-authorization).
  Not met as a distinct *nominal type* the compiler enforces one cannot
  smuggle past — the language has no such enforcement mechanism to attach it
  to yet.
- "Algorithms and policies are explicit, versioned, and deployment-bound" —
  **met** for every policy this package expresses (algorithm allow-list, key
  allow-list and grace window, leeway bound, password-cost bounds, policy
  version for rehash) — every one is a caller-supplied parameter, never a
  default this package picks silently.
- "Security tests cover the named attack classes" — **met for the classes a
  pure decision procedure can exercise**: fixation, rotation, expiry,
  revocation, concurrent-session policy, CSRF, algorithm/key confusion,
  clock-skew bounding, replay, and — as of this tranche — OAuth state/
  redirect-uri substitution, PKCE downgrade and method confusion, and
  authorization-code replay all have passing named tests (see
  [Local evidence](#local-evidence)). Credential stuffing/rate-limiting and
  live timing measurement are not exercised, consistent with the non-claims
  above.
- "No auth middleware success grants unrelated effects" — **met, compiler-
  checked**: see [Authentication is not authorization](#authentication-is-not-authorization)'s
  "No auth-middleware success grants unrelated effects" paragraph. Every
  `std.auth` function (OAuth functions included) declares an empty `uses`
  clause and the module declares no `permit`, asserted by the same generic
  structural test every standard-library package runs.

## Local evidence

`std/auth`'s three source files (`auth.spx`, `examples.spx`, `tests.spx`) pass
`semaprax check`, `semaprax run` (exit `0`), and `semaprax test` (`project
tests passed`) against the reference interpreter. `tests/project/
standard_library/auth_backend_audit.rs` additionally executes the package's
examples and conformance module on native C11 (`-O0`/`-O2`) and repeated Core
Wasm, the same three-backend audit pattern `db_jobs_backend_audit.rs`
established for issue #102. All of this is local, interpreter/generated-code
evidence from this repository's own test suite; no hosted, device, or
production deployment is described or implied.

`tests/project/standard_library.rs`'s
`every_public_declaration_has_a_std_identity_contracts_examples_and_conformance`
runs over every package in `std/packages.json`, `std.auth` included, and
independently confirms canonical source formatting, the `@id` identity
prefix, an empty effect/permit inventory, and that the conformance module
imports every library declaration — all without this package needing its own
copy of that check.

The OAuth/PKCE addition's negative controls were verified by deliberate
mutation, not merely written and trusted: `oauth_pkce_challenge_is_valid`'s
`S256` branch was temporarily changed from `has_valid_s256_hash` to a bare
`true`, `auth_executes_on_all_three_backends` was re-run and failed
(`Returned(16)`, `std.auth.tests.summary_oauth_policy`'s exact bit), and the
change was reverted and the suite re-run green before this tranche's commit —
proving `s256_needs_real_hash` genuinely exercises the check it names, not
merely a rejection path a deleted check would also satisfy.
