# Glossary

Use this page when a new word interrupts a tutorial. Each definition describes
how the term is used in this handbook.

| Term | Meaning |
| --- | --- |
| ABI | The agreement about values, ownership, failure, and calling conventions across a compiled interface. |
| Agent | A program with explicit task, state, proposal, authorization, operation, and result roles. |
| Artifact | A produced file or package, such as a binary, report, or SDK. |
| Backend | The implementation that executes or lowers checked code, such as the interpreter, C11 route, or Core Wasm route. |
| Binding | A name attached to a value, as in `let count = 3;`. Some APIs also use “binding” for an exact checked association between inputs. |
| Borrow | Temporary access to a value without taking ownership of it. |
| Caller / callee | The function making a call / the function being called. |
| Candidate | A proposed program revision that can be inspected and checked before publication. |
| Canonical | Written in the one representation selected by a format's rules. |
| Capability | Explicit authority supplied for a particular operation. |
| Capsule | A package of revision-bound data for inspection, replay, or an evidence workflow. |
| Checkpoint | Retained execution state used by a selected recovery protocol. |
| Cleanup | Releasing owned values or resources in the checked order when their lifetimes end. |
| Contract | A function's requirements and promises, written with `requires` and `ensures`. |
| Copy scalar | A basic value, such as an integer or boolean, that can be copied without consuming its owner. |
| Declaration | A definition that introduces something named, such as a function, type, field, or law. |
| Diagnostic | A compiler message explaining a problem or warning, usually with a stable `SPX-...` code. |
| Digest | A hash identifying exact bytes. A digest is not, by itself, permission to perform an operation. |
| Effect | An operation category declared by a function, such as writing standard output. |
| Entry point | The function where application execution starts. |
| Evidence | Data produced or checked for a particular claim, subject, revision, and method. |
| Export | A selected declaration made available through a package interface. |
| Fixture | Fixed test input or a controlled implementation used to make a test reproducible. |
| HIR | The compiler's high-level intermediate representation after names and types have been resolved. |
| Host | The environment supplying runtime services, tools, storage, or external operation handlers. |
| Immutable | Not reassigned or changed through the binding in question. |
| Import | A declaration selected from another module or an explicitly provided host interface. |
| Journal | An ordered record of execution progress used by a recovery protocol. |
| Law | A named rule tracked independently of an implementation body. |
| LawSet | The selected collection of laws and evidence requirements to account for. |
| Manifest | The file describing project inputs, entry modules, tests, exports, and configuration. |
| Module | A named group of source declarations. A `.spx` file begins with its module declaration. |
| Move / consume | Transfer ownership so the old binding cannot be used as its former owner. |
| Owned value | A value with a tracked owner responsible for its transfer and cleanup. |
| Precondition | A requirement on a function's inputs, written with `requires`. |
| Postcondition | A promise about a function's result, written with `ensures`. |
| Profile | The particular type, ownership, execution, or packaging rules selected for a workflow. |
| Proposal | Typed input describing a requested action, before authorization. |
| Reducer | Checked logic that combines state and an outcome to choose the next agent step. |
| Replay | Rechecking retained data against the subject and rules that give it meaning. |
| Revision | The identity of a particular source or project snapshot. |
| Scalar | One basic value, such as a number or boolean. |
| Semantic graph | Structured information about declarations, types, effects, contracts, and relationships. |
| Stable ID | The persistent identity written with `@id`, separate from the display name. |
| Tail expression | The final expression that supplies a block's value. A loop body may require one even when its value is discarded. |
| Target | The selected execution or output form, such as native code or WebAssembly. |
| Typed hole | A marked incomplete part of a candidate with a known type/context for checked filling. |
| UTF-8 | The byte encoding used for text. A Unicode character can occupy more than one byte. |
| Variant | A type whose value is one of several named cases, each with its own fields. |

**Return to:** [Essentials](../language/essentials.md),
[project profiles](../projects/profiles.md), or [Agent programs](../agents/programs.md).
