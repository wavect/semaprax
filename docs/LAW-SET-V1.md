# LawSet v1

Status: additive selected-policy interface; its executable gate is
`workspace project_assurance_manifest::law_set`. This document defines the
bounded LAW-01 profile, not a claim of universal proof coverage. Native source
declarations are defined by [Native Law Declarations v1](NATIVE-LAW-DECLARATIONS-V1.md).

`assurance_manifest::law_set` answers which explicitly named laws a retained
Project must preserve. The protected baseline is supplied independently by the
caller selecting `LawPolicy`. Candidate reports cannot select that baseline or
reduce its inventory. Law identity grants no authority and inventory is not proof.

## Selection and provenance

The library accepts explicit typed `LawModule` inputs. Native law sources lower
to that one representation; JSON and prose never form an alternate policy
input. A module records `module_id`, its owning Project `source_path`, declared
assumption IDs, and its law definitions. Exact normalized module bytes, their
digest, and the retained source digest are included in the inventory. Source
owners absent from the Project remain visibly missing. Removing an input module,
deleting its laws, or removing its source from Project sources cannot delete
protected rows.

A `LawDefinition` has a persistent `law_id`, closed typed `selector`, declared
`assumption_ids`, `requires_laws`, and an `evidence` requirement. Evidence uses
the existing assurance lattice: runtime guarded, compiler proved, model checked,
SMT proved, or theorem proved. There is no second assurance classification order.
Assumption IDs must resolve within the selected modules; their presence remains
open rather than manufacturing proof. Law dependencies must resolve and be acyclic.

The admitted selectors are:

| Kind | Typed subject and proposition | Evidence owner |
| --- | --- | --- |
| `contract` | Persistent function ID, precondition/postcondition, parsed scalar proposition | Existing Project assurance obligation |
| `scalar_relational` | Explicit typed scalar binders and a separately stated pure proposition | Open until an independently verified relational proof attachment exists |
| `forbid_reaches` | Claim ID and persistent `from`/`to` declaration IDs | Existing architecture evaluator |
| `protocol_realizers_bound` | Claim ID and persistent protocol ID | Existing architecture evaluator; realizer binding only |
| `model_property` | Closed `authorization`/`handle` reference model and exact registered invariant | Existing bounded model checker and model descriptor |
| `source_protocol_safety` | Retained protocol ID, checked pure dispatcher, exact public forwarding caller, success state, charge label and explicit finite bounds | Source-executed complete transition table and bounded checker; only closed exploration supplies `model_checked` |

Contract selectors accept scalar literals, variables, unary and binary operators.
They are parsed and canonically formatted, not executed as assertions. Calls,
projections, blocks, and other selectors are refused explicitly. A clause must
resolve uniquely within the named declaration. Reordering clauses remaps the
law to the existing positional obligation ID for the exact proposition. Duplicate
matching clauses are ambiguous and refused. Legacy obligation IDs are unchanged.
Function display names and formatting are not law identity inputs. Changing a
proposition, subject, requirement, assumptions, or dependencies changes the
versioned semantic digest and cannot silently retarget a protected law ID.
Scalar relational selectors have no function subject. Their binder names and
types are part of the proposition identity. The checker validates the typed
boolean expression and refuses calls, effects, undeclared variables, and
unsupported quantification. Merely naming an SMT or theorem evidence class
leaves the coverage row open; no proof filename supplies evidence.

Model selectors bind only registered invariants of the two existing reference
models. The checker actually explores the selected reference model under its
existing fixed bounds. A covered model law proves that reference-model property;
it does not assert the arbitrary Project implementation conforms to that model.
Evidence records state `reference_model_only` and bind the model digest. There
is no caller-authored transition system, assertion-string escape, solver launch,
or ambient authority.

`source_protocol_safety` is a separate Project-source method. Its law module
must name the protocol's retained source path. The admitted public caller
forwards its two scalar inputs exactly once to the pure dispatcher, and no
other checked function may call or reference that dispatcher. The checker
executes every state/event pair, compares declared and disabled transitions,
then explores the resulting finite system. Missing/unsupported realizers,
uncovered transitions, abstract traces and exhausted bounds remain visible
missing, unsupported or open law rows. A held `protocol_realizers_bound` claim
cannot satisfy this source method: `via` identity alone proves no ordering.

## Inventory, report, and policy

`LawSet::derive(revision, proof_profile, modules)` binds the complete inventory
to the retained Project revision, ProgramRoot, and caller-selected proof-profile
identity. `LawSet::replay` decodes independently and derives the exact expected
bytes against a separately held revision and profile. A stale ProgramRoot,
changed source inventory, or substituted profile is refused.

`LawPolicy::strict(baseline)` requires a nonempty protected inventory.
`LawPolicy::deliberate_empty(baseline)` requires an empty baseline. It cannot
replace a nonempty protected baseline. Applications that do not select this
additive API retain existing assurance behavior and exact report bytes.

