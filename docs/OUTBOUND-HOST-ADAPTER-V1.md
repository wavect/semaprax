# Outbound Host Adapter v1

Status: implementation slice for issue #193; deterministic fixture evidence
only. No hosted or public-network support claim follows from this module.

Audience: runtime integrators and reviewers of outbound application effects.

`src/outbound_host_adapter/` connects pure `std` policy packages to bounded
HTTPS through an injected host boundary. It covers typed
GET/POST/PUT/PATCH/DELETE, operational export, signed webhooks, and a
provider-neutral email envelope. Policy reads no environment and starts no
threads or timers. `NativeHttpsAdapter` performs physical HTTPS with host-supplied
TLS policy and no ambient proxies, redirects, or retries. Most tests use a
recording fixture without network I/O. Native-adapter integration uses only a
configured private-root loopback TLS listener, not a system root or public
endpoint.

## Authority and admission

An `OutboundCapability` names the deployment binding, invocation, exact HTTPS
origins, and hard request, response, deadline, field, and label limits. It is
move-only and is consumed by one call, preventing this API from reusing a grant
for automatic retry. Its public Rust constructor is a **trusted embedder seam**,
not an unforgeable token against arbitrary Rust code: calling it asserts that
deployment policy already authorized the operation. SEMAPRAX source cannot call
that constructor. The capability is ordinary host configuration, not proof that
an endpoint was reached. Evidence cannot be converted back into it.

Endpoint syntax, exact origin membership, deadline, request size, content type,
delivery identity, idempotency key, export cardinality, and protected-field
shape are checked before `OutboundAdapter::send`. Origins have no wildcard;
userinfo, fragments, insecure schemes, controls, and noncanonical policy origins
are refused. Redirects and retries are absent from the adapter contract. A
trusted custom transport implementation is required to issue the one prepared
request exactly once, but arbitrary Rust implementations can violate that
contract. Only `NativeHttpsAdapter` mechanically configures no proxy, no
redirect, and no retry.
Exact origin admission is not IP pinning or DNS-rebinding prevention; a host
that requires network-range policy must enforce it in its resolver or network
boundary.

Webhook headers are boundary-owned. Callers cannot inject authorization or
signature headers. `WebhookSigningSecret` is explicit, move-only, and redacted
from `Debug`; its owned Rust array is zeroized on drop, with no claim about
caller, allocator, transport, process-dump, or operating-system copies.
HMAC-SHA-256 v2 binds a domain separator, `POST`, deployment binding, canonical
origin, canonical request target, exact delivery ID, idempotency key, content
type, and body. The delivery ID and idempotency key are also included in
evidence; neither authorizes replay.

Idempotency is receiver-enforced: this adapter sends the key but cannot prove
the receiver honored it. The one-shot helpers keep no durable state. The
optional host-owned durable ledger described below preserves only local
intent/disposition knowledge; uncertain settlement remains uncertain and never
authorizes an automatic retry, even when a key was sent.

## Host-owned reconciliation ledger

`HostDeliveryLedger` is an optional bounded reconciliation primitive for
host code that owns a prepared-request dispatch boundary. It is deliberately
separate from `OutboundCapability` and `DeliveryEvidence`: a trusted host
chooses to invoke `reconcile` *before* its one-shot adapter action, supplying
its bounded deployment/invocation/idempotency identity and the exact
`PreparedRequest`.
The ledger independently derives the existing canonical request digest and
requires its idempotency component to be the one exact boundary-owned
`idempotency-key` header. It records only the digest plus the closed local
disposition.
Legacy POST requests retain the v1 digest domain and byte layout. Non-POST
requests use the v2 domain and bind the selected method before the remaining
request fields, so extending the adapter does not silently rewrite existing
email, webhook, export, or POST evidence commitments.

The existing `deliver_*` convenience calls remain stateless, one-shot helpers.
The additive `prepare_email_delivery` plus `EmailDeliverySession::reconcile`
route is the first in-tree family integration: it validates and constructs the
exact request before entering the ledger, then lets only a fresh exact identity
enter the injected adapter closure. A matching request gets a newly derived
authority-free receipt and never calls `send`; a changed request conflicts
before dispatch. Fresh physical work still requires the injected capability
inside `PreparedEmailDelivery`.

