# Owned String Records v1

Status: additive internal implementation and focused gates authored. Verification
is pending until the remaining OPT implementation batch is complete. This is not
public ABI, hosted release or completed-ticket evidence.

The profile admits acyclic monomorphic records containing owned `string`,
owned `Bytes`, the eight direct Copy scalars, and further records with those
fields. At least one String leaf selects this profile. Independent source and
HIR walks enforce the cleanup bounds: depth 64, 256 owned leaves, and 4096
visited fields. Generic records, arrays, views, maps, callables, classes,
variants and resources do not become part of this profile.

Parameters require explicit `own` or `borrow`. Results own their leaves.
Constructors stage fields in source order. Owned calls stage arguments left to
right and transfer them together at the existing call commit boundary.
Immutable updates require an exact named owned base, stage replacement leaves
in order, transfer retained leaves, and settle superseded leaves through the
canonical partial-update plan. Whole String replacement is a separate profile.

Owned destructuring binds every owned terminal; nested records require exact
recursive patterns. A borrowed destructure never grants cleanup authority.
Ordinary String reads retain their existing clone semantics, including owning
reads through record projections. Double record moves, borrowed updates,
borrow escapes and owned wildcards reject at compile time.

String leaves use `core.string.drop` in the existing recursive lifecycle shape.
Construction and independent replay accept exact String terminals, preserving
canonical structural inventory and runtime cleanup order. Immutable updates
select CleanupPlan v9 or a validated later composition. Failure selection stays
sticky; postconditions and non-result cleanup precede result publication.

The physical private leaf carrier is eight bytes aligned to eight: a native
String pointer or an aggregate Wasm owned UTF-8 token. Native owned shells keep
those fields inert while canonical plan locals hold live owners. Borrowed
native signatures carry typed leaf aliases, including aliases through nested
record paths. The frozen public Bytes field size, classifiers, descriptors and
export profiles are unchanged. New String records gain no public export ABI.

The `language` harness module `owned_string_records_v1` owns canonical source
and graph round trips, exact projected String lifecycle assertions, default
parameter/double-move/update-base refusals, forged ownership and cleanup order,
repeated interpreter envelope replay, C11 O0/O2 allocation settlement through a
foreign/duplicate-free detecting allocator, and repeated aggregate Wasm calls
with a bounded owner arena. `aggregate_layout::tests` independently forges the
String field kind to Copy and Bytes and checks both target layouts. Existing
String-record signature refusal controls now use an unsupported array field,
so the stable SPX-T309 boundary remains exercised rather than removed.

The broader #599 request remains open until the authored gate runs and any
remaining requested record shapes are implemented. Map transport and expanded
Wasm Text Toolkit profiles have separate owners and are not implied here.
