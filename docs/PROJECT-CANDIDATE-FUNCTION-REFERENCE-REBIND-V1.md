# Candidate Function Reference Rebind v1

Status: Partial, authored and unrun. This contract adds one authority-free candidate
navigation operation. It does not promote cross-revision references generally.

Audience: agent authors, compiler contributors, and embedding hosts.

## Purpose

An exact-image function reference intentionally becomes stale after a semantic
candidate changes the Project. Agents still need a conservative way to ask
whether the same explicit stable function identity survives in that candidate
without falling back to names, spans, paths, or unchecked graph data.

`ProjectCandidate::rebind_function_reference` accepts the exact candidate
digest and one canonical reference exported from that candidate's exact base
image. It independently derives the base and final images, resolves the source
reference normally, and delegates selection to the existing conservative
cross-revision image rebind. The candidate layer does not duplicate or weaken
the image selector. The candidate digest selects an already admitted immutable
candidate; this operation does not claim to replay externally supplied
candidate bytes.

## Result

The bounded `semaprax.project-candidate-function-reference-rebind.v1` report
binds:

- the exact candidate, base/final Project and workspace revisions;
- the independently derived base/final image revisions;
- the complete existing image rebind report;
- the requirement that any accepted destination reference pass ordinary exact
  destination-image resolution.

The nested report either rejects with its closed stage/reason or returns a
fresh exact destination reference for the same unique explicit stable function
identity. It distinguishes unchanged, changed, and moved source provenance.
Stable identity survival does not prove unchanged signature, contracts, body,
behavior, compatibility, ancestry, external-consumer migration, or approval.

The operation mutates neither candidate nor source, retains no image, invokes
no execution or filesystem path, and grants no source, retention, approval, or
publication authority. An empty candidate produces the ordinary identical-image
rejection rather than pretending that a cross-revision transition occurred.

## Bounds and diagnostics

The wrapper is bounded to the existing 256 KiB image rebind report plus 128 KiB
of closed candidate provenance. Reference input retains the existing 16 KiB
image-reference bound. Existing `SPX-G363`/`SPX-G364` diagnostics own malformed,
stale, tampered, or oversized exact references and image reports. `SPX-G490`
owns an invalid compiler-produced wrapper input and `SPX-G491` owns wrapper
capacity. Failures return no partial reference.

## Remaining work

There is no v5/generated-client/MCP route yet, no arbitrary source/destination
image pair in one protocol session, no automatic handle migration, and no
semantic or external compatibility proof. Those remain separate work.