This email session deliberately settles **disposition-only**. It discards even
a bounded nonempty accepted provider response before it writes replay state and
its receipt type exposes no response bytes. Therefore exact replay needs the
canonical request, identity/idempotency, policy commitment, and closed
disposition only; it neither pretends to replay provider payloads nor retains
them in process memory. A panic after adapter entry leaves both the pre-dispatch
policy commitment and the ledger's pre-reserved `Uncertain { Transport }`
terminal record sticky, so an exact replay can reconstruct the uncertainty
without adapter entry. A normal ledger refusal rolls back a newly staged policy
commitment. The policy commitment domain-separates the policy ID, sorted
allowed origins, and every request/response/deadline/export limit, so a host
cannot silently replay through a changed policy with the same policy ID.
Policy commitments and session identities are SHA-256 values;
no request, raw identity, credential, or response bytes are stored. This does
not silently change legacy helper behavior.

For an exact identity and request digest, a later `reconcile` call returns the
remembered disposition and never enters its dispatch closure. A different
digest under the same identity refuses before dispatch. `Accepted`, `Rejected`,
and every `Uncertain` observation are sticky; uncertainty is specifically never
auto-retried. `NotDispatched` is also retained for the same exact-pair
non-redispatch rule, so an explicit new host attempt must use a distinct
invocation identity rather than quietly reuse the first attempt. The ledger
reserves a conservative `Uncertain { Transport }` disposition immediately
before it invokes the host closure; if that closure unwinds after a physical
start, normal Rust unwinding leaves the reservation intact and a caught caller
still cannot retry through the ledger.

The ledger has a fixed maximum of 256 entries, rejects a full ledger before
dispatch, stores no request body, endpoint, headers, response body, or raw
identity components, and renders a deterministic diagnostic snapshot using
only SHA-256 identity and request commitments. Those hashes are not
confidential redaction: low-entropy inputs can be guessed and tested offline.
Plain `reconcile` remains process-local. The additive `reconcile_durable`
requires an injected `LedgerCheckpointStore` and acknowledges the provisional
`Uncertain { Transport }` checkpoint before entering the physical dispatch
closure, then acknowledges the terminal observation afterwards. Only
`CheckpointCommit::Committed` permits dispatch. A known-not-committed intent is
removed without dispatch; an uncertain intent stays sticky without dispatch.
If terminal persistence is known failed or uncertain, the live ledger falls
back to the already acknowledged provisional uncertainty. No timer, thread, or
automatic retry is created.

HTTP, webhook, and email sessions add a typed durable envelope above that
lower ledger. `semaprax.outbound.delivery-session-checkpoint.v1` binds one
closed delivery kind, the canonical lower checkpoint and its digest, and a
sorted vector of session-identity, complete-policy, request, and lower-ledger
identity commitments. It is bounded to 192 KiB and independently rejects a
wrong kind, capacity, digest, noncanonical JSON, unknown members, or any
one-to-one binding mismatch before a session can be restored. The typed store
sees that complete envelope for both the provisional intent and terminal
observation; it never receives raw identities, request bytes, headers, bodies,
responses, signing keys, or an adapter capability.

`HttpDeliverySession`, `WebhookDeliverySession`, and `EmailDeliverySession`
each expose typed `reconcile_durable`, `session_checkpoint`, and
`restore_authenticated` APIs. A trusted storage host grants the restore
capability against the exact outer digest and capacity. Exact restored replay
never enters an adapter; an unacknowledged intent never enters an adapter; and
an unacknowledged terminal state remains the already persisted uncertainty.
This closes a local crash window only. It does not prove a remote receipt,
receiver idempotency, durable-store freshness, or distributed exactly-once
delivery.

### Authenticated service HTTP intent facts

The additive native-host
`outbound_delivery_store::service_invocation::authenticated_intent` module
provides `read_http_intent` and `deliver_http_durable_authenticated`. This is a
storage prerequisite for checked webhook admission; the reference host still
uses its existing v1 JSON-event or OTLP route. It does not yet select a new
event protocol or invoke the scaffold webhook policy.

