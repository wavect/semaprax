# Native callable settlement proof v1

Status: private, authority-free compiler/host proof format. This format binds
one exact callable-v2 descriptor to one compiler-derived settlement graph. It
does not define callable ABI v3, a provider export, a settlement entry point,
loader admission, resource authority, or physical cleanup.

## Purpose and trust boundary

`SPXNPRF1` lets the compiler serialize its current settlement proof and lets the
unpublished native host parse the bytes independently. Successful parsing proves
that the bytes are canonical, internally consistent, bounded, and paired with
the exact embedded callable-v2 contract. It does not authenticate code origin,
prove that a finalizer ran, or grant permission to load code or touch a resource.

The complete proof is one immutable byte string of at most 65,536 bytes. It
embeds the exact, unchanged `SPXNABI2` descriptor and the binary graph, so an
admitter never needs to reread a companion file. All integers are unsigned
little-endian `u32`. Fingerprints are 32 raw SHA-256 bytes. Text is a `u32`
byte length followed by nonempty, NUL-free UTF-8. No pointer, `usize`, native
`bool`, C layout, alignment, secret, capability, module instance, owner slot,
generation, callback, or finalizer handle appears on the wire.

## Proof envelope

The byte order is exact:

| Field | Encoding |
| --- | --- |
| Magic | eight bytes `SPXNPRF1` |
| Version | `u32 = 1` |
| Header size | `u32 = 20` |
| Total size | `u32`, exactly the complete proof length |
| Schema fingerprint | 32 bytes |
| Embedded-v2 fingerprint | 32 bytes |
| Settlement-graph fingerprint | 32 bytes |
| Envelope fingerprint | 32 bytes |
| Embedded-v2 length | `u32`, nonzero |
| Embedded-v2 bytes | exact canonical `SPXNABI2` descriptor |
| Settlement-graph length | `u32`, nonzero |
| Settlement-graph bytes | canonical graph encoding below |

There are no reserved or trailing bytes. The existing independent callable-v2
decoder validates the embedded descriptor, including its target and complete
call contract. A v1 or v2 loader must reject `SPXNPRF1` before opening a native
image; no loader accepts this proof format.

## Settlement graph

The graph byte order is:

1. graph version, `u32 = 1`;
2. function identity as framed text;
3. recovery-contract fingerprint, 32 bytes;
4. source callable-v2 call-contract fingerprint, 32 bytes;
5. trace-path-certificate fingerprint, 32 bytes;
6. resource count, `u32`;
7. checkpoint count, `u32`, followed by that many checkpoints;
8. start-checkpoint count and checkpoint IDs; and
9. progress-edge count followed by that many edges.

Each checkpoint contains its dense one-based ID, resource-state count, one
`u32` state tag per dense zero-based owner, outcome, abort cleanup order, and
accept cleanup order. State tags are `1 = live`, `2 = provisional result`,
`3 = finalizing`, `4 = dead`, and `5 = published`. `finalizing` and `published`
are terminal receipt states and are rejected in proof checkpoints.

Outcome tags are `0 = none`, `1 = scalar success`, `2 = semantic failure`, and
`3 = owned success`; owned success is followed by its owner ordinal. Each
cleanup order is a `u32` count followed by owner ordinals.

Each edge contains `from`, `to`, and an action tag. Action `1 = finalize` and
`2 = stage owned result` are followed by one owner ordinal. Action
`3 = certify outcome` is followed by one 32-byte nonzero trace-evidence value.

## Fingerprints

The four domains are distinct:

```text
semaprax.native-callable-settlement-proof-schema.v1\0
semaprax.native-callable-settlement-proof-v2-bytes.v1\0
semaprax.native-callable-settlement-proof-graph.v1\0
semaprax.native-callable-settlement-proof-envelope.v1\0
```

Payload hashes prepend the domain and then length-frame the payload with an
eight-byte big-endian length. The schema payload is the exact implementation-
pinned schema statement. The envelope fingerprint length-frames, in order, the
schema fingerprint, embedded-v2 fingerprint, graph fingerprint, embedded-v2
length encoded as four little-endian bytes, and graph length encoded likewise.
The envelope does not hash itself. The graph independently carries the exact
source v2 call-contract and trace-path-certificate fingerprints, and the host
requires both to equal the embedded v2 descriptor. This hash DAG is acyclic.

These public hashes prove deterministic binding and corruption detection, not
authenticity, authorization, code provenance, signature validity, or collision
impossibility.

## Independent validation

The host rejects before returning proof data unless all of these hold:

- envelope magic, version, sizes, fingerprints, embedded lengths, and total
  65,536-byte ceiling are exact;
- the v2 descriptor passes its existing strict independent parser for the
  current target;
- graph text, tags, counts, conversions, byte ranges, and canonical re-encoding
  are exact and bounded;
- resources are nonzero and at most 4,096, checkpoints are nonzero and at most
  65,536, and checked resource/checkpoint/start/edge work is at most 1,000,000;
- checkpoint IDs are dense, cleanup vectors contain exactly the required owners,
  at most one owner is provisional, and outcome/result shapes agree;
- there is exactly one all-live, nonterminal start checkpoint with ID 1;
- progress edges move strictly forward, are unique by edge and source/action,
  are reachable in encoded order, preserve cleanup-order continuity, and leave
  no nonterminal checkpoint without an outgoing edge; and
- function, owned-resource count, result shape, v2 call contract, and trace
  certificate agree across the two embedded artifacts.

The compiler enforces the same global byte ceiling while writing the graph; it
does not first allocate an unbounded graph and reject it afterward.

## Compatibility and nonclaims

`SPXNPRF1` is additive and incompatible with `SPXNABI1` and `SPXNABI2`. There is
no negotiation, downgrade, or fallback. Callable-v2 descriptor bytes, provider
bytes, symbols, public seven-file bundles, CLI behavior, and loader behavior are
unchanged. The proof surface exists only for tests and the unpublished host
feature; default external consumers cannot import it.

This format is not callable ABI v3 and reserves no v3 magic or version. It does
not define execute, settle, frame, action, or receipt wire schemas; authenticate
the separately stored trace-path certificate; create exact-instance authority;
load a library; call native code; prove quiescence; recover from failure inside a
finalizer; or open `SPX-B104`. Android, iOS, imported finalizers, callbacks,
async, concurrency, fork recovery, hot reload, code signing, and malicious-code
containment remain outside this milestone.