`derive_report(revision, candidate, policy)` evaluates the union of protected
baseline and candidate law IDs. Protected identities must retain their semantic
digests. Every required law receives one row before any counts are computed:

- `present`: its selected evidence and required laws satisfy the policy;
- `missing`: definition, source module, declaration, or proposition is absent;
- `unsupported`: an admitted subject has no supported assurance view, or an
  architecture claim is unevaluable;
- `awaiting_evidence`: the requirement is unmet, a claim is violated, an
  assumption remains open, or a required law is not covered.

`required`, `covered`, `missing`, `unsupported`, and `open` counts come from
these same rows. `accepted` is true only when every required row is covered.
`require_satisfied` independently replays the complete report then refuses an
unaccepted result. `verify_report` checks report integrity without demanding
coverage, so missing and open inventories remain inspectable.

The report binds baseline/candidate inventory digests, implementation revision,
ProgramRoot, proof profile, exact law definitions and digests, source provenance,
legacy obligation associations, evidence, module dependency inventory, and the
independently derived Project assurance digest. Dependencies are settled in
bounded topological order. An empty candidate “all passed” document cannot
replace any of these independently derived facts.

### Bounded workflow projection

`law_set::workflow::summary` and `detail` rederive and replay the complete
selected report before projecting it. A summary page contains at most 64 stable
law IDs with their status, reason, and obligation ID. Every page repeats the
complete acceptance verdict and required/covered/missing/unsupported/open
counts; an empty final page cannot hide a failure. `detail` selects one exact
law row with provenance and the existing evidence record. Both use a caller
selected 256..65536 byte output bound and refuse rather than truncate.

The detail's repair target is implementation or proof. A proposed change to
the protected law, domain, or inventory still goes through
`ProtectedLawBaseline::review` and its separate specification approval route.
These views are read-only and do not grant source or publication authority.
The first profile replays reports derived without attached installed proof
tokens; proof-bearing reports need their own exact installed-tool replay before
they can use this projection.

For strict reports with opaque checked proof tokens,
`workflow::strict_summary` and `strict_detail` rederive the full strict report
from the retained Project, host-selected policy, and independently held proof
tokens. They retain a failed strict verdict and complete counts on every page;
tampered proof, policy, candidate, or report bytes refuse before detail output.
The projection does not launch a solver or turn an open proof into a verified
one.

## Canonical wire and capacities

The envelope schema is `semaprax.law-set.v1`, with exactly `payload`,
`payload_digest`, and `schema`. Inventory payloads have exactly `project_revision`,
`program_root`, `proof_profile`, `modules`, and `laws`. Report payloads additionally
identify `report_schema: semaprax.law-set-report.v1` and carry the report fields
above. Inventory and report are independently replayed using their distinct APIs.

Objects use recursively lexicographic keys, compact JSON, and one final LF.
Modules and laws are sorted by their IDs; assumption and dependency arrays are
sorted unique sets. Unknown kinds, selectors, fields, duplicate keys, duplicate
identities, noncanonical bytes, and digest mismatch are refusals. SHA-256 uses
the existing length-framed digest function with domains
`semaprax.law-set.payload.v1\0`, `semaprax.law-semantics.v1\0`, and
`semaprax.law-module.v1\0`. Semantic digests exclude persistent law ID and
provenance, which have independent identity and binding fields.

| Capacity | Maximum |
| --- | ---: |
| Canonical inventory, module collection, report, or input envelope | 1 MiB |
| Law modules | 256 |
| Laws across modules | 1,024 |
| Assumption or dependency references per law; assumptions per module | 64 |
| Identifier bytes | 512 |
| Contract selector bytes / AST nodes | 4,096 / 1,024 |

Ordinary Project and architecture/model checker capacities also apply. No bound
failure truncates inventory or turns an incomplete exploration green.

## Diagnostics and compatibility

- `SPX-LW101`: invalid/unknown/duplicate/noncanonical input, unresolved reference,
  unsupported selector, ambiguous proposition, or cyclic definition.
- `SPX-LW102`: law input/output or inventory capacity exhaustion.
- `SPX-LW104`: revision, ProgramRoot, profile, protected semantic identity, or
  independent replay mismatch.
- `SPX-LW105`: unexpectedly empty strict policy or nonempty deliberate-empty baseline.
- `SPX-LW106`: fully replayed inventory fails the selected coverage policy.

The existing Project and single-file assurance envelopes, obligation IDs, source
syntax, formatter, graph, native backend, and Wasm backend are unchanged. This
surface does not invoke a theorem prover, certify runtime conformance, mutate
source, grant publication permission, or select its own protected baseline.

The additive [Protected Law Intent v1](PROTECTED-LAW-INTENT-V1.md) profile binds
an independently held base revision and the conservative specification closure,
separates editable implementation bodies, and requires exact host approval for
unknown intent changes at configured managed publication boundaries.