The authenticated marker uses the **same** identity filename as the original
pending marker. Its bounded (512-byte maximum) canonical UTF-8 wire contains
six newline-terminated fields, in order:

1. `semaprax.outbound.authenticated-http-intent.v1`;
2. the exact `sha256:` pending identity key;
3. a lowercase `sha256:` HTTP body commitment;
4. a canonical nonnegative `i64` signing timestamp;
5. the literal first-attempt number `1`;
6. a 64-character lowercase HMAC-SHA-256 tag.

The body commitment hashes the domain
`semaprax.outbound.authenticated-http-intent.body.v1\0`, the body's little-endian
u64 byte length, and the actual request body bytes. It is explicitly a body
commitment, not an endpoint/header/policy commitment. The MAC uses an
independently held 32-byte host key, the domain
`semaprax.outbound.authenticated-http-intent.mac.v1\0`, and the first five fields
including their newlines. The wire stores no raw payload or key; a body digest
can still disclose guessable low-entropy inputs. The timestamp is a host-supplied
fact; this generic primitive does not establish that an event envelope signed
that timestamp. That binding belongs to the future event encoder and caller.

A read is confined to the caller-held directory and exact identity name.
`Absent` requires an explicit no-follow absence observation. The original
fixed marker returns `LegacyBlocked` with no fabricated prior facts; unknown,
noncanonical, oversized, wrong-key, identity-transplanted, or changed records
refuse. The read performs no write and chooses no latest checkpoint. An
absence observation is not a reservation: another invocation can create the
marker before dispatch. Every new delivery still requires its own successful
atomic create-new, exact-content check, sync and named recheck, followed by
the ordinary typed provisional checkpoint ACK. Existing markers of either
version are never upgraded, replaced, or forgiven. Both directions of a
version switch therefore retain the no-redispatch rule. A retained authenticated
intent proves a reserved first attempt, not adapter entry or remote receipt;
reader authentication does not establish freshness against store deletion or
rollback. File-only versus namespace-synced durability retains the existing
store distinction.

Focused regressions live under
`outbound_delivery_store::service_invocation::tests::authenticated_intent` in
the native-host library harness: real dispatch/reopen, exact facts, changed
request/policy/capacity restart, both legacy transitions, tamper/wrong-key/name
binding, invalid-time nonmutation, competing absence observations and atomic
creation, a marker-only crash window, symlink refusal, and oversized records.
The owning native-host library selector passed locally: 19 cases, including
the eight new authenticated-intent cases and the existing durable-service
regressions. This does not establish checked webhook policy or provider
acceptance.

The native host crate now supplies `OutboundDeliveryStore`, an additive local
implementation of the typed HTTP, webhook, and email checkpoint-store
interfaces. Construction requires a caller-held `HeldDirectory`; it does not
read configuration, resolve a storage path, create a directory, or grant
authority. Each checkpoint is written once under a deterministic
kind-plus-SHA-256 filename with create-new semantics and a 192 KiB per-file
limit. Before `Committed`, both a new write and an idempotent existing write
reopen the current namespace entry through a bounded, sync-capable regular-file
admission, compare its exact bytes, successfully sync that held file, and
recheck that the name still resolves to the same authenticated object/content.
That successful named recheck is the lock-free ACK linearization point; a later
external replacement is outside the guarantee. This commit path requires write
access to the checkpoint file; inability to acquire or sync that authority
stays `Uncertain`, never a read-only ACK. Recovery `load` uses a separate
bounded read-only admission. A sparse oversized file is refused before
digesting; Unix opens are nonblocking so a FIFO cannot stall admission.
Changed, unreadable, or uncertain files fail closed. Recovery loads only the
exact digest retained by the host, then still requires the existing
authenticated restore API and its independently supplied digest/capacity
capability. The store does not enumerate or select a latest checkpoint. Hosts
remain responsible for directory provisioning, retention/quota, and the
trusted reference that identifies a checkpoint.

