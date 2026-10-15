# OPT824 error-output custody — bounded source review

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Reviewed private sealed6e265 successor, source SHA bindings in sibling JSON. No source modification, build, tests or allocation measurements.

## Exact obstruction
workspace_graph.rs5533–5538 selects sticky budget refusal and immediately returns Err(vec![limit_error(...)]) with owned Vec<Diagnostic>. bounded_output.rs reserve233–263 refuses every subsequent reservation once overflowed, including0. Diagnostic has nonzero sized String/Option/Cow/Span fields (diagnostic.rs27–36); a nonempty Vec necessarily requests nonzero backing. Thus remaining0 + exact nonempty owned-Vec error + complete same-payer prepayment cannot all hold. Converting an inline failure at the outer wrapper cannot resolve this: it just moves the same acquisition to an unpriced publication point. No reset, auxiliary quota, forecast credit or suppressed output is justified.

## Smallest genuine internal carrier route
An internal failure enum can hold One(Diagnostic) inline or Existing(Vec<Diagnostic>) for untouched external/multi-error failures. It must preserve every ordered diagnostic and all six public fields; no collapsing multiple errors, parsing messages or remapping the first refusal. Propagate through defaults::default_expr -> import_stub::default_expr -> borrowed_imports::{construct,construct_receiving} -> borrowed_leaf::ExpectedProgram::{construct,construct_receiving} -> resolved core. import_stub byte_helper_authorized currently promotes a single Diagnostic to Vec at line15; that promotion is another actual boundary. The historical full owning default and receiving APIs remain independently compiled controls.

There is currently no acquired reporter owner/slot before the first reserve_builder_structure(0). Public build_owned and resolved-core wrappers return Result<_,Vec<Diagnostic>> (workspace_graph.rs1035 and4114+) so the affine inline failure must either remain inline through a redesigned report boundary, or acquire a genuinely prepaid output slot before publication. No such prepayment may be manufactured after sticky refusal. Eager slot acquisition at constructor entry would change zero admission and first refusal sequence; moving it outside the watched receiver would hide cost. Preserve the current owned API until an explicitly owned/report-caller acquisition and contract are designed, and describe diagnostic output as the explicit residual.

## Independent bounded acquisition target
diagnostics.rs builder_bytes_help212–255 clones dominant(path:String), pending(path:String), and sequential(path:String) from TLS before any budgeted formatting. These clones occur even for a sticky refusal. Borrow these exact immutable strings only within thread-local Ref guards; the selected renderer uses only built-in str/usize/bool Display, without callbacks mutating those cells. Keep pending before sequential before dominant branch priority and every literal exactly. No new truth cache or payer. Original TLS setup still owns its strings and remains a separate residual; removal of reporting clones does not fund the error Vec. Await root ownership coordination with scope_checkpoint before writing diagnostics.rs.

## Required qualification
Compare complete original/current Diagnostic values and serialized fields for every live/precharge/pending/sequential resolving/sequential retained/dominant/empty branch, UTF8 and NUL path inputs. Observe actual old/current clones and complete output Drop. At0 and sticky refusal require byte-exact error and unchanged first refusal/used values; assert allocation-free only for the proposed internal inline/static portion, never whole public Vec output. Run the unchanged full original constructor and receiving overlay whole observers/caps. No performance or whole-funding claim follows from this source review.
