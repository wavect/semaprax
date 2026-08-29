# Project Profile Admission v1

Status: authored but unrun; authority-neutral internal Project boundary.

Audience: compiler contributors, Project profile authors, and promotion
reviewers.

Project Profile Admission v1 is the sole exhaustive Phase-A dispatcher from an
exact parsed Project manifest and its already linked entry HIR into one closed
schema-selected target profile. It prevents a profile from being parsed and
implemented by individual generators while remaining absent from ordinary
Project construction.

## Authority boundary

`src/project/admission/` receives only the exact manifest, retained resolved
entry program, and Project/workspace/graph subject facts constructed by the
ordinary Project builder. It has no path, handle, filesystem, process,
publication, transport, persistence, or target-execution authority.

A successful prepared admission is invocation-owned compiler state, not a
proof capsule, receipt, capability, or reusable authority. It cannot bypass
HIR validation or authorize a later effect. Public descriptor consumers must
still independently replay descriptor bytes against the retained HIR.

## Closed dispatcher

The dispatcher is exhaustive over `ProjectProfile`:

- v1 scalar, v2 borrowed text, v3 byte data, and v4-v7 command profiles invoke
  their unchanged existing target admission paths;
- v8 owned data and v10 owned UTF-8 derive and independently replay their
  existing canonical descriptor before invoking the unchanged descriptor-bound
  owned-data Wasm admission path; and
- v9 flat owned records derive and independently replay their distinct
  canonical descriptor before invoking the existing flat-record Wasm admission
  path.

The prepared state retains the v8/v9/v10 descriptor as an authenticated
Phase-A fact. It exposes no public constructor or mutable field. Project v9 is
therefore admitted through the same ordinary Project and Revision Store paths
as every other schema without changing its descriptor, npm carrier, native
provider, or generated package.

## Compatibility

Project v1-v8 manifest bytes, target bytes, carriers, and publication behavior
are required to remain unchanged; the preservation gates are authored but
unrun. Project v10 keeps its existing descriptor and target behavior. The
additive behavior is that a semantically valid v9 subject reaches the existing
descriptor-driven target route instead of the former unconditional `SPX-W115`
placeholder rejection. The closed execution-envelope verifier's invalid-schema
diagnostic now names v1 through v10 because those are its exact admitted
schemas; no earlier successful envelope bytes change.

The Project execution v1 envelope schema is unchanged. Its independent verifier
admits the exact v9 and v10 Project schema strings already rendered by retained
Project execution; no other schema is accepted. Pathless Web-build rejection
diagnostics now select v9 and v10 explicitly rather than relying on a v8-only
fallback assertion.

## Evidence and non-claims

Focused evidence covers ordinary v9 Project construction, descriptor-bound npm
preparation, v9/v10 execution replay, exact pathless-build diagnostics, and
Revision Store round trips across v1-v10. Existing profile and protocol known
answers remain the preservation gate.

The implementation and evidence are authored but unrun. This contract does not
promote Project v8, v9, or v10; publish an npm or Rust package; widen Agent
Transport; add a public aggregate ABI; execute a target; or turn prepared state
into persistent cache or evidence authority.