`OutboundDeliveryStore::new` preserves the default `FileOnly` acknowledgment:
file-content sync does not flush the containing directory. A trusted host can
explicitly select `OutboundCheckpointSyncMode::NamespaceSynced` through
`with_sync_mode`, still supplying its independently held directory capability.
The mode is a durability request, not a path or authority grant. Both new and
idempotent existing commits require, in order, exact byte comparison, successful
held-file sync, successful held-directory sync, then the same named identity
and content recheck before `Committed`. Neither an existing filename nor a
previous successful acknowledgment permits either sync to be skipped. Either
sync failure, unsupported directory sync, or failed final recheck yields
`Uncertain`; no adapter dispatch or automatic storage retry is authorized by
that result. A same-content replacement object during directory sync refuses
at the final named recheck.

The safe platform facade performs directory sync through the already-held
Unix directory descriptor, with identity rechecks around the OS call. Non-Unix
platforms return `Unsupported`, never a no-op success; the local outbound store
currently compiles on Linux, macOS, and Windows, so the stronger store mode is
available on Linux/macOS and fails closed on Windows. `Committed` records these
successful OS synchronization calls and the final named recheck, not an
unconditional hardware/filesystem power-loss guarantee. It does not sync or
authenticate ancestor directories, create durable provisioning authority,
coordinate multiple processes, select a latest record, or prevent later
external replacement/rollback. Reopen/tamper, ordered sync/refusal, and typed
HTTP/webhook/email no-redispatch tests use local temporary directories and
recording adapters only; no provider or network call is made by these tests.

This is local adapter implementation evidence, not real-provider acceptance,
provider authentication/interoperability, hosted execution, or authorization to
use credentials or public endpoints. Those require separately authorized
operator evidence and are not part of this implementation.

### Read-only disposition checkpoints

`HostDeliveryLedger::checkpoint` and the corresponding method on the HTTP,
email, webhook, and structured-export sessions produce an immutable
`LedgerCheckpoint`. Its additive
`semaprax.outbound.delivery-ledger-checkpoint.v1` JSON wire contains exactly
`schema`, `capacity`, and `entries`. Each entry contains exactly an identity
commitment, a request commitment, and the closed local disposition, ordered by
identity commitment. The older one-way diagnostic `render` format is unchanged.
Neither format contains endpoint, headers, signing keys, raw identity parts,
request/response bytes, or provider errors. SHA-256 commitments can still expose
guessable low-entropy inputs to offline comparison.

The checkpoint digest domain is
`semaprax.outbound.delivery-ledger-checkpoint.v1\0`; SHA-256 covers that domain,
the canonical wire byte length as little-endian u64, and the exact wire bytes.
`decode(bytes, expected_digest)` bounds input to 98,304 bytes before hashing or
JSON parsing, requires the independently retained digest, and validates the
closed schema, 1–256 capacity, entry count no greater than capacity, lowercase
SHA-256 values, unique identities, closed dispositions, and byte-exact canonical
re-rendering. Accepted statuses must be 2xx; rejected statuses are the remaining
100–599 values. Duplicate fields/entries, reordered entries, alternate JSON
spellings, unknown members, and invalid status/reason combinations refuse.
The maximum 256-entry checkpoint fits below the wire limit.

An imported checkpoint offers read-only `lookup` by exact identity and prepared
request. Unknown identities remain unknown; changed request bytes conflict.
`verify_against` compares its complete state and capacity against a still-live
ledger; each family session exposes `verify_checkpoint` for the same comparison.
`merge` computes a bounded union of compatible observations without modifying
either input. Capacity must match, and any request or disposition disagreement
refuses, including replacing an uncertainty with acceptance. A panic-reserved
uncertainty is preserved when exported and imported.

The checkpoint by itself remains offline observation transport. It cannot
construct a capability, delivery receipt, store, or dispatch. A trusted storage
host may separately grant a move-only `LedgerRestoreCapability` bound to the
exact independently retained checkpoint digest and capacity, then call
`restore_authenticated`. That explicit authority restores only commitment and
disposition state; exact known requests replay without dispatch and conflicting
requests refuse. Constructing the restore capability around attacker-selected
bytes authenticates nothing: the host must authenticate provenance/freshness
and protect against rollback. The module performs no filesystem I/O or fsync,
does not retain raw identities or request bytes, and cannot prove remote receipt
or receiver idempotency. It closes the local ACK-before-dispatch crash window
for a correctly implemented store, not the distributed exactly-once problem.

