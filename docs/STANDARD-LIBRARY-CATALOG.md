# Standard library catalog

Status: generated from `std/` through the `semaprax doc` documentation model by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.

Audience: agents and humans choosing a standard-library declaration.

Use this catalog to find a bundled declaration and its required project profile.
Each entry is compiler-verified and exercised by its package's interpreter,
native C11, and Core Wasm conformance module. [Standard Library v1](STANDARD-LIBRARY-V1.md)
owns the full contract; `std/catalog.json` is the machine-readable catalog.

To use a package, add its dependency in `semaprax.toml`, then import its stable
identity. For example: `[dependencies] std.num = "^0.1.0"` and
`use function @id("std.num.abs") from std.num as abs;`. Set `[package] profile`
to the package's listed profile; omit it for `scalar`. Bundled packages need no
source checkout, cache, or network access.

## `std.agent`

Package `std/agent`, tier `agent`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.agent = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.agent.task`

```semaprax
record Task {
    objective: Bytes,
    budget: i64,
}
```

### `std.agent.context`

```semaprax
record Context {
    objective: Bytes,
    budget: i64,
    epoch: i64,
}
```

### `std.agent.observation`

```semaprax
record Observation {
    budget: i64,
    epoch: i64,
}
```

### `std.agent.outcome`

```semaprax
record Outcome {
    value: Bytes,
    status: i64,
}
```

### `std.agent.initialize`

```semaprax
fn initialize(task: own Task) -> Context
    ensures result.epoch == 0
```

### `std.agent.observe`

```semaprax
fn observe(context: borrow Context) -> Observation
    ensures result.budget == context.budget
    ensures result.epoch == context.epoch
```

### `std.agent.advance`

```semaprax
fn advance(context: own Context) -> Context
    requires context.epoch >= 0 && context.epoch < 9223372036854775807
    ensures result.epoch >= 1
```

### `std.agent.outcome-bytes`

```semaprax
fn outcome_bytes(outcome: own Outcome) -> Bytes
```

### `std.agent.outcome-status`

```semaprax
fn outcome_status(outcome: borrow Outcome) -> i64
    ensures result == outcome.status
```

### `std.agent.stage-after-initialize`

```semaprax
fn stage_after_initialize(next: i64) -> bool
    ensures result == (next == 1)
```

### `std.agent.stage-after-observe`

```semaprax
fn stage_after_observe(next: i64) -> bool
    ensures result == (next == 2)
```

### `std.agent.stage-after-authorize`

```semaprax
fn stage_after_authorize(next: i64) -> bool
    ensures result == (next == 3)
```

### `std.agent.stage-after-reduce`

```semaprax
fn stage_after_reduce(next: i64) -> bool
    ensures result == (next == 1 || next == 4 || next == 5 || next == 6)
```

### `std.agent.stage-transition`

```semaprax
fn stage_transition(current: i64, next: i64) -> bool
```

### `std.agent.stage-is-terminal`

```semaprax
fn stage_is_terminal(stage: i64) -> bool
    ensures result == (stage != 0 && stage != 1 && stage != 2 && stage != 3)
```

### `std.agent.retry-admitted`

```semaprax
fn retry_admitted(attempt: i64, max_attempts: i64) -> bool
    ensures result == (attempt >= 0 && attempt < max_attempts)
```

### `std.agent.retry-delay`

```semaprax
fn retry_delay(attempt: i64, ceiling: i64) -> i64
    requires attempt >= 0 && ceiling >= 1 && ceiling <= 1073741824
    ensures result >= 1 && result <= ceiling
```

## `std.async`

Package `std/async`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.async = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.async.clamp_wait_ms`

```semaprax
fn clamp_wait_ms(timeout_ms: usize) -> usize
    ensures result <= 30000usize
```

### `std.async.next_timeout_ms`

```semaprax
fn next_timeout_ms(attempt: usize, base_ms: usize, cap_ms: usize) -> usize
    requires base_ms >= 1usize && base_ms <= cap_ms && cap_ms <= 30000usize
    ensures result >= base_ms && result <= cap_ms
```

### `std.async.should_retry`

```semaprax
fn should_retry(state: usize, attempts: usize, max_attempts: usize) -> bool
```

### `std.async.next_handle`

```semaprax
fn next_handle(current: usize, count: usize) -> usize
    requires count >= 1usize && count <= 8usize && current >= 1usize && current <= count
    ensures result >= 1usize && result <= count
```

### `std.async.remaining_ms`

```semaprax
fn remaining_ms(elapsed_ms: usize, budget_ms: usize) -> usize
    ensures result <= budget_ms
```

### `std.async.stream_ended`

```semaprax
fn stream_ended(chunk: borrow Slice<u8>) -> bool
```

## `std.auth`

