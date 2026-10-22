# Compact residual HIR construction v1

Audience: compiler contributors and reviewers.

Status: source implementation only; executable qualification pending. OPT #855.

`ResolvedResidualType` is a transparent owned Type box. Its raw public HIR field
is an untrusted model input, like public ResolvedExpr/ResolvedExprKind fields:
constructing, cloning, or editing it grants no source/loan/cleanup/backend
admission. `hir::validate` and ordinary independent source and cleanup replay
remain mandatory. Existing hostile language tests mutate the same Type through
DerefMut and still must be rejected. No language syntax, schema or runtime ABI
is added. This Rust model surface has no opacity-based authority contract.

All repository construction uses `hir::resolved_residual_type!`. Its zero-owned-
argument admission helper reserves a fixed actual layout before the macro
constructs any new Box/Type/Option/Self receiver. There is no `try_new(Type)`
shipping function or owned Type forwarding parameter. Fallible old producers
remain before this new frontier: resolve_function_type and cache Type decode
keep their existing local; monomorphize's substitute_type remains after every
earlier field and before the admission, using its old result payload as a
field-local. Producer failure selects its original diagnostic without trying
new admission. No original work or nested allocation debit is removed.

The bank contains actual Type box backing; two Option<Self> initializer/current
receiving carriers; two Self initializer/receiving carriers; Box::new's returned
header and its tuple-field receiving header separately; Box::new's real Type
parameter; the reservation length argument; the required-reservation bool and
the new zero-owned-argument helper's outward bool. Self aggregates give no credit
for inner Box headers. The old try_new Type parameter is absent and receives no
fee. The initial reservation's own cold failure remains the ordinary unreceived
caller boundary; this is not #825/full-constructor-ABI or Diagnostic funding.
Without an active builder the existing ordinary raw-HIR construction behavior
remains. Under an active builder every refusal is sticky and precedes the box.
No allocator/ceiling/refund/fake payer or retained permit is introduced.

The original Main7c complete opaque expression model, original constructor and
all nested controls remain independently compiled under
`hir::residual_constructor_before`. Its complete original cache implementation
is test-only too, so its old nominal wrapper is never routed through current
construction. The current original four residual controls retain every assertion
and use the actual macro; compound cloned Type producers keep their original
position. A fifth control checks producer versus new argument order, floor
refusal before allocation, and restored sticky/current nested-owner behavior.
The native interop depth fixture remains parser-independent public HIR input;
its data, observer windows and assertions are unchanged beyond the constructor
call. No frontend setup or invented physical saving is attributed to it.

Current cache encoding still writes the Type wire directly and creates no new
wire node/depth; decoding still charges real Type backing and runs the original
Type decoder before live construction. Native/Wasm/interpreter dereferences,
capacity walkers and all existing backend/hostile/cache assertions are unchanged
from Main7c. The representation's allocation/gain qualification is separate;
this correction claims only the explicit receiving frontier. No current PASS or
whole-head support follows from source checks.