## High-level HTTPS requests

`deliver_http` and `prepare_http_delivery` admit an explicit `HttpMethod`,
canonical HTTPS endpoint, stable request and idempotency identities, optional
content type, bounded public headers, body, and deadline. The physical adapter
receives the selected method; the canonical request digest binds that method
in addition to endpoint, ordered headers, body, deadline, redirect limit, and
response limit. GET bodies are refused. Redirects and transport retries remain
fixed at zero for every method.

Header admission is deliberately narrow. Names must be canonical lowercase,
values are bounded and control-free, duplicates refuse, and callers cannot
supply the boundary-owned `content-type` or `idempotency-key` names.
Credential-shaped names (`authorization`, `proxy-authorization`, `cookie`,
`set-cookie`, and `x-api-key`) and the shared exact protected-name aliases
(`api-key`, `bearer-token`, `session-token`, `access-token`, `refresh-token`,
and deployment-signing/SMTP names) are refused from the caller-controlled
value. Matching remains exact after ASCII case and dash/underscore
normalization rather than guessing from arbitrary substrings. A trusted
provider adapter may apply deployment-owned credentials outside the request
value; this module does not read a secret store or expose credentials to
source, debug output, checkpoints, or evidence.

`HttpDeliverySession` gives the same bounded disposition replay
as email/webhook/export sessions. An exact identity and complete request,
including method, replays without entering the adapter; changing only the
method conflicts before dispatch. Accepted response bytes are intentionally
dropped by the session, while the one-shot `deliver_http` result may return a
bounded accepted body. Its typed durable route persists the complete policy and
request binding alongside the lower intent/disposition checkpoint; panic,
deadline, transport, and response-overflow uncertainty stay sticky. This does
not claim remote exactly-once delivery, DNS pinning, provider authentication,
or public-network support.

## Email envelope

`deliver_email` accepts one explicit `EmailRequest` and consumes the same
single-use capability before handing an `application/vnd.semaprax.email.v1+json`
request to the injected adapter. The body is a canonical, bounded envelope with
the sender, authored-order recipient and attachment vectors, optional Reply-To,
subject, and body bytes. Duplicate recipients and attachment names are refused;
the boundary never reorders caller-authored vectors because their order can be
meaningful to a provider. Raw body and attachment bytes use lowercase hex in
that envelope, so the host adapter receives an exact byte sequence without a
text-decoding ambiguity. They are omitted from request debug output and from
delivery evidence, which retains only the prepared-request digest.

Admission tightens the pure `std.email` policy boundary at the host seam:
sender, recipients, and Reply-To require an ASCII mailbox with exactly one `@`,
safe local atoms with no leading/trailing/consecutive dots, and dotted domain
labels of ASCII alphanumeric/hyphen bytes (1 through 63 bytes, no edge hyphen).
Recipients are 1 through 64. Subject is required, refuses every ASCII control,
and is at most 256 bytes. Bodies are at most 8 KiB; there may be at most four
attachments, each with a closed safe file name, a one-slash media type, and at
most 2 KiB of bytes. The JSON writer is capped by the deployment's existing
request limit as well, so escaping/encoding cannot turn admitted members into an
unbounded wire body.

`verify_email_envelope` is the authority-free replay side: it bounds input
before allocating member vectors, requires the closed schema/key/type shape,
lowercase-even hex, admitted mailbox and attachment members, and byte-exact
canonical JSON re-rendering. It then requires the decoded bytes to be the exact
body of a prepared email request with the boundary-owned content type and
delivery headers. Unknown, duplicate, reordered, malformed-hex, and
over-bound encodings refuse. This verifies one request representation only; it
does not recover a capability, prove provider receipt, or authorize a retry.

This is **not SMTP** and it neither resolves MX records nor manages an SMTP or
provider credential. A deployment selects a provider endpoint and trusted
adapter implementation; that adapter may apply deployment-owned authentication
outside the caller-controlled request. The request value and evidence never
grant that authority. Email content is intentionally delivered to the selected
provider adapter, so callers must apply the pure `std.email`/redaction policy to
their source data before constructing an email request; this Rust host boundary
does not infer secret classification from arbitrary bytes.