Package `std/auth`, tier `portable`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.auth = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.auth.secret`

---------------------------------------------------------------------
Secret<T>: an opaque, non-leaking scalar handle (issue #191)
---------------------------------------------------------------------
This is the `.spx`-visible nominal wrapper the issue asks for. It is
deliberately narrow, and the narrowness is load-bearing, not an
oversight — every claim below was checked against the compiler, not
assumed:

1. Whole-value equality is already refused for every nominal type.
   `left == right` on two `Secret<T>` values (or any other `record`/
   `variant`) fails to compile with `SPX-T207` ("aggregate equality is
   outside the executable comparison profile"), raised by
   `reject_aggregate_equality` in `src/source_verify/diagnostics.rs` for
   every `Type::Named`. This is an existing, general compiler invariant
   `Secret<T>` inherits for free — it is not a new check this package
   adds, and it cannot be bypassed by this or any other package. Proven
   directly against the compiler in
   `src/authentication/secret_source_tests.rs`.
2. The language has no `Debug`/`Display`/string-interpolation/reflection
   facility at all: the lexer (`src/lexer.rs`) recognizes ordinary string
   literals and their escapes and nothing else, so there is no construct
   in `.spx` that turns an arbitrary value — `Secret<T>` included — into
   text. "Cannot be printed" is therefore a fact about the whole
   language, not a property this package had to implement.
3. The compiler's generic-copy-type admission for a user-declared record
   (`SPX-T223`, "generic copy type `Secret` accepts only direct `i64` or
   `bool` arguments") allows a use-site instantiation of `Secret<T>` for
   exactly `T = i64` or `T = bool`; `Secret<usize>` — the type this
   package's own session/state code uses everywhere else — fails to
   compile with exactly that diagnostic, checked directly and not
   asserted (`src/authentication/secret_source_tests.rs`). A fully
   generic `fn f<T>(...) -> Secret<T>` fails separately with
   `SPX-T224`/`SPX-T226` ("must return/use the direct-scalar profile"),
   so no single generic `wrap`/`expose` function can cover both
   instantiations even where one is admitted at all (see point 5).
   `Secret<Bytes>` (with an `own Bytes` parameter) was also checked and,
   perhaps surprisingly, statically admitted — but is unexplored beyond
   that static check, since interpreter execution of it is unverified
   (see point 4), so this package does not build a byte-carrying
   `Secret<T>` on that basis alone. `Secret<T>` therefore ships as an
   opaque numeric handle or opaque decided-flag wrapper here; real secret
   bytes (a password, a signing key, a session token) remain the
   existing `borrow Slice<u8>` idiom this package's token/CSRF/OAuth
   functions already use and never retain past one call, and the real
   byte-shaped secret material itself lives only in the Rust host layer
   under `src/authentication/**`, which has its own independent
   non-leak proof (no `Clone`, redacted `Debug`, zeroize-on-drop).
   `Secret<T>` here is the source-language half of the same discipline,
   applied to the opaque scalars a deployment hands this pure decision
   layer (a numeric session/key handle, or an already-decided boolean
   like the kind `token_verification_admits`'s `has_valid_signature`
   takes).
4. CAPACITY CEILING, checked directly and not worked around: calling any
   function whose parameter or return type mentions a user generic
   record fails on the interpreter backend with `SPX-F102`
   ("interpreter admission failed (unsupported_callee)"), even though
   `semaprax check` admits the declarations and the calls statically.
   Isolated by direct bisection with throwaway `secret_wrap_i64`/
   `secret_expose_i64` functions (since removed — see point 5): a bare
   record literal (`Secret<i64> { value: 42 }`) followed by direct field
   access (`.value`) executes cleanly on the interpreter; substituting
   either step for a call to a function typed over `Secret<i64>`
   reproduces `SPX-F102` at that exact call. This is a real
   interpreter-backend admission gap for user generic records used
   across a function boundary, not specific to secrets or to this
   package — `semaprax run`/`semaprax test` hit it identically. Closing
   it is `src/interpreter.rs` work outside this change's lease and
   outside a single-package change's scope; reported here rather than
   silently narrowed around. CONFIRMED by a later tranche's own hostile
   test, which first mis-guessed the opposite (that this was NOT
   generic-specific) and was caught by its own suite: `Identity`/
   `Authorization` below are plain, non-generic records, and a call
   across a function boundary typed over either DOES execute — on the
   specific interpreter seam that later tranche's test actually exercises
   (`crate::interpreter::retained_call`, used by hosted/embedding
   multi-argument calls). That seam's `copy_records::flat_copy_record`
   extension (`src/interpreter/retained_call/copy_records.rs`)
   specifically admits a non-generic record with only closed scalar
   leaves across a function boundary, while still excluding a generic one
   by its `record.type_parameters.is_empty()` check — so `Secret<i64>`
   still hits `SPX-F102` on that same seam. "Generic" was the right word
   all along for this shape; see that section's point 4 for the full
   story, including which seam was actually tested and how the first,
   broader guess was found wrong.
5. A second, independent reason no wrap/expose/match function over
   `Secret<T>` ships: this package's own conformance-completeness gate
   (`tests/project/standard_library.rs`'s
   `every_public_declaration_has_a_std_identity_contracts_examples_and_conformance`)
   requires `std.auth.tests` to `use function` import every `@id`'d
   declaration in this module, and `use function` cannot import a
   function whose signature mentions a record at all (`SPX-G172`,
   "function signature leaves the admitted scalar/Copy workspace
   domain") — checked directly by adding such functions and watching
   both gates in turn, not assumed. Point 4 and point 5 independently
   rule out the same shape, so `Secret<T>` values stay call-local here:
   constructed and read with a literal and `.value`, exactly like
   `secret_self_check` below.

```semaprax
record Secret<T> {
    value: T,
}
```

### `std.auth.identity`

---------------------------------------------------------------------
Identity vs. Authorization: distinct nominal types (issue #191)
---------------------------------------------------------------------
The module comment above and `docs/AUTHENTICATION-SESSIONS-V1.md`'s
"Authentication is not authorization" section already establish this at
the *function-signature* level: no function in this module takes a
session or token state and returns a capability, role, or permission, and
every decision function here is `bool`- or closed-`usize`-typed. Issue
#191's acceptance mapping graded that "met at the function-signature
level" but explicitly "not met as a distinct nominal type the compiler
enforces one cannot smuggle past — the language has no such enforcement
mechanism to attach it to yet." That claim is now checked directly against
the compiler, exactly like `Secret<T>` above, and found to be fixable
without any compiler change:

1. `Identity` names an authentication outcome ("who you are" — a subject
   handle a caller obtained by some means this pure layer does not
   perform, e.g. `session_is_usable` returning `true`). `Authorization`
   names an independent authorization decision ("what you may do"), one a
   caller derives from an `Identity` plus a policy input this package does
   not and cannot supply (the same posture `token_verification_admits`'s
   `has_valid_signature` takes for a signature: an opaque, deployment-
   supplied bit). Nothing here lets a caller manufacture an `Authorization`
   directly from an `Identity` — both are constructed by the caller from
   caller-held scalars, call-local, exactly like `Secret<T>` (see point 4
   below for why no function of this package can do the constructing
   instead).
2. **The compiler already refuses to substitute one for the other**,
   checked directly (not assumed) in
   `src/authentication/identity_authorization_source_tests.rs`: a function
   parameter typed `Identity` rejects an `Authorization` value, and a
   function parameter typed `Authorization` rejects an `Identity` value,
   both with `SPX-T205` ("argument ... expects ... received ..."), the
   compiler's ordinary nominal-type argument check — not a new mechanism
   this package adds, so it cannot be bypassed by this or any other
   package once the two types are declared distinctly, exactly like
   `SPX-T207`'s aggregate-equality refusal above. A positive control in
   the same file proves each type is independently still accepted by a
   function that actually wants it, so the negative result is a real
   refusal and not an unrelated parse failure.
3. Whole-value equality is refused for `Identity`/`Authorization` exactly
   as for `Secret<T>` (`SPX-T207`, the same general `Type::Named`
   invariant) and there is still no print/format/reflection facility in
   the language (`src/lexer.rs`) that could serialize either into text.
   Both non-leak properties `Secret<T>`'s doc comment states are inherited
   for free, not re-implemented.
4. CAPACITY CEILING — first mis-stated by this tranche, then caught and
   corrected by its own hostile test suite
   (`src/authentication/identity_authorization_source_tests.rs`), which is
   the honest way to read this point: an early version claimed the
   `SPX-F102` interpreter ceiling `Secret<T>`'s doc comment (point 4)
   names is not generic-specific, because a throwaway `Identity`-typed
   call reproduced `SPX-F102` too. That claim was checked against the
   WRONG execution seam. `crate::interpreter::retained_call` (the
   retained multi-argument call seam this tranche's test actually
   exercises, used for hosted/embedding calls, distinct from the plain
   `admitted_resolved_functions` path `semaprax run`/`semaprax test` use)
   has its own `copy_records::flat_copy_record` admission extension
   (`src/interpreter/retained_call/copy_records.rs`) that specifically
   admits a **non-generic** record — `record.type_parameters.is_empty()`
   at the declaration, no type argument at the use site — whose fields
   are all closed scalars, across a function boundary. `Identity` and
   `Authorization` both qualify, so a call typed over either one executes
   cleanly through that seam: `calling_a_function_typed_over_identity_
   executes_on_the_retained_call_interpreter` proves it, checking the
   actual returned value, not merely the absence of an error. `Secret<T>`
   does NOT qualify — it is declared with a type parameter, so
   `Secret<i64>` fails `flat_copy_record`'s check regardless of which
   scalar fills `T` — and `calling_a_function_typed_over_generic_secret_
   is_rejected_by_the_retained_call_interpreter` confirms `SPX-F102` still
   fires for `Secret<i64>` on this identical seam. So "generic" was the
   right discriminator for `Secret<T>` all along, on this seam; this
   tranche's error was generalizing a narrow, correct claim into a
   broader one ("any record with no `Bytes` field") without first
   checking a non-generic record against the specific seam the claim was
   about. Whether the plain `semaprax run`/`semaprax test` path (which
   this tranche did separately observe refusing a non-generic record too,
   via manual CLI probes, not via a committed test) treats non-generic
   records the same way as `retained_call` is NOT re-verified here and is
   left as an open question, not a second guess.
5. The same conformance-completeness consequence as `Secret<T>` point 5
   follows: no function whose signature mentions `Identity` or
   `Authorization` ships here, so `std.auth.tests` imports both types with
   `use type` (never `use function` for a function that does not exist)
   and `identity_authorization_self_check` below constructs and reads
   both types call-locally, exactly like `secret_self_check`.

```semaprax
record Identity {
    subject_id: usize,
}
```

### `std.auth.authorization`

```semaprax
record Authorization {
    permitted: bool,
}
```

### `std.auth.security.ct_bytes_equal`

This module is the pure, effect-free decision-procedure layer for issue
#191's authentication/session profile: session lifecycle legality, CSRF
and cookie policy, constant-time secret comparison, closed-algorithm token
verification, password-hash *policy* bounds, audit-event safety, and an
OAuth/OIDC authorization-code callback policy (state binding, redirect-uri
matching, PKCE challenge validity, and single-use code freshness).

It performs no I/O, declares no `permit`, and calls no `uses`-gated
operation. Every function is a pure predicate or pure state transition over
caller-supplied scalars and borrowed byte slices, exactly the idiom
`std.db` and `std.jobs` use for their own pure decision layers. A "secret"
in this package is never anything more than a `borrow Slice<u8>` the
caller already held and passed explicitly: nothing here reads an
environment variable, a file, or a keyring, and nothing here retains a
byte it was not handed for the duration of one call. See
`docs/AUTHENTICATION-SESSIONS-V1.md` for the threat model, the exact
non-claims (most importantly: this package does not and cannot compute a
real password hash or a real signature — both need a capability this
bounded interpreter does not expose without a new dependency or a new host
operation, neither of which this change is permitted to add).
---------------------------------------------------------------------
Constant-time comparison
---------------------------------------------------------------------
Compares every byte of both views without ever branching the *iteration
count* on where (or whether) a difference occurs. The loop condition is
`index < limit` alone — it never reads `mismatches` — so this function
always performs exactly `limit` byte reads and exactly `limit` comparisons
regardless of input content, unlike `std.bytes.equals`, whose `while index
< length && same` stops at the first differing byte and therefore leaks a
timing signal proportional to the shared prefix length. Any comparison of
a session id, a bearer token, a CSRF token, or a signature/MAC MUST use
this function, never `std.bytes.equals`.

```semaprax
fn ct_bytes_equal(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.auth.session.state_is_valid`

---------------------------------------------------------------------
Session lifecycle
---------------------------------------------------------------------

States (a `usize` tag, exactly the `std.jobs` idiom): 0 active, 1
rotated_out, 2 revoked, 3 idle_expired, 4 absolute_expired, 5 logged_out.
Only `active` is nonterminal. Every terminal state is sticky: none of the
transition functions below ever move a terminal state back to `active`,
which is the concrete defense against session fixation (an attacker who
captured a session id before it rotated, expired, or was revoked can never
resurrect it by presenting it again) and against replaying a stale
decision (mirrors AGENTS.md's "failure selection is sticky").

```semaprax
fn session_state_is_valid(state: usize) -> bool
```

### `std.auth.session.state_is_terminal`

```semaprax
fn session_state_is_terminal(state: usize) -> bool
```

### `std.auth.session.idle_expired`

```semaprax
fn session_idle_expired(now_tick: usize, idle_deadline_tick: usize) -> bool
```

### `std.auth.session.absolute_expired`

```semaprax
fn session_absolute_expired(now_tick: usize, absolute_deadline_tick: usize) -> bool
```

### `std.auth.session.is_usable`

The single question authentication middleware asks per request. Note what
this does NOT answer: whether the caller may perform any particular
action. That is a separate, independently supplied authorization decision
— see the module comment above and `docs/AUTHENTICATION-SESSIONS-V1.md`'s
"Authentication is not authorization" section. No function in this module
takes a session state and returns a capability, a role, or a permission.

```semaprax
fn session_is_usable(state: usize, now_tick: usize, idle_deadline_tick: usize, absolute_deadline_tick: usize) -> bool
```

### `std.auth.session.next_state_on_access`

```semaprax
fn session_next_state_on_access(state: usize, now_tick: usize, idle_deadline_tick: usize, absolute_deadline_tick: usize) -> usize
    ensures result <= 5usize
```

### `std.auth.session.rotate_is_legal`

```semaprax
fn session_rotate_is_legal(state: usize) -> bool
```

### `std.auth.session.next_state_on_rotate`

Rotation issues a brand-new session id bound to the same subject and
retires this one to `rotated_out` rather than deleting it outright, so a
later presentation of the retired id is observable (see the next
function) instead of silently failing closed with no signal at all.

```semaprax
fn session_next_state_on_rotate(state: usize) -> usize
    ensures result <= 5usize
```

### `std.auth.session.rotated_id_reuse_is_attack_signal`

A caller presenting a `rotated_out` id is a fixation/theft signal: the id
was valid once and is now being replayed after the legitimate holder
(or an attacker who raced it) rotated past it. The documented response is
to revoke the whole session family the rotation produced, not merely to
refuse this one request; this predicate only names the signal, since
"family" tracking is caller/storage state this pure layer does not hold.

```semaprax
fn session_rotated_id_reuse_is_attack_signal(state: usize) -> bool
```

### `std.auth.session.revoke_is_legal`

```semaprax
fn session_revoke_is_legal(state: usize) -> bool
```

### `std.auth.session.next_state_on_revoke`

```semaprax
fn session_next_state_on_revoke(state: usize) -> usize
    ensures result <= 5usize
```

### `std.auth.session.next_state_on_logout`

```semaprax
fn session_next_state_on_logout(state: usize) -> usize
    ensures result <= 5usize
```

### `std.auth.session.concurrent_limit_exceeded`

Concurrent-session policy: an explicit deployment choice
(`evict_oldest_on_limit`), never a silent default. 0 = admit, 1 = evict
the oldest active session and admit, 2 = refuse the new session.

```semaprax
fn session_concurrent_limit_exceeded(active_count: usize, max_concurrent: usize) -> bool
```

### `std.auth.session.concurrent_admission_decision`

```semaprax
fn session_concurrent_admission_decision(active_count: usize, max_concurrent: usize, evict_oldest_on_limit: bool) -> usize
    ensures result <= 2usize
```

### `std.auth.csrf.token_matches`

---------------------------------------------------------------------
CSRF and cookie policy
---------------------------------------------------------------------
Double-submit comparison MUST be constant-time: a CSRF token is exactly
the kind of secret-shaped value `ct_bytes_equal`'s doc comment names.

```semaprax
fn csrf_token_matches(presented: borrow Slice<u8>, expected: borrow Slice<u8>) -> bool
```

### `std.auth.csrf.required_for_method`

```semaprax
fn csrf_required_for_method(is_state_changing_method: bool) -> bool
```

### `std.auth.cookie.same_site_is_recognized`

SameSite: 0 = None, 1 = Lax, 2 = Strict. `None` is never accepted as safe.

```semaprax
fn cookie_same_site_is_recognized(same_site_mode: usize) -> bool
```

### `std.auth.cookie.attributes_are_safe`

```semaprax
fn cookie_attributes_are_safe(http_only: bool, secure: bool, same_site_mode: usize) -> bool
```

### `std.auth.token.algorithm_is_allowed`

---------------------------------------------------------------------
Token verification
---------------------------------------------------------------------

`alg_id`: 0 is the reserved "none"/unsigned marker and is never allowed;
1 and 2 name the two symmetric/asymmetric algorithm families this policy
admits (bound to real algorithm identities by the deployment, not by this
package — see the non-claims section of the owning spec). Any other value
is an algorithm-confusion attempt and is refused exactly like "none".

```semaprax
fn token_algorithm_is_allowed(alg_id: u8) -> bool
```

### `std.auth.token.key_id_is_allowed`

Key ids are public identifiers, not secrets (the key *material* is the
secret, and never appears in this package), so an ordinary
short-circuiting scan carries no timing-attack surface here.

```semaprax
fn token_key_id_is_allowed(key_id: u8, allowed_key_ids: borrow Slice<u8>) -> bool
```

### `std.auth.token.key_is_current_or_in_grace`

A key rotation keeps the previous key valid only until an explicit,
bounded grace deadline; after that the previous key is refused exactly
like any unknown key, so a rotated-out key cannot verify tokens forever.

```semaprax
fn token_key_is_current_or_in_grace(key_id: u8, active_key_id: u8, previous_key_id: u8, now_tick: usize, previous_key_grace_until_tick: usize) -> bool
```

### `std.auth.token.leeway_is_bounded`

Clock-skew leeway is bounded so no deployment configuration can turn it
into an unbounded acceptance window (the failure case AGENTS.md and this
issue both name explicitly).

```semaprax
fn token_leeway_is_bounded(leeway_ticks: usize) -> bool
```

### `std.auth.token.time_is_valid`

```semaprax
fn token_time_is_valid(now_tick: usize, not_before_tick: usize, expires_at_tick: usize, leeway_ticks: usize) -> bool
    requires token_leeway_is_bounded(leeway_ticks)
```

### `std.auth.token.replay_is_fresh`

```semaprax
fn token_replay_is_fresh(seen_before: bool) -> bool
```

### `std.auth.token.issuer_matches`

Issuer and audience are compared with the same constant-time primitive as
any other credential-shaped byte string: it costs nothing extra at these
sizes and removes one more thing a reviewer has to re-verify per caller.

```semaprax
fn token_issuer_matches(issuer: borrow Slice<u8>, expected_issuer: borrow Slice<u8>) -> bool
```

### `std.auth.token.audience_matches`

```semaprax
fn token_audience_matches(audience: borrow Slice<u8>, expected_audience: borrow Slice<u8>) -> bool
```

### `std.auth.token.claims_are_valid`

The closed policy check: every named attack class this profile's token
verification defends against (downgrade to "none", algorithm confusion,
unknown or stale key, issuer/audience substitution, expired or
not-yet-valid token, unbounded skew, replay) collapses to one boolean a
caller can gate a route on. `has_valid_signature` MUST come from a real
signature/MAC verification the deployment performs with its own key
material; this pure package cannot compute one (see the module comment
and the owning spec's non-claims) and takes it only as an opaque,
already-decided `bool`.

```semaprax
fn token_claims_are_valid(issuer: borrow Slice<u8>, expected_issuer: borrow Slice<u8>, audience: borrow Slice<u8>, expected_audience: borrow Slice<u8>, alg_id: u8, key_id: u8, active_key_id: u8, previous_key_id: u8, now_tick: usize, not_before_tick: usize, expires_at_tick: usize, leeway_ticks: usize, previous_key_grace_until_tick: usize, seen_before: bool) -> bool
    requires token_leeway_is_bounded(leeway_ticks)
```

### `std.auth.token.verification_admits`

```semaprax
fn token_verification_admits(has_valid_signature: bool, issuer: borrow Slice<u8>, expected_issuer: borrow Slice<u8>, audience: borrow Slice<u8>, expected_audience: borrow Slice<u8>, alg_id: u8, key_id: u8, active_key_id: u8, previous_key_id: u8, now_tick: usize, not_before_tick: usize, expires_at_tick: usize, leeway_ticks: usize, previous_key_grace_until_tick: usize, seen_before: bool) -> bool
    requires token_leeway_is_bounded(leeway_ticks)
```

### `std.auth.password.memory_cost_within_bounds`

---------------------------------------------------------------------
Password-hash policy bounds (NOT a hash function — see the spec's
non-claims section for exactly why this package stops here)
---------------------------------------------------------------------

```semaprax
fn password_memory_cost_within_bounds(memory_cost_kib: usize) -> bool
```

### `std.auth.password.time_cost_within_bounds`

```semaprax
fn password_time_cost_within_bounds(time_cost: usize) -> bool
```

### `std.auth.password.parallelism_within_bounds`

```semaprax
fn password_parallelism_within_bounds(parallelism: usize) -> bool
```

### `std.auth.password.policy_within_bounds`

Bounding both ends of every parameter is the direct defense against the
named failure "password-hash parameters can cause denial of service": a
deployment cannot configure a cost so low it is not memory-hard, nor so
high that hashing one password exhausts a request worker.

```semaprax
fn password_policy_within_bounds(memory_cost_kib: usize, time_cost: usize, parallelism: usize) -> bool
```

### `std.auth.password.needs_rehash`

```semaprax
fn password_needs_rehash(stored_policy_version: usize, current_policy_version: usize) -> bool
```

### `std.auth.audit.event_is_safe`

---------------------------------------------------------------------
Audit-event safety
---------------------------------------------------------------------
The closed refusal: an audit event is unsafe if it carries ANY of the
named secret-bearing fields, regardless of what else it carries. Adding a
new secret-bearing field to a real event type means adding its flag here
too — the check is exhaustive over this fixed list, not over whatever
fields a caller happens to think of.

```semaprax
fn audit_event_is_safe(carries_raw_password: bool, carries_password_hash: bool, carries_session_token: bool, carries_bearer_token: bool, carries_csrf_token: bool, carries_authorization_header: bool) -> bool
```

### `std.auth.audit.event_is_complete`

```semaprax
fn audit_event_is_complete(has_event_kind: bool, has_outcome: bool, has_timestamp: bool) -> bool
```

### `std.auth.oauth.state_matches`

---------------------------------------------------------------------
OAuth/OIDC authorization-code callback policy (the issue's "OAuth/OIDC
adapter interface as a later part of the same profile if scope permits")
---------------------------------------------------------------------
Same posture as token verification: this package decides over
caller-supplied signals only. It never opens a redirect, never calls a
token endpoint, and — exactly like `token_verification_admits`'s
`has_valid_signature` — never computes the SHA-256 hash PKCE's `S256`
method needs. `oauth_pkce_challenge_is_valid`'s `has_valid_s256_hash`
parameter is that same opaque, already-decided `bool`, for the same reason:
this pure, effect-free layer has no cryptographic primitive to compute one.

`state` binds an authorization request to its callback (the OAuth CSRF
defense) and `redirect_uri` is compared against the single value the
deployment registered for this client — both public, request-shaped
values, but compared with `ct_bytes_equal` anyway, exactly like
`token_issuer_matches`/`token_audience_matches`: it costs nothing extra at
these sizes and removes one more comparison a reviewer has to re-verify.

```semaprax
fn oauth_state_matches(presented: borrow Slice<u8>, expected: borrow Slice<u8>) -> bool
```

### `std.auth.oauth.redirect_uri_matches`

```semaprax
fn oauth_redirect_uri_matches(presented: borrow Slice<u8>, expected: borrow Slice<u8>) -> bool
```

### `std.auth.oauth.authorization_code_is_fresh`

An authorization code is single-use by construction: this is the same
"replay is fresh" question `token_replay_is_fresh` answers, under the
domain-specific name a caller composing an OAuth callback reads directly.

```semaprax
fn oauth_authorization_code_is_fresh(seen_before: bool) -> bool
```

### `std.auth.oauth.pkce_method_is_allowed`

`method_id`: 0 is the reserved "no PKCE" marker and is never allowed — this
policy requires PKCE on every authorization-code exchange; 1 names `plain`
and 2 names `S256`. Any other value is refused identically to "none", the
same closed-allow-list idiom `token_algorithm_is_allowed` uses.

```semaprax
fn oauth_pkce_method_is_allowed(method_id: u8) -> bool
```

### `std.auth.oauth.pkce_challenge_is_valid`

`plain` compares the verifier directly against the challenge with the
constant-time primitive (the verifier is secret-shaped up to the moment of
exchange). `S256` cannot be computed here; `has_valid_s256_hash` is the
deployment's own SHA-256 comparison result, taken as opaque input. Any
method outside the closed allow-list above is refused, never treated as
one of the two known methods by default.

```semaprax
fn oauth_pkce_challenge_is_valid(method_id: u8, code_verifier: borrow Slice<u8>, code_challenge: borrow Slice<u8>, has_valid_s256_hash: bool) -> bool
```

### `std.auth.oauth.authorization_request_is_valid`

The authorization-request-side check a caller performs before redirecting
the user agent to the identity provider: the PKCE method it is about to
commit to is one this policy allows. State and redirect-uri values here are
the ones this deployment is about to mint, checked against themselves only
to keep one call shape with the callback-side check below.

```semaprax
fn oauth_authorization_request_is_valid(state: borrow Slice<u8>, expected_state: borrow Slice<u8>, redirect_uri: borrow Slice<u8>, expected_redirect_uri: borrow Slice<u8>, pkce_method_id: u8) -> bool
```

### `std.auth.oauth.callback_is_valid`

The closed policy check for an authorization-code callback: state
(CSRF binding), redirect URI, PKCE challenge, and single-use freshness all
collapse to one boolean a caller can gate a token exchange on — the same
composition idiom `token_claims_are_valid` uses for its own attack classes.

```semaprax
fn oauth_callback_is_valid(state: borrow Slice<u8>, expected_state: borrow Slice<u8>, redirect_uri: borrow Slice<u8>, expected_redirect_uri: borrow Slice<u8>, pkce_method_id: u8, code_verifier: borrow Slice<u8>, code_challenge: borrow Slice<u8>, has_valid_s256_hash: bool, code_seen_before: bool) -> bool
```

### `std.auth.secret.self_check`

No `secret_wrap_i64`/`secret_expose_i64`/`*_handles_match` function ships
here, even though each one individually compiles: the package's own
conformance-completeness gate
(`tests/project/standard_library.rs`'s
`every_public_declaration_has_a_std_identity_contracts_examples_and_conformance`)
requires `std.auth.tests` to `use function` import every `@id`'d
declaration in this module — checked directly, not assumed, by adding
exactly these functions and watching that gate fail with "conformance
module does not import `std.auth.secret.wrap_i64`" before removing them
again. `use function` cannot import a function whose signature mentions a
record (`SPX-G172`, "function signature leaves the admitted scalar/Copy
workspace domain" — see point 4 above), so no wrap/expose/match function
over `Secret<T>` can satisfy both gates at once today. `Secret<T>` values
therefore stay call-local, constructed and read with a literal and `.value`
exactly like `secret_self_check` below — the only shape this tranche can
both ship and prove complete.

```semaprax
fn secret_self_check() -> bool
```

### `std.auth.identity_authorization.self_check`

Positive-control round-trip and distinctness proof, call-local exactly
like `secret_self_check`: an `Identity` and an `Authorization` are built
from caller-held scalars and read back through `.subject_id`/`.permitted`
directly, never through a function typed over either record (see point 5
above). The compile-time proof that the two types cannot be substituted
for one another lives in
`src/authentication/identity_authorization_source_tests.rs`, not here,
because a hostile case that must fail to compile cannot live inside a
package whose own test suite must build cleanly (the same reason
`secret_source_tests.rs` is a standalone Rust fixture and not part of this
package).

```semaprax
fn identity_authorization_self_check() -> bool
```

## `std.bytes`

Package `std/bytes`, tier `core`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.bytes = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.bytes.byte_to_i64`

```semaprax
fn byte_to_i64(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.bytes.get_or`

```semaprax
fn get_or(view: borrow Slice<u8>, index: usize, fallback: i64) -> i64
    requires fallback >= -1 && fallback <= 255
    ensures result >= -1 && result <= 255
```

### `std.bytes.index_of`

```semaprax
fn index_of(view: borrow Slice<u8>, needle: u8) -> i64
    ensures result >= -1
```

### `std.bytes.position_of`

```semaprax
fn position_of(next_index: usize) -> i64
    requires next_index >= 1usize
    ensures result >= 0
```

### `std.bytes.count`

```semaprax
fn count(view: borrow Slice<u8>, needle: u8) -> usize
    ensures result <= byte_len(view)
```

### `std.bytes.is_ascii`

```semaprax
fn is_ascii(view: borrow Slice<u8>) -> bool
```

### `std.bytes.equals`

```semaprax
fn equals(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.bytes.starts_with`

```semaprax
fn starts_with(view: borrow Slice<u8>, prefix: borrow Slice<u8>) -> bool
```

### `std.bytes.ends_with`

```semaprax
fn ends_with(view: borrow Slice<u8>, suffix: borrow Slice<u8>) -> bool
```

### `std.bytes.read_u16_le`

```semaprax
fn read_u16_le(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 2usize <= byte_len(view)
    ensures result >= 0 && result <= 65535
```

### `std.bytes.read_u16_be`

```semaprax
fn read_u16_be(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 2usize <= byte_len(view)
    ensures result >= 0 && result <= 65535
```

### `std.bytes.read_u32_le`

```semaprax
fn read_u32_le(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 4usize <= byte_len(view)
    ensures result >= 0 && result <= 4294967295
```

### `std.bytes.read_u32_be`

```semaprax
fn read_u32_be(view: borrow Slice<u8>, offset: usize) -> i64
    requires offset + 4usize <= byte_len(view)
    ensures result >= 0 && result <= 4294967295
```

### `std.bytes.is_space`

ASCII whitespace: space, horizontal tab, carriage return, line feed.  No
Unicode class is implied.

```semaprax
fn is_space(byte: u8) -> bool
```

### `std.bytes.trim_start`

```semaprax
fn trim_start(view: borrow Slice<u8>) -> usize
    ensures result <= byte_len(view)
```

### `std.bytes.trim_end`

```semaprax
fn trim_end(view: borrow Slice<u8>) -> usize
    ensures result <= byte_len(view)
```

### `std.bytes.is_blank`

```semaprax
fn is_blank(view: borrow Slice<u8>) -> bool
```

### `std.bytes.field_end`

Delimited fields keep empty fields: `a,,b` is three fields, and a trailing
delimiter opens one final empty field.

```semaprax
fn field_end(view: borrow Slice<u8>, start: usize, delimiter: u8) -> usize
    requires start <= byte_len(view)
    ensures result >= start && result <= byte_len(view)
```

### `std.bytes.field_start`

```semaprax
fn field_start(view: borrow Slice<u8>, start: usize, delimiter: u8) -> usize
    requires start <= byte_len(view)
    ensures result >= start && result <= byte_len(view)
```

### `std.bytes.field_count`

```semaprax
fn field_count(view: borrow Slice<u8>, delimiter: u8) -> usize
    ensures result >= 1usize
```

## `std.collections`

Package `std/collections`, tier `alloc`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.collections = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.collections.vec.with-capacity`

```semaprax
fn with_capacity<T>(capacity: usize) -> Vec<T>
```

### `std.collections.vec.push`

```semaprax
fn push<T>(values: own Vec<T>, value: T) -> Vec<T>
```

### `std.collections.vec.len`

```semaprax
fn len<T>(values: borrow Vec<T>) -> usize
```

### `std.collections.vec.capacity`

```semaprax
fn capacity<T>(values: borrow Vec<T>) -> usize
```

### `std.collections.vec.get`

```semaprax
fn get<T>(values: borrow Vec<T>, index: usize) -> T
```

### `std.collections.vec.reserve-exact`

```semaprax
fn reserve_exact<T>(values: own Vec<T>, additional: usize) -> Vec<T>
```

### `std.collections.vec.set`

```semaprax
fn set<T>(values: own Vec<T>, index: usize, value: T) -> Vec<T>
```

### `std.collections.vec.clear`

```semaprax
fn clear<T>(values: own Vec<T>) -> Vec<T>
```

## `std.core`

Package `std/core`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.core = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.core.ordering.less`

```semaprax
fn ordering_less() -> i64
    ensures result == -1
```

### `std.core.ordering.equal`

```semaprax
fn ordering_equal() -> i64
    ensures result == 0
```

### `std.core.ordering.greater`

```semaprax
fn ordering_greater() -> i64
    ensures result == 1
```

### `std.core.compare`

```semaprax
fn compare(left: i64, right: i64) -> i64
    ensures result >= -1 && result <= 1
    ensures result != 0 || left == right
    ensures result == 0 || left != right
```

### `std.core.min`

```semaprax
fn min(left: i64, right: i64) -> i64
    ensures result <= left && result <= right
    ensures result == left || result == right
```

### `std.core.max`

```semaprax
fn max(left: i64, right: i64) -> i64
    ensures result >= left && result >= right
    ensures result == left || result == right
```

### `std.core.clamp`

```semaprax
fn clamp(value: i64, low: i64, high: i64) -> i64
    requires low <= high
    ensures result >= low && result <= high
```

### `std.core.in_range`

```semaprax
fn in_range(value: i64, low: i64, high: i64) -> bool
    requires low <= high
    ensures result == (value >= low && value <= high)
```

### `std.core.bool_to_i64`

```semaprax
fn bool_to_i64(value: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.core.i64_to_bool`

```semaprax
fn i64_to_bool(value: i64) -> bool
    ensures result == (value != 0)
```

### `std.core.xor`

```semaprax
fn xor(left: bool, right: bool) -> bool
    ensures result == (left != right)
```

### `std.core.implies`

```semaprax
fn implies(premise: bool, conclusion: bool) -> bool
    ensures result == (!premise || conclusion)
```

## `std.data.csv`

Package `std/data-csv`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.csv = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.csv.field_count`

```semaprax
fn field_count(record: borrow Slice<u8>) -> usize
    ensures result >= 1usize
    ensures result <= byte_len(record) + 1usize
```

### `std.data.csv.has_balanced_quotes`

```semaprax
fn has_balanced_quotes(record: borrow Slice<u8>) -> bool
```

### `std.data.csv.is_well_formed_record`

```semaprax
fn is_well_formed_record(record: borrow Slice<u8>) -> bool
```

### `std.data.csv.field_end`

Quote-aware field cursors over one CSV record.  A field ends at the first
comma outside quotes; `""` inside a quoted field is one escaped quote and
never ends the field.

```semaprax
fn csv_field_end(record: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(record)
    ensures result >= start && result <= byte_len(record)
```

### `std.data.csv.field_start`

```semaprax
fn csv_field_start(record: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(record)
    ensures result >= start && result <= byte_len(record)
```

### `std.data.csv.field_is_quoted`

```semaprax
fn csv_field_is_quoted(record: borrow Slice<u8>, start: usize) -> bool
    requires start <= byte_len(record)
```

### `std.data.csv.content_start`

```semaprax
fn csv_content_start(record: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(record)
    ensures result >= start && result <= byte_len(record)
```

### `std.data.csv.content_end`

```semaprax
fn csv_content_end(record: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(record)
    ensures result >= csv_content_start(record, start) && result <= byte_len(record)
```

## `std.data.json`

Package `std/data-json`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.json = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.at_is`

```semaprax
fn at_is(input: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.data.json.at_in`

```semaprax
fn at_in(input: borrow Slice<u8>, index: usize, low: u8, high: u8) -> bool
```

### `std.data.json.failure`

```semaprax
fn failure(input: borrow Slice<u8>, offset: usize) -> usize
    ensures result > byte_len(input)
```

### `std.data.json.is_failure`

```semaprax
fn is_failure(input: borrow Slice<u8>, scan: usize) -> bool
```

### `std.data.json.failure_offset`

```semaprax
fn failure_offset(input: borrow Slice<u8>, scan: usize, fallback: usize) -> usize
```

### `std.data.json.skip_whitespace`

```semaprax
fn skip_whitespace(input: borrow Slice<u8>, start: usize) -> usize
    ensures result <= byte_len(input)
```

### `std.data.json.hex_at`

```semaprax
fn hex_at(input: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 15
```

### `std.data.json.code_unit`

```semaprax
fn code_unit(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1 && result <= 65535
```

### `std.data.json.escape_kind`

```semaprax
fn escape_kind(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= 0 && result <= 3
```

### `std.data.json.escape_end`

```semaprax
fn escape_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.string_end`

```semaprax
fn string_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.is_string`

```semaprax
fn is_string(input: borrow Slice<u8>) -> bool
```

## `std.data.json.dec`

Package `std/data-json-dec`, tier `alloc`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.data.json.dec = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.dec.at_is`

```semaprax
fn at_is(input: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.data.json.dec.at_in`

```semaprax
fn at_in(input: borrow Slice<u8>, index: usize, low: u8, high: u8) -> bool
```

### `std.data.json.dec.failure`

```semaprax
fn failure(input: borrow Slice<u8>, offset: usize) -> usize
    ensures result > byte_len(input)
```

### `std.data.json.dec.is_failure`

```semaprax
fn is_failure(input: borrow Slice<u8>, scan: usize) -> bool
```

### `std.data.json.dec.hex_at`

```semaprax
fn hex_at(input: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 15
```

### `std.data.json.dec.code_unit`

```semaprax
fn code_unit(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1 && result <= 65535
```

### `std.data.json.dec.escape_kind`

```semaprax
fn escape_kind(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= 0 && result <= 3
```

### `std.data.json.dec.escape_end`

```semaprax
fn escape_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.dec.string_end`

```semaprax
fn string_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.dec.token_end`

```semaprax
fn token_end(input: borrow Slice<u8>, index: usize) -> usize
```

### `std.data.json.dec.simple_scalar`

```semaprax
fn simple_scalar(byte: u8) -> i64
    ensures result >= -1 && result <= 92
```

### `std.data.json.dec.byte_code`

```semaprax
fn byte_code(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.data.json.dec.code_byte`

```semaprax
fn code_byte(value: i64) -> u8
```

### `std.data.json.dec.scalar_at`

```semaprax
fn scalar_at(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1 && result <= 1114111
```

### `std.data.json.dec.utf8_len`

```semaprax
fn utf8_len(scalar: i64) -> usize
    ensures result <= 4usize
```

### `std.data.json.dec.utf8_at`

```semaprax
fn utf8_at(scalar: i64, index: usize) -> i64
    ensures result >= -1 && result <= 255
```

### `std.data.json.dec.emit_len`

```semaprax
fn emit_len(input: borrow Slice<u8>, index: usize) -> usize
    ensures result <= 4usize
```

### `std.data.json.dec.emit_at`

```semaprax
fn emit_at(input: borrow Slice<u8>, index: usize, offset: usize) -> i64
    ensures result >= -1 && result <= 255
```

### `std.data.json.dec.scan-string`

```semaprax
fn scan_string(input: borrow Slice<u8>, start: usize, measure: bool) -> usize
```

### `std.data.json.dec.decoded_len`

```semaprax
fn decoded_len(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.dec.decoded_size`

```semaprax
fn decoded_size(input: borrow Slice<u8>) -> usize
```

### `std.data.json.dec.decode-into`

```semaprax
fn decode_into(input: borrow Reader, output: own Writer) -> Writer
    requires match borrow input { Reader { data: source, position: start } => match borrow output { Writer { data: target, position: write } => start <= byte_len(bytes_as_slice(source)) && !is_failure(bytes_as_slice(source), decoded_len(bytes_as_slice(source), start)) && write <= byte_len(bytes_as_slice(target)) && decoded_len(bytes_as_slice(source), start) <= byte_len(bytes_as_slice(target)) - write, }, }
```

### `std.data.json.dec.capacity`

```semaprax
fn capacity() -> usize
    ensures result == 256usize
```

### `std.data.json.dec.prefix_eq`

```semaprax
fn prefix_eq(left: borrow Slice<u8>, right: borrow Slice<u8>, length: usize) -> bool
```

### `std.data.json.dec.slice_eq`

```semaprax
fn slice_eq(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.data.json.dec.decoded_eq`

```semaprax
fn decoded_eq(input: borrow Slice<u8>, start: usize, expect: borrow Slice<u8>) -> bool
```

### `std.data.json.dec.decoded_token_eq`

Compares two decoded JSON string tokens for equality by walking both
through the existing pull surface (`emit_at`, `emit_len`, `token_end`)
with no intermediate buffer, so escapes decode before comparison and
`"ab"` equals `"ab"`.

```semaprax
fn decoded_token_eq(input: borrow Slice<u8>, left: usize, right: usize) -> bool
```

## `std.data.json.digits`

Package `std/data-json-digits`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.data.json.digits = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.digits.i64_len`

```semaprax
fn i64_len(value: i64) -> i64
    ensures result >= 1 && result <= 20
```

### `std.data.json.digits.i64_byte`

```semaprax
fn i64_byte(value: i64, index: i64) -> i64
    ensures result >= -1 && result <= 57
```

### `std.data.json.digits.literal_len`

```semaprax
fn literal_len(kind: i64) -> i64
    ensures result >= 0 && result <= 5
```

### `std.data.json.digits.literal_word`

```semaprax
fn literal_word(kind: i64) -> i64
    ensures result >= 0
```

### `std.data.json.digits.literal_byte`

```semaprax
fn literal_byte(kind: i64, index: i64) -> i64
    ensures result >= -1 && result <= 255
```

## `std.data.json.doc`

Package `std/data-json-doc`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.json.doc = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.doc.at_in`

```semaprax
fn at_in(input: borrow Slice<u8>, index: usize, low: u8, high: u8) -> bool
```

### `std.data.json.doc.skip_space`

```semaprax
fn skip_space(input: borrow Slice<u8>, start: usize) -> usize
    ensures result <= byte_len(input)
```

### `std.data.json.doc.string_end`

```semaprax
fn string_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.doc.digits_end`

```semaprax
fn digits_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.doc.number_end`

```semaprax
fn number_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.doc.literal_end`

```semaprax
fn literal_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.doc.advance`

```semaprax
fn advance(input: borrow Slice<u8>, index: usize, step: i64) -> usize
```

### `std.data.json.doc.step_action`

```semaprax
fn step_action(input: borrow Slice<u8>, index: usize, mode: i64, stack: i64) -> i64
```

### `std.data.json.doc.next_state`

```semaprax
fn next_state(action: i64, mode: i64, stack: i64) -> i64
```

### `std.data.json.doc.document_end`

```semaprax
fn document_end(input: borrow Slice<u8>, start: usize, depth_limit: usize) -> usize
```

### `std.data.json.doc.whole_end`

```semaprax
fn whole_end(input: borrow Slice<u8>, depth_limit: usize) -> usize
```

### `std.data.json.doc.is_document`

```semaprax
fn is_document(input: borrow Slice<u8>) -> bool
```

### `std.data.json.doc.byte_same`

```semaprax
fn byte_same(input: borrow Slice<u8>, left: usize, right: usize) -> bool
```

### `std.data.json.doc.span_same`

```semaprax
fn span_same(input: borrow Slice<u8>, left: usize, right: usize, length: usize) -> bool
```

### `std.data.json.doc.next_key`

```semaprax
fn next_key(input: borrow Slice<u8>, key: usize, depth_limit: usize) -> usize
```

### `std.data.json.doc.key_before`

```semaprax
fn key_before(input: borrow Slice<u8>, first: usize, key: usize, depth_limit: usize) -> bool
```

### `std.data.json.doc.object_keys`

```semaprax
fn object_keys(input: borrow Slice<u8>, open: usize, depth_limit: usize) -> bool
```

### `std.data.json.doc.unique_end`

```semaprax
fn unique_end(input: borrow Slice<u8>, depth_limit: usize) -> usize
```

### `std.data.json.doc.is_unique`

```semaprax
fn is_unique(input: borrow Slice<u8>) -> bool
```

## `std.data.json.token`

Package `std/data-json-token`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.json.token = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.token.at_is`

```semaprax
fn at_is(input: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.data.json.token.digit_at`

```semaprax
fn digit_at(input: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 9
```

### `std.data.json.token.digits_end`

```semaprax
fn digits_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.integer_end`

```semaprax
fn integer_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.fraction_end`

```semaprax
fn fraction_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.exponent_end`

```semaprax
fn exponent_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.number_end`

```semaprax
fn number_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.is_number`

```semaprax
fn is_number(input: borrow Slice<u8>) -> bool
```

### `std.data.json.token.word_is`

```semaprax
fn word_is(input: borrow Slice<u8>, start: usize, first: u8, second: u8, third: u8, fourth: u8) -> bool
```

### `std.data.json.token.literal_kind`

```semaprax
fn literal_kind(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= 0 && result <= 3
```

### `std.data.json.token.literal_end`

```semaprax
fn literal_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.token.is_literal`

```semaprax
fn is_literal(input: borrow Slice<u8>) -> bool
```

### `std.data.json.token.i64_or`

```semaprax
fn i64_or(input: borrow Slice<u8>, start: usize, fallback: i64) -> i64
```

## `std.data.json.utf8`

Package `std/data-json-utf8`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.json.utf8 = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.utf8.at_is`

```semaprax
fn at_is(input: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.data.json.utf8.at_in`

```semaprax
fn at_in(input: borrow Slice<u8>, index: usize, low: u8, high: u8) -> bool
```

### `std.data.json.utf8.offset_at`

```semaprax
fn offset_at(input: borrow Slice<u8>, index: usize, base: u8, span: i64) -> i64
    requires span >= 0 && span <= 127
    ensures result >= -1 && result <= 127
```

### `std.data.json.utf8.continuation_at`

```semaprax
fn continuation_at(input: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 63
```

### `std.data.json.utf8.sequence_kind`

```semaprax
fn sequence_kind(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= 0 && result <= 4
```

### `std.data.json.utf8.is_surrogate`

```semaprax
fn is_surrogate(value: i64) -> bool
```

### `std.data.json.utf8.is_shortest`

```semaprax
fn is_shortest(kind: i64, value: i64) -> bool
```

### `std.data.json.utf8.scalar_at`

```semaprax
fn scalar_at(input: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1 && result <= 1114111
```

### `std.data.json.utf8.sequence_end`

```semaprax
fn sequence_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.utf8.utf8_end`

```semaprax
fn utf8_end(input: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.json.utf8.is_utf8`

```semaprax
fn is_utf8(input: borrow Slice<u8>) -> bool
```

## `std.data.json.write`

Package `std/data-json-write`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.data.json.write = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.json.write.byte_code`

```semaprax
fn byte_code(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.data.json.write.code_byte`

```semaprax
fn code_byte(value: i64) -> u8
```

### `std.data.json.write.hex_digit`

```semaprax
fn hex_digit(value: i64) -> i64
    ensures result >= 48 && result <= 102
```

### `std.data.json.write.escape_len`

```semaprax
fn escape_len(byte: u8) -> usize
    ensures result >= 1usize && result <= 6usize
```

### `std.data.json.write.escape_byte`

```semaprax
fn escape_byte(byte: u8, index: usize) -> i64
    ensures result >= -1 && result <= 255
```

### `std.data.json.write.width_at`

```semaprax
fn width_at(input: borrow Slice<u8>, index: usize) -> usize
```

### `std.data.json.write.encoded_at`

```semaprax
fn encoded_at(input: borrow Slice<u8>, index: usize, offset: usize) -> i64
    ensures result >= -1 && result <= 255
```

### `std.data.json.write.quoted-range-len`

```semaprax
fn quoted_range_len(input: borrow Slice<u8>, start: usize) -> usize
    ensures result >= 2usize
```

### `std.data.json.write.quoted_len`

```semaprax
fn quoted_len(input: borrow Slice<u8>) -> usize
    ensures result >= 2usize
```

### `std.data.json.write.quoted_byte`

```semaprax
fn quoted_byte(input: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 255
```

### `std.data.json.write.quoted-remaining-len`

```semaprax
fn quoted_remaining_len(input: borrow Reader) -> usize
    requires match borrow input { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
    ensures result >= 2usize
```

### `std.data.json.write.quoted-into`

```semaprax
fn quoted_into(input: borrow Reader, output: own Writer) -> Writer
    requires match borrow input { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && quoted_remaining_len(input) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.data.json.write.digit_code`

```semaprax
fn digit_code(value: usize) -> i64
    ensures result >= 48 && result <= 57
```

### `std.data.json.write.usize_len`

```semaprax
fn usize_len(value: usize) -> usize
    ensures result >= 1usize && result <= 20usize
```

### `std.data.json.write.usize_byte`

```semaprax
fn usize_byte(value: usize, index: usize) -> i64
    ensures result >= -1 && result <= 57
```

### `std.data.json.write.count-into`

```semaprax
fn count_into(value: usize, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && usize_len(value) <= byte_len(bytes_as_slice(data)) - position, }
```

## `std.data.toml`

Package `std/data-toml`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.data.toml = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.data.toml.is_bare_byte`

```semaprax
fn is_bare_byte(byte: u8) -> bool
```

### `std.data.toml.is_bare_key`

```semaprax
fn is_bare_key(value: borrow Slice<u8>) -> bool
```

### `std.data.toml.is_blank`

```semaprax
fn is_blank(line: borrow Slice<u8>) -> bool
```

### `std.data.toml.is_comment`

```semaprax
fn is_comment(line: borrow Slice<u8>) -> bool
```

### `std.data.toml.assignment_index`

```semaprax
fn assignment_index(line: borrow Slice<u8>) -> i64
    ensures result >= -1
```

### `std.data.toml.scan_failure`

```semaprax
fn scan_failure(record: borrow Slice<u8>, offset: usize) -> usize
    ensures result > byte_len(record)
```

### `std.data.toml.hex_run_end`

```semaprax
fn hex_run_end(record: borrow Slice<u8>, start: usize, count: usize) -> usize
```

### `std.data.toml.basic_escape_end`

The escape at `start` (the backslash itself) admits the TOML basic-string
escapes `\b \t \n \f \r \" \\`, `\uXXXX` and `\UXXXXXXXX`; any other byte
after the backslash fails the scan.

```semaprax
fn basic_escape_end(record: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.toml.basic_quoted_key_end`

Offset just past a `"..."` key starting at `start`, or a failure encoding
(a result greater than `byte_len(record)`) for an unterminated string, a
raw control byte below 0x20, or any escape `basic_escape_end` rejects.

```semaprax
fn basic_quoted_key_end(record: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.toml.literal_quoted_key_end`

Offset just past a `'...'` key starting at `start`, or a failure encoding
for an unterminated string or a raw control byte below 0x20. A literal
key admits no escapes: a backslash is an ordinary byte.

```semaprax
fn literal_quoted_key_end(record: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.toml.bare_key_end`

Offset just past a bare key starting at `start`, or a failure encoding when
no bare-key byte is admitted at `start`.
The scan walks bytes one at a time with `byte_get`, then calls `byte_range`
once after the loop, reusing `is_bare_key` on the exact `[start, index)`
sub-slice the scan found.
This is a design choice, not a compiler constraint. `byte_range` IS admitted
inside a `while` body: `hir::validation::iterator_loops` has its own
admission arm for `ResolvedExprKind::ByteRange`, requiring an authenticated
byte-slice alias, and that node never reaches
`owned_buffer::require_admitted_while_operation` (which governs `ByteOp`
calls such as `byte_len`/`byte_get`/`bytes_set`). Verified directly against
this compiler. An earlier revision of this comment claimed the opposite.
One post-loop call is preferred because it decodes one range-descriptor
carrier per key instead of one per byte, and because it is
not a no-op check: it is the only call in this module that actually decodes
a range-descriptor carrier end to end (build it from `start`/`index`, hand
it across the Wasm host boundary, read it back), so a corrupt carrier on
any backend surfaces here as a scan failure rather than silently passing.

```semaprax
fn bare_key_end(record: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.toml.key_end`

Offset just past any admitted key form at `start` - bare, basic-quoted, or
literal-quoted - dispatching on the opening byte, or a failure encoding
from whichever scanner applies.

```semaprax
fn key_end(record: borrow Slice<u8>, start: usize) -> usize
```

### `std.data.toml.value_start`

Start offset of the value after the assignment `delimiter`, skipping
spaces and tabs.

```semaprax
fn value_start(record: borrow Slice<u8>, delimiter: usize) -> usize
    ensures result <= byte_len(record)
```

### `std.data.toml.quote_state`

Offset of the first unquoted `#` at or after `start`, or the end of the
record when none appears. A `#` inside a basic or literal quoted span
never ends the value.

```semaprax
fn quote_state(byte: u8, basic: bool, literal: bool) -> i64
    ensures result >= 0 && result <= 3
```

### `std.data.toml.value_content_end`

```semaprax
fn value_content_end(record: borrow Slice<u8>, start: usize) -> usize
    ensures result <= byte_len(record)
```

### `std.data.toml.value_end`

End offset of the value after the assignment `delimiter`: the last byte
before an unquoted `#` comment or end of line, with trailing spaces and
tabs excluded.

```semaprax
fn value_end(record: borrow Slice<u8>, delimiter: usize) -> usize
    ensures result <= byte_len(record)
    ensures result >= value_start(record, delimiter)
```

## `std.db`

Package `std/db`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.db = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.db.descriptor.tag_is_valid`

```semaprax
fn descriptor_tag_is_valid(tag: u8) -> bool
```

### `std.db.descriptor.mismatch_class`

```semaprax
fn descriptor_mismatch_class(expected: borrow Slice<u8>, actual: borrow Slice<u8>) -> usize
    ensures result <= 4usize
```

### `std.db.descriptor.matches`

```semaprax
fn descriptor_matches(expected: borrow Slice<u8>, actual: borrow Slice<u8>) -> bool
```

### `std.db.identifier.is_safe_byte`

```semaprax
fn identifier_is_safe_byte(byte: u8) -> bool
```

### `std.db.identifier.is_valid`

```semaprax
fn identifier_is_valid(name: borrow Slice<u8>) -> bool
```

### `std.db.transaction.can_begin`

```semaprax
fn transaction_can_begin(state: usize) -> bool
```

### `std.db.transaction.next_on_begin`

```semaprax
fn transaction_next_on_begin(state: usize) -> usize
    ensures result <= 4usize
```

### `std.db.transaction.next_on_commit`

```semaprax
fn transaction_next_on_commit(state: usize) -> usize
    ensures result <= 4usize
```

### `std.db.transaction.next_on_rollback`

```semaprax
fn transaction_next_on_rollback(state: usize) -> usize
    ensures result <= 4usize
```

### `std.db.transaction.next_on_connection_lost`

```semaprax
fn transaction_next_on_connection_lost(state: usize) -> usize
    ensures result <= 4usize
```

### `std.db.transaction.is_open`

```semaprax
fn transaction_is_open(state: usize) -> bool
```

### `std.db.transaction.is_settled`

```semaprax
fn transaction_is_settled(state: usize) -> bool
```

### `std.db.migration.is_out_of_order`

```semaprax
fn migration_is_out_of_order(last_applied: u8, candidate: u8) -> bool
```

### `std.db.migration.is_duplicate`

```semaprax
fn migration_is_duplicate(applied: borrow Slice<u8>, candidate: u8) -> bool
```

### `std.db.migration.missing_predecessor`

```semaprax
fn migration_missing_predecessor(applied_count: u8, candidate: u8) -> bool
```

### `std.db.migration.checksum_drift`

```semaprax
fn migration_checksum_drift(recorded: u8, candidate: u8) -> bool
```

### `std.db.limits.within_bounds`

```semaprax
fn limits_within_bounds(rows: usize, max_rows: usize, bytes: usize, max_bytes: usize, columns: usize, max_columns: usize, elapsed_ms: usize, max_elapsed_ms: usize) -> bool
```

### `std.db.limits.should_stop`

```semaprax
fn limits_should_stop(consumed_rows: usize, max_rows: usize) -> bool
```

## `std.email`

Package `std/email`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.email = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.email.byte_is_header_separator`

Issue #193's outbound-email slice: the pure, effect-free decision layer an
email adapter consults BEFORE it opens a connection, assembles a message,
or hands anything to a transport. It performs no I/O, declares no
`permit`, calls no `uses`-gated operation, and never accepts, retains, or
returns a credential. This is the shape `std.log.redact` and `std.auth`
already established here: policy as data, transport left to the caller.

What this package is NOT: it does not send email, resolve MX records,
speak SMTP, authenticate to a provider, or render a MIME body. Those need
network and secret capability and remain future work. Nothing here is
evidence that outbound delivery exists.

Every function is scalar-in/scalar-out or `borrow Slice<u8>`-in/
scalar-out, so a message's bytes are never copied into or out of this
package.
---------------------------------------------------------------------
Header injection
---------------------------------------------------------------------
The one property here that is a security boundary rather than a
convenience. SMTP separates headers by CRLF, so a caller-controlled value
(a display name, a subject, a Reply-To taken from user input) carrying CR
or LF can append arbitrary headers or terminate the header block and
inject a body. NUL is refused alongside them because it truncates in a C
transport regardless of the length the adapter believes it has.

This is deliberately a refusal, not a sanitizer: a value that fails is
rejected, never silently stripped, so a caller cannot ship a message whose
content quietly differs from what it assembled.

```semaprax
fn byte_is_header_separator(candidate: u8) -> bool
```

### `std.email.value_is_header_safe`

```semaprax
fn value_is_header_safe(field: borrow Slice<u8>) -> bool
```

### `std.email.local_part_len_admitted`

---------------------------------------------------------------------
RFC 5321 size limits
---------------------------------------------------------------------
Three separate predicates rather than one, because an adapter reporting
"address too long" without saying which half was over is exactly the
diagnostic failure this repository keeps finding elsewhere. A local part is
at most 64 octets, a domain at most 255, and the whole forward path at most
254. Each half must also be non-empty: `@example.com` and `user@` are
refusals, not zero-length successes.

```semaprax
fn local_part_len_admitted(length: usize) -> bool
```

### `std.email.domain_len_admitted`

```semaprax
fn domain_len_admitted(length: usize) -> bool
```

### `std.email.address_len_admitted`

```semaprax
fn address_len_admitted(length: usize) -> bool
```

### `std.email.at_sign_count`

---------------------------------------------------------------------
Address shape
---------------------------------------------------------------------
Exactly one `@`. Counting rather than finding-the-first is deliberate:
`a@b@c` has a plausible first `@` and is not an address, and an adapter
that split on the first separator would send it somewhere.

```semaprax
fn at_sign_count(address: borrow Slice<u8>) -> usize
```

### `std.email.at_sign_index`

The index of the single `@`, defined only where there is exactly one, so a
caller cannot ask this about `a@b@c` and receive a usable answer.

```semaprax
fn at_sign_index(address: borrow Slice<u8>) -> usize
    requires at_sign_count(address) == 1usize
```

### `std.email.domain_shape_admitted`

A domain must carry at least one dot and may not begin or end with one.
`user@localhost` is a valid address in some deployments and is refused here
on purpose: an outbound adapter that accepts it will queue mail for a name
no public resolver can answer.

```semaprax
fn domain_shape_admitted(address: borrow Slice<u8>, start: usize) -> bool
    requires start <= byte_len(address)
```

### `std.email.address_admitted`

The composed judgement an adapter actually calls. Every clause is one of
the predicates above, so a refusal can always be attributed to a named rule
rather than to this function as a whole.

```semaprax
fn address_admitted(address: borrow Slice<u8>) -> bool
```

### `std.email.recipient_count_admitted`

---------------------------------------------------------------------
Envelope budgets and completeness
---------------------------------------------------------------------
One deployment-independent recipient ceiling. An adapter that accepts an
unbounded recipient list turns one caller-controlled field into a fan-out
amplifier; 64 is deliberately small, and an adapter profile is free to
lower it, never to leave it unstated.

```semaprax
fn recipient_count_admitted(count: usize) -> bool
```

### `std.email.envelope_is_complete`

A message with no sender, no recipient, or no subject is not a message an
adapter should attempt; each is a distinct flag so a caller learns which
one it failed to assemble. An empty body is admitted - it is a legitimate
message - while an absent sender is not.

```semaprax
fn envelope_is_complete(has_sender: bool, has_recipient: bool, has_subject: bool) -> bool
```

### `std.email.value_is_header_safe_guarded`

---------------------------------------------------------------------
Redaction guard (issue #193)
---------------------------------------------------------------------
`value_is_header_safe` above refuses only CR/LF/NUL -- the header
injection boundary. It has no notion of a header's own secret-classified
content: a Reply-To, a Subject, or a display name that echoes back an SMTP
credential or a session token used to authenticate the send is ordinary
header-safe text and passes unchanged. This composes that byte-safety
check with `std.log.redact.event_is_safe`'s six caller-declared flags,
exactly like `std.log.append-event-guarded` gates a log event and
`std.http.value_is_header_safe_guarded` gates an outbound HTTP header, so
a header value the caller has classified as a secret is refused before it
is ever assembled into a message.

```semaprax
fn value_is_header_safe_guarded(field: borrow Slice<u8>, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.email.header-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn header_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.email.subject-len-admitted`

---------------------------------------------------------------------
Message budgets, header fields, and attachments (issue #193)
---------------------------------------------------------------------
Everything above judges an address and a header value's injection safety.
A message is more than its envelope: a subject long enough to fold across
lines, a body big enough to exhaust a provider's quota, and an attachment
whose declared filename is the one byte sequence that reaches a recipient's
filesystem are all caller-controlled and all unbounded until stated here.

This slice states them. It sends nothing, encodes nothing, and opens no
connection: every function is scalar-in/scalar-out or
`borrow Slice<u8>`-in/scalar-out, and a `true` answer is a decision about
an already-assembled value, never authority to transmit it.

```semaprax
fn subject_len_admitted(length: usize) -> bool
```

### `std.email.header-line-len-admitted`

RFC 5322's 998-octet line ceiling, stated rather than assumed. A header
value longer than this is refused instead of being folded by the adapter:
folding is a rewrite, and a rewrite of a caller's value is exactly where
an injected sequence gets reassembled into something else.

```semaprax
fn header_line_len_admitted(length: usize) -> bool
```

### `std.email.body-len-limit`

```semaprax
fn body_len_limit() -> usize
```

### `std.email.body-len-admitted`

```semaprax
fn body_len_admitted(length: usize) -> bool
```

### `std.email.attachment-count-admitted`

An adapter that accepts an unbounded attachment list turns one caller into
a provider-quota amplifier, exactly as an unbounded recipient list turns
it into a fan-out amplifier.

```semaprax
fn attachment_count_admitted(count: usize) -> bool
```

### `std.email.attachment-len-limit`

```semaprax
fn attachment_len_limit() -> usize
```

### `std.email.attachment-len-admitted`

```semaprax
fn attachment_len_admitted(length: usize) -> bool
```

### `std.email.total-size-limit`

```semaprax
fn total_size_limit() -> usize
```

### `std.email.total-size-admitted`

The per-part ceilings do not compose into a whole-message ceiling on their
own: sixteen attachments each inside their own limit still exceed what a
provider will accept. The total is checked separately, with both operands
already bounded so the sum cannot wrap.

```semaprax
fn total_size_admitted(body_len: usize, attachment_bytes: usize) -> bool
```

### `std.email.header-name-byte-admitted`

---------------------------------------------------------------------
Header fields
---------------------------------------------------------------------
A field name is an RFC 5322 printable-ASCII token with no colon: a name
that can carry a colon can carry a whole second header.

```semaprax
fn header_name_byte_admitted(candidate: u8) -> bool
```

### `std.email.header-name-admitted`

```semaprax
fn header_name_admitted(name: borrow Slice<u8>) -> bool
```

### `std.email.header-field-admitted`

A complete field: a token name, an injection-safe value, and a line that
fits. All three, because each alone lets a different malformed header out.

```semaprax
fn header_field_admitted(name: borrow Slice<u8>, value: borrow Slice<u8>) -> bool
```

### `std.email.filename-byte-admitted`

---------------------------------------------------------------------
Attachments
---------------------------------------------------------------------
A filename reaches a recipient's filesystem. Path separators, quotes,
backslashes, semicolons and control bytes are refused rather than escaped:
escaping is a rewrite, and a `Content-Disposition` parameter is exactly
where a rewrite gets un-escaped by someone else's parser.

```semaprax
fn filename_byte_admitted(candidate: u8) -> bool
```

### `std.email.filename-admitted`

```semaprax
fn filename_admitted(name: borrow Slice<u8>) -> bool
```

### `std.email.media-type-byte-admitted`

`type/subtype`, exactly one solidus, each half a non-empty token. A media
type carrying parameters is refused here: a parameter is another place a
caller-controlled value crosses into a header.

```semaprax
fn media_type_byte_admitted(candidate: u8) -> bool
```

### `std.email.media-type-admitted`

```semaprax
fn media_type_admitted(value: borrow Slice<u8>) -> bool
```

### `std.email.attachment-admitted`

```semaprax
fn attachment_admitted(filename: borrow Slice<u8>, media_type: borrow Slice<u8>, length: usize) -> bool
```

### `std.email.message-admitted`

The composed message judgement. Every clause is one of the predicates
above, so a refusal is attributable to a named rule rather than to this
function as a whole, and a caller learns which budget it exceeded.

```semaprax
fn message_admitted(subject: borrow Slice<u8>, recipient_count: usize, body_len: usize, attachment_count: usize, attachment_bytes: usize) -> bool
```

### `std.email.message-admitted-guarded`

The same judgement with the shared redaction policy's six caller-declared
flags applied, so a message the caller has classified as carrying a secret
in its subject or body is refused before it is assembled, independent of
every budget above.

```semaprax
fn message_admitted_guarded(subject: borrow Slice<u8>, recipient_count: usize, body_len: usize, attachment_count: usize, attachment_bytes: usize, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

## `std.encoding`

Package `std/encoding`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.encoding = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.encoding.byte_value`

```semaprax
fn byte_value(byte: u8) -> i64
    ensures result >= 0 && result <= 255
```

### `std.encoding.hex_value`

```semaprax
fn hex_value(byte: u8) -> i64
    ensures result >= -1 && result <= 15
```

### `std.encoding.is_hex_digit`

```semaprax
fn is_hex_digit(byte: u8) -> bool
    ensures result == (byte >= 48u8 && byte <= 57u8 || byte >= 65u8 && byte <= 70u8 || byte >= 97u8 && byte <= 102u8)
```

### `std.encoding.decode_hex_byte`

```semaprax
fn decode_hex_byte(high: u8, low: u8) -> i64
    ensures result >= -1 && result <= 255
```

### `std.encoding.encode_hex_lower`

```semaprax
fn encode_hex_lower(value: i64) -> i64
    requires value >= 0 && value <= 15
    ensures result >= 48 && result <= 102
```

### `std.encoding.encode_hex_upper`

```semaprax
fn encode_hex_upper(value: i64) -> i64
    requires value >= 0 && value <= 15
    ensures result >= 48 && result <= 70
```

### `std.encoding.base64_value`

```semaprax
fn base64_value(byte: u8) -> i64
    ensures result >= -1 && result <= 63
```

### `std.encoding.is_base64_digit`

```semaprax
fn is_base64_digit(byte: u8) -> bool
    ensures result == (byte >= 65u8 && byte <= 90u8 || byte >= 97u8 && byte <= 122u8 || byte >= 48u8 && byte <= 57u8 || byte == 43u8 || byte == 47u8)
```

### `std.encoding.encode_base64_digit`

```semaprax
fn encode_base64_digit(value: i64) -> i64
    requires value >= 0 && value <= 63
    ensures result >= 43 && result <= 122
```

### `std.encoding.decode_base64_quad`

```semaprax
fn decode_base64_quad(first: u8, second: u8, third: u8, fourth: u8) -> i64
    ensures result >= -1 && result <= 16777215
```

## `std.encoding.base64`

Package `std/encoding-base64`, tier `core`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.encoding.base64 = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.encoding.base64.len`

Padded standard Base64 encoding over a borrowed byte view.  There is no
buffer and no allocation: `base64_byte` is a pull-based digit accessor, so a
caller writes the encoded digits wherever it wants, one at a time, in any
order.

```semaprax
fn base64_len(input: usize) -> usize
    ensures result % 4usize == 0usize
```

### `std.encoding.base64.byte_at_or_zero`

```semaprax
fn byte_at_or_zero(view: borrow Slice<u8>, index: usize) -> i64
    ensures result >= 0 && result <= 255
```

### `std.encoding.base64.byte`

```semaprax
fn base64_byte(view: borrow Slice<u8>, index: usize) -> i64
    requires index < base64_len(byte_len(view))
    ensures result >= 43 && result <= 122
```

## `std.env`

Package `std/env`, tier `hosted`, status partial. Required project profile: `environment-io.v1`. Dependency: `std.env = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.env.count`

```semaprax
fn count() -> usize
    uses { process.environment.read }
```

### `std.env.name-len`

```semaprax
fn name_len(index: usize) -> usize
    uses { process.environment.read }
```

### `std.env.value-len`

```semaprax
fn value_len(index: usize) -> usize
    uses { process.environment.read }
```

### `std.env.name-is`

```semaprax
fn name_is(index: usize, key: borrow str) -> bool
    uses { process.environment.read }
```

### `std.env.index-of`

```semaprax
fn index_of(key: borrow str) -> usize
    uses { process.environment.read }
```

### `std.env.name-into`

```semaprax
fn name_into(index: usize, output: own Writer) -> Writer
    uses { process.environment.read }
```

### `std.env.value-into`

```semaprax
fn value_into(index: usize, output: own Writer) -> Writer
    uses { process.environment.read }
```

## `std.env.policy`

Package `std/env-policy`, tier `core`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.env.policy = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.env.policy.name-byte-is-valid`

Effect-free name and `NAME=VALUE` assignment policy for environment-style
byte data. Every function here is a pure offset computation over borrowed
bytes: no host, capability, or environment access, no allocation.
A byte admitted inside a portable environment-variable name: an ASCII
letter, digit, or underscore.

```semaprax
fn name_byte_is_valid(byte: u8) -> bool
```

### `std.env.policy.name-starts-with-digit`

Whether `name`'s first byte is an ASCII digit; a portable name never
begins with one.

```semaprax
fn name_starts_with_digit(name: borrow Slice<u8>) -> bool
```

### `std.env.policy.name-is-valid`

Portable environment-variable name validity: non-empty, does not begin
with a digit, and every byte is `A-Z`, `a-z`, `0-9` or `_`. This is the
POSIX portable character set for names (IEEE Std 1003.1 XBD 8.1, the
"Environment Variable" definition built on the portable filename
character set); it claims exactly that portable subset and nothing
broader such as locale-specific or vendor-extended name shapes.

```semaprax
fn name_is_valid(name: borrow Slice<u8>) -> bool
```

### `std.env.policy.value-is-valid`

Environment-variable value validity: contains no NUL byte. Values are
otherwise arbitrary bytes; this predicate does not require UTF-8 or any
other text encoding, and an empty value is valid.

```semaprax
fn value_is_valid(value: borrow Slice<u8>) -> bool
```

### `std.env.policy.assignment-separator`

Offset of the first `=` in a `NAME=VALUE` assignment view, or
`byte_len(view)` when no `=` is present. A separator byte can never sit at
`byte_len(view)` itself, so that bound doubles as the "not found"
sentinel: the same convention `std.env.index-of` uses `count()` for a
missing key index.

```semaprax
fn assignment_separator(view: borrow Slice<u8>) -> usize
    ensures result <= byte_len(view)
```

### `std.env.policy.assignment-name-end`

End offset of the name span: identical to the separator offset, since a
well-formed assignment's name occupies exactly `view[0..separator]`.

```semaprax
fn assignment_name_end(view: borrow Slice<u8>) -> usize
    ensures result <= byte_len(view)
```

### `std.env.policy.assignment-value-start`

Start offset of the value span, one byte past the separator; clamped to
`byte_len(view)` when no separator is present so the offset always stays
in bounds for a caller that slices `view` with it.

```semaprax
fn assignment_value_start(view: borrow Slice<u8>) -> usize
    ensures result <= byte_len(view)
```

### `std.env.policy.name-range-is-valid`

Whether `view` is a well-formed `NAME=VALUE` assignment: an `=` is
present, the name span (1) is a valid portable name, and the value span
(2) contains no NUL byte. A view with no `=`, an empty name, or a NUL
anywhere in the name is rejected.

```semaprax
fn name_range_is_valid(view: borrow Slice<u8>, end: usize) -> bool
    requires end <= byte_len(view)
```

### `std.env.policy.value-range-is-valid`

```semaprax
fn value_range_is_valid(view: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(view)
```

### `std.env.policy.name-span-is-valid`

Issue #100 found this composition (a single `byte_range` sub-slice
consumed by a looping callee) reporting a corrupt slice carrier on the
Core Wasm lane; that turned out to be a hand-rolled harness decoder that
predated `byte_range`'s range-descriptor tag (issue #100, comment
5626417135), not a backend defect, so this reuses `byte_range` again per
issue #218 rather than the index-walking `name_range_is_valid`/
`value_range_is_valid` workaround (kept above as their own gated
functions, unchanged).

```semaprax
fn name_span_is_valid(view: borrow Slice<u8>, end: usize) -> bool
    requires end <= byte_len(view)
```

### `std.env.policy.value-span-is-valid`

```semaprax
fn value_span_is_valid(view: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(view)
```

### `std.env.policy.assignment-is-valid`

```semaprax
fn assignment_is_valid(view: borrow Slice<u8>) -> bool
```

## `std.export.policy`

Package `std/export-policy`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.export.policy = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.export.policy.batch_size_admitted`

Issue #193's missing exporter interface. Six observability/notification
families now ship as `std/` decision layers -- `std.log` (+
`std.log.redact`), `std.metrics`, `std.tracing`, `std.http`, `std.webhook`,
`std.email` -- and every one of them can judge an observation. None of
them can be handed to a sink: nothing in this repository decided a batch
size, a payload budget, a target identity, a backpressure response, or a
retry schedule for an actual emission.

An exporter in a language with no ambient filesystem, process, or network
authority cannot itself export: this package grants no such authority and
performs no I/O. What it admits instead is the judgement a host adapter
(one written in a hosting language, wired to a real transport outside
SEMAPRAX's capability model) consults BEFORE it emits a batch to a sink --
exactly the shape `std.webhook.attempt_admitted`/`backoff_seconds` already
established for a single delivery, generalized here to cover a batch of
observations rather than one webhook attempt, and made available to all
six families rather than owned by any one of them. It declares no
`permit`, calls no `uses`-gated operation, and never accepts, retains, or
returns a payload's or a target's raw bytes beyond the `borrow Slice<u8>`
this package's own predicates scan.

What this package is NOT: it is not a working exporter. Calling
`export_admitted` and receiving `true` is a decision, not a delivery --
nothing here opens a connection, writes a byte, advances a queue's actual
storage, or retries anything. `try_admit_export` returns the depth a
caller's own counter should become; it does not enqueue anything itself.
A settlement or concurrency model is proof data, not permission to perform
a physical finalizer, exactly per this repository's own invariant, and an
admitted export decision must not itself be, or imply, one. Nor does this
package invent a seventh injection-safety vocabulary: target-identity
safety is `std.http.value_is_header_safe`, the same predicate
`std.email.value_is_header_safe` already mirrors byte-for-byte, reused
rather than re-scanned so a target identifier is judged by exactly the
rule every other family's sink identifier already is.

`attempt_admitted` and `backoff_seconds` below intentionally do NOT import
`std.webhook`'s functions of the same shape: `std.webhook` is a
family-specific leaf package for one delivery protocol, and this package
is meant to sit underneath all six families, so the dependency edge must
not run from the general layer to one specific family. The two packages
therefore carry the same retry-admission algorithm on purpose -- a
deliberate, documented shape-parity, not the kind of duplication this
repository's own change protocol warns against, which is duplicating a
*check* inside a single package's control flow, not two packages sharing a
vocabulary by convention.

Every function is scalar-in/scalar-out or `borrow Slice<u8>`-in/
scalar-out, so a payload, a target identifier, or a signing/transport
credential is never copied into or out of this package.
---------------------------------------------------------------------
Batch admission
---------------------------------------------------------------------
A caller assembling a batch of observations (log events, metric series,
spans, webhook deliveries, or outbound emails) to hand a sink adapter in
one call needs two independent, deployment-independent ceilings: how many
records the batch may carry, and how many serialized bytes it may occupy.
A host that buffers an unbounded batch before deciding anything about it
has handed an attacker (or a runaway producer) a memory cost with no
admission in front of it, exactly the failure `std.webhook.payload_len_admitted`
already refuses for one inbound delivery. A batch of zero records is not
an export attempt, so the record count is refused at zero the same way
`std.webhook.attempt_admitted` refuses an attempt numbered zero; the byte
ceiling has no such lower bound, since a batch of small or empty-valued
records can legitimately serialize to very few bytes.

```semaprax
fn batch_size_admitted(count: usize) -> bool
```

### `std.export.policy.batch_bytes_admitted`

```semaprax
fn batch_bytes_admitted(length: usize) -> bool
```

### `std.export.policy.target_id_len_admitted`

---------------------------------------------------------------------
Target identity admission
---------------------------------------------------------------------
The sink a batch is destined for is named by an opaque identifier this
package never parses as a URL, a host, or a path -- `std.url` already owns
that grammar, and reimplementing it here would be exactly the "seventh
vocabulary" this package's header disclaims. What this package admits
instead is the identifier's *shape*: bounded length, and free of the bytes
that let a caller-controlled identifier splice a second header, field, or
record into whatever text-oriented registry, log line, or label a host
adapter later renders it into -- `std.http.value_is_header_safe`, reused
rather than reimplemented. Refused at zero length: an empty target names
no sink at all.

```semaprax
fn target_id_len_admitted(length: usize) -> bool
```

### `std.export.policy.target_id_admitted`

```semaprax
fn target_id_admitted(target: borrow Slice<u8>) -> bool
```

### `std.export.policy.queue_depth_max`

---------------------------------------------------------------------
Backpressure / drop-policy admission
---------------------------------------------------------------------
One deployment-independent ceiling on how many batches may be queued for a
sink at once, and the admission that actually has teeth: once a queue
already holds `queue_depth_max()` batches, the next attempt returns the
fixed refusal sentinel -1 (mirroring `std.metrics.try_admit_series`'s own
-1-for-out-of-range idiom) rather than growing the queue past its bound or
returning the unchanged depth as if admission had succeeded. The refusal
signal IS the drop decision this package admits: a host adapter that
receives -1 drops (or refuses to enqueue) the batch, rather than this
package choosing between a drop-oldest and a refuse-newest policy itself
-- that choice is the host's, made from data this package supplies, not an
authority this package exercises.

```semaprax
fn queue_depth_max() -> i64
```

### `std.export.policy.try_admit_export`

```semaprax
fn try_admit_export(existing_depth: i64) -> i64
    requires existing_depth >= 0
    requires existing_depth <= queue_depth_max()
    ensures result == -1 || result >= 1 && result <= queue_depth_max()
```

### `std.export.policy.attempt_admitted`

---------------------------------------------------------------------
Retry / backoff admission
---------------------------------------------------------------------
A sink adapter retrying a failed emission without a ceiling turns one
failing sink into an outbound amplifier, exactly the reasoning
`std.webhook.attempt_admitted`/`backoff_seconds` already carry for a
single webhook delivery (see the package header for why this package
carries its own copy rather than importing theirs). Attempts are counted
from 1, so 0 is refused rather than treated as "before the first".

```semaprax
fn attempt_admitted(attempt: i64) -> bool
```

### `std.export.policy.backoff_seconds`

Exponential backoff with a fixed base and a hard ceiling, defined for
exactly the admitted attempt numbers so a caller cannot ask about attempt
9 and receive a plausible delay.

```semaprax
fn backoff_seconds(attempt: i64) -> i64
    requires attempt_admitted(attempt)
    ensures result >= 1
    ensures result <= 60
```

### `std.export.policy.export_admitted`

---------------------------------------------------------------------
Composed export admission
---------------------------------------------------------------------
The judgement a host adapter actually calls before it emits one batch to
one sink: the sink's queue has room for another batch, the batch's record
count and serialized byte length are both within budget, and the target
identifier is admitted. Every clause is one of the named predicates (or,
for the depth check, the same two-sided range `try_admit_export` enforces
through its own `requires`), so a refusal is always attributable to a
named rule. This function decides; it never enqueues, sends, or retries
anything itself.

```semaprax
fn export_admitted(existing_depth: i64, batch_count: usize, batch_bytes: usize, target: borrow Slice<u8>) -> bool
```

## `std.format`

Package `std/format`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.format = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.format.byte`

```semaprax
fn byte(value: i64) -> u8
    requires value >= 0 && value <= 255
```

### `std.format.usize-len`

```semaprax
fn usize_len(value: usize) -> usize
    ensures result >= 1usize && result <= 20usize
```

### `std.format.usize-byte`

```semaprax
fn usize_byte(value: usize, index: usize) -> u8
    requires index < usize_len(value)
```

### `std.format.digit-byte`

```semaprax
fn digit_byte(value: usize) -> u8
    requires value <= 9usize
```

### `std.format.i64-len`

```semaprax
fn i64_len(value: i64) -> usize
    ensures result >= 1usize && result <= 20usize
```

### `std.format.i64-byte`

```semaprax
fn i64_byte(value: i64, index: usize) -> u8
    requires index < i64_len(value)
```

### `std.format.append-str`

```semaprax
fn append_str(value: borrow str, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && byte_len(str_as_bytes(value)) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.append-usize`

```semaprax
fn append_usize(value: usize, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && usize_len(value) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.append-i64`

```semaprax
fn append_i64(value: i64, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && i64_len(value) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.append-bool`

```semaprax
fn append_bool(value: bool, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && if value { 4usize } else { 5usize } <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.pad-len`

```semaprax
fn pad_len(content: usize, width: usize) -> usize
    ensures result >= content && result >= width
```

### `std.format.append-fill`

`count` copies of one fill byte, preflighted against the live capacity.

```semaprax
fn append_fill(fill: u8, count: usize, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && count <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.append-str-left`

Left-aligned text in a field of `width`: the content, then fill bytes.  A
content longer than the field is written in full and never truncated.

```semaprax
fn append_str_left(value: borrow str, width: usize, fill: u8, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && pad_len(byte_len(str_as_bytes(value)), width) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.format.append-usize-right`

Right-aligned decimal in a field of `width`: fill bytes, then the digits.

```semaprax
fn append_usize_right(value: usize, width: usize, fill: u8, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && pad_len(usize_len(value), width) <= byte_len(bytes_as_slice(data)) - position, }
```

## `std.fs`

Package `std/fs`, tier `hosted`, status partial. Required project profile: `filesystem-io.v3`. Dependency: `std.fs = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.fs.file-info`

```semaprax
record FileInfo {
    kind: usize,
    size: usize,
}
```

### `std.fs.write-outcome`

/ Publication outcome of one checked atomic replacement. Uncertain is not
/ permission to retry: the host must resolve the attempted publication.

```semaprax
variant WriteOutcome {
    Published,
    NotPublished,
    Uncertain,
}
```

### `std.fs.read`

```semaprax
fn read(path: own Path, max: usize) -> Reader
    uses { fs.read }
    requires path_valid(path)
```

### `std.fs.write-new`

```semaprax
fn write_new(path: own Path, writer: own Writer) -> usize
    uses { fs.write }
    requires path_valid(path)
```

### `std.fs.metadata`

```semaprax
fn metadata(path: own Path) -> FileInfo
    uses { fs.read }
    requires path_valid(path)
```

### `std.fs.list`

```semaprax
fn list(path: own Path, max: usize) -> Reader
    uses { fs.read }
    requires path_valid(path)
```

### `std.fs.create-dir`

```semaprax
fn create_dir(path: own Path) -> usize
    uses { fs.write }
    requires path_valid(path)
```

### `std.fs.remove`

```semaprax
fn remove(path: own Path) -> usize
    uses { fs.write }
    requires path_valid(path)
```

### `std.fs.write-atomic`

```semaprax
fn write_atomic(path: own Path, writer: own Writer) -> usize
    uses { fs.write }
    requires path_valid(path)
```

### `std.fs.write-atomic-checked`

/ Replace the target with the writer's logical prefix and retain the
/ provider's closed publication outcome. ABI defects still fail closed.

```semaprax
fn write_atomic_checked(path: own Path, writer: own Writer) -> WriteOutcome
    uses { fs.write }
    requires path_valid(path)
```

### `std.fs.directory.entry-length`

```semaprax
fn entry_length(reader: borrow Reader) -> usize
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.fs.directory.entry-byte`

```semaprax
fn entry_byte(reader: borrow Reader, index: usize) -> u8
    requires index < entry_length(reader)
```

### `std.fs.directory.next-entry`

```semaprax
fn next_entry(reader: own Reader) -> Reader
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.fs.listing.entry-end`

```semaprax
fn listing_entry_end(listing: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(listing)
    ensures result >= start && result <= byte_len(listing)
```

### `std.fs.listing.next-start`

```semaprax
fn listing_next_start(listing: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(listing)
    ensures result >= start && result <= byte_len(listing)
```

### `std.fs.listing.entry-count`

```semaprax
fn listing_entry_count(listing: borrow Slice<u8>) -> usize
```

### `std.fs.listing.name-is-dot`

```semaprax
fn listing_name_is_dot(listing: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(listing)
```

### `std.fs.listing.name-is-dotdot`

```semaprax
fn listing_name_is_dotdot(listing: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(listing)
```

### `std.fs.listing.byte-clean`

```semaprax
fn listing_byte_clean(byte: u8) -> bool
```

### `std.fs.listing.name-clean`

```semaprax
fn listing_name_clean(listing: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(listing)
```

### `std.fs.listing.entry-valid`

```semaprax
fn listing_entry_valid(listing: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(listing)
```

### `std.fs.listing.valid`

```semaprax
fn listing_valid(listing: borrow Slice<u8>) -> bool
```

## `std.http`

Package `std/http`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.http = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.http.digit_value`

```semaprax
fn digit_value(byte: u8) -> i64
    ensures result >= -1 && result <= 9
```

### `std.http.digit_at`

```semaprax
fn digit_at(view: borrow Slice<u8>, index: usize) -> i64
    ensures result >= -1 && result <= 9
```

### `std.http.byte_is`

```semaprax
fn byte_is(view: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.http.lower`

```semaprax
fn lower(byte: u8) -> u8
```

### `std.http.method_is_valid`

```semaprax
fn method_is_valid(method: borrow Slice<u8>) -> bool
```

### `std.http.status_code`

```semaprax
fn status_code(response: borrow Slice<u8>) -> i64
    ensures result >= -1 && result <= 999
```

### `std.http.is_success`

```semaprax
fn is_success(code: i64) -> bool
```

### `std.http.terminator`

```semaprax
fn terminator(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.has_header_end`

```semaprax
fn has_header_end(response: borrow Slice<u8>) -> bool
```

### `std.http.header_end`

```semaprax
fn header_end(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.body_len`

```semaprax
fn body_len(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.name_at`

```semaprax
fn name_at(response: borrow Slice<u8>, index: usize) -> bool
```

### `std.http.length_start`

```semaprax
fn length_start(response: borrow Slice<u8>) -> usize
    ensures result <= byte_len(response)
```

### `std.http.skip_blanks`

```semaprax
fn skip_blanks(view: borrow Slice<u8>, cursor: usize) -> usize
    ensures result >= cursor
```

### `std.http.decimal_at`

```semaprax
fn decimal_at(view: borrow Slice<u8>, start: usize) -> i64
    ensures result >= -1
```

### `std.http.content_length`

```semaprax
fn content_length(response: borrow Slice<u8>) -> i64
    ensures result >= -1
```

### `std.http.byte_is_request_separator`

---------------------------------------------------------------------
Outbound request safety (issue #193)
---------------------------------------------------------------------
Everything above judges a response this package received. These judge a
request a caller is about to assemble, which is the direction that can be
made to attack someone else.

CR, LF and NUL in a caller-controlled header value or request target are
the HTTP request-splitting and response-splitting boundary: a value
carrying CRLF ends the current field and begins a new one, so an attacker
who controls part of a header can inject headers, a body, or an entire
second request. This refuses such a value rather than stripping the bytes,
exactly as `std.email.value_is_header_safe` refuses them for SMTP, so a
caller can never ship a request whose bytes quietly differ from what it
assembled.

```semaprax
fn byte_is_request_separator(byte: u8) -> bool
```

### `std.http.value_is_header_safe`

```semaprax
fn value_is_header_safe(field: borrow Slice<u8>) -> bool
```

### `std.http.byte_is_tchar_symbol`

RFC 9110 field-name is a `token`: one or more of the tchar set. Admitting
anything outside it is what lets a space or a colon split one field into
two, so this is a closed allowlist rather than a denylist of separators.
Split from `byte_is_tchar` rather than written as one chain: the single
expression exceeded the cleanup-replay skeleton-work budget (SPX-H006).
The set is RFC 9110's tchar punctuation: ! # $ % & ' * + - . ^ _ ` | ~

```semaprax
fn byte_is_tchar_symbol(byte: u8) -> bool
```

### `std.http.byte_is_tchar`

```semaprax
fn byte_is_tchar(byte: u8) -> bool
```

### `std.http.header_name_is_token`

```semaprax
fn header_name_is_token(name: borrow Slice<u8>) -> bool
```

### `std.http.request_target_is_safe`

An origin-form request target must also carry no space: a space ends the
target in the request line, so a target containing one shifts the HTTP
version field and is a request-line injection in its own right.

```semaprax
fn request_target_is_safe(target: borrow Slice<u8>) -> bool
```

### `std.http.target_is_origin_form`

---------------------------------------------------------------------
Userinfo refusal (issue #193): the classic credential-in-a-URL leak
---------------------------------------------------------------------
RFC 7230 Section 5.3.1 defines origin-form as `absolute-path [ "?" query ]`
— it starts with `/` and carries no authority component at all, so a `@`
byte inside one is ordinary path or query content (an email address in a
query value, say) and must not be refused. Every other request-target form
this package's caller might assemble — absolute-form (`scheme://user:pass
@host/path`, the proxy form) and authority-form (`host:port`, the CONNECT
form) — carries an authority component, and RFC 7230 Section 5.3.3 is
explicit that a sender "MUST NOT generate" userinfo in either: `user:pass@`
is deprecated precisely because it ships a credential in the one part of a
request line every intermediary and access log sees in clear text. This is
therefore a flat refusal of `@` outside origin-form, not a URL parser: it
never inspects a scheme, a host, or a port, so it cannot mistake one
request-target form for another.

```semaprax
fn target_is_origin_form(target: borrow Slice<u8>) -> bool
```

### `std.http.byte_is_at_sign`

```semaprax
fn byte_is_at_sign(byte: u8) -> bool
```

### `std.http.non_origin_target_admits_no_userinfo`

```semaprax
fn non_origin_target_admits_no_userinfo(target: borrow Slice<u8>) -> bool
```

### `std.http.request_target_admits_no_userinfo`

```semaprax
fn request_target_admits_no_userinfo(target: borrow Slice<u8>) -> bool
```

### `std.http.request_line_admitted`

The composed judgement a caller asks once, before assembling anything.

```semaprax
fn request_line_admitted(method: borrow Slice<u8>, target: borrow Slice<u8>) -> bool
```

### `std.http.value_is_header_safe_guarded`

---------------------------------------------------------------------
Outbound header value redaction guard (issue #193)
---------------------------------------------------------------------
`value_is_header_safe` above refuses only the request-splitting bytes; it
has no notion of a value's own secret-classified content (an Authorization
header's bearer token, say). This composes that byte-safety check with
`std.log.redact.event_is_safe`'s six caller-declared secret-bearing flags,
exactly like `std.log.append-event-guarded` gates a log event, so an
outbound header whose value the caller has classified as a secret is
refused before it is ever assembled into a request, independent of whether
its bytes would otherwise pass the separator scan.

```semaprax
fn value_is_header_safe_guarded(field: borrow Slice<u8>, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.http.header-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn header_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.http.diagnostic-target-byte-admitted`

Diagnostic/display target policy, not request routing: only a bounded plain
origin path. Queries, fragments, percent escapes and userinfo are refused,
never stripped into a misleading URL. A classified secret path is refused
even if its spelling passes. This is not endpoint or DNS authorization.

```semaprax
fn diagnostic_target_byte_admitted(value: u8) -> bool
```

### `std.http.diagnostic-target-admitted`

```semaprax
fn diagnostic_target_admitted(target: borrow Slice<u8>, classified_secret: bool) -> bool
```

### `std.http.scheme-byte-matches`

---------------------------------------------------------------------
Outbound client policy (issue #193)
---------------------------------------------------------------------
Everything above judges bytes a server or a client already has: a request
line, a header name, a status line, a body length read off a parsed
message. None of it decides whether an outbound request should be made at
all. This slice is that decision layer: transport, deadline, body budget,
redirect policy, a normalized outcome taxonomy, and the retry rule that
keeps an at-most-once effect from becoming an at-least-once one.

It performs no I/O, declares no `permit`, calls no `uses`-gated operation,
and resolves no name. Endpoint *identity* -- whether a host and port were
allowlisted at all, and whether the target is an internal address -- is
`std.net`'s job and is passed in here as the caller's already-made
decision rather than re-derived, so there is exactly one allowlist rule in
the standard library instead of two that can disagree.

```semaprax
fn scheme_byte_matches(scheme: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.http.scheme-is-https`

Exactly `https`, case-folded. A scheme check that accepts a prefix admits
`https-evil`, and one that is case-sensitive is bypassed by `HTTPS`.

```semaprax
fn scheme_is_https(scheme: borrow Slice<u8>) -> bool
```

### `std.http.scheme-is-http`

```semaprax
fn scheme_is_http(scheme: borrow Slice<u8>) -> bool
```

### `std.http.transport-admitted`

Cleartext is refused rather than upgraded. Silently rewriting `http` to
`https` hides a deployment's mistake; refusing it surfaces one.

```semaprax
fn transport_admitted(scheme: borrow Slice<u8>) -> bool
```

### `std.http.deadline-millis-max`

---------------------------------------------------------------------
Deadlines
---------------------------------------------------------------------
An outbound call with no deadline is an unbounded hold on the caller's
own budget. The ceiling is stated so that a caller cannot pass a
"deadline" of a day and call the request bounded.

```semaprax
fn deadline_millis_max() -> i64
```

### `std.http.deadline-admitted`

```semaprax
fn deadline_admitted(budget_millis: i64) -> bool
```

### `std.http.deadline-remaining`

The budget left, never negative and never larger than the budget itself,
so a caller that passes a clock reading older than its own start cannot
manufacture extra time.

```semaprax
fn deadline_remaining(started_at: i64, now: i64, budget_millis: i64) -> i64
    requires started_at >= 0
    requires now >= started_at
    requires deadline_admitted(budget_millis)
```

### `std.http.deadline-expired`

```semaprax
fn deadline_expired(started_at: i64, now: i64, budget_millis: i64) -> bool
    requires started_at >= 0
    requires now >= started_at
    requires deadline_admitted(budget_millis)
```

### `std.http.request-body-limit`

---------------------------------------------------------------------
Body budgets
---------------------------------------------------------------------
Two separate ceilings on purpose: what this process is willing to send is
a different decision from how much of a remote answer it is willing to
buffer, and a response budget derived from the request budget is how a
small request ends up authorizing a large allocation.

```semaprax
fn request_body_limit() -> usize
```

### `std.http.response-body-limit`

```semaprax
fn response_body_limit() -> usize
```

### `std.http.request-body-admitted`

```semaprax
fn request_body_admitted(length: usize) -> bool
```

### `std.http.response-body-admitted`

```semaprax
fn response_body_admitted(length: usize) -> bool
```

### `std.http.redirect-depth-max`

---------------------------------------------------------------------
Redirects
---------------------------------------------------------------------
A redirect is a second request to an address the caller never chose. Each
hop is re-judged against the same endpoint policy as the first, the chain
is bounded, and credentials do not travel across an origin boundary.

```semaprax
fn redirect_depth_max() -> i64
```

### `std.http.redirect-depth-admitted`

```semaprax
fn redirect_depth_admitted(depth: i64) -> bool
```

### `std.http.status-is-redirect`

```semaprax
fn status_is_redirect(code: i64) -> bool
```

### `std.http.redirect-preserves-method`

307 and 308 replay the original method and body; 301, 302 and 303 are the
codes clients historically rewrite to GET. Stating which is which keeps a
redirect from silently turning a POST into a GET, or a GET into a replayed
POST.

```semaprax
fn redirect_preserves_method(code: i64) -> bool
    requires status_is_redirect(code)
```

### `std.http.redirect-forwards-credentials`

All three components must match. A redirect that keeps the host but drops
to cleartext, or keeps the name but changes the port, is a different
origin and must not carry the first request's Authorization header.

```semaprax
fn redirect_forwards_credentials(same_scheme: bool, same_host: bool, same_port: bool) -> bool
```

### `std.http.redirect-admitted`

The whole hop. `target_endpoint_admitted` is `std.net.endpoint-admitted`'s
answer for the *new* location, not the original one: re-checking the
allowlist at every hop is the property that stops a redirect from being
the way out of it.

```semaprax
fn redirect_admitted(code: i64, depth: i64, target_scheme: borrow Slice<u8>, target: borrow Slice<u8>, target_endpoint_admitted: bool) -> bool
```

### `std.http.outcome-admitted`

---------------------------------------------------------------------
Normalized outcomes
---------------------------------------------------------------------
One closed vocabulary instead of a transport's own error strings, so a
caller's retry logic is written against a stable code and a diagnostic
never carries a URL, a header, or a credential out of the adapter.

 0 completed              9 protocol-error
 1 endpoint-refused      10 server-transient
 2 dns-failed            11 uncertain
 3 tls-failed
 4 connect-failed
 5 deadline-exceeded
 6 cancelled
 7 redirect-refused
 8 body-limit-exceeded

```semaprax
fn outcome_admitted(outcome: i64) -> bool
```

### `std.http.outcome-may-have-reached-origin`

Whether the request may already have been processed by the origin. This
is the distinction that decides whether retrying a non-idempotent request
risks a duplicate effect: a DNS failure never reached anything, a
deadline may have.

```semaprax
fn outcome_may_have_reached_origin(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.http.outcome-is-uncertain`

An outcome whose delivery state is genuinely unknown. `uncertain` is a
first-class answer rather than a failure: an adapter that reports "failed"
for a request that may have succeeded invites exactly the duplicate the
retry rule below exists to prevent.

```semaprax
fn outcome_is_uncertain(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.http.outcome-is-retryable`

A retry can only help when the condition is transient. An endpoint refused
by policy, a refused redirect, and an over-budget body are all decisions
that will be made again identically, so retrying them is pure amplification.

```semaprax
fn outcome_is_retryable(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.http.method-equals`

---------------------------------------------------------------------
Idempotence and retry
---------------------------------------------------------------------

```semaprax
fn method_equals(method: borrow Slice<u8>, expected: borrow Slice<u8>) -> bool
```

### `std.http.method-is-idempotent`

The closed idempotent set. POST and PATCH are absent on purpose: repeating
either is a second external effect, which is exactly what this issue's
"no automatic retries of non-idempotent outbound requests" rule forbids.

```semaprax
fn method_is_idempotent(method: borrow Slice<u8>) -> bool
```

### `std.http.attempt-admitted`

```semaprax
fn attempt_admitted(attempt: i64) -> bool
```

### `std.http.retry-admitted`

The rule with teeth. A retry is admitted only when the outcome is
transient, the attempt budget is not spent, and one of three things is
true: the request never reached the origin, the method is idempotent by
definition, or the caller supplied an idempotency key the origin can
deduplicate against. A non-idempotent request with no key that may already
have been processed is refused, which is the uncertain-delivery case.

```semaprax
fn retry_admitted(method: borrow Slice<u8>, outcome: i64, attempt: i64, has_idempotency_key: bool) -> bool
    requires outcome_admitted(outcome)
```

### `std.http.outbound-request-admitted`

---------------------------------------------------------------------
The composed outbound request
---------------------------------------------------------------------
Every clause is one of the named predicates above, so a refusal is always
attributable to a stated rule rather than to this function as a whole. A
`true` answer is a decision about a request's shape and policy, never a
capability to perform it.

```semaprax
fn outbound_request_admitted(method: borrow Slice<u8>, scheme: borrow Slice<u8>, target: borrow Slice<u8>, endpoint_allowlisted: bool, body_len: usize, budget_millis: i64) -> bool
```

## `std.io`

Package `std/io`, tier `portable`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.io = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.io.reader`

```semaprax
record Reader {
    data: Bytes,
    position: usize,
}
```

### `std.io.writer`

```semaprax
record Writer {
    data: Bytes,
    position: usize,
}
```

### `std.io.reader.from-bytes`

```semaprax
fn reader_from_bytes(data: own Bytes) -> Reader
    ensures result.position == 0usize
```

### `std.io.reader.cursor`

```semaprax
fn reader_position(reader: borrow Reader) -> usize
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.reader.remaining`

```semaprax
fn reader_remaining(reader: borrow Reader) -> usize
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.reader.peek`

```semaprax
fn reader_peek(reader: borrow Reader) -> u8
    requires match borrow reader { Reader { data, position } => position < byte_len(bytes_as_slice(data)), }
```

### `std.io.reader.advance`

```semaprax
fn reader_advance(reader: own Reader, count: usize) -> Reader
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.reader.finish`

```semaprax
fn reader_finish(reader: own Reader) -> Bytes
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.writer.from-bytes`

```semaprax
fn writer_from_bytes(data: own Bytes) -> Writer
    ensures result.position == 0usize
```

### `std.io.writer.cursor`

```semaprax
fn writer_position(writer: borrow Writer) -> usize
    requires match borrow writer { Writer { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.writer.remaining`

```semaprax
fn writer_remaining(writer: borrow Writer) -> usize
    requires match borrow writer { Writer { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.writer.write-u8`

```semaprax
fn writer_write_u8(writer: own Writer, value: u8) -> Writer
    requires match borrow writer { Writer { data, position } => position < byte_len(bytes_as_slice(data)), }
```

### `std.io.writer.finish`

`writer_finish` returns the caller-provided buffer: bytes in `[0, position)`
were initialized by `writer_write_u8`; the remaining suffix is unchanged.

```semaprax
fn writer_finish(writer: own Writer) -> Bytes
    requires match borrow writer { Writer { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

## `std.io.lines`

Package `std/io-lines`, tier `portable`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.io.lines = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.io.lines.line-end`

Line processing over a borrowed byte view.  `line_end` is the absolute offset
of the first line-feed at or after `start`, or the view length when the view
carries no further terminator.  A line's content excludes that line feed and
one immediately preceding carriage return, so canonical LF and CRLF inputs
yield the same content bytes.

```semaprax
fn line_end(view: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(view)
    ensures result >= start && result <= byte_len(view)
```

### `std.io.lines.line-terminated`

```semaprax
fn line_terminated(view: borrow Slice<u8>, start: usize) -> bool
    requires start <= byte_len(view)
```

### `std.io.lines.line-content-len`

```semaprax
fn line_content_len(view: borrow Slice<u8>, start: usize) -> usize
    requires start <= byte_len(view)
    ensures result <= byte_len(view) - start
```

### `std.io.lines.reader.line-len`

```semaprax
fn reader_line_len(reader: borrow Reader) -> usize
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.lines.reader.line-complete`

```semaprax
fn reader_line_complete(reader: borrow Reader) -> bool
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.io.lines.reader.line-into`

Copies the current line's content into the caller's Writer capacity after an
exact preflight.  The borrowed Reader keeps its position; `reader_next_line`
is the consuming transition past the line and its terminator.

```semaprax
fn reader_line_into(input: borrow Reader, output: own Writer) -> Writer
    requires match borrow input { Reader { data: source, position: start } => match borrow output { Writer { data: target, position: write } => start <= byte_len(bytes_as_slice(source)) && write <= byte_len(bytes_as_slice(target)) && line_content_len(bytes_as_slice(source), start) <= byte_len(bytes_as_slice(target)) - write, }, }
```

### `std.io.lines.reader.next-line`

```semaprax
fn reader_next_line(reader: own Reader) -> Reader
    requires match borrow reader { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

## `std.jobs`

Package `std/jobs`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.jobs = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.jobs.state.is_valid`

```semaprax
fn state_is_valid(state: usize) -> bool
```

### `std.jobs.state.is_terminal`

```semaprax
fn state_is_terminal(state: usize) -> bool
```

### `std.jobs.state.holds_lease`

```semaprax
fn state_holds_lease(state: usize) -> bool
```

### `std.jobs.state.awaits_worker`

```semaprax
fn state_awaits_worker(state: usize) -> bool
```

### `std.jobs.lease.is_current`

```semaprax
fn lease_is_current(lease_generation: usize, job_generation: usize, now_tick: usize, deadline_tick: usize) -> bool
```

### `std.jobs.lease.is_expired`

```semaprax
fn lease_is_expired(now_tick: usize, deadline_tick: usize) -> bool
```

### `std.jobs.claim.is_legal`

```semaprax
fn claim_is_legal(state: usize, is_due: bool) -> bool
```

### `std.jobs.begin_execution.is_legal`

```semaprax
fn begin_execution_is_legal(state: usize, lease_generation: usize, job_generation: usize, now_tick: usize, deadline_tick: usize) -> bool
```

### `std.jobs.heartbeat.is_legal`

```semaprax
fn heartbeat_is_legal(state: usize, lease_generation: usize, job_generation: usize, now_tick: usize, deadline_tick: usize) -> bool
```

### `std.jobs.completion.is_legal`

```semaprax
fn completion_is_legal(state: usize, lease_generation: usize, job_generation: usize, now_tick: usize, deadline_tick: usize) -> bool
```

### `std.jobs.outcome.kind_is_valid`

```semaprax
fn outcome_kind_is_valid(kind: usize) -> bool
```

### `std.jobs.attempt.is_within_ceiling`

```semaprax
fn attempt_is_within_ceiling(attempt: u8, max_attempts: u8) -> bool
```

### `std.jobs.retry.should_dead_letter`

```semaprax
fn retry_should_dead_letter(attempt: u8, max_attempts: u8) -> bool
```

### `std.jobs.retry.next_state_after_outcome`

`kind` is one of: 0 success, 1 retryable failure, 2 permanent failure, 3
uncertain (a post-publication I/O failure whose outcome the caller cannot
observe; see docs/DURABLE-JOBS-V1.md's "delivery uncertainty" section for
exactly how far this decision procedure goes and where it stops).

```semaprax
fn retry_next_state_after_outcome(kind: usize, attempt: u8, max_attempts: u8) -> usize
    ensures result <= 9usize
```

### `std.jobs.retry.backoff_ticks`

Bounded exponential backoff: doubles `base_ticks` once per attempt already
made, capped at `max_ticks` so a runaway attempt counter can never produce
an unbounded wait — "Unbounded retries or schedules" is explicitly out of
scope for this profile.

```semaprax
fn retry_backoff_ticks(attempt: u8, base_ticks: usize, max_ticks: usize) -> usize
    ensures result <= max_ticks
```

### `std.jobs.idempotency.enqueue_outcome`

0 = fresh (no existing job for this idempotency key; enqueue creates one),
1 = duplicate (the key already names a job whose stored payload descriptor
matches the candidate; enqueue returns the existing job rather than
creating a second one), 2 = conflict (the key already names a job whose
descriptor differs; enqueue is a closed refusal, never a silent merge).

```semaprax
fn idempotency_enqueue_outcome(key_exists: bool, existing_descriptor: borrow Slice<u8>, candidate_descriptor: borrow Slice<u8>) -> usize
    ensures result <= 2usize
```

### `std.jobs.revision.is_known`

A job is bound to the handler/schema revision that was current at enqueue
time. `current_revision` only ever grows (like `std.db.migration`'s
gapless ledger), so a bound revision is known exactly when it lies in
`1..=current_revision`; anything else (including `0`, never assigned) is
refused rather than guessed at.

```semaprax
fn revision_is_known(job_bound_revision: u8, current_revision: u8) -> bool
```

### `std.jobs.revision.requires_refusal`

```semaprax
fn revision_requires_refusal(job_bound_revision: u8, current_revision: u8) -> bool
```

### `std.jobs.payload.schema_is_compatible`

```semaprax
fn payload_schema_is_compatible(expected_descriptor: borrow Slice<u8>, actual_descriptor: borrow Slice<u8>) -> bool
```

### `std.jobs.schedule.is_due`

```semaprax
fn schedule_is_due(now_tick: usize, next_run_tick: usize) -> bool
```

### `std.jobs.schedule.missed_windows`

```semaprax
fn schedule_missed_windows(now_tick: usize, next_run_tick: usize, interval_tick: usize) -> usize
    requires interval_tick > 0usize
```

### `std.jobs.schedule.catch_up_next_run`

The catch-up policy this profile implements is "skip missed": a recurring
job that was due several windows ago fires once for the most recent due
window, not once per missed window, and `max_catch_up` bounds how far a
long-dead clock can jump the next run in one step.

```semaprax
fn schedule_catch_up_next_run(now_tick: usize, next_run_tick: usize, interval_tick: usize, max_catch_up: usize) -> usize
    requires interval_tick > 0usize
```

### `std.jobs.schedule.recurrence_is_exhausted`

```semaprax
fn recurrence_is_exhausted(occurrences_run: usize, max_occurrences: usize) -> bool
```

### `std.jobs.cancel.is_legal`

```semaprax
fn cancel_is_legal(state: usize) -> bool
```

### `std.jobs.cancel.next_state`

```semaprax
fn cancel_next_state(state: usize) -> usize
    ensures result <= 9usize
```

### `std.jobs.compensation.is_required`

A cancellation reaching a job that already started running (observed by
`ever_ran`) may have left a partial external effect behind; a permanent
failure that already started running is in the same position. Neither
case can be reasoned about further at this pure layer: `true` means "a
compensation hook must run", never that this profile ran one.

```semaprax
fn compensation_is_required(ever_ran: bool, final_state: usize) -> bool
```

### `std.jobs.uncertain.retry_is_permitted`

```semaprax
fn uncertain_retry_is_permitted(is_idempotent_handler: bool, attempt: u8, max_attempts: u8) -> bool
```

### `std.jobs.uncertain.reconcile`

`decision` is the explicit, human- or operator-supplied reconciliation of
an `UNCERTAIN` outcome: 0 confirmed succeeded, 1 confirmed failed, 2 retry.
A retry decision only proceeds automatically when the handler is declared
idempotent and the attempt ceiling is not yet reached; otherwise the job
stays `UNCERTAIN` rather than being silently retried or silently
dead-lettered — "no automatic retry of an uncertain non-idempotent
operation" is this function's central refusal.

```semaprax
fn uncertain_reconcile(decision: usize, is_idempotent_handler: bool, attempt: u8, max_attempts: u8) -> usize
    ensures result <= 9usize
```

### `std.jobs.budget.reserve_is_legal`

```semaprax
fn budget_reserve_is_legal(reserved_cost: usize, this_attempt_cost: usize, max_cost: usize) -> bool
```

## `std.log`

Package `std/log`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.log = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.log.event`

```semaprax
record Event {
    level: u8,
    sequence: usize,
    name: Bytes,
    message: Bytes,
}
```

### `std.log.level-len`

```semaprax
fn level_len(level: u8) -> usize
    requires level <= 5u8
```

### `std.log.level-byte`

```semaprax
fn level_byte(level: u8, index: usize) -> u8
    requires level <= 5u8
    requires index < level_len(level)
```

### `std.log.fixed-len`

```semaprax
fn fixed_len(kind: u8) -> usize
```

### `std.log.fixed-byte`

```semaprax
fn fixed_byte(kind: u8, index: usize) -> u8
    requires kind <= 4u8
    requires index < fixed_len(kind)
```

### `std.log.write-fixed`

```semaprax
fn write_fixed(kind: u8, output: own Writer) -> Writer
    requires kind <= 4u8
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && fixed_len(kind) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.log.write-level`

```semaprax
fn write_level(level: u8, output: own Writer) -> Writer
    requires level <= 5u8
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && level_len(level) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.log.quote-bytes`

```semaprax
fn quote_bytes(data: own Bytes, output: own Writer) -> Writer
    requires is_utf8(bytes_as_slice(data))
    requires match borrow output { Writer { data: output_data, position } => position <= byte_len(bytes_as_slice(output_data)) && quoted_len(bytes_as_slice(data)) <= byte_len(bytes_as_slice(output_data)) - position, }
```

### `std.log.event-json-len`

```semaprax
fn event_json_len(event: borrow Event) -> usize
    requires event.level <= 5u8
```

### `std.log.append-event`

```semaprax
fn append_event(event: own Event, output: own Writer) -> Writer
    requires match borrow event { Event { level, sequence: _, name: _, message: _ } => level <= 5u8, }
    requires match borrow event { Event { level: _, sequence: _, name, message } => is_utf8(bytes_as_slice(name)) && is_utf8(bytes_as_slice(message)), }
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && event_json_len(event) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.log.level-enabled`

Level filtering. Levels run 0 (trace) to 5 (fatal); an event is enabled when
its level is at or above the threshold.

```semaprax
fn level_enabled(level: u8, threshold: u8) -> bool
    requires level <= 5u8 && threshold <= 5u8
```

### `std.log.discard-event`

The explicit "dropped by policy" transition: it consumes the event, leaves
the caller's Writer exactly as it was, and writes nothing.

```semaprax
fn discard_event(event: own Event, output: own Writer) -> Writer
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.log.event-admitted`

True when the event both passes the threshold and fits the live capacity.

```semaprax
fn event_admitted(event: borrow Event, threshold: u8, output: borrow Writer) -> bool
    requires match borrow event { Event { level, sequence: _, name: _, message: _ } => level <= 5u8, }
    requires threshold <= 5u8
```

### `std.log.append-event-if`

Writes the event only when it passes the threshold; a filtered event is
consumed and the Writer is returned untouched.  Capacity is required only
for an event that is actually written.

```semaprax
fn append_event_if(event: own Event, threshold: u8, output: own Writer) -> Writer
    requires threshold <= 5u8
    requires match borrow event { Event { level, sequence: _, name: _, message: _ } => level <= 5u8, }
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)), }
    requires event_admitted(event, threshold, output) || match borrow event { Event { level, sequence: _, name: _, message: _ } => !level_enabled(level, threshold), }
```

### `std.log.append-event-guarded`

---------------------------------------------------------------------
Redaction-guarded writing (issue #193)
---------------------------------------------------------------------
The caller's six secret-bearing classification flags -- exactly
std.log.redact.event_is_safe's own parameter list -- decide whether the
event is written at all. This writer calls event_is_safe itself rather
than trusting a caller-computed boolean, so the redaction decision is
made inside the writer that actually emits bytes rather than somewhere
upstream that a caller could skip or get wrong. An unsafe event is
refused outright (mirroring append_event_if's own admitted/discarded
transition) rather than partly redacted: this package's Event has no
separate labeled-field structure to redact one field of, so refusing the
whole event is the sound choice for the fields this schema actually has.

```semaprax
fn append_event_guarded(event: own Event, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool, output: own Writer) -> Writer
    requires match borrow event { Event { level, sequence: _, name: _, message: _ } => level <= 5u8, }
    requires match borrow event { Event { level: _, sequence: _, name, message } => is_utf8(bytes_as_slice(name)) && is_utf8(bytes_as_slice(message)), }
    requires match borrow output { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && event_json_len(event) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.log.field-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn field_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.log.event-name-byte-admitted`

---------------------------------------------------------------------
Stable event identities and structured fields (issue #193)
---------------------------------------------------------------------
This package's `Event` carries a `name` and a `message`, and until here
neither had a grammar. That is the difference between a log a query can be
written against and a log a human greps: a stable event identity is the
one field an alert, a dashboard, and a regression test all bind to, so it
has to be a closed spelling rather than whatever string the call site
happened to interpolate. The `message` stays free text for a reader; the
identity does not.

The field budget below is the structured-logging analogue of metric label
cardinality. An event whose field set is assembled from attacker-supplied
keys is an unbounded-arity observation, and the shared redaction policy
already owns the ceiling, so it is reused here rather than restated.

Nothing here writes, ships, or stores anything. These are decisions about
values a caller has already assembled, made before `append_event` is
reached.

```semaprax
fn event_name_byte_admitted(candidate: u8) -> bool
```

### `std.log.event-name-admitted`

A dotted lowercase identifier: `http.request.rejected`, not
`Rejected request from 203.0.113.9`. Uppercase, spaces and punctuation are
refused rather than folded or stripped, because an identity that two call
sites spell differently is two identities, and one an adapter rewrote is
an identity nobody can predict from the source.

```semaprax
fn event_name_admitted(name: borrow Slice<u8>) -> bool
```

### `std.log.level-admitted`

```semaprax
fn level_admitted(level: u8) -> bool
```

### `std.log.field-count-admitted`

The same per-observation arity budget the shared redaction policy owns,
reused rather than restated so a log event's field set and a span's
attribute set cannot drift to different ceilings.

```semaprax
fn field_count_admitted(count: usize) -> bool
```

### `std.log.field-bytes-limit`

Arity alone does not bound an event: thirty-two fields of a megabyte each
are inside every count budget and still exhaust a log pipeline. The total
serialized field payload is bounded separately.

```semaprax
fn field_bytes_limit() -> usize
```

### `std.log.field-bytes-admitted`

```semaprax
fn field_bytes_admitted(total: usize) -> bool
```

### `std.log.message-len-limit`

The message stays free text, but not unbounded free text: a message is
what a reader sees, and one large enough to dominate a log line is a
denial of service on the reader as much as on the pipeline.

```semaprax
fn message_len_limit() -> usize
```

### `std.log.message-len-admitted`

```semaprax
fn message_len_admitted(length: usize) -> bool
```

### `std.log.structured-event-admitted`

The composed judgement a structured-logging caller makes before it
assembles an Event at all. Every clause is one of the named predicates
above, so a refusal is attributable to a stated rule.

```semaprax
fn structured_event_admitted(name: borrow Slice<u8>, level: u8, message_len: usize, field_count: usize, field_bytes: usize) -> bool
```

### `std.log.structured-event-admitted-guarded`

The same judgement with the shared redaction policy's six caller-declared
flags applied. None of the shape rules above has any notion of classified
content: an event whose fields carry a session token passes every one of
them, which is exactly the gap `append_event_guarded` closes at the writer
and this closes one step earlier, before the event is built.

```semaprax
fn structured_event_admitted_guarded(name: borrow Slice<u8>, level: u8, message_len: usize, field_count: usize, field_bytes: usize, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.log.observed-status`

A telemetry sink's failure is not the application's failure. The primary
status survives unless the deployment explicitly chose to fail closed on
export, in which case the export failure is reported as itself rather than
disguised as an application error. Stated here, at the observability root,
because every adapter in this family can reach it.

```semaprax
fn observed_status(primary_status: i64, export_status: i64, fail_closed_on_export: bool) -> i64
```

## `std.log.redact`

Package `std/log-redact`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.log.redact = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.log.redact.marker_len`

Issue #193's redaction-policy slice: the pure, effect-free decision layer
a structured-logging, metrics, tracing, webhook, or email adapter consults
before a field it is about to serialize, export, or attach leaves the
process. It performs no I/O, declares no `permit`, and calls no
`uses`-gated operation, exactly the idiom `std.auth`'s audit-event-safety
layer already established for this repository (see `std/auth/src/auth.spx`
and its `audit_event_is_safe`/`audit_event_is_complete` pair).

`std.log` (the existing package) already renders a caller-assembled
`name`/`message` event to JSON with exact capacity preflight; it carries no
per-field redaction policy. This package supplies that policy as data a
caller of `std.log`, a future metrics/trace exporter, or a future
webhook/email adapter can consult BEFORE calling into its own writer, so a
secret-bearing field is refused or replaced with a fixed, visible marker
rather than serialized. It does not itself write JSON, sign a webhook, or
send an email: those adapters remain future work (see the tracking issue).

Two things this package does NOT claim: it does not scan a field's runtime
*content* for secret-shaped values (entropy, key-pattern, or otherwise) -
classification is the caller's declared boolean, exactly like
`std.auth.audit.event_is_safe` takes already-decided flags rather than
scanning bytes; and it does not allocate, own, or forward the field's raw
bytes anywhere - every function here is scalar-in/scalar-out or
`borrow Slice<u8>`-in/scalar-out, so a secret's bytes are never accepted,
retained, or returned by this package at all.
---------------------------------------------------------------------
The redacted marker
---------------------------------------------------------------------
A fixed, deterministic ASCII marker: `[REDACTED]`. Every function that
renders a refused field writes exactly these ten bytes, at every level and
across every adapter, so a downstream reader (a human, a log shipper, or a
hostile-input regression) can recognize a redaction on sight instead of
guessing from an adapter-specific placeholder.

```semaprax
fn marker_len() -> usize
```

### `std.log.redact.marker_byte`

```semaprax
fn marker_byte(index: usize) -> u8
    requires index < marker_len()
```

### `std.log.redact.bytes_match_marker`

The byte-exact check any adapter's own writer output can be verified
against: true only when `candidate` is exactly the ten marker bytes in
order, false for anything else (including a candidate that merely starts
with them, or a field's own raw content that happens to share a prefix).

```semaprax
fn bytes_match_marker(candidate: borrow Slice<u8>) -> bool
```

### `std.log.redact.event_is_safe`

---------------------------------------------------------------------
Field/event safety
---------------------------------------------------------------------
The closed refusal: an observation (a log event, a metric's labels, a
trace span's attributes, a webhook payload, or an email body/header) is
unsafe if it carries ANY of these six secret-bearing fields, regardless of
what else it carries. This mirrors `std.auth.audit.event_is_safe` exactly
(same shape: named boolean flags, one per secret-bearing field, closed
over an explicit exhaustive list) but generalized past authentication
events to the six adapter families this issue names: a raw password and an
API key are structured-logging/metrics leaks, a bearer token and a session
token are logging/tracing leaks, a webhook signing secret is the webhook
adapter's own credential, and an SMTP/API credential is the email
adapter's. Adding a new secret-bearing field to a real observation type
means adding its flag here too - the check is exhaustive over this fixed
list, not over whatever fields a caller happens to think of.

```semaprax
fn event_is_safe(carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.log.redact.field_count_within_budget`

---------------------------------------------------------------------
Cardinality budget
---------------------------------------------------------------------
One deployment-independent bound on the number of fields/labels a single
observation may carry, the direct defense against the named failure
"metric label cardinality can cause memory/cost denial of service" and its
structured-logging analogue (an attacker-controlled field set with
unbounded arity). 32 mirrors this package's own `web_exports` ceiling and
is deliberately small: a real exporter profile is free to raise it, but
never to leave it unstated.

```semaprax
fn field_count_within_budget(count: usize) -> bool
```

### `std.log.redact.normalized-name-byte`

Additive named-field projection policy. Names are bounded ASCII identifiers;
case and '-'/'_' spelling cannot bypass the closed protected-name inventory.
This is not content-based secret discovery: unknown secret values still need
the caller's explicit classification. No raw secret input is needed here.

```semaprax
fn normalized_name_byte(value: u8) -> u8
```

### `std.log.redact.name-byte-admitted`

```semaprax
fn name_byte_admitted(value: u8) -> bool
```

### `std.log.redact.field-name-admitted`

```semaprax
fn field_name_admitted(name: borrow Slice<u8>) -> bool
```

### `std.log.redact.normalized-name-matches`

```semaprax
fn normalized_name_matches(name: borrow Slice<u8>, expected: borrow Slice<u8>) -> bool
```

### `std.log.redact.field-name-is-protected`

```semaprax
fn field_name_is_protected(name: borrow Slice<u8>) -> bool
```

### `std.log.redact.commitment-shape-admitted`

Optional already-produced commitment: absent, or exactly 64 lowercase hex
bytes. This checks shape only. It neither computes a hash nor authenticates
a claimed commitment, and callers must not disguise a credential as one.

```semaprax
fn commitment_shape_admitted(commitment: borrow Slice<u8>) -> bool
```

### `std.log.redact.projected-field-admitted`

Validate the bytes an adapter proposes to emit, never the original secret.
Protected fields/values require the exact visible marker, with no prefix,
suffix, or secret-length dependence. Public raw values cannot carry a
misleading redaction commitment. This adds no serialization/send authority.

```semaprax
fn projected_field_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.log.redact.projection-len-admitted`

```semaprax
fn projection_len_admitted(length: usize) -> bool
```

## `std.mem`

Package `std/mem`, tier `alloc`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.mem = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.mem.box.new`

```semaprax
fn new<T>(value: T) -> Box<T>
```

### `std.mem.box.get`

```semaprax
fn get<T>(value: borrow Box<T>) -> T
```

### `std.mem.box.into-inner`

```semaprax
fn into_inner<T>(value: own Box<T>) -> T
```

## `std.metrics`

Package `std/metrics`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.metrics = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.metrics.counter-increment`

Counters, gauges, and histograms are modeled as plain scalar state the
caller threads itself: a checked update takes the previous value and
returns the next one (or a sentinel), so no owned aggregate ever needs to
cross a package's module boundary for this package's public surface.

```semaprax
fn counter_increment(value: i64, delta: i64) -> i64
    requires value >= 0
    requires delta >= 0
    requires !add_overflows(value, delta)
```

### `std.metrics.gauge-clamped`

```semaprax
fn gauge_clamped(value: i64, minimum: i64, maximum: i64) -> i64
    requires minimum <= maximum
```

### `std.metrics.histogram-observe-count`

```semaprax
fn histogram_observe_count(count: i64) -> i64
    requires count >= 0
    requires !add_overflows(count, 1)
```

### `std.metrics.histogram-observe-sum`

```semaprax
fn histogram_observe_sum(sum: i64, value: i64) -> i64
    requires !add_overflows(sum, value)
```

### `std.metrics.cardinality-limit`

---------------------------------------------------------------------
Cardinality budget (issue #193)
---------------------------------------------------------------------
One deployment-independent bound on the number of distinct label-value
combinations ("series") a single metric may register, the direct defense
against unbounded label cardinality causing a memory/cost denial of
service. try_admit_series is the part with teeth: once a registry already
holds cardinality_limit() series, the next attempt returns the fixed
refusal sentinel -1 (mirroring std.data.json.write.quoted_byte's own
-1-for-out-of-range idiom) rather than silently growing past the bound or
silently returning the unchanged count as if the admission had succeeded.

```semaprax
fn cardinality_limit() -> i64
```

### `std.metrics.try-admit-series`

```semaprax
fn try_admit_series(existing_count: i64) -> i64
    requires existing_count >= 0
    requires existing_count <= cardinality_limit()
    ensures result == -1 || result >= 1 && result <= cardinality_limit()
```

### `std.metrics.byte-is-label-name-start`

---------------------------------------------------------------------
Labels (issue #193)
---------------------------------------------------------------------
Everything above admits a metric's numeric state. Nothing above attaches a
label to it or looks at a label's bytes, so `try_admit_series` counted
abstract series and a caller-controlled label name or value could carry
whatever bytes a Prometheus-style text exporter would then choke on. This
section is that missing decision layer: pure predicates over
`borrow Slice<u8>`, no I/O, no `permit`/`uses`, refuse rather than
sanitize -- the same shape `std.email` and `std.http` already established
for their own injection boundaries.

Label NAME validity is a closed allowlist, following
`std.http.header_name_is_token`'s reasoning: the bytes that break a
metrics exposition format's `name="value"` grammar are exactly the ones a
denylist forgets. A label name in the Prometheus text format is
`[a-zA-Z_][a-zA-Z0-9_]*`, so only that admitted set -- never anything
merely "not yet seen to be dangerous" -- is admitted.

```semaprax
fn byte_is_label_name_start(byte: u8) -> bool
```

### `std.metrics.byte-is-label-name-char`

```semaprax
fn byte_is_label_name_char(byte: u8) -> bool
```

### `std.metrics.label-name-max-len`

```semaprax
fn label_name_max_len() -> usize
```

### `std.metrics.label-name-is-valid`

```semaprax
fn label_name_is_valid(name: borrow Slice<u8>) -> bool
```

### `std.metrics.byte-is-label-value-unsafe`

Label VALUE safety is the opposite shape: a denylist of the exact bytes a
Prometheus-style text exposition format breaks on, because a label value
is otherwise arbitrary text and has no admitted-character grammar to
allowlist. A value is written `"..."` in the exposition format, and its
own escaping rule only covers backslash, double quote, and line feed
(`\\`, `\"`, `\n`) -- so those three are refused outright rather than
escaped, matching `std.email.value_is_header_safe` and
`std.http.value_is_header_safe`'s refuse-don't-sanitize discipline. CR and
NUL are refused alongside them for the same defensive reason those two
packages already refuse them next to their own format's separator: CR
pairs with LF in most real line-oriented consumers of this text, and NUL
truncates a C-string-backed writer regardless of the length this package
believes the value has.

Written as a `match` rather than a chained `||` of five `byte == ...`
comparisons: `std.http.byte_is_tchar`'s single-expression form hit the
cleanup-replay skeleton-work budget (`SPX-H006`) at three ranges plus one
helper call, so a five-literal disjunction scanned inside this package's
own `while` loop is designed out from the start rather than discovered by
a failing gate.

```semaprax
fn byte_is_label_value_unsafe(byte: u8) -> bool
```

### `std.metrics.value-is-label-safe`

```semaprax
fn value_is_label_safe(field: borrow Slice<u8>) -> bool
```

### `std.metrics.label-value-max-len`

```semaprax
fn label_value_max_len() -> usize
```

### `std.metrics.label-value-len-admitted`

A label value may be empty (an unset label is a legitimate series key), so
this is an upper bound only, unlike the name's lower-bounded length.

```semaprax
fn label_value_len_admitted(length: usize) -> bool
```

### `std.metrics.label-count-max`

One deployment-independent bound on how many labels a single series may
carry, alongside the per-name and per-value byte bounds above: three
separate budgets, because a caller told only "labels rejected" without
which budget it hit is exactly the diagnostic failure this repository
keeps finding elsewhere.

```semaprax
fn label_count_max() -> i64
```

### `std.metrics.label-count-admitted`

```semaprax
fn label_count_admitted(count: i64) -> bool
```

### `std.metrics.label-admitted`

The composed judgement a caller asks once per label, before attaching it
to a series -- in the style of `std.email.address_admitted` and
`std.http.request_line_admitted`. Every clause is one of the predicates
above, so a refusal can always be attributed to a named rule.

```semaprax
fn label_admitted(name: borrow Slice<u8>, value: borrow Slice<u8>) -> bool
```

### `std.metrics.try-admit-labeled-series`

Cardinality admission that actually sees the label, rather than counting
an abstract series as `try_admit_series` does above. A label that fails
`label_admitted` is refused with the same fixed `-1` sentinel a registry
already at `cardinality_limit()` returns, so a caller cannot tell "bad
label" apart from "budget exhausted" by the return value alone, but it can
never register a series keyed by a label this package refused.

```semaprax
fn try_admit_labeled_series(existing_count: i64, name: borrow Slice<u8>, value: borrow Slice<u8>) -> i64
    requires existing_count >= 0
    requires existing_count <= cardinality_limit()
    ensures result == -1 || result >= 1 && result <= cardinality_limit()
```

### `std.metrics.label-admitted-guarded`

---------------------------------------------------------------------
Redaction guard (issue #193)
---------------------------------------------------------------------
Everything above judges a label's SHAPE: a name grammar and a value
denylist keep a series out of a Prometheus-style exposition format's own
injection boundary, but neither one has any notion of a label's
caller-classified secret content. A password or API key that carries no
CR/LF/NUL/quote/backslash is a perfectly shape-valid label value and would
be admitted and exported by `label_admitted`/`try_admit_labeled_series`
alone -- this closes exactly that gap, composing the shape check with
`std.log.redact.event_is_safe`'s six caller-declared flags exactly like
`std.log.append-event-guarded` gates a log event.

```semaprax
fn label_admitted_guarded(name: borrow Slice<u8>, value: borrow Slice<u8>, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.metrics.try-admit-labeled-series-guarded`

The registration entry point a real exporter would actually call: a
secret-classified label never grows the registered series count, refused
with the same fixed `-1` sentinel a shape-invalid label or an exhausted
budget already returns, so a caller cannot tell the three refusal reasons
apart from the return value alone but can never register a series keyed by
a label this package refused for any of them.

```semaprax
fn try_admit_labeled_series_guarded(existing_count: i64, name: borrow Slice<u8>, value: borrow Slice<u8>, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> i64
    requires existing_count >= 0
    requires existing_count <= cardinality_limit()
    ensures result == -1 || result >= 1 && result <= cardinality_limit()
```

### `std.metrics.label-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn label_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.metrics.try-admit-projected-series`

```semaprax
fn try_admit_projected_series(existing_count: i64, name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> i64
    requires existing_count >= 0 && existing_count <= cardinality_limit()
    ensures result == -1 || result >= 1 && result <= cardinality_limit()
```

### `std.metrics.byte-is-metric-name-start`

---------------------------------------------------------------------
Metric identity, counter monotonicity, and histogram buckets (issue #193)
---------------------------------------------------------------------
Everything above judges a label and a series budget. A metric is more
than its labels: it has a name that has to stay the same across releases
for a dashboard to keep working, a unit that has to be stated for a value
to mean anything, a kind that decides how a consumer may aggregate it, and
-- for a histogram -- a bucket layout that has to be monotonic for a
quantile computed from it to be anything but noise.

A metric name is the highest-leverage cardinality decision in the whole
package: a name assembled from a request path creates a new metric per
request, which no per-metric series budget can contain because each one is
its own metric. The grammar below refuses that shape by construction.

Nothing here records, aggregates, or exports a measurement.

```semaprax
fn byte_is_metric_name_start(byte: u8) -> bool
```

### `std.metrics.byte-is-metric-name-char`

```semaprax
fn byte_is_metric_name_char(byte: u8) -> bool
```

### `std.metrics.metric-name-max-len`

```semaprax
fn metric_name_max_len() -> usize
```

### `std.metrics.metric-name-admitted`

Lowercase only, and never starting with a digit. Uppercase is refused
rather than folded for the same reason a span name refuses it: folding
hides a caller's mistake, and two names that differ only in case would
otherwise become two metrics that look like one on a dashboard.

```semaprax
fn metric_name_admitted(name: borrow Slice<u8>) -> bool
```

### `std.metrics.byte-is-unit-char`

A unit is optional but never arbitrary: `ms`, `by`, `1`, `req/s`. An empty
unit is admitted and means "dimensionless", stated rather than left to a
consumer to guess.

```semaprax
fn byte_is_unit_char(byte: u8) -> bool
```

### `std.metrics.metric-unit-admitted`

```semaprax
fn metric_unit_admitted(unit: borrow Slice<u8>) -> bool
```

### `std.metrics.metric-kind-admitted`

0 counter, 1 gauge, 2 histogram, 3 up-down counter. The kind is what tells
a consumer whether summing two values is meaningful, so it is a closed
vocabulary rather than a free-form string.

```semaprax
fn metric_kind_admitted(kind: i64) -> bool
```

### `std.metrics.counter-delta-admitted`

---------------------------------------------------------------------
Counter monotonicity
---------------------------------------------------------------------
A counter only ever goes up. A negative delta is refused rather than
applied, because a consumer computing a rate from a decreasing counter
reads it as a process restart and silently discards the interval.

```semaprax
fn counter_delta_admitted(delta: i64) -> bool
```

### `std.metrics.counter-reset-detected`

The reset a consumer must actually handle: a value lower than the last one
it saw. Reporting this as data, rather than papering over it, is what lets
a rate calculation restart deliberately instead of producing a negative
rate or an enormous one.

```semaprax
fn counter_reset_detected(previous: i64, current: i64) -> bool
    requires previous >= 0
    requires current >= 0
```

### `std.metrics.delta-admitted-for-kind`

An up-down counter is the one kind for which a negative delta is
legitimate, which is precisely why it is a different kind rather than a
counter with a relaxed rule.

```semaprax
fn delta_admitted_for_kind(kind: i64, delta: i64) -> bool
    requires metric_kind_admitted(kind)
```

### `std.metrics.bucket-count-max`

---------------------------------------------------------------------
Histogram buckets
---------------------------------------------------------------------
Bucket bounds are checked pairwise, in the order a caller supplies them,
rather than as a whole array: this package has no array-of-i64 parameter
shape, and a pairwise rule is the same property stated one step at a time.
A caller walks its own bounds and asks once per adjacent pair.

```semaprax
fn bucket_count_max() -> usize
```

### `std.metrics.bucket-count-admitted`

At least one finite bound, and a stated ceiling: an unbounded bucket
layout is per-metric cardinality by another name, since every bucket is a
series in most exposition formats.

```semaprax
fn bucket_count_admitted(count: usize) -> bool
```

### `std.metrics.bucket-bound-follows`

Strictly increasing, not merely non-decreasing. Two equal adjacent bounds
create a bucket that can never be selected, and a quantile interpolated
across it divides by its zero width.

```semaprax
fn bucket_bound_follows(previous: i64, next: i64) -> bool
```

### `std.metrics.bucket-selects`

Upper-inclusive, lower-exclusive, matching the cumulative convention every
mainstream exposition format uses. Stating it here keeps a caller from
choosing the other convention and shifting every quantile by one bucket.

```semaprax
fn bucket_selects(value: i64, lower_bound: i64, upper_bound: i64) -> bool
    requires bucket_bound_follows(lower_bound, upper_bound)
```

### `std.metrics.cumulative-count-follows`

Cumulative bucket counts are non-decreasing by construction: a later
bucket contains every observation an earlier one did. A pair that
decreases is a corrupt or reordered histogram, not a small one.

```semaprax
fn cumulative_count_follows(previous: i64, next: i64) -> bool
```

### `std.metrics.histogram-totals-admitted`

A histogram's own consistency: the count in its final cumulative bucket
can never exceed the observation count, and the observation count can
never be negative. A caller that reports more bucketed observations than
it made has a counting bug this refuses to export.

```semaprax
fn histogram_totals_admitted(observation_count: i64, final_cumulative_count: i64) -> bool
```

### `std.metrics.overflow-series-slot`

---------------------------------------------------------------------
Deterministic overflow aggregation
---------------------------------------------------------------------
`try_admit_series` above refuses past the per-metric limit. Refusal is one
deterministic answer; the other the issue names is aggregation, and this
is it: excess series fold into one reserved slot rather than vanishing, so
the total stays correct even though the attribution is lost. The slot is a
fixed index, so the same overflowing sequence produces the same layout on
every run and on every backend.

```semaprax
fn overflow_series_slot() -> i64
```

### `std.metrics.series-slot`

The slot an incoming series is written to: its own next index while the
budget holds, and the reserved overflow slot afterwards. Deterministic in
both branches and total over every admitted count, so no caller has to
invent behaviour for the overflowing case.

```semaprax
fn series_slot(existing_count: i64) -> i64
    requires existing_count >= 0
    ensures result >= 0 && result <= cardinality_limit()
```

### `std.metrics.series-was-aggregated`

```semaprax
fn series_was_aggregated(existing_count: i64) -> bool
    requires existing_count >= 0
```

### `std.metrics.deployment-series-limit`

A whole-deployment ceiling above the per-metric one. A process holding a
hundred metrics each inside `cardinality_limit()` is still well past what
a collector will accept, and no per-metric budget can see that.

```semaprax
fn deployment_series_limit() -> i64
```

### `std.metrics.deployment-series-admitted`

```semaprax
fn deployment_series_admitted(total_series: i64) -> bool
```

### `std.metrics.metric-admitted`

The composed metric judgement. Every clause is one of the named
predicates above, so a refusal is attributable to a stated rule.

```semaprax
fn metric_admitted(name: borrow Slice<u8>, unit: borrow Slice<u8>, kind: i64, label_count: i64) -> bool
```

## `std.net`

Package `std/net`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.net = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.net.port_is_valid`

```semaprax
fn port_is_valid(port: usize) -> bool
```

### `std.net.wait_is_timeout`

```semaprax
fn wait_is_timeout(state: usize) -> bool
```

### `std.net.wait_is_readable`

```semaprax
fn wait_is_readable(state: usize) -> bool
```

### `std.net.wait_is_closed`

```semaprax
fn wait_is_closed(state: usize) -> bool
```

### `std.net.is_label_byte`

```semaprax
fn is_label_byte(byte: u8) -> bool
```

### `std.net.host_is_valid`

```semaprax
fn host_is_valid(host: borrow Slice<u8>) -> bool
```

### `std.net.digit_or_ten`

```semaprax
fn digit_or_ten(byte: u8) -> usize
    ensures result <= 10usize
```

### `std.net.is_ipv4`

```semaprax
fn is_ipv4(host: borrow Slice<u8>) -> bool
```

### `std.net.ipv4-octet`

---------------------------------------------------------------------
Literal-address classification and endpoint allowlists (issue #193)
---------------------------------------------------------------------
`host_is_valid` and `is_ipv4` above judge spelling. Spelling is not
reachability policy: `127.0.0.1`, `169.254.169.254`, `10.0.0.1`, and
`localhost` are all perfectly well-spelled and are exactly the targets a
server-side request forgery aims an outbound client at. This slice adds
the classification an outbound adapter consults BEFORE it resolves or
connects, plus the exact-match allowlist rule that decides whether a
caller-supplied endpoint was authorized at all.

Nothing here resolves a name, opens a socket, or grants network
authority. Every function is scalar-in/scalar-out or
`borrow Slice<u8>`-in/scalar-out. A `true` answer is a decision about a
spelling, never a permission to connect: the capability model still owns
that, and this package declares no `permit`.

What this deliberately does NOT do: it does not decide anything about a
name's *resolved* address. A hostname that passes `host_is_public_target`
can still resolve to a loopback address (DNS rebinding). Defending that
requires re-checking the resolved literal at connect time, which is the
host adapter's job; this package supplies the literal classifier that
check must use, and says so rather than implying the name check is enough.

```semaprax
fn ipv4_octet(host: borrow Slice<u8>, group: usize) -> usize
    requires is_ipv4(host)
    requires group < 4usize
```

### `std.net.ipv4-is-loopback`

127.0.0.0/8. The whole /8 is loopback, not only 127.0.0.1: an adapter that
refuses the one canonical spelling and admits 127.0.0.2 has refused
nothing.

```semaprax
fn ipv4_is_loopback(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-private`

RFC 1918: 10/8, 172.16/12, 192.168/16. The 172 range is a /12, not a /8 or
a /16, so both ends of its second octet are checked rather than assumed.

```semaprax
fn ipv4_is_private(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-link-local`

169.254.0.0/16. This is the range that carries cloud instance-metadata
services, which is why an outbound client that forgets it leaks
credentials rather than merely reaching a private host.

```semaprax
fn ipv4_is_link_local(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-unspecified`

0.0.0.0/8: "this network". On many stacks 0.0.0.0 connects to localhost.

```semaprax
fn ipv4_is_unspecified(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-shared`

100.64.0.0/10, the carrier-grade NAT shared address space.

```semaprax
fn ipv4_is_shared(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-multicast`

224.0.0.0/4 multicast and 240.0.0.0/4 reserved, including the
255.255.255.255 limited broadcast address at the top of the reserved
range. Neither is a legitimate unicast target for an outbound client.

```semaprax
fn ipv4_is_multicast(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-reserved`

```semaprax
fn ipv4_is_reserved(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.ipv4-is-internal`

The closed refusal an outbound adapter actually calls on a literal
address. Adding a newly-reserved range means adding it here; the check is
exhaustive over this stated list, not over whatever a caller remembers.

```semaprax
fn ipv4_is_internal(host: borrow Slice<u8>) -> bool
    requires is_ipv4(host)
```

### `std.net.lowered-label-byte`

```semaprax
fn lowered_label_byte(byte: u8) -> u8
```

### `std.net.host-equals-ignoring-case`

Case-insensitive exact equality over host spellings. DNS names are
case-insensitive, so an allowlist that compares bytes literally admits
`example.com` and refuses `EXAMPLE.com`, or worse, is bypassed by a
deployment that wrote its entry in a different case.

```semaprax
fn host_equals_ignoring_case(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.net.host-is-loopback-name`

The reserved loopback *name*. A host that never looks at literals still
reaches its own loopback interface through this spelling.

```semaprax
fn host_is_loopback_name(host: borrow Slice<u8>) -> bool
```

### `std.net.host-is-public-target`

A well-spelled host that is neither a reserved name nor an internal
literal. This is a spelling judgement about the target a caller asked for,
not about the address it will resolve to; see this section's opening note
on DNS rebinding.

```semaprax
fn host_is_public_target(host: borrow Slice<u8>) -> bool
```

### `std.net.allowlist-entry-admitted`

---------------------------------------------------------------------
Endpoint allowlist
---------------------------------------------------------------------
An allowlist entry is a well-spelled host and a valid port, both stated.
A entry with an unstated port is not an entry: "example.com" with the port
left to the caller authorizes port 22 as readily as port 443.

```semaprax
fn allowlist_entry_admitted(entry_host: borrow Slice<u8>, entry_port: usize) -> bool
```

### `std.net.endpoint-matches-allowlist-entry`

Exact host and exact port, deliberately with no suffix or wildcard rule.
A suffix match on `example.com` admits `evil-example.com` under a naive
implementation and `example.com.attacker.test` under a careless one; both
are the standard way endpoint allowlists are bypassed. A deployment that
needs several subdomains states several entries.

```semaprax
fn endpoint_matches_allowlist_entry(host: borrow Slice<u8>, port: usize, entry_host: borrow Slice<u8>, entry_port: usize) -> bool
```

### `std.net.endpoint-admitted`

The composed judgement. `allow_internal` is the deployment's explicit,
separate decision to authorize an internal target (a sidecar, a test
fixture); it defaults to nothing and must be passed as `true` on purpose,
so an internal endpoint is never reachable merely because someone wrote it
into an allowlist by accident.

```semaprax
fn endpoint_admitted(host: borrow Slice<u8>, port: usize, entry_host: borrow Slice<u8>, entry_port: usize, allow_internal: bool) -> bool
```

## `std.num`

Package `std/num`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.num = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.num.i64_min`

```semaprax
fn i64_min() -> i64
    ensures result == -9223372036854775807 - 1
```

### `std.num.i64_max`

```semaprax
fn i64_max() -> i64
    ensures result == 9223372036854775807
```

### `std.num.sign`

```semaprax
fn sign(value: i64) -> i64
    ensures result >= -1 && result <= 1
    ensures result == 0 || value != 0
```

### `std.num.abs`

```semaprax
fn abs(value: i64) -> i64
    requires value != -9223372036854775807 - 1
    ensures result >= 0
    ensures result == value || result == 0 - value
```

### `std.num.is_even`

```semaprax
fn is_even(value: i64) -> bool
    ensures result == (value % 2 == 0)
```

### `std.num.is_odd`

```semaprax
fn is_odd(value: i64) -> bool
    ensures result == (value % 2 != 0)
```

### `std.num.div_euclid`

```semaprax
fn div_euclid(dividend: i64, divisor: i64) -> i64
    requires divisor != 0
    requires dividend != -9223372036854775807 - 1 || divisor != -1
```

### `std.num.rem_euclid`

```semaprax
fn rem_euclid(dividend: i64, divisor: i64) -> i64
    requires divisor != 0
    requires dividend != -9223372036854775807 - 1 || divisor != -1
    ensures result >= 0
```

### `std.num.gcd`

```semaprax
fn gcd(left: i64, right: i64) -> i64
    requires left >= 0 && right >= 0
    ensures result >= 0
```

### `std.num.pow`

```semaprax
fn pow(base: i64, exponent: i64) -> i64
    requires exponent >= 0
    ensures exponent > 0 || result == 1
```

### `std.num.isqrt`

```semaprax
fn isqrt(value: i64) -> i64
    requires value >= 0
    ensures result >= 0 && result <= 3037000499
    ensures result * result <= value
```

### `std.num.digit_count`

```semaprax
fn digit_count(value: i64) -> i64
    ensures result >= 1 && result <= 19
```

### `std.num.is_power_of_two`

```semaprax
fn is_power_of_two(value: i64) -> bool
    ensures !result || value > 0
```

### `std.num.log2_floor`

```semaprax
fn log2_floor(value: i64) -> i64
    requires value > 0
    ensures result >= 0 && result <= 62
```

### `std.num.log10_floor`

```semaprax
fn log10_floor(value: i64) -> i64
    requires value > 0
    ensures result >= 0 && result <= 18
```

## `std.num.overflow`

Package `std/num-overflow`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.num.overflow = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.num.overflow.add_overflows`

```semaprax
fn add_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.sub_overflows`

```semaprax
fn sub_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.neg_overflows`

```semaprax
fn neg_overflows(value: i64) -> bool
    ensures result == (value == -9223372036854775807 - 1)
```

### `std.num.overflow.mul_overflows`

```semaprax
fn mul_overflows(left: i64, right: i64) -> bool
```

### `std.num.overflow.wrapping_add`

```semaprax
fn wrapping_add(left: i64, right: i64) -> i64
```

### `std.num.overflow.wrapping_sub`

```semaprax
fn wrapping_sub(left: i64, right: i64) -> i64
```

### `std.num.overflow.wrapping_neg`

```semaprax
fn wrapping_neg(value: i64) -> i64
```

### `std.num.overflow.wrapping_mul`

```semaprax
fn wrapping_mul(left: i64, right: i64) -> i64
    ensures mul_overflows(left, right) || result == left * right
```

### `std.num.overflow.saturating_add`

```semaprax
fn saturating_add(left: i64, right: i64) -> i64
```

### `std.num.overflow.saturating_sub`

```semaprax
fn saturating_sub(left: i64, right: i64) -> i64
```

### `std.num.overflow.saturating_neg`

```semaprax
fn saturating_neg(value: i64) -> i64
```

### `std.num.overflow.saturating_abs`

```semaprax
fn saturating_abs(value: i64) -> i64
    ensures result >= 0
```

### `std.num.overflow.saturating_mul`

```semaprax
fn saturating_mul(left: i64, right: i64) -> i64
```

## `std.path`

Package `std/path`, tier `portable`, status partial. Required project profile: `useful-data.v1`. Dependency: `std.path = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.path.is_absolute`

```semaprax
fn is_absolute(path: borrow Slice<u8>) -> bool
```

### `std.path.has_trailing_separator`

```semaprax
fn has_trailing_separator(path: borrow Slice<u8>) -> bool
```

### `std.path.segment_count`

```semaprax
fn segment_count(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.file_name_start`

```semaprax
fn file_name_start(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.parent_end`

```semaprax
fn parent_end(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

### `std.path.extension_start`

```semaprax
fn extension_start(path: borrow Slice<u8>) -> usize
    ensures result <= byte_len(path)
```

## `std.path.normalize`

Package `std/path-normalize`, tier `portable`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.path.normalize = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.path.normalize.seg-end`

Lexical POSIX normalization over a borrowed view and a logical length.
Segments are separated by `/`; `.` segments vanish and a `..` segment
cancels the nearest retained segment to its left.  A reverse scan decides
retention without a stack: walking right to left, a `..` raises the skip
count, and an ordinary segment either consumes one skip or survives.

```semaprax
fn seg_end(view: borrow Slice<u8>, length: usize, start: usize) -> usize
    requires start <= length && length <= byte_len(view)
    ensures result >= start && result <= length
```

### `std.path.normalize.seg-start`

```semaprax
fn seg_start(view: borrow Slice<u8>, length: usize, from: usize) -> usize
    requires from <= length && length <= byte_len(view)
    ensures result >= from && result <= length
```

### `std.path.normalize.seg-is-dot`

```semaprax
fn seg_is_dot(view: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(view)
```

### `std.path.normalize.seg-is-dotdot`

```semaprax
fn seg_is_dotdot(view: borrow Slice<u8>, start: usize, end: usize) -> bool
    requires start <= end && end <= byte_len(view)
```

### `std.path.normalize.skip-from`

The reverse retention walk as a forward maximum-prefix sum: `..` raises the
balance, an ordinary segment lowers it, and `.` leaves it alone.  The
clamped maximum over every prefix starting at `from` is exactly the skip
count a right-to-left walk would carry when it reaches that point.

```semaprax
fn skip_from(view: borrow Slice<u8>, length: usize, from: usize) -> usize
    requires from <= length && length <= byte_len(view)
```

### `std.path.normalize.seg-retained`

```semaprax
fn seg_retained(view: borrow Slice<u8>, length: usize, start: usize) -> bool
    requires start <= length && length <= byte_len(view)
```

### `std.path.normalize.is-absolute`

```semaprax
fn is_absolute(view: borrow Slice<u8>, length: usize) -> bool
    requires length <= byte_len(view)
```

### `std.path.normalize.leading-parents`

```semaprax
fn leading_parents(view: borrow Slice<u8>, length: usize) -> usize
    requires length <= byte_len(view)
```

### `std.path.normalize.kept-bytes`

Bytes and count of the segments the walk keeps, excluding separators.

```semaprax
fn kept_bytes(view: borrow Slice<u8>, length: usize) -> usize
    requires length <= byte_len(view)
```

### `std.path.normalize.kept-count`

```semaprax
fn kept_count(view: borrow Slice<u8>, length: usize) -> usize
    requires length <= byte_len(view)
```

### `std.path.normalize.normalized-len`

`.` for an empty relative result and `/` for an empty absolute one, so a
normalized path is never zero bytes.

```semaprax
fn normalized_len(view: borrow Slice<u8>, length: usize) -> usize
    requires length <= byte_len(view)
    ensures result >= 1usize
```

### `std.path.normalize.parent-region`

Pull-based normalized output: byte `index` of the normalized form, computed
from the source view alone.  The emitted body is the retained segments in
source order, preceded for a relative path by the parents the walk could
not cancel, joined by single separators.

```semaprax
fn parent_region(parents: usize) -> usize
```

### `std.path.normalize.emitted-offset`

The body offset at which the retained segment beginning at `start` is
emitted, counting the leading parents region and one separator per earlier
retained segment.

```semaprax
fn emitted_offset(view: borrow Slice<u8>, length: usize, start: usize) -> usize
    requires start <= length && length <= byte_len(view)
```

### `std.path.normalize.body-owner`

The source segment start whose emitted bytes cover `body`, or `length` when
`body` names a separator or a leading parent byte.

```semaprax
fn body_owner(view: borrow Slice<u8>, length: usize, body: usize) -> usize
    requires length <= byte_len(view)
    ensures result <= length
```

### `std.path.normalize.normalized-byte`

Pull-based normalized output: byte `index` of the normalized form, computed
from the source view alone.  The emitted body is the retained segments in
source order, preceded for a relative path by the parents the walk could not
cancel, joined by single separators.  A byte that names neither a retained
segment nor a parent is the separator between two units.

```semaprax
fn normalized_byte(view: borrow Slice<u8>, length: usize, index: usize) -> u8
    requires length <= byte_len(view) && index < normalized_len(view, length)
```

### `std.path.normalize.path-length`

```semaprax
fn normalized_path_len(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.normalize.into`

Writes the normalized form into caller-supplied capacity and returns the
normalized Path over it.  The borrowed input keeps its bytes and length.

```semaprax
fn path_normalize(path: borrow Path, buffer: own Bytes) -> Path
    requires path_valid(path) && normalized_path_len(path) <= byte_len(bytes_as_slice(buffer))
```

## `std.path.value`

Package `std/path-value`, tier `portable`, status partial. Required project profile: `owned-data-api.v1`. Dependency: `std.path.value = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.path.value.path`

A Path is caller-owned byte storage plus a logical POSIX lexical prefix.
It does not grant filesystem or platform path authority.

```semaprax
record Path {
    data: Bytes,
    length: usize,
}
```

### `std.path.value.prefix-valid`

```semaprax
fn path_prefix_valid(data: borrow Slice<u8>, length: usize) -> bool
    requires length <= byte_len(data)
```

### `std.path.value.valid`

```semaprax
fn path_valid(path: borrow Path) -> bool
```

### `std.path.value.from-bytes`

```semaprax
fn path_from_bytes(data: own Bytes) -> Path
    requires path_prefix_valid(bytes_as_slice(data), byte_len(bytes_as_slice(data)))
```

### `std.path.value.length`

```semaprax
fn path_length(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.capacity`

```semaprax
fn path_capacity(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.absolute`

```semaprax
fn path_is_absolute(path: borrow Path) -> bool
    requires path_valid(path)
```

### `std.path.value.segment-count`

```semaprax
fn path_segment_count(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.file-name-start`

```semaprax
fn path_file_name_start(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.parent-end`

```semaprax
fn path_parent_end(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.extension-start`

```semaprax
fn path_extension_start(path: borrow Path) -> usize
    requires path_valid(path)
```

### `std.path.value.byte-at`

```semaprax
fn path_byte_at(path: borrow Path, index: usize) -> u8
    requires path_valid(path) && index < path.length
```

### `std.path.value.parent`

```semaprax
fn path_parent(path: own Path) -> Path
    requires path_valid(path)
```

### `std.path.value.finish`

```semaprax
fn path_finish(path: own Path) -> Bytes
    requires path_valid(path)
```

### `std.path.value.join-length`

```semaprax
fn path_join_length(base: borrow Path, child: borrow Path) -> usize
    requires path_valid(base) && path_valid(child)
```

### `std.path.value.join`

```semaprax
fn path_join(base: borrow Path, child: borrow Path, buffer: own Bytes) -> Path
    requires path_valid(base) && path_valid(child) && path_join_length(base, child) <= byte_len(bytes_as_slice(buffer))
```

## `std.process`

Package `std/process`, tier `hosted`, status partial. Required project profile: `process-io.v1`. Dependency: `std.process = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.process.argv`

```semaprax
record Argv {
    data: Bytes,
    count: usize,
    length: usize,
}
```

### `std.process.output`

```semaprax
record Output {
    data: Bytes,
}
```

### `std.process.byte-usize`

```semaprax
fn byte_usize(value: u8) -> usize
```

### `std.process.u32-byte`

```semaprax
fn u32_byte(value: usize, index: usize) -> u8
    requires value <= 65536usize
    requires index < 4usize
```

### `std.process.reader-non-nul`

```semaprax
fn reader_non_nul(input: borrow Reader) -> bool
    requires match borrow input { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.process.argv-new`

```semaprax
fn argv_new(data: own Bytes) -> Argv
    requires byte_len(bytes_as_slice(data)) >= 4usize
```

### `std.process.argv-count`

```semaprax
fn argv_count(argv: borrow Argv) -> usize
    requires match borrow argv { Argv { data, count, length } => length >= 4usize && length <= byte_len(bytes_as_slice(data)) && count <= 16usize, }
```

### `std.process.argv-push`

```semaprax
fn argv_push(input: own Reader, argv: own Argv) -> Argv
    requires match borrow input { Reader { data, position } => position <= byte_len(bytes_as_slice(data)) && reader_non_nul(input), }
    requires match borrow argv { Argv { data, count, length } => count < 16usize && length >= 4usize && length <= byte_len(bytes_as_slice(data)) && byte_len(bytes_as_slice(data)) - length >= 4usize + reader_remaining(input), }
```

### `std.process.argument-is-admissible`

```semaprax
fn argument_is_admissible(argument: borrow Slice<u8>, is_program: bool) -> bool
```

### `std.process.argv-view-count`

```semaprax
fn argv_view_count(view: borrow Slice<u8>) -> usize
    ensures result <= 16usize
```

### `std.process.run`

```semaprax
fn run(tool: usize, argv: own Argv, stdin: own Reader, timeout_ms: usize, stdout_max: usize, stderr_max: usize) -> Output
    uses { process.execute }
    requires timeout_ms >= 1usize && timeout_ms <= 30000usize
    requires stdout_max <= 65504usize && stderr_max <= 65504usize && stdout_max <= 65504usize - stderr_max
    requires match borrow argv { Argv { data, count, length } => count <= 16usize && length >= 4usize && length <= byte_len(bytes_as_slice(data)), }
    requires match borrow stdin { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.process.header-byte`

```semaprax
fn header_byte(output: borrow Output, index: usize) -> u8
    requires index < 32usize
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= 32usize, }
```

### `std.process.word-u32`

```semaprax
fn word_u32(output: borrow Output, offset: usize) -> usize
    requires offset <= 24usize
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= offset + 8usize, }
```

### `std.process.header-zeroes`

```semaprax
fn header_zeroes(output: borrow Output, start: usize, count: usize) -> bool
    requires start <= 32usize && count <= 32usize - start
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= 32usize, }
```

### `std.process.termination-kind-raw`

```semaprax
fn termination_kind_raw(output: borrow Output) -> usize
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= 32usize, }
```

### `std.process.termination-code-raw`

```semaprax
fn termination_code_raw(output: borrow Output) -> usize
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= 32usize, }
```

### `std.process.termination-valid`

```semaprax
fn termination_valid(output: borrow Output) -> bool
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) >= 32usize, }
```

### `std.process.lengths-valid`

```semaprax
fn lengths_valid(output: borrow Output, total: usize) -> bool
    requires total >= 32usize && total <= 65536usize
    requires match borrow output { Output { data } => byte_len(bytes_as_slice(data)) == total, }
```

### `std.process.valid`

```semaprax
fn valid(output: borrow Output) -> bool
```

### `std.process.termination-kind`

```semaprax
fn termination_kind(output: borrow Output) -> usize
    requires valid(output)
```

### `std.process.termination-code`

```semaprax
fn termination_code(output: borrow Output) -> usize
    requires valid(output)
    requires termination_kind(output) <= 1usize
```

### `std.process.settlement-is-exit`

```semaprax
fn settlement_is_exit(kind: usize) -> bool
```

### `std.process.settlement-is-signal`

```semaprax
fn settlement_is_signal(kind: usize) -> bool
```

### `std.process.settlement-exit-code`

```semaprax
fn settlement_exit_code(kind: usize, code: usize) -> usize
    requires settlement_is_exit(kind)
```

### `std.process.stdout-len`

```semaprax
fn stdout_len(output: borrow Output) -> usize
    requires valid(output)
```

### `std.process.stderr-len`

```semaprax
fn stderr_len(output: borrow Output) -> usize
    requires valid(output)
```

### `std.process.copy-range`

```semaprax
fn copy_range(output: borrow Output, start: usize, length: usize, target: own Writer) -> Writer
    requires valid(output)
    requires match borrow output { Output { data } => start <= byte_len(bytes_as_slice(data)) && length <= byte_len(bytes_as_slice(data)) - start, }
    requires match borrow target { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && length <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.process.stdout-into`

```semaprax
fn stdout_into(output: borrow Output, target: own Writer) -> Writer
    requires valid(output)
    requires match borrow target { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && stdout_len(output) <= byte_len(bytes_as_slice(data)) - position, }
```

### `std.process.stderr-into`

```semaprax
fn stderr_into(output: borrow Output, target: own Writer) -> Writer
    requires valid(output)
    requires match borrow target { Writer { data, position } => position <= byte_len(bytes_as_slice(data)) && stderr_len(output) <= byte_len(bytes_as_slice(data)) - position, }
```

## `std.random`

Package `std/random`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.random = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.random.normalize_seed`

```semaprax
fn normalize_seed(value: i64) -> i64
    ensures result >= 1 && result <= 2147483646
```

### `std.random.next_seed`

```semaprax
fn next_seed(seed: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    ensures result >= 1 && result <= 2147483646
```

### `std.random.advance`

```semaprax
fn advance(seed: i64, steps: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    requires steps >= 0 && steps <= 100000
    ensures result >= 1 && result <= 2147483646
```

### `std.random.sample_below`

```semaprax
fn sample_below(seed: i64, upper: i64) -> i64
    requires seed >= 1 && seed <= 2147483646
    requires upper > 0 && upper <= 2147483647
    ensures result >= 0 && result < upper
```

## `std.test`

Package `std/test`, tier `test`, status partial. Required project profile: `scalar`. Dependency: `std.test = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.test.equal_i64`

```semaprax
fn equal_i64(actual: i64, expected: i64) -> bool
    ensures result == (actual == expected)
```

### `std.test.equal_bool`

```semaprax
fn equal_bool(actual: bool, expected: bool) -> bool
    ensures result == (actual == expected)
```

### `std.test.failure_unless`

```semaprax
fn failure_unless(condition: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.test.failure_if`

```semaprax
fn failure_if(condition: bool) -> i64
    ensures result == 0 || result == 1
```

### `std.test.failure_bit_unless`

```semaprax
fn failure_bit_unless(condition: bool, failure_bit: i64) -> i64
    requires failure_bit > 0
    ensures result == 0 || result == failure_bit
```

### `std.test.bit_for`

Failure masks. A case reports its own bit, so a nonzero test result names
exactly which checks failed instead of counting them.

```semaprax
fn bit_for(index: i64) -> i64
    requires index >= 0 && index <= 62
    ensures result > 0
```

### `std.test.bit_is_set`

```semaprax
fn bit_is_set(mask: i64, index: i64) -> bool
    requires mask >= 0 && index >= 0 && index <= 62
```

### `std.test.record_failure`

Accumulates one case's verdict. The bit must be unclaimed, so two cases
cannot silently share a bit and hide one another.

```semaprax
fn record_failure(mask: i64, index: i64, passed: bool) -> i64
    requires mask >= 0 && index >= 0 && index <= 62 && !bit_is_set(mask, index)
    ensures result >= mask
```

### `std.test.first_failure`

```semaprax
fn first_failure(mask: i64) -> i64
    requires mask >= 0
    ensures result >= -1 && result <= 62
```

### `std.test.failure_count`

```semaprax
fn failure_count(mask: i64) -> i64
    requires mask >= 0
    ensures result >= 0 && result <= 63
```

## `std.test.bytes`

Package `std/test-bytes`, tier `test`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.test.bytes = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.test.bytes.snapshot`

```semaprax
record Snapshot {
    name: Bytes,
    expected: Bytes,
}
```

### `std.test.bytes.snapshot-comparison`

```semaprax
record SnapshotComparison {
    equal: bool,
    expected_len: usize,
    actual_len: usize,
    first_difference: usize,
}
```

### `std.test.bytes.equal`

```semaprax
fn equal_bytes(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.test.bytes.equal-remaining`

```semaprax
fn equal_remaining(left: borrow Reader, right: borrow Reader) -> bool
    requires match borrow left { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
    requires match borrow right { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

### `std.test.bytes.failure-bit-equal`

```semaprax
fn failure_bit_equal_bytes(left: borrow Slice<u8>, right: borrow Slice<u8>, failure_bit: i64) -> i64
    requires failure_bit > 0
    ensures result == 0 || result == failure_bit
```

### `std.test.bytes.failure-bit-equal-remaining`

```semaprax
fn failure_bit_equal_remaining(left: borrow Reader, right: borrow Reader, failure_bit: i64) -> i64
    requires failure_bit > 0
    requires match borrow left { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
    requires match borrow right { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
    ensures result == 0 || result == failure_bit
```

### `std.test.bytes.snapshot-new`

```semaprax
fn snapshot_new(name: own Bytes, expected: own Bytes) -> Snapshot
```

### `std.test.bytes.compare-snapshot`

```semaprax
fn compare_snapshot(snapshot: borrow Snapshot, actual: borrow Reader) -> SnapshotComparison
    requires match borrow actual { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
```

## `std.text`

Package `std/text`, tier `core`, status partial. Required project profile: `useful-text-consumer.v1`. Dependency: `std.text = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.text.byte_len`

```semaprax
fn text_byte_len(value: borrow str) -> i64
```

### `std.text.contains`

```semaprax
fn contains(value: borrow str, needle: borrow str) -> bool
```

### `std.text.equals`

```semaprax
fn equals(left: borrow str, right: borrow str) -> bool
```

### `std.text.is_empty`

```semaprax
fn is_empty(value: borrow str) -> bool
```

### `std.text.starts_with`

```semaprax
fn starts_with(value: borrow str, prefix: borrow str) -> bool
```

## `std.time`

Package `std/time`, tier `core`, status partial. Required project profile: `scalar`. Dependency: `std.time = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.time.milliseconds`

```semaprax
fn milliseconds(seconds: i64) -> i64
    requires seconds >= 0 && seconds <= 9223372036854775
    ensures result >= 0
    ensures result % 1000 == 0
```

### `std.time.seconds_floor`

```semaprax
fn seconds_floor(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0
```

### `std.time.seconds_ceil`

```semaprax
fn seconds_ceil(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0
```

### `std.time.subsecond_milliseconds`

```semaprax
fn subsecond_milliseconds(milliseconds: i64) -> i64
    requires milliseconds >= 0
    ensures result >= 0 && result < 1000
```

### `std.time.deadline_reached`

```semaprax
fn deadline_reached(now_milliseconds: i64, deadline_milliseconds: i64) -> bool
    requires now_milliseconds >= 0 && deadline_milliseconds >= 0
    ensures result == now_milliseconds >= deadline_milliseconds
```

### `std.time.remaining_milliseconds`

```semaprax
fn remaining_milliseconds(now_milliseconds: i64, deadline_milliseconds: i64) -> i64
    requires now_milliseconds >= 0 && deadline_milliseconds >= 0
    ensures result >= 0
```

### `std.time.elapsed_milliseconds`

```semaprax
fn elapsed_milliseconds(start_milliseconds: i64, end_milliseconds: i64) -> i64
    requires start_milliseconds >= 0 && end_milliseconds >= start_milliseconds
    ensures result >= 0
```

### `std.time.saturating_add_milliseconds`

```semaprax
fn saturating_add_milliseconds(left: i64, right: i64) -> i64
    requires left >= 0 && right >= 0
    ensures result >= left && result >= right
```

## `std.tracing`

Package `std/tracing`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.tracing = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.tracing.byte_is_lower_hex`

Issue #193's tracing slice: the pure, effect-free decision layer that
admits or refuses a W3C Trace Context `traceparent`/`tracestate` header,
derives a child span context from an admitted parent, and judges a
sampling decision's consistency, all BEFORE a caller creates, propagates,
or exports a span. It performs no I/O, declares no `permit`, calls no
`uses`-gated operation, and never accepts, retains, or returns anything
beyond the header bytes and scalars it is asked to judge.

What this package is NOT: it does not generate a trace-id, a parent-id, or
a span-id — `child_context_admitted` takes the child's span-id as a
caller-supplied argument and only judges it, because generating one needs
a randomness capability this package does not have and must not pretend
to. It does not create, start, or export a span, and it does not
implement a sampler: `sampling_decision_consistent` judges whether an
outbound sampled bit is consistent with an inbound one, it does not choose
one. Nor does it split a raw `tracestate` header into its comma-separated
list-members: that needs a general delimiter scanner tolerant of the
header's optional surrounding whitespace, which this package still does
not implement, so `tracestate_member_admitted` takes an already-isolated
`key=value` member as its own slice rather than a whole header, and only
the two aggregate budgets a caller can compute without parsing (list-member
count, combined header length) cover the whole `tracestate` value. Key
admission covers only the spec's `simple-key` form; a `multi-tenant-key`
(`tenant-id "@" system-id`) that would be spec-valid is refused here.
`tracestate_mutation_admitted` checks one candidate key against one
sibling at a time (front-of-list placement and non-duplication); enforcing
it across a full list is the caller's loop, not this package's.

It also does not implement the spec's forward-compatibility allowance for
a higher-version header carrying additional bytes past position 55 (W3C
Trace Context section 3.2.4): `traceparent_shape_admitted` refuses any header
whose length is not exactly 55, including a spec-valid longer header from
a future version that a tolerant receiver would still accept. Handling
that needs slicing an unbounded trailing region, which this predicate
layer intentionally avoids; a caller that must support future versions
should truncate to the first 55 bytes itself before calling this
predicate. Nothing here is evidence that trace generation, propagation,
sampling, or export exists.

Every function is scalar-in/scalar-out or `borrow Slice<u8>`-in/
scalar-out, so a header's bytes are never copied into or out of this
package.
---------------------------------------------------------------------
Hex alphabet
---------------------------------------------------------------------
`traceparent` is lowercase-hex-only (W3C Trace Context Section 3.2.2): an
uppercase hex digit is a well-formed hex character in general but is not
an admitted `traceparent` byte, so it is refused here rather than
silently downcased.

```semaprax
fn byte_is_lower_hex(candidate: u8) -> bool
```

### `std.tracing.byte_at_equals`

---------------------------------------------------------------------
Field scanning primitives
---------------------------------------------------------------------
Every field-level predicate below is built from these three scans over a
caller-supplied byte range, rather than each reimplementing its own bounds
check. A range that runs past the end of the header is refused rather than
read.

```semaprax
fn byte_at_equals(header: borrow Slice<u8>, index: usize, expected: u8) -> bool
```

### `std.tracing.byte_at`

```semaprax
fn byte_at(header: borrow Slice<u8>, index: usize) -> u8
```

### `std.tracing.hex_run_admitted`

```semaprax
fn hex_run_admitted(header: borrow Slice<u8>, start: usize, length: usize) -> bool
```

### `std.tracing.field_is_all_zero`

The property that makes an all-numeric-zero trace-id or parent-id
detectable: every byte in the range is the ASCII digit `0`, which is
exactly the hex spelling of sixteen zero bytes.

```semaprax
fn field_is_all_zero(header: borrow Slice<u8>, start: usize, length: usize) -> bool
```

### `std.tracing.traceparent_len`

---------------------------------------------------------------------
Wire shape
---------------------------------------------------------------------
`version "-" trace-id "-" parent-id "-" trace-flags` is exactly 2 + 1 + 32
+ 1 + 16 + 1 + 2 = 55 bytes wide for the one version this package admits.

```semaprax
fn traceparent_len() -> usize
```

### `std.tracing.dash_positions_admitted`

```semaprax
fn dash_positions_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.version_is_forbidden`

---------------------------------------------------------------------
Version
---------------------------------------------------------------------
Version `ff` is forbidden outright (W3C Trace Context Section 3.2.2.1); every
other two-digit lowercase-hex version, including ones not yet defined,
shares this package's fixed 55-byte shape.

```semaprax
fn version_is_forbidden(header: borrow Slice<u8>) -> bool
```

### `std.tracing.version_field_admitted`

```semaprax
fn version_field_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.trace_id_admitted`

---------------------------------------------------------------------
Trace-id and parent-id: the all-zero refusal
---------------------------------------------------------------------
The highest-value predicate in this package. Both an all-zero trace-id and
an all-zero parent-id are declared invalid by the spec (Section 3.2.2.3,
Section 3.2.2.4): a receiver that only checks hex shape accepts a header meant to
carry no trace context at all as if it were a real one.

```semaprax
fn trace_id_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.parent_id_admitted`

```semaprax
fn parent_id_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.trace_flags_field_admitted`

---------------------------------------------------------------------
Trace-flags and the sampled bit
---------------------------------------------------------------------
Every byte value is an admitted `trace-flags` field; the seven reserved
bits carry no admission rule of their own (Section 3.2.2.5), only a hex-shape one.

```semaprax
fn trace_flags_field_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.traceparent_sampled`

The sampled flag is the least-significant bit of the decoded flags byte
(Section 3.2.2.5.1), which is entirely determined by the final hex digit: the
first digit's contribution is always a multiple of 16, hence always even.

```semaprax
fn traceparent_sampled(header: borrow Slice<u8>) -> bool
    requires traceparent_shape_admitted(header)
```

### `std.tracing.traceparent_shape_admitted`

---------------------------------------------------------------------
Composed traceparent admission
---------------------------------------------------------------------
The judgement a receiver actually calls. Every clause is one of the named
predicates above, so a refusal is always attributable to a named rule
rather than to this function as a whole.

```semaprax
fn traceparent_shape_admitted(header: borrow Slice<u8>) -> bool
```

### `std.tracing.tracestate_member_count_admitted`

---------------------------------------------------------------------
Tracestate budgets
---------------------------------------------------------------------
The two `tracestate` (Section 3.3) limits a caller can enforce without parsing a
single list-member out of the header: at most 32 members (Section 3.3.1.1), and a
combined header this package treats as bounded at the 512-character
threshold the spec recommends a vendor propagate (Section 3.3.1.5) — an admission
ceiling rather than a truncation rule, so an oversized header is refused
outright instead of silently shortened.

```semaprax
fn tracestate_member_count_admitted(count: usize) -> bool
```

### `std.tracing.tracestate_combined_length_admitted`

```semaprax
fn tracestate_combined_length_admitted(length: usize) -> bool
```

### `std.tracing.slice_equal`

---------------------------------------------------------------------
General byte-field comparison
---------------------------------------------------------------------
Two views are equal exactly when they are the same length and agree at
every index; a length mismatch refuses without reading a single byte.
Shared below by child-context consistency (fixed-width trace/parent-id and
trace-flags fields) and `tracestate` key comparison (variable-width keys).

```semaprax
fn slice_equal(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool
```

### `std.tracing.trace_context_fields_admitted`

---------------------------------------------------------------------
Outbound field admission
---------------------------------------------------------------------
The trace-id, parent-id, and trace-flags fields of one traceparent context,
each supplied as its own slice starting at index zero rather than embedded
in a full 55-byte header. `hex_run_admitted` and `field_is_all_zero` are
reused unchanged at `start = 0`; the exact-length check comes first for
every field, since `hex_run_admitted` alone only scans the first `length`
bytes and would silently accept a trailing extra byte it never reads —
exactly the "stops one byte early" failure shape this repository has hit
before. `version` is excluded here on purpose: it names the wire format of
a `traceparent` header, not a property of the trace context these three
fields carry, so `child_context_admitted` below reuses this without ever
taking a version argument. Factored out of both callers below (rather than
inlined at each) specifically because inlining it twice combined enough
independent `&&`/`!` branches in one function body to exceed the
cleanup-replay skeleton-work budget (SPX-H006); as a named call, each use
site is one opaque unit to that replay instead of a dozen more branches.

```semaprax
fn trace_context_fields_admitted(trace_id: borrow Slice<u8>, parent_id: borrow Slice<u8>, trace_flags: borrow Slice<u8>) -> bool
```

### `std.tracing.trace_context_fields_admitted_guarded`

Composes trace-context shape with the caller's classification of six
secret-bearing categories. It does not inspect secret bytes or tracestate;
adapters must classify those before calling this pure policy predicate.

```semaprax
fn trace_context_fields_admitted_guarded(trace_id: borrow Slice<u8>, parent_id: borrow Slice<u8>, trace_flags: borrow Slice<u8>, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.tracing.outbound_traceparent_fields_admitted`

The judgement an adapter assembling an outbound header actually calls
before joining `version "-" trace_id "-" parent_id "-" trace_flags` with
dashes and sending it. `version_field_admitted` is reused unchanged: its
two checked positions (0 and 1) are exactly a standalone two-byte version
field's only two bytes.

```semaprax
fn outbound_traceparent_fields_admitted(version: borrow Slice<u8>, trace_id: borrow Slice<u8>, parent_id: borrow Slice<u8>, trace_flags: borrow Slice<u8>) -> bool
```

### `std.tracing.child_context_admitted`

---------------------------------------------------------------------
Child-context derivation
---------------------------------------------------------------------
Whether a child span context derived from an admitted parent is itself
admitted and consistent with that parent (W3C Trace Context Section 3.2.2,
Section 3.3.1.3): the same trace-id throughout, a parent-id that differs
from the parent's own (a child cannot be its own parent), and trace-flags
carried forward unchanged — this predicate only checks that inheritance
held, it never decides a new sampling outcome; `sampling_decision_consistent`
below is where a changed flags byte gets judged. The child's span-id is
supplied by the caller (an argument, not a return value) because this
package has no randomness capability to generate one.

```semaprax
fn child_context_admitted(parent_trace_id: borrow Slice<u8>, parent_parent_id: borrow Slice<u8>, parent_trace_flags: borrow Slice<u8>, child_trace_id: borrow Slice<u8>, child_span_id: borrow Slice<u8>, child_trace_flags: borrow Slice<u8>) -> bool
```

### `std.tracing.trace_flags_sampled`

---------------------------------------------------------------------
Sampling-decision admission
---------------------------------------------------------------------
The sampled bit read from a standalone two-byte trace-flags field, the
counterpart to `traceparent_sampled` for a field that has not been
embedded in a full 55-byte header. Every other bit of trace-flags is
reserved and carries no admission rule (Section 3.2.2.5): any two-hex-digit
value is a legal flags byte, so nothing here narrows that further.

```semaprax
fn trace_flags_sampled(trace_flags: borrow Slice<u8>) -> bool
    requires byte_len(trace_flags) == 2usize && hex_run_admitted(trace_flags, 0usize, 2usize)
```

### `std.tracing.sampling_decision_consistent`

Whether an outbound sampling decision is consistent with the inbound one
it follows. The spec's one-way rule: a participant may raise an unsampled
context to sampled, but must never lower a sampled context back to
unsampled downstream, so the only refused transition is
sampled-inbound-then-unsampled-outbound. This decides consistency; it
never picks a sampling outcome itself.

```semaprax
fn sampling_decision_consistent(inbound_trace_flags: borrow Slice<u8>, outbound_trace_flags: borrow Slice<u8>) -> bool
    requires byte_len(inbound_trace_flags) == 2usize && hex_run_admitted(inbound_trace_flags, 0usize, 2usize) && byte_len(outbound_trace_flags) == 2usize && hex_run_admitted(outbound_trace_flags, 0usize, 2usize)
```

### `std.tracing.tracestate_key_shape_admitted`

---------------------------------------------------------------------
`tracestate` list-member shape
---------------------------------------------------------------------
A `simple-key`: 1 to 256 bytes, first byte a lowercase letter only (never a
digit), every remaining byte a lowercase letter, a digit, or one of the
four punctuation symbols `_ - * /` (Section 3.3.1.1). The `multi-tenant-key`
form (`tenant-id "@" system-id`) is out of scope; see the package header.

```semaprax
fn tracestate_key_shape_admitted(key: borrow Slice<u8>) -> bool
```

### `std.tracing.tracestate_value_shape_admitted`

A `value`: 1 to 256 bytes from the `chr` set — printable ASCII 0x20-0x7E
excluding the two delimiter bytes `,` and `=` (Section 3.3.1.2) — whose final
byte is not a space, per the grammar's trailing `nblk-chr`: a value may
carry interior whitespace but may not trail it.

```semaprax
fn tracestate_value_shape_admitted(value: borrow Slice<u8>) -> bool
```

### `std.tracing.tracestate_member_admitted`

The judgement a caller with one already-isolated `key=value` member slice
actually calls; splitting the raw `tracestate` header into such slices
remains the caller's job (see the package header). Counting `=` and
locating its one admitted occurrence share a single pass: counting rather
than accepting the first occurrence is deliberate, mirroring
`std.email.at_sign_count`, since a stray extra `=` before the intended one
would otherwise still look like a plausible split point.

```semaprax
fn tracestate_member_admitted(member: borrow Slice<u8>) -> bool
```

### `std.tracing.tracestate_mutation_admitted`

---------------------------------------------------------------------
`tracestate` ordering and duplication
---------------------------------------------------------------------
A mutating vendor's entry must become the list's first member (Section 3.3.1.3)
and its key must not already belong to another member. Checked pairwise —
one candidate key against one sibling key at a time, since this package
does not walk an unbounded member list (see the package header); a caller
enforcing this across N members calls it once per sibling with
`first_member_key` held fixed.

```semaprax
fn tracestate_mutation_admitted(mutated_key: borrow Slice<u8>, first_member_key: borrow Slice<u8>, sibling_key: borrow Slice<u8>) -> bool
```

### `std.tracing.tracestate-field-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn tracestate_field_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.tracing.span-name-byte-admitted`

---------------------------------------------------------------------
Spans (issue #193)
---------------------------------------------------------------------
Everything above is propagation: the bytes that travel between processes.
A span is what a process records about its own work, and none of its own
fields -- an operation name, a kind, a status, a duration, an attribute
set -- has had a rule until here. Each of them is caller-controlled, and
three of them are the direct analogue of metric label cardinality: an
operation name assembled from a request path, an attribute set with
unbounded arity, and a duration read from an untrusted clock.

Nothing here starts, ends, records, or exports a span. These are
decisions about values a caller has already assembled, and this package
still declares no `permit` and performs no I/O.

```semaprax
fn span_name_byte_admitted(candidate: u8) -> bool
```

### `std.tracing.span-name-admitted`

A span name identifies an *operation*, not an occurrence of one. The
alphabet is deliberately narrow and lowercase so that `GET /users/4711`
cannot be a span name: a name carrying an identifier makes every request
its own series downstream, which is metric-label cardinality wearing a
different hat. Uppercase is refused rather than folded, because folding
hides the caller's mistake instead of reporting it.

```semaprax
fn span_name_admitted(name: borrow Slice<u8>) -> bool
```

### `std.tracing.span-kind-admitted`

The closed kind vocabulary: 0 internal, 1 server, 2 client, 3 producer,
4 consumer. A sixth kind means changing this function, not passing a
larger number.

```semaprax
fn span_kind_admitted(kind: i64) -> bool
```

### `std.tracing.span-status-admitted`

0 unset, 1 ok, 2 error. `unset` is distinct from `ok` on purpose: a span
nobody judged is not a span that succeeded.

```semaprax
fn span_status_admitted(status: i64) -> bool
```

### `std.tracing.span-duration-limit-nanos`

One hour in nanoseconds. A span longer than this is far likelier to be a
clock that moved, an end timestamp that was never written, or an
attacker-supplied duration than a unit of work, and admitting it lets one
bad value dominate every latency aggregate built from the trace.

```semaprax
fn span_duration_limit_nanos() -> i64
```

### `std.tracing.span-duration-admitted`

A span's clock readings are data, not authority: an end before its start
is refused rather than clamped, so a negative duration never becomes a
zero-duration span that silently disappears from a latency distribution.

```semaprax
fn span_duration_admitted(start_nanos: i64, end_nanos: i64) -> bool
```

### `std.tracing.span-attribute-count-admitted`

The same per-observation arity budget the shared redaction policy already
owns, reused rather than restated so a span's attribute set and a log
event's field set cannot drift to different ceilings.

```semaprax
fn span_attribute_count_admitted(count: usize) -> bool
```

### `std.tracing.span-attribute-admitted`

A span attribute is a named field proposed for export, so it is judged by
exactly the shared projection policy -- protected names, classified
values, the visible marker, commitment shape -- with this package's own
name grammar layered on top. A secret attribute is refused, never
truncated or hashed into something that looks harmless.

```semaprax
fn span_attribute_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.tracing.fresh-span-id-admitted`

---------------------------------------------------------------------
Fresh identity and hostile inbound propagation
---------------------------------------------------------------------
A trace-id or span-id this process mints must come from a declared
randomness capability, not from a counter, a clock, or an echo of
whatever arrived on the wire. `randomness_declared` is the caller's
statement that it holds one; this package cannot generate an identifier
itself and does not pretend to. The generated value must also differ from
the inbound one it replaces, so "generate a fresh id" cannot be satisfied
by reusing the attacker's.

```semaprax
fn fresh_span_id_admitted(span_id: borrow Slice<u8>, inbound_span_id: borrow Slice<u8>, randomness_declared: bool) -> bool
```

### `std.tracing.fresh-trace-id-admitted`

```semaprax
fn fresh_trace_id_admitted(trace_id: borrow Slice<u8>, inbound_trace_id: borrow Slice<u8>, randomness_declared: bool) -> bool
```

### `std.tracing.inbound-context-adoption-admitted`

Two separate questions that a naive implementation collapses into one.
Whether an inbound `traceparent` is well-formed is a parsing question,
answered above. Whether this process should *continue* that trace is a
trust question: a well-formed header from an unauthenticated peer is a
caller-chosen trace-id, which lets anyone on the internet stitch their
traffic into an internal trace, replay a trace-id to correlate internal
work, or inflate a backend's per-trace storage at will. A well-formed
context is adopted only from a trusted peer.

```semaprax
fn inbound_context_adoption_admitted(inbound_shape_admitted: bool, peer_is_trusted: bool) -> bool
```

### `std.tracing.fresh-root-required`

The complement, and the reason the two questions are separate: whenever a
context is not adopted -- malformed, or well-formed but untrusted -- this
process starts a fresh root rather than repairing, partially reusing, or
silently dropping the inbound one. A repaired hostile context is still a
hostile context.

```semaprax
fn fresh_root_required(inbound_shape_admitted: bool, peer_is_trusted: bool) -> bool
```

### `std.tracing.span-admitted`

The composed span judgement. Every clause is one of the named predicates
above, so a refusal is attributable to a stated rule.

```semaprax
fn span_admitted(name: borrow Slice<u8>, kind: i64, status: i64, start_nanos: i64, end_nanos: i64, attribute_count: usize) -> bool
```

### `std.tracing.span-admitted-guarded`

The same judgement with the shared redaction policy's six caller-declared
flags applied: a span the caller has classified as carrying a secret in
its name or attributes is refused before it is recorded, independent of
every shape rule above.

```semaprax
fn span_admitted_guarded(name: borrow Slice<u8>, kind: i64, status: i64, start_nanos: i64, end_nanos: i64, attribute_count: usize, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

## `std.url`

Package `std/url`, tier `portable`, status partial. Required project profile: `scalar`. Dependency: `std.url = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.url.is_scheme_start`

```semaprax
fn is_scheme_start(byte: u8) -> bool
```

### `std.url.is_scheme_continue`

```semaprax
fn is_scheme_continue(byte: u8) -> bool
```

### `std.url.is_unreserved`

```semaprax
fn is_unreserved(byte: u8) -> bool
```

### `std.url.is_percent_triplet`

```semaprax
fn is_percent_triplet(marker: u8, high: u8, low: u8) -> bool
```

### `std.url.decode_percent_triplet`

```semaprax
fn decode_percent_triplet(marker: u8, high: u8, low: u8) -> i64
    ensures result >= -1 && result <= 255
```

## `std.webhook`

Package `std/webhook`, tier `portable`, status partial. Required project profile: `useful-data.v2`. Dependency: `std.webhook = "^0.1.0"`. Targets: `interpreter`, `native-c11`, `core-wasm`.

### `std.webhook.replay_window_seconds`

Issue #193's webhook slice: the pure, effect-free decision layer an
inbound-webhook receiver consults BEFORE it parses a payload or acts on
one, and an outbound sender consults before it attempts a delivery. It
performs no I/O, declares no `permit`, calls no `uses`-gated operation,
and never accepts, retains, or returns a signing secret.

What this package is NOT: it does not compute or verify an HMAC, open a
connection, or retry a delivery. Constant-time MAC comparison needs a
primitive this language does not yet admit, and getting it wrong is worse
than not shipping it, so signature *verification* is deliberately absent
while signature *envelope* policy is present. Nothing here is evidence
that webhook delivery or verification exists.

Every function is scalar-in/scalar-out or `borrow Slice<u8>`-in/
scalar-out, so neither a payload nor a secret is copied into or out of
this package.
---------------------------------------------------------------------
Replay window
---------------------------------------------------------------------
The property that actually stops a captured-and-replayed delivery. A
receiver that verifies a signature but ignores the timestamp accepts a
valid signed request forever; the signature stays valid because the bytes
never change. Both directions are bounded on purpose: a timestamp far in
the future is as much a forgery signal as one far in the past, and a
receiver that clamps only the past accepts a clock-skewed forgery
indefinitely.

```semaprax
fn replay_window_seconds() -> i64
```

### `std.webhook.timestamp_within_window`

```semaprax
fn timestamp_within_window(signed_at: i64, now: i64) -> bool
```

### `std.webhook.signature_hex_len`

---------------------------------------------------------------------
Signature envelope shape
---------------------------------------------------------------------
A receiver must decide what a signature header may look like before it
compares anything. A hex-encoded SHA-256 MAC is 64 lowercase hex digits:
exactly that, nothing shorter, nothing uppercase, nothing with separators.
Refusing early means a malformed header never reaches a comparison
routine at all.

```semaprax
fn signature_hex_len() -> usize
```

### `std.webhook.byte_is_lower_hex`

```semaprax
fn byte_is_lower_hex(candidate: u8) -> bool
```

### `std.webhook.signature_shape_admitted`

```semaprax
fn signature_shape_admitted(signature: borrow Slice<u8>) -> bool
```

### `std.webhook.payload_len_admitted`

---------------------------------------------------------------------
Payload budget
---------------------------------------------------------------------
One deployment-independent ceiling on an inbound body. A receiver that
buffers an unbounded payload before deciding anything about it has handed
an attacker a memory cost with no authentication in front of it. 65536 is
deliberately small; a profile may lower it, never leave it unstated.

```semaprax
fn payload_len_admitted(length: usize) -> bool
```

### `std.webhook.attempt_admitted`

---------------------------------------------------------------------
Delivery attempt budget
---------------------------------------------------------------------
A sender retrying without a ceiling turns one failing endpoint into an
outbound amplifier. Attempts are counted from 1, so 0 is not a legitimate
attempt number and is refused rather than treated as "before the first".

```semaprax
fn attempt_admitted(attempt: i64) -> bool
```

### `std.webhook.backoff_seconds`

Exponential backoff with a fixed base and a hard ceiling, defined for
exactly the admitted attempt numbers so a caller cannot ask about attempt
9 and receive a plausible delay.

```semaprax
fn backoff_seconds(attempt: i64) -> i64
    requires attempt_admitted(attempt)
    ensures result >= 1
    ensures result <= 60
```

### `std.webhook.delivery_admitted`

---------------------------------------------------------------------
Composed receiver admission
---------------------------------------------------------------------
The judgement an inbound receiver makes before it parses anything. Every
clause is one of the predicates above, so a refusal is always attributable
to a named rule.

```semaprax
fn delivery_admitted(signature: borrow Slice<u8>, payload_len: usize, signed_at: i64, now: i64) -> bool
```

### `std.webhook.delivery_admitted_guarded`

---------------------------------------------------------------------
Redaction guard (issue #193)
---------------------------------------------------------------------
Everything above judges signature shape, payload budget, and replay
window -- none of it has any notion of a payload's own content. A payload
that accidentally embeds the webhook's own signing secret in cleartext (a
caller-assembled body that echoes its configuration back, say) passes
every check above unchanged. This composes the existing admission with
`std.log.redact.event_is_safe`'s six caller-declared flags, exactly like
`std.log.append-event-guarded` gates a log event, so a delivery the caller
has classified as carrying a secret in its payload is refused before an
attempt is ever made, independent of signature, size, or timestamp.

```semaprax
fn delivery_admitted_guarded(signature: borrow Slice<u8>, payload_len: usize, signed_at: i64, now: i64, carries_password: bool, carries_api_key: bool, carries_bearer_token: bool, carries_session_token: bool, carries_webhook_signing_secret: bool, carries_smtp_credential: bool) -> bool
```

### `std.webhook.payload-field-projection-admitted`

policy owns protected names, classified values, marker and commitment shape;
this package retains its existing byte/size grammar. This does not send,
authenticate a commitment, discover secrets in arbitrary content, or make
legacy unguarded shape helpers secret-safe.

```semaprax
fn payload_field_projection_admitted(name: borrow Slice<u8>, classified_secret: bool, projection: borrow Slice<u8>, commitment: borrow Slice<u8>) -> bool
```

### `std.webhook.key-byte-admitted`

---------------------------------------------------------------------
Delivery identity, idempotency, and uncertain delivery (issue #193)
---------------------------------------------------------------------
The section above judges one delivery in isolation: is this signature
well-shaped, is this body inside budget, is this timestamp fresh. None of
it can tell a first delivery from the same delivery arriving twice, and
`attempt_admitted`/`backoff_seconds` bound how often a sender retries
without ever saying whether retrying *this* delivery is safe at all.

That is the gap this slice closes. A webhook is an external effect: a
retry after a response that never arrived either recovers a lost delivery
or performs the effect a second time, and the sender cannot tell which.
The only honest answer is a first-class `uncertain` outcome plus a rule
that refuses to retry it unless the receiver was given an idempotency key
it can deduplicate against.

As everywhere else in this package, nothing here delivers, signs,
deduplicates, or stores anything. These are decisions about
already-assembled identifiers and an already-observed outcome.

```semaprax
fn key_byte_admitted(candidate: u8) -> bool
```

### `std.webhook.idempotency-key-len-admitted`

Long enough not to collide by accident, short enough to bound what a
receiver must store and index. A key outside this range is refused rather
than truncated: truncating two distinct keys to the same prefix is how a
deduplication table starts suppressing deliveries it should have made.

```semaprax
fn idempotency_key_len_admitted(length: usize) -> bool
```

### `std.webhook.idempotency-key-admitted`

A key is opaque: it carries no structure a receiver is expected to parse,
and deliberately admits no byte that could terminate a header line, open
a quoted string, or survive into a log field as punctuation.

```semaprax
fn idempotency_key_admitted(key: borrow Slice<u8>) -> bool
```

### `std.webhook.delivery-id-len`

A delivery identifier names one delivery across all of its attempts, so a
receiver's log and a sender's log can be joined without either side
guessing. Exactly 32 lowercase hex digits: the same shape discipline the
signature envelope already uses, one length down.

```semaprax
fn delivery_id_len() -> usize
```

### `std.webhook.delivery-id-admitted`

```semaprax
fn delivery_id_admitted(delivery_id: borrow Slice<u8>) -> bool
```

### `std.webhook.outcome-admitted`

---------------------------------------------------------------------
Delivery outcomes
---------------------------------------------------------------------
One closed vocabulary for what happened to an attempt:

 0 delivered              3 uncertain
 1 refused-permanently    4 signature-invalid
 2 transient-failure      5 duplicate-suppressed

```semaprax
fn outcome_admitted(outcome: i64) -> bool
```

### `std.webhook.outcome-is-uncertain`

`uncertain` is the outcome a sender records when the request left but no
response arrived. It is not a failure and must not be reported as one:
collapsing it into "failed" is what makes a retry look free.

```semaprax
fn outcome_is_uncertain(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.webhook.outcome-is-terminal`

```semaprax
fn outcome_is_terminal(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.webhook.outcome-is-retryable`

```semaprax
fn outcome_is_retryable(outcome: i64) -> bool
    requires outcome_admitted(outcome)
```

### `std.webhook.retry-admitted`

The rule. A transient failure never reached a handler, so retrying it
duplicates nothing. An uncertain outcome may already have been handled,
so it is retried only when the receiver was given an idempotency key it
can deduplicate against; without one, the sender must stop and surface the
uncertainty rather than guess.

```semaprax
fn retry_admitted(outcome: i64, attempt: i64, has_idempotency_key: bool) -> bool
    requires outcome_admitted(outcome)
```

### `std.webhook.duplicate-suppression-admitted`

A receiver that has already handled this key answers `duplicate-suppressed`
rather than performing the effect again. Suppression requires a key: a
receiver with no key has nothing to deduplicate on and must treat the
delivery as new.

```semaprax
fn duplicate_suppression_admitted(has_idempotency_key: bool, key_already_seen: bool) -> bool
```

### `std.webhook.delivery-plan-admitted`

The composed sender-side judgement before an attempt leaves: the existing
envelope policy, a well-formed delivery identity, and -- for a delivery
the sender intends to be able to retry at all -- an idempotency key. A
sender that declares `retryable` without a key is refused here rather than
discovering the problem after the first uncertain response.

```semaprax
fn delivery_plan_admitted(signature: borrow Slice<u8>, payload_len: usize, signed_at: i64, now: i64, delivery_id: borrow Slice<u8>, idempotency_key: borrow Slice<u8>, intends_retry: bool) -> bool
```
