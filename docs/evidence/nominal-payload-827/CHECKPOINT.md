# Nominal payload 827: private source checkpoint v1

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Basis: `4ec48420b6e78b1035a66cc406893c46c3028464`. This is an unfinished private source checkpoint, not an integration candidate or a completed optimization. No Cargo, build, test, runtime, or performance qualification has run on it.

The draft introduces an explicitly funded immutable outer nominal sequence, case-field coordinates retaining the complete outer cases owner, exact legacy Vec wire projections, and separate legacy constructors. The original pre-change files are retained in full under `original/`, bound by `source-inventory.json`. The complete old index constructor is independently compiled under cfg(test) in `nominal_original.rs`; the existing complete old resolver remains present with representation-only adapters.

The actual workspace paid owned and borrowed resolver entries forward their existing byte reservation owner into the new nominal constructor. No owner, cap, work credit, canonical cleanup reordering, or verifier exemption is introduced. Identity-only and public no-owner compatibility routes still retain legacy deep copies; final linker and decoded cache activation are residuals. No whole-product removal or fit claim is made.

## Required before integration

* Complete construction-local recursive field/case refusal rollback: the current source-shaped index scratch does not yet cover every temporary Vec exit. No allocation after selected refusal may be introduced.
* Restrict source-shaped scratch to construction; host-mutated published legacy payloads must not be dropped through an obsolete source frontier. Complete hostile-input cases must remain observable.
* Complete explicit paid mutation overflow handling and mechanical owned/mutable consumers. No hidden copy-on-write or unfunded make_mut is permitted.
* Audit every actual added map/header/control-block/carrier layout against the same owner, including simultaneous old/new storage during acquisition. Current draft is not a full physical payment proof.
* Add complete old/current index and actual receiver wire/order/diagnostic/first-refusal/iterative-Drop controls, and join every new child into all source-custody contracts. Existing payload controls are authored only, not executed.
* Integrate the separate 828 fixed base bundle only after its producer has an explicit bounded work contract. Existing byte callback does not grant declaration-work authority. The private 828 modules are not registered here.

The checkpoint intentionally remains unmerged while these conditions are unresolved.