## Settlement and evidence

A response is `Accepted` only for 2xx. Other complete responses are `Rejected`.
A fixture or physical adapter may report `NotDispatched` only when it knows the
transport was never started. Deadline, response overflow, and every failure
after start are `Uncertain`, because the remote service may have acted before
the local observation failed. The boundary performs no automatic retry in any
case.

`DeliveryEvidence` binds the schema, deployment, invocation, policy, canonical
origin, complete prepared-request digest, delivery ID, idempotency key, and
settlement. Its bounded canonical decoder re-derives the origin and delivery
identities from the exact prepared request and checks the caller-retained
settlement. This is diagnostic evidence only: it is not a capability, delivery
receipt from the remote party, or permission to retry.

Operational exports encode sorted, duplicate-free labels and fields with fixed
cardinality and value limits. A protected field has no plaintext variant at the
boundary: it renders `[REDACTED]` plus an optional SHA-256 commitment. The
boundary also normalizes ASCII case and `-`/`_` spelling for the closed
credential-name inventory shared with `std.log.redact`; a protected name cannot
carry a public field or label value. Matching is exact, not substring-based, so
an ordinary name such as `password_hash` is not silently classified. This is a
bounded name policy, not content-based secret discovery. The
`export_after_primary` result keeps the primary application outcome separate
from export settlement, so exporter failure cannot replace it.

### Typed metrics and completed spans

`prepare_metric_export` adds a closed metric wire rather than asking a host to
interpret an arbitrary event. Each observation carries a stable metric ID, a
separate observation ID, sorted duplicate-free labels, and exactly one of
`counter_increment`, `gauge`, or `histogram_observation`. Counter increments
must be nonzero. Values are integers, avoiding non-canonical NaN and infinity
spellings. Label count uses the deployment policy, each label value uses the
fixed export-value bound, and the session ledger supplies an additional hard
per-process observation ceiling. Protected names are refused before adapter
entry; no global metric registry or unbounded cardinality set is allocated.

`prepare_span_export` carries one completed span with canonical lowercase
nonzero trace and span IDs, an optional distinct parent span ID, a bounded
name, elapsed microseconds, a closed `unset`/`ok`/`error` status, and sorted
duplicate-free attributes. Public protected-name attributes refuse, while an
explicit `ProtectedExportValue` can only become `[REDACTED]` and an optional
commitment. IDs are caller inputs: this layer deliberately does not read
ambient randomness or a clock. A trusted host that generates IDs must acquire
its own declared entropy capability before constructing a span.

`TraceContext` provides that explicit construction and propagation path. A
trusted host injects `TraceEntropyCapability`, whose successful fill contract
requires uniformly selected cryptographically secure random or pseudorandom
bytes; providers unable to guarantee that fail instead of setting the W3C
random-ID flag falsely. Fresh contexts request exactly one 16-byte trace ID
and one 8-byte span ID, while inbound and child contexts request one new
8-byte span ID. The boundary accepts only the fixed W3C
`traceparent` version `00` shape with lowercase, nonzero identifiers and only
the sampled and random-trace-id flags. It preserves admitted trace flags,
records the received span as the parent, and emits the new local span in the
outbound header. Entropy failure, zero output, identifier collision, unknown
versions, reserved flags, and malformed or extended inputs fail closed before
any export. `SpanExport::from_trace_context` binds the same typed context into
the completed-span wire; parsing an inbound header never grants outbound
authority.

`TraceState` is the corresponding closed carrier for one W3C `tracestate`
field. It admits at most 512 serialized bytes, 32 unique members, and 256
bytes per key or opaque value. Keys use the W3C lowercase identifier alphabet;
values are printable ASCII without `,` or `=`, and their opaque bytes remain
unchanged. W3C-permitted empty/OWS-only members are skipped; duplicate keys,
controls, invalid key/value shapes, and any over-bound input refuse direct
`TraceState` admission. Only optional whitespace surrounding members is
removed, yielding one canonical comma-delimited output field.
`TraceContext::from_headers` parses `traceparent` before attempting its
companion state, so an invalid parent discards `tracestate` without touching
the entropy capability, while invalid state is independently discarded and
does not invalidate a usable parent. This deliberately bounded v1 carrier
does not accept or reconstruct repeated raw HTTP fields.

