# Consuming owned-record Vec renewal v1

Status: locally qualified for the bounded v1 profile.
Audience: compiler ownership implementers and agents authoring consuming loops.

This profile composes the existing checked consuming iterator with an outer
mutable `Vec<Row>` whose element is an admitted monomorphic owned leaf record.
It does not raise collection, byte payload, allocation-event or fuel limits.
The owning [record iterator](OWNED-RECORD-ITERATOR-V3.md) and
[ordinary Vec renewal](VEC-LOOP-RENEWAL-V1.md) remain the underlying contracts.

## Admitted route

An ordinary `for own item in vec_into_iter<Row>(inputs)` may assign
`output = vec_push<Row>(output, item)` to an already live outer mutable Vec of
that exact record type. A consuming record match may instead transform an
admitted input row into a different independently admitted owned-leaf output
row and conditionally push it. Both current nominal declarations must satisfy
the owned-record family; equality of the input and output nominal IDs is not
required. An owned pattern-field move must refer to that current arm's exact
field ID, type, binding identity and ownership mode; no field permission escapes
the arm or permits an arbitrary Bytes place. The receiver and pushed result retain exact destination types and
ordinary grouped argument ownership. The ordinary argument route may compute a
fallible owned row, such as `vec_clone_at<Row>(later, 0usize)`. This does not
permit otherwise closed constructors or allocation in a source loop: every
argument retains its existing source/HIR/effect/ownership checks.

The first refusal-only resource fact covers `vec_clone_at<Row>(empty, index)` with a `usize` literal index
as that push's row operand. `empty` must be a direct immutable local initialized
before the loop by `vec_with_capacity<Row>(capacity)` with an ordinary admitted literal capacity,
of the same authenticated
owned-leaf record type. No alias, capture, mutation, replacement or escape is
accepted; only exact typed `vec_len` and literal-index `vec_clone_at` reads may use
it. Source and HIR independently inspect the complete current function and the
actual consuming renewal site. The fact is local and is not serialized as a
supplied empty-vector certificate. Copied call expressions carry no authority.
A resolved whole-place argument retains its local binding's `Own` mode; the
independently authenticated `vec_len`/`vec_clone_at` parameter supplies the
borrow. This representation does not transfer the empty carrier. The complete
reference census still excludes every consuming use, projection and alias.

The ordinary runtime clone still performs its length/index check and selects
`semaprax.vec.v1` code 2 before any payload copy; private Core-Wasm token 14
remains the same. Staging, failure selection, loans, cleanup and publication are
unchanged. Only unreachable clone payload work is excluded from the capacity
flow. Unknown/nonempty carriers, aliases, computed indices and allocating
helpers retain conservative charges and `SPX-T267` in loops. The cumulative
32-event and 2MiB caps are unchanged; there is no refund.

Independent classification recovers the actual initialized mutable Step slot,
exact Done/Yield cases and field identities, owned yielded row, remainder and
one `iter_next` latch from the generated iterator structure. The receiver is an
unprojected live outer owner; exact builtin declaration, type arguments, result
type and all argument ownership modes are checked. No generated display name,
attached cleanup schema, supplied graph fact or arbitrary While is authority.
Scalar, Copy-row, generic, primitive String/Bytes and non-push operations are
outside this additive classifier; their previous profiles remain unchanged.

## Reservation and failure

When the current declaration index contains no explicitly authored record, this
additive inventory is necessarily empty. Its collectors return that negative
result before reserving expression traversal scratch. Record presence grants no
site: the complete independent element, body and iterator proof still runs with
its existing reservation. The static forecast remains conservative; this absence
check does not establish that either original application fits its builder cap.

The existing cleanup-plan v15 protocol reserves the receiver's original history
position before evaluating the right side. Arguments stage left to right and
transfer only at the existing grouped call commit. Successful replacement is
published at the original position, preserving every unrelated live owner,
including an owner declared later. A failed argument or full-vector push settles
the staged receiver, current item, retained remainder and untouched owners in
canonical order, without replacing the selected status or publishing a result.
Cleanup vectors are neither sorted nor repaired downstream. No resource refund
or ownership grant is obtained from this profile.

Graph v79 is selective. Its ordered `consuming_owned_record_renewal` v1 inventory
records execution, condition, body, item, call site, receiver and exact element
type, and composes finite-scratch v78, static-text v77 and every prior selected
fact. Source/HIR/cleanup replay rederive the actual authority; graph evidence
cannot grant it. Frozen from-parts projections refuse the new route because they
lack the complete current declaration/source authority. Existing v15 cleanup
wire encoding and runtime/backend operations are reused unchanged.

All 17 owning checks passed. The original fixture retained the later cell,
round-tripped canonical source and verified its graph. Its failed clone
argument kept Core-Wasm token 14 and settled the staged owners. Interpreter,
native O0/O2 and strict Core-Wasm checks covered same-type and conditional
row transfers, exact index and capacity status, unpublished results,
exactly-once settlement and reentry. The HIR and retained graph owners
verified exact reservation, current record layout and origin, and refusal of
forged transitions, sites and schema. Agent and runtime cost effects remain
unmeasured.
