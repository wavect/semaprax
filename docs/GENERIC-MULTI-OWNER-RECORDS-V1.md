# Generic Multi-Owner Records v1

Status: implemented with five focused source/HIR/graph checks and the
all-eight-scalar allocation-accounted runtime corpus passing locally. Hosted
promotion and public generic ABI remain separate.

An internal generic record function may accept multiple separately owned,
individually admitted record parameters and return an explicitly declared
admitted owning record. Existing parameter, substitution, aggregate-field,
owned-leaf and concrete-instance bounds apply. The first focused corpus uses
two distinct owners and all eight Copy marker substitutions; it does not add
an arbitrary two-parameter semantic limit.

The result may combine the owners into a nested record, reconstruct their
owning leaves into a different declared shape, or replace an owning field using
a separate owner. Every field and return type remains nominally exact. Equal
ownership layout is never a cast. Unused owners are cleaned according to the
ordinary function plan; no implicit duplication or sharing is introduced.

Call arguments evaluate and stage left to right. Staging one owner does not
commit other arguments. All owned arguments transfer at the existing declared
call commit boundary. Failure while evaluating a later argument cleans staged
values and remaining caller owners exactly once, retaining the first failure.
Postconditions and non-result cleanup precede result publication.

Source checks every admitted substitution independently, including unused
functions, and uses substituted field types to track moves. HIR authenticates
all symbolic signatures, materialized bodies and call argument ownership.
Cleanup inventory and plan are independently replayed without sorting or
repair. Repeating one owned value in two parameter positions rejects with
`SPX-O101`; this differs from repeated type arguments, which copy only type
information. Forged ownership modes, field paths, transfer sequences and result
types reject at their existing checked boundaries.

Implementation removes the single-owner count from the source composition
profile, HIR type/profile selection and exact materialized-function association.
Each owning parameter must individually satisfy the existing record classifier;
an unrelated String, resource, borrowed value or unsupported carrier cannot
ride along merely because another parameter is a valid record. Existing exact
substitution and full function-meaning checks remain mandatory.

Graph v34 already carries every concrete parameter's ownership and full
cleanup/call transfer facts; Graph v35 remains selected only by nonidentity
symbolic forwarding. Public signatures and Project/package export profiles
retain their separate admission rules.

Registered `tests/language/generic_multi_owner_next.rs` covers combining two
owners, replacing a nested owned field with cleanup v9, duplicate ownership
rejection, a three-owner case under the existing parameter bound, and refusal
of unrelated owning carriers. Positive cases cover all eight substitutions,
canonical source, exact graph replay and hostile HIR parameter ownership.
`generic_owned_function_runtime::multi_owner` passes reconstruction and nested
owner replacement, both success and failure while evaluating the second
argument after the first owner has staged. All eight substitutions execute on
interpreter, C11 O0/O2 and Core Wasm (11.66 seconds), with repeated calls,
allocation settlement and a direct-construction memory.copy comparison. These
are local execution results, not hosted or public ABI evidence.