`TracedHttpRequest` binds one `TraceContext` and optional admitted
`TraceState` to the ordinary bounded HTTP request path. It adds canonical
`traceparent` and, when supplied, canonical `tracestate` generated from typed
values after ordinary request validation. Both headers count toward the global
header limit and enter the prepared-request digest and therefore HTTP replay
identity. A caller cannot inject either through `HttpHeader`; raw repeated or
unvalidated vendor state has no path to an adapter. The trace context is
correlation data, not a permit: constructing it creates no socket, and
dispatch still requires the deployment-owned `OutboundCapability` plus an
injected adapter.

Metric observations and spans have separate domain-separated idempotency keys
and media types. A span's replay identity binds both its trace ID and its
trace-scoped span ID, while the provider-facing header retains the span ID.
Their `MetricExportSession` and `SpanExportSession` wrappers
reuse the same disposition-only ledger semantics: exact replay cannot enter an
adapter, payload drift conflicts, accepted response bytes are dropped, and
post-start failure remains uncertain. These are fixture-testable host
boundaries, not a hosted telemetry provider, durable exporter, global
aggregator, sampling engine, or claim of delivery.

Their authority-free `verify_metric_export` and `verify_span_export` paths
independently decode a closed v1 schema, enforce global bounds and canonical
ordering/JSON bytes, and bind the payload identity and media type to an exact
prepared request. Unknown, duplicate, reordered, malformed, over-bound, and
request-mismatched inputs refuse; successful verification cannot reconstruct a
capability or enter an adapter.

## Focused evidence

Once the module is wired from `src/lib.rs`, the focused selector is:

```sh
cargo test --locked -p semaprax --lib outbound_host_adapter::tests::
```

The corpus covers exact signing and replay binding, pre-dispatch refusal,
origin/userinfo/fragment checks, deadline/body maximum-plus-one, header and
identity injection, response maximum-plus-one, sticky uncertainty, redaction,
cardinality and duplicate-name refusal, and preservation of primary failure.
The email cases add canonical replay, header and address injection, recipient
and attachment cardinality, member maximum-plus-one, exact boundary admission,
noncanonical/unknown/duplicate/malformed-hex envelope hostility, and
after-start uncertainty. The integrated email session cases add a nonempty
accepted provider response that is intentionally not retained, exact replay
with zero adapter calls, changed-request and policy-commitment refusal before
dispatch, and an unwinding adapter attempt whose reserved uncertainty cannot be
retried. The reconciliation cases add bounded-capacity refusal,
duplicate exact replay without redispatch, changed-payload conflict refusal,
idempotency/header mismatch refusal, sticky uncertainty after an unwinding
dispatch closure, and deterministic commitment state. It performs no live
network operation.

The HTTP cases execute all five typed methods through the injected adapter,
cover endpoint/body/header/content-type maxima and credential-name refusal,
redacted debug surfaces, exact no-redispatch reconciliation, method-only
conflict, bounded response settlement, sticky panic uncertainty, and read-only
checkpoint verification. They exercise fixtures only, not a public endpoint.
The typed trace HTTP selector is `outbound_host_adapter::trace_http::tests::`.
It checks canonical typed `traceparent`/`tracestate` dispatch, raw reserved
header refusal, shared-header accounting, and changed context or vendor state
idempotency conflicts before redispatch. The trace-context selector covers
bounded hostile `tracestate` admission and paired-header ordering. These use
recording adapters only and do not claim a hosted collector or remote
propagation.

`outbound_host_adapter::native_tests::` independently exercises the repository
`NativeHttpsAdapter` against a private-root loopback TLS peer. It proves a
successful exact PUT request/body/target projection, a redirect returned as the
original 302 response, certificate rejection as a post-start transport uncertainty, the
declared-content-length response maximum, and a malformed prepared request
rejected before opening a socket. This is local physical loopback evidence
only: it does not prove DNS pinning, a system trust-store policy, public-PKI
interoperability, hosted execution, or remote delivery.

