# Image protocol conformance v1

Status: authored, unrun; no verified completion promotion.
Audience: compiler contributors and semantic agent client authors.

`ProjectSemanticImage::protocol_conformance` exposes canonical, source-backed
static protocol declarations and implementation bindings over one exact
admitted Project revision. `.spx` remains authoritative. This additive report
does not change Image v1 or the runtime Graph schema, whose declaration index
does not contain protocol or implementation nodes.

Each participating module binds its path, source digest and source revision to
the image digest, Project revision and existing semantic graph digest. The
compiler reparses retained canonical source and derives the bounded local
declaration tables through `static_protocol::declaration_facts`. Project
admission has already checked function bodies and linked calls; that local
producer is independently labeled as static signature evidence. Imported
synthetic stubs cannot satisfy a local implementation: original source is
checked before workspace synthesis, and the complete synthetic module is
checked again before linking.

The report is compact, recursively key-sorted JSON without a trailing LF,
bounded to 8 MiB. `verify_protocol_conformance` accepts only byte-for-byte
rederivation from the selected image. Invalid/oversized/stale requests use
existing Image diagnostics `SPX-G219`, `SPX-G220` and `SPX-G221`; source
conformance errors retain their `SPX-Q` diagnostics. An empty project inventory
is an explicit empty module list, not a claim about external protocols.
The 8 MiB ceiling applies to serialized output. Construction additionally uses
the existing 16-module Project bound and each producer's 4 MiB conservative
fact budget; it does not claim an 8 MiB peak allocation limit.

## Transport

The host-selected Image Diagnostic Protocol v4 adds two read-only methods:

- `protocol/conformance`: exact `image_revision`, optional retained
  `candidate_revision`, optional `offset` and `chunk_bytes`. Without a candidate
  it describes the held base image; with one it derives a checked image from
  that immutable candidate. The chunk envelope binds the session image and
  optional candidate; the full report binds the selected image.
- `candidate/interface-catalog`: exact image/candidate revisions, target local
  record stable ID, and optional chunk controls. It exposes the typed operation's
  required members and eligible existing implementation functions.

Chunk envelopes declare their complete report schema, total byte count, byte
offset and next offset. Offsets must be UTF-8 boundaries, and existing transport
chunk bounds apply. Reading has no registry mutation. Held source drift remains
absorbing. Both methods appear in v4 method/schema/client discovery with
`semantic_read` capability; v1–v3 method sets remain unchanged. V4's host-selected
diagnostics profile is still required; requests cannot enable that profile.

## Limits and evidence

Static conformance means exact checked member signatures and conservative
effect/precondition admission, as specified in
[Static Protocol Conformance v1](STATIC-PROTOCOL-CONFORMANCE-V1.md). It does not
provide dynamic dispatch, a runtime witness table, cross-module implementation
bindings, behavioral contract proof, target execution or publication authority.

`tests/image_protocol_conformance_v1.rs` authors exact replay, mutation/stale
rejection, source binding and empty inventory cases. The v4 transport regression
authors discovery, base/candidate chunk selection and legacy-profile exclusion.
No tests, compiler checks, interpreter runs or target executions were performed
for this change, at the user's request.
