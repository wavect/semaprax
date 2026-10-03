# Host-selected Strict Law Policy v1

An explicitly trusted local host may install a strict-law selection for a
`semaprax.manifest.v2` Project whose `law_sources` contain a nonempty complete
native LawSet. Installation requires a quiescent Project root: the caller must
stop concurrent Project/Workspace operations and hold exclusive host filesystem
authority. The compiler neither grants that authority nor installs a policy
from a candidate source, report, CLI flag or proof reference.

The installer writes a current-owner 0700 `.semaprax-strict-law` directory,
persists the original authenticated Project revision in its immutable `revisions`
store, then creates a current-owner 0600, single-link `SELECTED` marker last.
The exact canonical marker binds the stored baseline entry, original Project
revision, baseline LawSet JSON and proof profile, exact method requirements,
editable implementation body IDs, protected-intent digest and policy digest.
An interrupted installation with a partial directory refuses Project admission;
the compiler does not silently treat it as unselected. Host administrators own
manual repair or removal. Deliberate host removal of the entire marker directory
creates a new unselected policy state; the compiler cannot enforce a deleted
policy against the host that controlled its removal.

Every Project snapshot loader probes the fixed sidecar. A selected root cannot
return `ProjectSnapshot` or `Arc<ProjectRevision>` through generic
`with_authenticated_project` or indexed/incremental equivalents. A separate
inspection facade exposes check and semantic graph/context without execution,
build, publication or retained-revision escape. The strict session reconstructs
current laws from authenticated native law sources, replays original baseline
and policy independently, and requires opaque Project/native proof bundles
against exact current revision before exposing protected operations. Held source
and marker objects are rechecked before and after the operation.

Generic candidate, Git, protected-law-only and Project publication routes fail
closed on a selected root. The strict publication route binds the immediate
Workspace base and approved candidate under the ordinary Workspace lock, then
replays the host selection, original protected baseline, current native law
inventory, exact proof bundles and specification approval. An opaque invocation
permit is consumed by the shared semantic Workspace commit path. Both ordinary
Workspace patch apply and semantic Change apply check selection again before
staging and immediately before `ACTIVE` replacement, so they cannot bypass the
selected policy by omitting the Project wrapper. Candidate-controlled source or
manifest bytes cannot install, alter or remove the host sidecar.

Managed semantic Workspace generations retain every selected native law source
as its exact authored bytes under `semaprax.native-law.v1`, with a source digest
and canonical byte revision. Workspace Graph resolves an internal empty module
carrier for the law path; it contributes no executable declaration or call.
The managed manifest and Project revision bind the actual law source, not that
internal carrier. Omitting or changing a law source therefore fails exact
Project/Workspace replay before strict publication.

The sidecar is a local host filesystem authority boundary. It does not protect
against a host administrator who changes permissions, rewrites or deletes the
sidecar, nor does it revoke immutable Project revisions retained before the
host quiescent installation. Source proofs do not establish backend lowering.
Legacy explicit strict library/candidate APIs remain available on unselected
Projects and do not imply global admission.