The same local selector also carries one `HttpDeliverySession` through a real
TLS POST: its host-owned store retains each exact committed typed checkpoint,
and restore receives only the final committed bytes, digest, and capacity. A
one-byte checkpoint mutation refuses before restoration. The exact restored
request replays through a real `NativeHttpsAdapter` while a replacement loopback
listener proves no second TCP connection occurred. A separately valid request
under a changed complete host policy, and a separately valid changed event body
under the same identity, are both refused before that listener can observe a
connection. This is a host-owned loopback fixture; it does not claim that the
task-service fixture configuration (which selects only fixture adapters and no
capabilities) authorizes a physical send.

The sibling `native_delivery_tests` corpus exercises the two remaining typed
delivery families over the same kind of private-root loopback TLS boundary.
One explicit host policy/capability sends a signed webhook and one
provider-neutral email envelope to their exact paths; the peer checks the
boundary-owned media type, delivery and idempotency headers, an independently
calculated exact webhook HMAC, and a body marker without observing a secret.
The email peer also rejects credential-style headers. Each typed store retains
the provisional and terminal checkpoints. A tampered stored wire refuses before
restoration; exact restored replay, changed payload, changed signing secret, and
changed complete policy produce no second TCP connection. An untrusted private
peer settles each already-started request as transport uncertainty, never an
automatic retry. These are host-owned local fixtures only: they do not prove a
provider accepted a delivery, receiver idempotency, SMTP/MX support, provider
authentication, public-network behavior, durable-storage support, or authority
for the credential-free service fixture.

The selector also proves the service host-request bridge without treating a
configuration declaration as authority. A canonical host-mode service
configuration renders the bounded request-v1 handoff; a separate closed
decoder retains its OTLP collector origin and exact four required capability
names. The test then gives the host—not the request—a fresh policy and
capability. A target absent from that policy, or insecure configuration drift,
refuses before adapter work; only the separately granted policy matching the
decoded private-root loopback origin reaches the fixed `/v1/metrics` route.

The sibling `native_telemetry_tests` corpus completes that local vertical for
every fixed collector signal: separate matching host grants carry the decoded
origin to `/v1/metrics`, `/v1/spans`, and `/v1/events`. The private TLS peer
checks each route, media type, typed identity header, and canonical body marker.
After those three accepted requests settle, exact typed replay returns from the
corresponding session after the listener has exited, proving it does not start a
fourth physical connection. This is still only private-root loopback evidence:
the request handoff is intent rather than authority, the credential-free
service fixture grants no physical adapter, and it is not a hosted collector,
provider-interoperability, durable-storage, public-network, or DNS-rebinding
claim.

This is local physical loopback evidence for the host boundary, not an
authorization for the credential-free task-service fixture or a hosted
collector claim.

The checkpoint selector is
`outbound_host_adapter::ledger::checkpoint::tests::`. Its seven cases cover
deterministic all-disposition round trips, the empty-state known-answer digest,
commitment/live-state drift, exact lookup, monotonic union/conflict refusal,
panic-reserved uncertainty, full-capacity admission and byte/inventory limits,
hostile schemas/digests/statuses/noncanonical wires, and exports/imports from
the adapter sessions without extra adapter calls.

The durable-ledger selector is
`outbound_host_adapter::ledger::durable::tests::`. Its four cases prove the
intent ACK precedes physical adapter entry, known-failed and ambiguous intent
commits do not dispatch, a lost terminal ACK remains sticky uncertainty, and
an exact host-authorized restart replays without constructing another physical
attempt. This injects an in-memory store fixture only; it is not filesystem,
database, hosted collector, or remote-delivery evidence.

The typed telemetry selectors are
`outbound_host_adapter::metrics::tests::` and
`outbound_host_adapter::spans::tests::`. They cover canonical label/attribute
ordering, all three metric kinds, exact no-dispatch replay, response-byte
discard, protected-name and malformed-context refusal, redaction, sticky
transport uncertainty, per-record maxima, and session maximum-plus-one. These
tests inject a recording adapter and perform no network I/O.
