# Source Agent owned wait v1 — combined SourceLive v8 contract

Status: **reviewed bounded design; implementation in progress; no runtime or R20 completion claim**.
Audience: compiler, interpreter, journal, SDK and typed-runtime implementers and reviewers.

The [embedded source Agent syntax](LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md#additive-embedded-execution-metadata-v070-candidate) owns the frontend association. Existing owned-frame v1, Copy model-wait v1/source journal v7, public ordinary event enums and all predecessor bytes remain frozen. Sections 8–14 are normative refinements and supersede conflicting shorthand in earlier sections.

## 1. First executable slice and exact checked association

Implement one initialized State -> real observe -> external model -> checked Proposal -> real authorize transition, ending in an opaque retained next-stage owner. The public route accepts one additively admitted v2 State owner; it does not claim source initialize/reduce/Step/multiple turns. The following required packet implements those remaining stages before #296 can close. No identity-only Agent-loop claim.

Frontend association is exactly `(Agent ID, existing propose-operation ID, distinct same-module ordinary helper ID)`. Runtime independently checks the retained HIR association, source revision, exact State/Observation/Proposal declarations, operation roles, helper signature/body and compiler cleanup vectors. No caller strings construct this proof.

Helper shape:
```spx
fn park_proposal(state: own State, observation: Observation) -> State
    yields Observation -> Proposal
{ let proposal = yield observation; state }
```
Each declaration keeps its authored persistent ID. The helper has exactly two parameters: own flat State and Copy Observation, in that order; exactly one direct top-level identity yield of that second parameter, no calls/transformation/mutation/extra owned local, and whole first-parameter identity result. Contracts use only currently admitted borrowed Copy scalar reads. State remains the existing v1 flat owned profile, including all eight Copy scalar kinds and bounded Bytes leaves. Request/answer admit scalar or flat nominal Copy records of 1..8 scalar fields, no nested aggregate/variant/Bytes/borrowed carrier. Observation and Proposal must be flat records for this Agent bridge. This is a separately checked `source-owned-frame.v2` profile, not widening v1 predicates.

Observe executes ONCE before helper start, under the existing ordinary Observe StageReservation and its current stage-count/fuel limits. The helper receives the already produced checked Observation; it contains no observer call. Historical reconstruction performs actual observe only under the existing ReplayStageReservation, compares its exact result, and does not add an uncharged observer. A live DecodedProposal is projected by the schema owner once to declaration-ordered Copy fields; model bytes are not reparsed by the continuation owner. Recovery uses the retained authoritative raw settlement with the same existing compiler decoder, never a host-substituted carrier.

## 2. Copy-channel v2 immutable wire facts

Define C as compact recursively key-sorted canonical JSON, arrays preserved, no LF, duplicate/unknown keys refused, numbers only integer carrier facts. All digests below are `sha256:` + lower64, SHA256(domain including one NUL || exact payload). Floating carriers use existing fixed integer bit encoding, not JSON floating numbers. Channel values and nominal field order use the existing checked `channel_arguments_v1` codec/shape validator without changing its bytes.

V2 signature has exactly `profile,function,parameters,result,request,response,yield_count`. Profile is `source-owned-frame.v2`; parameters is the declaration-order array of `{id,mode,type}` (`own` State then `copy` Observation); result/request/response are exact checked type keys, yield_count=1. V1 signature is untouched.

V2 Created additional fields are exactly `scope,plan_digest,cleanup_plan_digest,signature,argument,argument_digest,copy_arguments,copy_arguments_digest,max_steps,max_total_steps`. `copy_arguments` is the declaration-order array of `{parameter,value}` (exactly one Observation parameter). The owned `argument` preserves the existing v1 structural input encoding, but hashes in the new domain. Created common row fields follow the v2 journal schema. No extra host-declared nominal facts.

V2 frame is exactly `{owned_root,copy_arguments}`: owned_root is the unchanged compiler-owned v1 structural frame representation, copy_arguments is the above array. Checkpoint payload is exactly `{schema,scope,plan_digest,cleanup_plan_digest,signature,argument_digest,copy_arguments_digest,frame,frame_digest,request,request_digest,reserved_total,consumed_total,sequence}`. Schema is `semaprax.source-owned-frame-checkpoint.v2`. The envelope authentication is the existing key helper under the distinct v2 authentication domain; structural decoding executes no interpreter. Saved request must equal the checked second parameter.

Completed additional fields extend the existing completion facts with `answer,answer_digest`: the exact already checked Copy Proposal sidecar. The State result remains unchanged. Cleanup/result transfer consumes the same owner and this inert sidecar together. Sidecar alone never grants owner restoration, authorization or dispatch.

| Domain (literal trailing NUL) | Exact payload after domain |
|---|---|
| `semaprax.source-owned-frame-plan.v2` | C({source_revision,agent,model_operation,helper,signature,checked_graph,cleanup_plan_digest}); checked_graph is the exact complete selected checked graph object, not a source-text subset |
| `semaprax.source-owned-frame-cleanup-plan.v2` | exact existing canonical compiler cleanup-plan JSON bytes, no reordering |
| `semaprax.source-owned-frame-args.v2` | C(argument) |
| `semaprax.source-owned-frame-copy-args.v2` | C(copy_arguments) |
| `semaprax.source-owned-frame-frame.v2` | C(frame) |
| `semaprax.source-owned-frame-checkpoint.v2` | C(checkpoint payload above) |
| `semaprax.source-owned-frame-request.v2` | C({scope,plan_digest,value:request}) |
| `semaprax.source-owned-frame-answer.v2` | C({scope,plan_digest,value:answer}) |
| `semaprax.source-owned-frame-result.v2` | C({argument_digest,answer_digest}) (whole identity State + Proposal sidecar) |
| `semaprax.source-owned-frame-cleanup.v2` | C(the existing receipt object) |
| `semaprax.source-owned-frame-checkpoint-authentication.v2` | exact canonical checkpoint payload bytes, HMAC-SHA256 with existing key helper |

These are new recipes. Do NOT reinterpret the implemented frozen v1 plan hash. V2 checkpoint <=65536 decoded bytes, each channel <=32768 bytes, full frame/Bytes bounds unchanged. Decode rejects excessive depth >24 and length/shape before owner allocation. Host wrong leaf/field count/order/nominal/RecordBytes answer fails before answer append, retains root, zero ordinary effect.

## 3. One authoritative Source Live v8 store and identity

The opt-in profile is `semaprax.live-invocation.source-persisted-journal.v8`. It is a new append-record profile, not a companion owned-frame journal nor a relabelled v7 checkpoint document. Existing ordinary entry BODY encodings and public SourceJournalEntry projection stay unchanged; v8 combines ordinary bodies and owned bodies at one true causal sequence. Every row is C(JSON) + LF with common keys `schema,invocation,generation,seq,prev_mac,kind,authentication`; body keys must exactly match the selected closed kind. A single same-key HMAC chain authenticates all rows. MAC is lower64. First prev_mac is 64 zeroes. Unknown kind/key/duplicate/noncanonical/reference/reordered phase is rejected before restoration.

Use the existing pinned Unix registered FD implementation: independently granted exact scope, held directory/file dev+inode, creator PID, same-FD bounded read/append/sync, private mode/euid/nlink, no-follow relative creation/recovery, exclusive flock. The additive SourceLive sink delegates each event/preflight to that held lease. Generic pathname CheckpointStore is explicitly unsupported for v8. It does not overwrite/truncate or commit a second checkpoint. Child PID refuses before I/O/evaluation/host/release/result and after callbacks; inherited Drop is close-only. Live owner backing drops before lease unlock. Caller supplies current protected history and store registration; no global uniqueness, rollback-proof, copied-store or HMAC-as-authority claim.

Expected scope is `{program_root:S,invocation_id:I,policy_epoch:epoch}`; S is exact standalone source revision, epoch independent expected caller policy epoch. Initial SourceLive execution E retains all existing model/deployment/instance/task/typed registry/effect ceilings/budget facts. Binding B IS the v2 plan digest; its pinned payload already contains the exact HIR Agent/propose/helper association. Define:

| Domain (trailing NUL) | C payload |
|---|---|
| `semaprax.live-invocation.source-id.v8` | {execution:E,owned_wait_binding:B} |
| `semaprax.source-agent-owned-wait.attempt.v1` | {invocation:I,turn:T,attempt:N,binding:B} |
| `semaprax.source-agent-owned-wait.generation.v1` | {scope,execution:E,binding:B,store_identity,limits}; limits={max_steps_per_stage,max_total_steps,max_stages,max_attempts,response_limit,journal_bytes:16777216,journal_rows:65536}; store_identity is four existing dev/inode u64 facts; nonrecursive, excludes generation/MAC/seq |
| `semaprax.source-agent-owned-wait.journal-name.v1` | invocation UTF-8 bytes rather than C; lower64 + `.source-owned-wait.jsonl` |
| `semaprax.live-invocation.source-record.v8` | C(row without authentication), HMAC with existing supplied key |
| `semaprax.source-agent-owned-wait.transfer.v1` | {scope,generation,turn:T,attempt:N,wait:Q,from:helperID,to:authorizeID,state_digest,proposal_digest} |
| `semaprax.source-agent-owned-wait.evidence.v1` | {schema:"semaprax.source-agent-owned-wait-evidence.v1",terminal_evidence_digest:Eterminal,ordinary_model_evidence_digest:M,binding:B,invocation:I,generation,waits,transfers,reserved_total,consumed_total,journal_sequence,journal_mac} |

The generation recipe uses the exact expected registration scope above. The `OwnedRunCreated.scope` wire projection keeps its existing closed keys `{program_root,invocation,policy_epoch}`. To derive generation from Created, require that exact closed shape and project its `invocation` value to the registration key `invocation_id`, preserving the other two values; then hash the unchanged generation payload and domain. This yields the same single generation as the independently retained physical registration. A digest computed directly from the unprojected wire scope is refused; no alternate generation recipe is admitted. The transfer recipe uses the exact Created wire scope and that single registration generation. The wire scope keys, row body keys, physical registration recipe and v1 store recipes remain unchanged. Computing either recipe supplies no registration, owner or append authority.

Evidence waits/transfers are ordered by true causal row sequence; each carries its exact original/completion/replay refs/digests/charges, not independently sorted IDs. Eterminal is the validated ordinary terminal evidence for this actual profile, M the privately computed typed durable model evidence of this same run. Evidence is data only. Before an ordinary terminal exists the bounded first-turn API exposes pending status/charged inventory only and refuses a terminal evidence root; it does not fabricate Eterminal or M. No arbitrary host digest input. Decode/encode structural checks are not evaluator work. Current CapabilityPolicy only supplies allows(operationID): additionally check independent expected scope/epoch/generation/turn/attempt/phase/operation against live registered lease. Existing ModelHostGrant, model request/binding, reservation units, adapter checkpoint eligibility, cancellation/deadline, refusal classes remain the sole model account/dispatch authority. No new budget ledger or ambient policy authority. First profile only ordinary unpriced new_bound_checkpointed adapter; io/policy/priced/ProgramRoot profiles refuse prewrite.

## 4. Closed v8 new bodies and actual causal grammar

Owned bodies below begin common row kind; T,N,Q references must match the existing current turn/attempt. Existing ordinary bodies remain identical. O is the original causal sequence, R a reservation sequence, F bound evaluation fuel, H lowercase hex checkpoint bytes, D full digest, K inert checked carrier, X immutable snapshot, V compiler-derived receipt. Each reservation has one closure; repeated replay references original phase only, every reservation spent in full.

```text
owned_state_committed {turn,state,argument_digest}                 // state=X
owned_wait_reserved {turn,attempt,wait,phase,replay_of,fuel}       // start|resume
owned_wait_prepared {turn,attempt,wait,reservation,observation_digest,checkpoint_digest,checkpoint,consumed}
owned_wait_completed {turn,attempt,wait,reservation,proposal,proposal_digest,result_digest,consumed}
owned_wait_failed {turn,attempt,wait,reservation,status,consumed}
owned_wait_replay_checked {turn,attempt,wait,reservation,original,result_digest,consumed}
owned_wait_cleanup_started {turn,attempt,wait,terminal,operations}
owned_wait_cleanup_settled {turn,attempt,wait,receipt}
owned_state_transfer_reserved {turn,attempt,wait,from,to,state_digest,proposal_digest,transfer_digest}
owned_state_transfer_completed {turn,attempt,wait,reservation,state,state_digest,proposal,proposal_digest,transfer_digest}
```

Consumed is ACK-observed steps, bounded by its reservation; all reserved F is spent, interrupted work is never inferred. Failed reservation is the current unclosed evaluator reservation, or null for abandonment/cancellation after an already closed phase; null requires consumed=0 and cannot mint evaluator credit. Commit/transfer state encoding is the bounded canonical structural X, never public interpreter Value. TransferCompleted cannot substitute State/Proposal facts from Reserved. Cleanup receipt kind is existing observed|host_confirmed. Host confirmation only scoped failed CleanupStarted, never successful State transfer and never a new HostConfirmed row.

Fresh order:
```text
RunOpened -> owned_state_committed -> ordinary Observe StageReservation/evaluation
-> TurnObserved -> owned_wait_reserved(start) -> owned_wait_prepared
-> ordinary AttemptIntent -> (AttemptSettled/AttemptUsage | AttemptFailed/AttemptUsage)
valid decode: owned_wait_reserved(resume) -> owned_wait_completed
-> owned_wait_cleanup_started(empty nonresult vector) -> owned_wait_cleanup_settled
-> ProposalAdmitted -> owned_state_transfer_reserved -> owned_state_transfer_completed
-> ordinary Authorize StageReservation -> actual checked authorize
-> existing AuthorizationConsumed or AuthorizationRefused
```

Observe uses a scoped borrow of this SAME root. Wait cleanup is non-result only; successful identity result owns all State leaves so no semantic leaf destruction occurs there. Physical finalization on failure consumes actual compiler vector once, callback after actual drop with per-operation panic catch/continued releases; guard failures poison/CleanupInDoubt. A no-op observer cannot fabricate release. ProposalRefused before resume preserves State owner for a bounded next attempt under the same observed turn, ordinary retry rules/charged fresh wait; SDK-decode ModelFailed remains early terminal, no ProposalRefused invented. Cancel/pure eval/shape failure selects ordinary Stop/refusal sticky status, not ModelFailed; preserves reservations and performs acknowledged failure cleanup only when no uncertain dispatch/cleanup forbids it.

All evaluator start/resume/historical phases require their own ACKed reservation under existing total fuel. Wait work does not increment Agent stage count. Ordinary observe/authorize replay remains separately charged and counted by existing rules. Before each candidate, preflight candidate plus ONLY decreasing outstanding maximum closure: checkpoint, raw settlement/usage, resume, cleanup receipt, proposal decision, transfer pair, authorize decision and contracted terminal room as still outstanding. Existing 16MiB/65536 caps apply. Keep fresh retry reserve after checked historical replay. Acknowledged model intent must never be stranded by re-reserving already emitted response/checkpoint/intent bytes.

## 5. Consuming root transfer and sealed methods

New internal types are nonClone/private: `RegisteredAgentState`, `PendingAgentStateTransfer`, `AgentStateTransferPermit`, `AuthorizedOwnedAgentTurn`. No public Value, root snapshot accessor, snapshot->owner constructor or ResultClaimed->RetainedValue re-admission. Checked inert plan/carrier data may clone; live State never does. Public rejection before any commit attempt returns untouched input owner. Once an append attempt occurs, uncertainty retains/poisons the live holder; no argument return.

Chosen private method contracts:
```rust
observe_registered_state(&mut RegisteredAgentState, CheckedObservePlan,
                         AckedOrdinaryStagePermit) -> CheckedObservation;
prepare_agent_state_transfer(CompletedOwnedWait, CheckedAuthorizePlan,
                            CheckedProposal, &RegisteredSourceLease)
    -> Result<PendingAgentStateTransfer, TransferRejection>; // pure; retains owner on reject
commit_agent_state_transfer(PendingAgentStateTransfer, AckedTransferPermit)
    -> RegisteredAgentState; // same actual root, not a reconstructed input
run_registered_authorize(&mut RegisteredAgentState, CheckedProposal,
                         CheckedAuthorizePlan, AckedOrdinaryStagePermit)
    -> CheckedAuthorization; // scoped borrowed root, no owner extraction
```

`CompletedOwnedWait` is an internal postcondition/cleanup-settled result holder; public owned-frame claim is NOT called. Prepare binds exact State/Proposal facts and source authorize ID. TransferReserved ACK precedes taking its root; move into destination RegisteredAgentState once, then TransferCompleted ACK precedes any authorize evaluator or consumer exposure. The receiver owns the actual original backing; while appending Completed it is private/poisonable and lease-held. Stage borrowed aliases/contract temporaries are drained before exclusivity checks/physical cleanup. This needs a narrow interpreter registered-root borrowed evaluator seam, not a public Arc/Value conversion.

Recovery classifications:
- committed/prepared/answered: authenticated legal fold + held lease grants one sealed restoration permit; charged historical observe/start/resume compare exact recorded facts; no additional caller Argument.
- AttemptIntent without recorded settlement: model in doubt, no redispatch/root transfer/effects.
- failed terminal before cleanup start: one registered failure owner may perform acknowledged compiler cleanup.
- CleanupStarted without receipt: cleanup in doubt, no new cleanup owner/retry; explicit scoped failed host confirmation only.
- successful CleanupSettled before TransferReserved: evidence-only old wait result; root delivery in doubt, no remint. This is conservative and may abandon a completed transition after crash.
- TransferReserved without Completed: transfer in doubt, no destination root remint/no authorize.
- TransferCompleted is the NEW destination-stage authority basis, separate from the settled wait result. Under the same held lease, a one-use permit materializes only the registered Agent stage owner from its exact destination checkpoint. Never restore an old wait owner, never expose a public Result. If Authorize was interrupted, existing charged ordinary historical stage replay reproduces its decision before subsequent consumption.
- after actual external effect intent (future packet), its existing uncertain-dispatch rules must block duplicate effect and inappropriate cleanup/root transfer.

This distinction must be independently tested: transfer is a new logical stage-owner basis, not a claim that v1 settled/claimed continuations can recover their results. Each held session issues at most one permit; copied/rolled-back authoritative stores remain outside caller history assurance. ACK loss before/after persistence poisons the live holder, and recovery chooses only authenticated persisted grammar.

## 6. Concrete first API/result and remaining seam

Opt-in checked constructor `AgentRuntimeV2::source_owned_model_wait_binding(helper_id, evaluation_fuel)` derives exact association/HIR proof and rejects F>runtime per-stage ceiling prewrite. First executable route is `prepare_owned_model_turn_durable(...) -> AuthorizedOwnedAgentTurn` with pinned registered source lease, checked binding, one consumed OwnedAgentStateArgument, existing model adapter, clock/cancellation/policy/key. Returned turn is opaque/nonClone and owns same State, checked Proposal, actual authorization decision/grant and live lease. It cannot be supplied to legacy RetainedValue reducers or external arbitrary effect callbacks. This API advertises only the owned model/authorize seam. A rejection yields existing stable failure class plus live opaque holder where necessary, never a public snapshot or double owner.

Next implementation MUST consume that turn into real effect authorization and owned reducer/Step transition; actual initialize must return the registered State directly, not clone through RetainedValue; Continue/Suspend/Complete/Fail must carry their actual compiler-owned roots and terminal cleanup. Multi-turn closure, cancellation and native/Wasm explicit refusal evidence are remaining R20 work. First route never claims full Agent operations migration.

## 7. Proposed exact runtime leases and discriminating gates

New children in owned_frame v2 plan/checkpoint/codec/fold/driver, interpreter owned_frame registered_stage/Copy-channel children; narrow current registrations/profile dispatch only. Add source_journal owned_wait_v8 codec/fold/capacity/evidence and pinned sink children plus narrow ordinary combined inventory delegations. Agent lifecycle iterative/source_live owned_wait and registered_stage children; narrow session choreography and driver/live decodedProposal-beforeadmission hook. ExecutionRevision typed opt-in binding/route children plus exact registrations. Proposal schema owner projection child. Provider adapter only existing checkpoint eligibility predicate reuse. Parent handles shared root exports/version/capacity accounted footprint/source audits. No parser/HIR frontend edits by core; R15 owns that packet. Exact functions/root edits must be enumerated after reviewed contract, before code.

Required gates: canonical format->parse->checked graph State/Obs/Proposal/helper association; old scalar-v1 and Copy-v7 bytes/API stable; exact request/answer nominal/leaf/count/order/RecordBytes negatives; actual observe count once; actual authorize output altered by checked body, not fixture simulation; same Weak State leaf backing across start/park/resume/transfer/authorize; no transfer via public claim/readmit; all start/resume/replay/cleanup/transfer/ordinary-stage ACK windows before/after persistence; absent/failed reservation zero evaluator/provider/effect; malformed SDK output preserves ModelFailed; lower compiler ProposalRefused retry preserves same State and charges; grants/budgets/cancel/uncertain provider/effect classes unchanged; substituted helper/State/source/epoch/pins/transfer target/carrier/digest/tampered evidence refused before protected operations; inherited lease guards and lock-before-backing-drop control; transferred-owner recovery exactly once and old settled/no-transfer tail zero remint; fuel each historical phase individually fits but sum exceeds F; decreasing near-capacity closure success; compiler physical cleanup canonical order, callback panic all leaves attempted; explicit native/C11/Wasm wait refusal remains. No Cargo was run preparing this proposal.


## 8. Closing ownership/wire review gaps (normative refinement)

This section replaces any looser shorthand in sections 3–6. It does not broaden Copy channels to owned variants.

### 8.1 Initial State and actual owned Decision

Do not accept/rebind a public scalar-v1 OwnedFrameArgument. Add private-field, nonClone `OwnedAgentStateArgument`, admitted only by `admit_owned_agent_state_input(binding: &CheckedOwnedAgentWaitBinding, input: OwnedFrameInput) -> Result<OwnedAgentStateArgument, OwnedAgentStateInputRejection>`. Rejection owns the untouched inert input and diagnostics. Binding derives State nominal/fields/all eight scalar facts and incoming/abandonment cleanup obligations from the actual checked v2 helper/compiler owner. Borrow-validate all shape/Unicode/float bits/field order before move/allocation. Its internal semantic-disposal plan remains attached; no arbitrary old plan replacement. Current v1 admission remains unchanged. No public cross-plan rebind. First source initialize is explicitly still remaining; caller inert input is not represented as an actual initialize execution.

Actual Fixture authorize returns the existing nominal Decision: Granted {seal: Bytes,budget:i64} or Refused {code:i64}. `CheckedAuthorization` is an opaque consuming staged result owning that exact actual interpreter Decision root (not bool, copied seal Vec, or a synthesized SDK grant). Add checked registered-stage output proof for precisely this existing two-case flat variant, with compiler-derived branch cleanup/disposal vectors and existing seal bound; no general owned variant checkpoint admission. The authorize evaluator borrows State, produces the owned Decision under existing ordinary Authorize StageReservation, defers semantic cleanup, drains borrow/contract aliases and retains both State and Decision until ACK closure. A returned Granted result passes existing postconditions/nonresult cleanup before staging its exact bounded snapshot. Structural snapshot bytes are inert; the actual live root is never replaced by them.

`AuthorizedOwnedAgentTurn` contains State + the actual Granted Decision/seal + checked Proposal + privately derived existing grant metadata + held registered lease. Construct only after `owned_authorization_ready` ACK and lease/PID/scope/phase recheck. The seal is borrowed to compute the unchanged authorization binding; no seal cloning into a second owned token. Future effect stage must consume this Decision/grant once through a sealed owner handoff. Refused/code or pure evaluator failure produces the existing authorization refusal/Stop class, then acknowledged compiler cleanup of any Decision and State roots; no ready turn. Abandoning a ready turn uses explicit acknowledged cleanup, not implicit receipt-producing Drop. Drop is backing-only under the held lock; no semantic settlement or grant reuse.

Authorize append failure after evaluator entry retains private staged Decision+State and poisons the holder. Recovery may materialize a staged Decision only from the current legal `owned_authorization_staged` snapshot, held lease and one-use destination-owner permit; interrupted evaluation with no staged row requires separately charged ordinary authorize reconstruction. Ready is not replayed into an additional grant: when current tail is ready, a one-use combined holder restoration consumes the exact current State+Decision authority; any later effect intent/cleanup/terminal retires that basis. No historical TransferCompleted can mint State after authorize refusal/cleanup/terminal. Real Weak tests must distinguish State leaves and separately allocated seal, verify no extra seal owner and branch-vector physical disposal once/order.

### 8.2 Retiring malformed parked waits without duplicating State

On compiler ProposalRefused (the lower compiler path only), append existing ProposalRefused ACK, then `owned_wait_retired` ACK. Retired proves this same parked attempt has no resume/completion and records original Prepared basis and immutable State/Observation digests. A private consuming `retire_parked_wait` drains Copy continuation locals and moves the SAME root into a pending registered turn State. It uses a compiler-derived identity-root transfer proof: all State leaves remain intact, no loans/extra owned locals, no semantic leaf release, no pending obligation dropped. This is not public cancellation/snapshot re-admission. Append `owned_state_rearmed` ACK with exact same State/Observation snapshot and digests before accepting next attempt. There is no evaluator during retirement/rearm, no fuel refund or new allowance; next attempt start/resume require fresh charged ACKed reservations, and observed input remains the one original TurnObserved.

Retired-only recovery is root-transfer-in-doubt, no restore/no retry. Rearmed current tail grants one registered State permit; it cannot restore a parked old attempt. Original park remains retired forever. Latest-legal-phase check rejects rearm after completion/dispatch-uncertainty/cleanup/terminal. Before refusal admission preflight retirement+rearm+next-attempt bounded closure or terminal branch. SDK malformed bytes that terminate before a compiler ProposalRefused retain ModelFailed semantics and never invent this transition. Real retry test must prove original State Weak identity survives live retire/rearm and next attempt, no observer rerun, no resume of invalid Proposal and all charges retained.

### 8.3 Generic owned cleanup covers pre-wait failures

Replace the proposed wait-specific cleanup row names with closed v8 `owned_cleanup_started` / `owned_cleanup_settled` bodies below. They are scoped to an exact live root basis, including initial committed State before any attempt. An owner is `state` or `decision`; turn is current T, attempt/wait are null before a wait exists. Started ACK always precedes physical operations; receipt settles that exact start. A failed observe before a wait uses basis=owned_state_committed, attempt=null, wait=null and the compiler-derived incoming State failure disposal vector. Observer failure cannot leak State or fabricate a model failure/attempt. If Decision exists, settle Decision first under its compiler output vector, then State; both ordering and sticky primary failure are fixed checked protocol facts. Guard failure or process abort after Started means cleanup in doubt/no automatic re-release. A host-confirmed receipt is only explicit scoped failed cleanup confirmation, never successful grant/root delivery.

### 8.4 Exact new v8 bodies (supersedes section 4 shorthand)

Common row keys are exactly `schema,invocation,generation,seq,prev_mac,kind,authentication` plus the kind-specific additional keys below; C sorts all keys recursively. Every snapshot/digest is checked against the referenced compiler plan and previous root basis before append/restoration. Initial RunOpened follows Created. Existing ordinary bodies retain their exact existing keys.

```text
owned_run_created {scope,execution,binding,signature,limits,store_identity}
owned_state_committed {turn,state,argument_digest,cleanup_plan_digest}
owned_wait_created {turn,attempt,wait,plan_digest,cleanup_plan_digest,signature,argument_digest,copy_arguments,copy_arguments_digest}
owned_wait_reserved {turn,attempt,wait,phase,replay_of,fuel}
owned_wait_prepared {turn,attempt,wait,reservation,observation_digest,checkpoint_digest,checkpoint,consumed}
owned_wait_completed {turn,attempt,wait,reservation,proposal,proposal_digest,result_digest,consumed}
owned_wait_failed {turn,attempt,wait,reservation,status,consumed}
owned_wait_replay_checked {turn,attempt,wait,reservation,original,result_digest,consumed}
owned_wait_retired {turn,attempt,wait,prepared,state_digest,observation_digest}
owned_state_rearmed {turn,attempt,wait,retired,state,state_digest,observation,observation_digest}
owned_cleanup_started {turn,attempt,wait,owner,basis,terminal,operations,operations_digest}
owned_cleanup_settled {turn,attempt,wait,owner,started,receipt,receipt_digest}
owned_state_transfer_reserved {turn,attempt,wait,from,to,state_digest,proposal_digest,transfer_digest}
owned_state_transfer_completed {turn,attempt,wait,reservation,state,state_digest,proposal,proposal_digest,transfer_digest}
owned_authorization_staged {turn,attempt,stage_reservation,transfer,state_digest,proposal_digest,decision,decision_digest,consumed}
owned_authorization_ready {turn,attempt,staged,state_digest,decision_digest,grant_digest}
```

State commit uses argument_digest; rearm/transfer state_digest equals the v2 argument digest of that same exact State snapshot. State is never included again in wait-created, only its exact previously committed/current-root digest; copy_arguments contains the already checked Observation. Thus owned-run-created and state-committed establish pre-wait authority, and wait-created establishes the standalone v2 Created facts of section 2 by reference. Row limits/nonce/identity never derive authority from embedded bytes.

Grant refusal causal branch: staged Refused decision -> existing AuthorizationRefused -> owned Decision cleanup if compiler vector nonempty -> owned State failure cleanup -> existing Stop/TerminalSnapshot. Pure authorize failure preserves root and ordinary class; a failure with no owned Decision uses only State cleanup. Granted branch: staged -> successful compiler nonresult cleanup (empty or checked actual vector) -> ready -> existing AuthorizationConsumed -> return opaque ready holder. Postcondition failure never emits ready. The first route returns no ordinary terminal while handing out a live holder, so no terminal evidence root is available.

AuthorizationConsumed and ready must form a paired fold gate before exposure. Reservation/append loss before the pair is complete keeps live holder poisoned; recovery completes no missing event by guessing. It either uses the legal staged reconstruction under charged ordinary replay or reports delivery in doubt. A current completed pair may restore exactly one combined ready holder under lease; any later stage/effect/cleanup consumes that basis permanently. Ready cannot be appended twice for a historical staged row.

### 8.5 Remaining exact encoding/hash objects

V2 standalone checkpoint envelope keys are exactly `{payload,authentication}`; payload is section 2's exact checkpoint object, authentication lowercase64 HMAC. Encode C(envelope)+LF. Its hash domain authenticates C(payload) without LF; structural checkpoint digest hashes C(payload), and source outer checkpoint-byte digest separately hashes the exact envelope bytes INCLUDING LF under `semaprax.source-agent-owned-wait.checkpoint.v1\0`.

Add SHA domains (all one trailing NUL):
- `semaprax.source-agent-owned-wait.operations.v1`: C({owner,basis,terminal,operations}); operations is the compiler canonical vector, never structural sort.
- `semaprax.source-agent-owned-wait.receipt.v1`: C(receipt); receipt exact existing observed/host_confirmed shape. This is distinct from pending operations digest.
- `semaprax.source-agent-owned-wait.decision.v1`: C({scope,turn,attempt,authorize,decision}); decision is `{declaration,case,fields}` with exact declaration/case/field IDs, declaration-ordered inert Bytes/scalar values. Bounds are checked before allocation; no general variant graph accepted.
- `semaprax.source-agent-owned-wait.grant.v1`: C({scope,turn,attempt,state_digest,proposal_digest,decision_digest,authorization_binding,budget}); authorization_binding uses the existing checked authorization binding semantics and seal bytes, computed privately; digest itself carries no authority.

Evidence wait element is exactly `{wait,turn,attempt,created,prepared,completed,retired,reservations}`; absent closure seqs are null; reservations is causal-order array `{seq,phase,replay_of,fuel,closure,consumed}`, interrupted consumed=null. Transfer element exactly `{wait,turn,attempt,reserved,completed,transfer_digest,state_digest,proposal_digest,authorization_staged,authorization_ready,decision_digest,grant_digest}`; absent seq/digests null. Their current terminal/cleanup associations additionally appear in evidence `cleanups`, causal-order array `{started,settled,owner,basis,operations_digest,receipt_digest}`; absent settled/receipt null. Therefore add `cleanups` to the exact evidence payload in section 3. All source stage/model/terminal facts remain bound by real Eterminal/M at full terminal; reserved_total includes wait plus ordinary/replay reservations, consumed_total only ACK-observed evaluator steps. The first partial route has no terminal evidence and no standalone new nonterminal evidence hash: expose bounded data-only current status/inventory.

Recovery always uses LAST legal owner phase, not a search for any old transferable row. Current owner keys/bases are tracked by fold and removed on acknowledged transfer/cleanup/effect consumption. At most one State and one staged Decision are live; no row may duplicate or recover a retired owner basis. Before writes, pure validator checks supplied initial State and projected Observation against prepared immutable facts; after writes, same-root ownership follows consuming internal proofs and registered lease, never snapshots provided by a caller.


## 9. Final authorization recovery and store pins

This section replaces any earlier "either" recovery choice and any successful-authorize "empty or actual vector" alternative.

### 9.1 Closed successful authorize cleanup admission

First profile admits the actual fixture authorize body ONLY when its compiler cleanup plan proves: successful non-result cleanup vector is empty; whole returned Decision is the result root; every Granted seal leaf is retained by that result; all scoped State/contract aliases are drained before staging. Requires/ensures failure vectors, Decision result-disposal vectors and State failure-disposal vectors remain compiler-derived and physically executed on the appropriate failure path. The runtime neither fabricates an empty vector nor suppresses a compiler vector. A body with a nonempty successful non-result vector is refused prewrite by the checked binding for this first profile. General successful authorize cleanup is a separate future extension. No cleanup row is emitted for a fictitious successful release.

`owned_authorization_staged` is appended only AFTER successful postconditions and the admitted empty non-result cleanup boundary. It preserves exact actual Decision output root. A source evaluation failure does not produce this row; it selects the ordinary failure plus acknowledged owned cleanup. Successful staged Decision is already checked, so recovery below NEVER reevaluates authorize after Staged.

### 9.2 Deterministic last-legal-tail table

Rows refer to the current attempt's stage/transfer/decision basis; all later cleanup/effect/terminal events invalidate earlier restoration bases. An authenticated historical row cannot override this table. Structural restore is not evaluation and requires the one-use combined permit under the held v8 lease. Fuel/charges already recorded remain spent; no new grant is inferred from unused reservation fuel.

| Current authenticated tail | Sealed owner restoration and ONLY allowed continuation |
|---|---|
| TransferCompleted before an Authorize StageReservation | One State holder; reserve ordinary authorize fuel ACK, then evaluate actual authorize once. |
| Authorize StageReservation with no Staged/decision closure | One State holder; separately ACK charged ordinary historical authorize reconstruction under existing replay semantics. Old reservation remains fully spent. Compare recorded predecessor State/Proposal, then stage actual Decision; no uncharged retry. |
| Staged(Granted), no Ready | One State+actual Granted Decision staged holder; no evaluator. Revalidate structural binding/postcondition proof, append Ready ACK then existing AuthorizationConsumed ACK, guard again, expose one opaque ready turn. |
| Staged(Refused), no AuthorizationRefused | One State+actual Refused Decision staged failure holder; no evaluator. Append existing AuthorizationRefused ACK, then acknowledged Decision and State failure cleanup in that order, then Stop/TerminalSnapshot. |
| Ready without matching AuthorizationConsumed | ResultDeliveryInDoubt; ZERO State/Decision/grant restoration, no Ready/Consumed completion guessed, no evaluator/effect/cleanup. This conservative crash boundary may abandon a valid pending grant. |
| Ready + exact matching AuthorizationConsumed, with no later consuming event | One combined State+actual Granted Decision+checked Proposal+privately derived grant holder, no evaluator. A single permit restores this current logical destination holder; no old wait result/public Result is recovered. It may enter the future effect stage only through sealed consuming grant handoff. |
| AuthorizationRefused with no owned cleanup started | One failed State+Refused Decision holder; no evaluator/grant. Perform acknowledged compiler Decision cleanup, then State cleanup, terminal. |
| Decision cleanup Started without Settled | CleanupInDoubt; ZERO owner restoration or physical retry. Explicit failed host confirmation may close THAT cleanup receipt, but first profile does not restore the leftover State; terminal recovery reports in-doubt, never a ready turn. |
| Decision cleanup Settled, before State cleanup Started | ZERO Decision restoration; one failed State holder may perform its acknowledged compiler cleanup, then terminal. Its active State basis is separately retained by fold and not derived from the disposed Decision. |
| State cleanup Started without Settled | CleanupInDoubt; ZERO owner restoration/release retry. Only explicit scoped failed host confirmation receipt/evidence; no ready turn. |
| State cleanup Settled, terminal absent or present | Evidence/status only, ZERO State/Decision/grant restoration. Remaining ordinary Stop/TerminalSnapshot metadata may be appended only from validated fold, without evaluator/physical cleanup. |
| Any host-confirmed failed receipt | No confirmed owner restoration or physical retry; only the exact still-live separate-owner basis permitted by this table can be considered. Decision-confirmation case is conservatively terminal/in-doubt in this first profile. |
| Any later effect intent, state handoff, failure cleanup or ordinary terminal | No earlier Ready/TransferCompleted/Staged restoration. Future effect packet must add its own exact current-owner phase table; first API refuses those unsupported phases. |

AuthorizationConsumed is a source authorization fact, not provider/effect dispatch. It must name the existing privately computed grant binding for the exact staged Decision/State/Proposal. Ready-only recovery is deliberately unavailable even though live successful ACK can continue to Consumed; after a live append failure the holder is poisoned, and caller cannot use it to append/transfer/release. No arbitrary recovery-mode choice or silent replay guesses exist.

### 9.3 Exact profile-specific store interface and narrow changes

V8 does NOT accept the current public v1 RegisteredJournalLease. Add crate-private `SourceOwnedWaitStoreRegistrationV8` and `SourceOwnedWaitLeaseV8` wrappers around reused held-FD mechanics. Registration is constructed only by the existing trusted physical-host registration entry point for this opt-in route, pins independently supplied exact scope + four store identity facts + v8 execution/binding/limits, and derives the exact v8 filename and generation by section 3 recipes. Registration is non-authoritative data unless the physical-host registration grant/lifetime assurance is actually present; no hash manufactures that grant.

Private constructors:
```rust
register_source_owned_wait_v8(directory: File, expected: SourceOwnedWaitStoreFactsV8,
                             grant: ExplicitStoreRegistrationGrant)
    -> SourceOwnedWaitStoreRegistrationV8;
fresh_source_owned_wait_v8(&SourceOwnedWaitStoreRegistrationV8)
    -> SourceOwnedWaitLeaseV8;
recover_source_owned_wait_v8(&SourceOwnedWaitStoreRegistrationV8,
                            expected_generation: &str)
    -> SourceOwnedWaitLeaseV8;
```

Expected facts are `{scope,execution,binding,limits,store_identity}`; scope invocation equals the v8 source-id digest, not old owned-frame invocation construction. Caller expected_generation must equal the registration-derived nonrecursive digest BEFORE file I/O. Constructors compute the basename themselves; no raw relative pathname argument. Fresh uses exact v8 `.source-owned-wait.jsonl` name, nofollow+exclusive create, held-file identity checks/lock and proves empty before first append/owner transfer. Recover opens that same name relative to held directory, rechecks full expected scope/profile/generation/pins and creator PID, then authenticates the closed v8 file before issuing any restoration permit. A v1 file/lease or same-id v1 filename is never accepted; wrong profile has no fallback probe/migration.

The reused mechanism receives a sealed private `StoreProfile` selector (`OwnedFrameV1` or `SourceOwnedWaitV8`) plus a profile-derived basename and immutable full scope; this selector is constructed only by the corresponding internal owner factory, not caller strings. Every borrowed operation checks its wrapper's expected profile plus existing PID/fullscope/identity before I/O. V8 sink/driver additionally validates expected generation and causal phase before evaluator/host/drop/result and after callbacks. The file's authenticated schema and Created facts must independently match the held profile; store name does not substitute for wire admission.

Exact narrow store lease extension requested: `src/resumable_effects/owned_frame/store.rs` and `store/unix.rs` may factor existing file registration/create/open/lock/validate/append machinery into a private profile-aware constructor helper; preserve existing v1 factories as exact wrappers using original v1 basename/digest recipes and byte-identical public behavior. Add `store/source_v8.rs` (and nonUnix refusal forwarding) for sealed v8 wrappers and profile facts; add owning tests for v1-v8 cross-profile/name/generation/scope substitution, nonempty fresh, symlink/path/pin/foreignPID/lock controls. No public v1 signature/struct field/digest changes. SourceLive v8 owns its sink adapter; the generic legacy pathname CheckpointStore remains refused. Any refactor that changes v1 known-answer names/generation/behavior is prohibited.


### 9.4 Fresh versus recovered registration ordering (replaces §9.3 constructors)

Fresh file identity does not exist before exclusive creation. Do not predict it. Split a trusted fresh grant from the complete independently retained recovery registration:

```rust
prepare_fresh_source_owned_wait_v8(directory: File,
    expected: FreshSourceOwnedWaitFactsV8,
    grant: ExplicitStoreRegistrationGrant) -> FreshSourceOwnedWaitGrantV8;
fresh_source_owned_wait_v8(FreshSourceOwnedWaitGrantV8)
    -> (SourceOwnedWaitStoreRegistrationV8, SourceOwnedWaitLeaseV8);
recover_source_owned_wait_v8(directory: File,
    registration: &SourceOwnedWaitStoreRegistrationV8,
    expected: FreshSourceOwnedWaitFactsV8,
    grant: ExplicitStoreRegistrationGrant) -> SourceOwnedWaitLeaseV8;
```

Fresh expected facts are exactly `{scope,execution,binding,limits,directory_identity}` with the two independently granted directory dev/inode facts, not file facts/generation. Prepare verifies creator/current grant, expected v8 invocation/scope and held directory identity. Fresh then derives the exact v8 name, exclusively creates nofollow, locks, validates actual file safety, obtains the actual two file dev/inode facts, and creates complete immutable registration `{scope,execution,binding,limits,store_identity,generation}` where store_identity combines granted directory facts and observed newly created file facts. Derive generation only NOW with the pinned nonrecursive recipe, prove held file empty, return lease+registration. No journal write, evaluator, host callback or State transfer happens during this registration factory.

Caller retains the complete registration OUT OF BAND under its protected-history/lifetime assurance before passing lease to the owned run constructor. Start requires an explicit current host registration-retained grant paired with the exact complete registration/lease identity. This is a physical host authority acknowledgement, not evidence or a hash that grants authority. If retention fails, close the empty held lease; no owner was admitted/transferred and no history was committed. The host may not recover from journal-derived file IDs or generation.

Recovery requires independently retained complete four pins and generation, plus current independently expected scope/execution/binding/limits/directory facts and registration grant. It compares expected facts, recomputes generation from those RETAINED full pins (never from opened file or journal) before file opening, verifies held directory pins, opens only the computed v8 filename, then verifies opened file equals retained file pins and all safety/PID/fullscope/profile checks. Authenticate legal v8 history afterward. An alternate/copied/rollback registration is outside protected caller assurance, never legitimized by a matching MAC.

The profile-specific constructor/delegation and narrow store lease paths in §9.3 remain; only the impossible pre-create file-identity requirement/signatures are replaced by this exact ordering. V1 constructors/public signatures/name/generation recipes remain frozen.

## 10. Exact replay reference types (normative clarification)

`OwnedWaitReserved.replay_of` names the original start/resume evaluator reservation sequence. `OwnedWaitReplayChecked.original` names that phase's original `OwnedWaitPrepared` or `OwnedWaitCompleted` result-closure sequence; the fold validates the closure's own reservation reference and the replay reservation's original phase. These are true combined journal sequence numbers, never projected ordinary-entry indexes.

A replayed `OwnedAuthorizationStaged.stage_reservation` names the newest fresh `ReplayStageReservation` that actually funds this reconstruction. Its consumed work is bounded by that fresh allowance. The original `StageReservation` remains spent. Only the newest active replay reservation may be closed; a historical ReplayChecked row cannot close or authorize advancement of a later replay reservation.

These refinements do not change the closed body keys, hash domains or old v1-v7 behavior. No successful empty cleanup rows exist: the success profile proves the nonresult vector empty before staging, then uses the existing Staged/Ready/Consumed pair grammar.

## 11. Pending consumption and terminal evidence completeness

Pending inventory reports exact `reserved_total` and a separate `consumed_recorded` lower bound over acknowledged rows with durable consumption facts. Ordinary Observe work has no such standalone consumed field. Its unavailable work is not zero, estimated from fuel, or refunded; every reservation remains spent in full. Pending inventory does not present this lower bound as exact `consumed_total` or terminal evidence.

A future terminal `consumed_total` means the exact ACK-observed consumption sum, not all physical work across interrupted processes. It requires a stricter v8 evidence gate: ordinary stage rows must be complete (`omitted_stage_rows = 0`), each must map to its actual original or replay execution reservation, and authorize consumption duplicated in Staged and ordinary stage evidence must reconcile and count once. Add wait-only evaluation consumption separately. Interrupted execution without an acknowledged consumed fact stays unavailable. If completeness, mapping, or reconciliation is absent, exact terminal evidence is refused; legacy truncated evidence is not silently promoted. This does not change any ordinary v1-v7 wire or accounting behavior.

## 12. Failed authorization before Staged

There is no ordinary SourceJournalEntry StageFailed body. Before any owned CleanupStarted or ordinary Stop is acknowledged, TransferCompleted remains the sole State/Proposal reconstruction basis. A fresh charged ReplayStageReservation may reconstruct the original authorize outcome using the exact original allowance F and unchanged checked source/inputs; never use leftover or a larger evaluator allowance. All prior reservations remain spent. Repeated interrupted reconstruction reserves F anew and never closes or credits the original reservation.

A reconstructed failure stays an opaque failed holder. A partial constructor seal is not a full Decision snapshot and never produces Staged or Ready. Failed Decision cleanup uses `basis` equal to the current original or fresh replay authorize reservation that actually produced that obligation. Its operations are exactly the compiler's actual Temporary partial-constructor vector or, after a complete body result, its actual provisional result vector. The sticky failure is recorded by CleanupStarted.terminal; no vector, flags or case tag is invented. If no Decision obligation exists, proceed directly to failed State cleanup based on TransferCompleted.

After CleanupStarted ACK, restoration and physical cleanup retry are forbidden. An observed (not host-confirmed) failed Decision cleanup receipt permits the separate pending State cleanup; a settled State permits failed status/evidence only. Host-confirmed Decision cleanup remains terminal/in doubt with zero restoration under §9.2. Ordinary Stop and TerminalSnapshot follow acknowledged owned cleanup, never precede it. A Stop observed before cleanup is conservatively in doubt with zero restoration, not permission to recover under an older reservation. Recoverable post-Stop failures would require a separately reviewed contract.

## 13. Full Decision snapshots and canonical cleanup facts

A full Decision snapshot D is exactly `{declaration,case,fields}`. `declaration` equals the actual checked authorize result nominal; `case` equals its actual Granted or Refused case identity; `fields` is the declaration-order array of `{identity,value}` with the exact checked case fields. Granted contains the bounded Bytes seal and i64 budget; Refused contains only i64 code. Values use the existing v1 structural Bytes/scalar leaf encodings. Unknown/missing/extra keys, wrong nominal/case/field/order/leaf, and a partial constructor are refused. A partial seal has only its failed reservation basis and actual Temporary disposal vector; it is never a D snapshot.

V8 operations use the exact existing compiler graph FinalizeAction JSON renderer, including any `active_case` and Temporary storage facts. Arrays retain the compiler canonical runtime order. Structural admission compares the complete vector with the actual checked vector selected for this owner and basis; outer C normalization changes object key order only. V1 owned-frame operations encoding is unchanged.

An observed receipt retains the exact existing `{kind,settlement,operations}` shape, with compiler-vector-order operation receipts `{operation,outcome}` matching that entire selected vector. Outcomes are completed or failed, and aggregate settlement matches them. Receipts do not change sticky selected failure. A host-confirmed receipt remains `{kind,confirmation_digest}`, scoped only to an already acknowledged failed CleanupStarted. Confirmation is proof data, not physical authority; the registered host must still provide the ordinary explicit confirmation authority.

## 14. Interrupted original wait reservations and sticky wait failure

An original StartReserved without Prepared, or ResumeReserved without Completed, is a spent interrupted reservation, not a closed phase and not evaluator credit. A new OwnedWaitReserved for the same phase may set replay_of to that original reservation's true sequence. It reserves the full original F again and becomes the sole newest funded evaluator basis. Repeated interrupted retries also name the original reservation, never the preceding replay reservation, and each spends a fresh F. The original reservation retains no closure and unknown consumption.

When there is no previously durable result for that phase, the first new Prepared or Completed closes only the newest fresh reservation. Retain separately the original reservation identity, that first durable result closure, and its actual funding reservation. No ReplayChecked is emitted for a missing old result. Later historical ReplayChecked.original names that first durable Prepared/Completed closure and validates its funding reservation's phase and original/replay_of link. An old ReplayChecked cannot close a later reservation. No wire fields or domains are added.

A retried Start evaluates the same checked helper with the exact committed/rearmed State and already checked Observation. A retried Resume uses the exact prior Prepared checkpoint and acknowledged authoritative raw settlement/checked Proposal; reconstructing the historical parked root first requires its own separately charged Start replay and successful ReplayChecked. No caller-substituted root, Observation or Proposal is admitted. No historical observation rerun or fuel refund is implied.

OwnedWaitFailed.status uses the existing owned-frame failure facts as exactly `{failure,language_status}`. Failure is one of language_failure, fuel_exhausted, host_abandoned, answer_type_mismatch, evaluation_rejected, handler_failed, call_depth_exceeded. Only language_failure carries the exact compiler-admitted NormalizedStatus; all other tags require null language_status. A failed wait selects that entire status carrier permanently: ensuing CleanupStarted.terminal equals it exactly, and every subsequent owner cleanup retains the same terminal even if cleanup itself fails. Other source-stage refusal/status carriers must come from their owning checked stage and are not manufactured from a wait failure.

A selected Refused Decision can retire structurally before State cleanup without physical Decision rows only when the sealed checked authorize proof establishes that the selected Refused case has an empty disposal vector. No host boolean supplies that proof. A nonempty Decision vector must receive its CleanupStarted/observed CleanupSettled pair before State cleanup. This refines section 8.4 and does not add fictional empty physical cleanup rows.

## 15. V2 checkpoint accounting and true sequence

`sequence` is the true combined causal sequence of the `OwnedWaitPrepared` row whose candidate carries the checkpoint, including the first durable Prepared after an interrupted original Start and fresh Start reservations. It is never a wait-local ordinal or a reservation sequence.

`reserved_total` includes all acknowledged ordinary and owned reservations through that Prepared candidate. `consumed_total` is §11's recorded lower bound (`consumed_recorded`), including this Prepared's actual consumed work plus prior acknowledged consumption; it is not an exact terminal total. Missing interrupted-reservation consumption remains unknown and never becomes zero/refunded work.

Structural checkpoint validation obtains all three expected facts from the independently validated candidate fold and compares them exactly. It never takes expected accounting from the checkpoint itself. Authentic checkpoint bytes grant no authority to append, evaluate, restore a root, publish a result or dispatch.

The 65536-byte checkpoint bound includes the complete canonical envelope and its single trailing LF. At the ceiling, closed shape checks still apply; one additional byte is refused before parsing or owner allocation.

## 16. Checked v8 ordinary validation identity

The actual typed execution binding E remains unchanged and is recorded in `OwnedRunCreated.execution`. A separate private v8 ordinary validation binding retains every checked ordinary execution field and uses only the independently recomputed `I = D(source-id.v8, {execution:E,owned_wait_binding:B})` as its invocation identity. Ordinary attempt, model intent and causal validation within v8 therefore use I; no host digest setter or mutation of E exists.

The closed context constructor joins genuine checked typed E and B with the held v8 lease and complete independently retained registration. It checks creator PID first, complete registration equality, all four current physical pins, exact standalone source revision, expected I/scope/epoch, E/B and all limits. The first route uses the full fixed ordinary stage fuel ceiling for helper evaluation; a differing evaluation fuel proof is refused by this context.

Context construction and structural validators are data-only. They do not acknowledge registration retention, enable append, restore an owner, dispatch a model/effect, or publish evidence. The live route still needs independent retention ACK and must recheck the held lease and current policy before and after every callback and physical operation.

## 17. SDK Proposal and Copy answer commitments

The ordinary SDK Proposal commitment and the owned helper's Copy answer commitment are distinct. A single checked projection takes the already decoded SDK Proposal from the exact B-owned schema and produces the declaration-ordered Copy channel carrier; it does not parse model bytes in the continuation owner.

`OwnedWaitCompleted.proposal` and each transfer's `proposal` are that exact Copy channel encoding. Their `proposal_digest` is the unchanged ordinary `semaprax.source-proposal.v2` hash of the SDK decoder's exact canonical Proposal document, including its LF. Ordinary `ProposalAdmitted` therefore retains its existing bytes and compares that same ordinary digest.

The projection separately derives `answer_digest = D(source-owned-frame-answer.v2, {scope,plan_digest:B,value:Copy carrier})`. `OwnedWaitCompleted.result_digest` is `D(source-owned-frame-result.v2, {argument_digest,answer_digest})`. Never substitute the ordinary digest for answer_digest or vice versa. Recovery obtains the same projection from the acknowledged authoritative raw settlement through the existing SDK decoder and checks every recorded carrier/digest. A raw digest pair or caller-reminted Copy carrier cannot replace this projection.

The existing SDK/Agent carrier profile remains unchanged. In this portable owned wait route, all Copy scalars additionally satisfy frozen frame scalar bounds, including usize<=u32::MAX. Proposal sidecars, commitments and canonical strings grant no ownership, authorization, append or dispatch authority.

## 18. Typed authenticated prefix validation

A closed private binder joins the genuine typed Context and held current physical lease with the exact authenticated v8 document. It checks the combined sequence/MAC chain, actual B-owned State/Decision shapes, distinct frozen ordinary State commitment, sealed Observation, authenticated raw SDK settlement and both Proposal commitments, and K against the independently folded prefix's true Prepared sequence and charged totals. A prospective encoded row uses that same binder with the exact prefix, next combined sequence and prior MAC.

This first data-only inventory profile stops before physical authorization Ready or cleanup. Without the retained-runtime extension in §20, every Ready row is refused. CleanupStarted and CleanupSettled remain refused until selected stage obligation binders exist. No caller-selected raw proof, heuristic partial cleanup vector, owner restoration, registration retention ACK, append permit, model/effect dispatch or terminal completion evidence follows from this inventory. Synthetic E/store tests are identified separately from the genuine typed Context and physical lease tests.

## 20. Retained-runtime Ready commitments

A private Context extension retains the genuine immutable typed runtime after joining it to E through the existing checked registry, task, model, source and effect-ceiling borrower. It keeps the same complete registration and held-lease checks. A Context without this retained runtime continues to refuse Ready.

Ready data admission uses the exact authenticated Staged Decision, transferred State and sealed SDK Proposal at the same true sequence, turn and attempt. The actual compiler seal and budget field identities select the bounded inert seal bytes and i64 budget. The checked runtime planner selects the operation and arguments. Authorization uses the unchanged fresh per-turn policy precursor `D(semaprax.agent-iteration-policy.v2\0, lifecycle.digest() || 0 || decimal(turn))`, frozen ordinary State bytes, SDK canonical Proposal, actual Granted case and seal.

Ready and the immediately following Consumed use exactly §8.5's source-owned grant commitment, including attempt, all three source sidecars, authorization binding and budget. The distinct physical target grant uses the unchanged target preimage binding I8 and `Some(E.ordinary().invocation())`, never the separate revision digest. Both commitments are retained separately; the target digest cannot substitute for the source-owned Ready commitment. Attempt or budget changes affect the source-owned commitment even where the frozen target preimage is unchanged.

These are inert commitment-consistency facts. A re-MACed mutation with stale downstream commitments is refused; coherently recomputed commitments do not prove that authorize evaluated or that any owner transferred. No physical TargetGrant, one-use ACK, append/retention authority, restoration, cleanup, effect dispatch or terminal evidence follows. Cleanup rows and the effect/reduce/next-turn grammar remain outside this extension. Source authority audits include the new commitment child.

## 21. Private inert effect settlement and successful Decision consumption

This successor foundation extends the closed v8 private proof-data grammar beyond ReadyPair. It does not enable production ACK minting, owner restoration, effect dispatch, reducer publication, or full Agent acceptance. Existing failed OwnedCleanupStarted/Settled retain their exact terminal failure and basis rules. New success consumption kinds cannot be substituted into that grammar. Existing hash recipes and ordinary schemas stay frozen.

### 21.1 Closed records and causal order

After exact Ready plus immediate matching AuthorizationConsumed, only an ordinary EffectIntent matching the actual checked operation and frozen request digest is admitted. Its exact combined sequence becomes the intent basis; tail EffectInDoubt grants no redispatch or restoration. An ordinary EffectObserved or EffectFailed must match that turn/attempt/operation and have exactly one preceding intent. Its sequence becomes the settlement basis; tail EffectSettlementUncommitted grants no publication.

The immediate next row is new `OwnedEffectSettlementRecorded {turn:u32,attempt:u32,intent:u32,settlement:u32,evidence:String,evidence_digest:String,result_wire:Option<String>}`. evidence is lowercase hex of the actual frozen TargetEvidence canonical_wire. Pure inventory decodes using TargetEvidence::decode, demands byte-identical canonical encoding and exact evidence digest, reconstructs/replays the frozen fuel1 request from the actual retained runtime/E/State/Decision/Proposal/attempt and separate physical target grant, and checks actual ordinary settlement payload/failure against target settlement/accounting. Returned requires the exact canonical Target result wire in result_wire (lowercase hex), replay_exchange_wire against that wire, and the existing accepted_result validator. For EffectObserved, its accepted canonical payload must equal the result payload. For Returned plus EffectFailed(HandlerFailed), the exact replayed result must fail that validator; omitted rejected payload is refused. This restricted NonReturned branch requires result_wire=None and evidence.result_digest=None, then the existing closed source-failure mapping plus replay_exchange_wire(request,None). NonReturned with any committed result digest (including malformed/type-mismatched or host-failed raw responses) refuses until exact raw-exchange support is separately admitted; replay_wire alone cannot prove omitted result bytes. This restricted first-effect profile requires turn0, actual Granted budget≥1 and dispatched=true, and reconstructs TargetAccounting from default using exact request bytes/fuel1/actual checked limits. Returned charges its exact committed raw wire; HostFailed/HostPanicked without a committed response charge0; ResultBudget charges only the existing bounded-response overflow sentinel. CancelledAfterDispatch refuses until its discarded response/cumulative accounting basis is separately specified. No per-turn default reset is a cumulative budget proof. No ordinary source-grant/physical-target-grant substitution is allowed. Returned evidence with noncanonical or wrong checked result schema is not an accepted Outcome. Refused or no-dispatch target evidence never becomes success. Real policy/cancel failure before target invocation needs a later separate no-dispatch grammar; this slice refuses it rather than fabricating target evidence.

Then new `OwnedEffectDecisionCleanupStarted {turn,attempt,staged,ready,consumed,intent,settlement,recorded,decision_digest,operations,operations_digest}` must name the exact full Granted Staged, Ready, Consumed, Intent, ordinary settlement and Recorded combined sequences. No replacement seal/budget/operation/State/Proposal or reservation basis is accepted. Only the actual compiler Granted result disposal vector in canonical runtime order is admitted. This is mandatory successful consumption of the Decision after a real effect settlement, including a failed effect; it is not a terminal status or authorization partial-constructor cleanup. No evaluator stage/fuel is fabricated for a physical finalizer.

New `OwnedEffectDecisionCleanupSettled {turn,attempt,started,receipt,receipt_digest}` must immediately follow the matching Started. receipt is the existing exact whole observed receipt: kind observed, settlement completed|failed, ordered operations with actual corresponding outcome completed|failed. Root uses the existing strict compiler receipt validator with the same Granted disposal vector; whole Receipt recipe is unchanged. A failed receipt yields EffectCleanupFailed and never reducer/Outcome authority. A completed receipt after successful canonical EffectObserved yields EffectDecisionReleased; completed receipt after failed effect yields EffectFailedState. Legacy failed cleanup/Stop refuses once effect progress exists; typed failed-State disposal is a later grammar, so old terminal pairs cannot replace the selected effect failure. Actual physical release and post-release ACK producers remain separately necessary.

### 21.2 New commitment, unchanged recipes

New `RecipeV8::EffectDecisionOperations` domain is `semaprax.source-agent-owned-wait.effect-decision-operations.v1\0`. Its exact object keys are `{turn,attempt,staged,ready,consumed,intent,settlement,recorded,decision_digest,operations}`. The full object is encoded with existing sorted-key canonical JSON, ordered arrays, strict integer/depth/size rejection. It does not reuse or alter the frozen failure Operations recipe `{owner,basis,terminal,operations}`. New records retain deny_unknown_fields and existing complete carrier/journal limits. Evidence wire maximum is1475bytes (16 length frames×8 +schema38 +4digests×71 +4identifiers×240 +5u64×8 +dispatched1 +longest settlement24). Evidence hex maximum2950characters. Exact raw result_wire maximum65536bytes, lowercase hex maximum131072characters, checked before decode/allocation; the complete Recorded row uses actual canonical serialization with these maxima, all five u32 values at4294967295, digest71 and envelope+MAC overhead. Unsupported evidence shapes outside these bounds refuse without authority. MAC chain envelope and previous row recipes remain unchanged.

### 21.3 Fold and inventory

Fold records exact sequence links and actual Staged full Decision; unique effect per consumed grant, immediate row pairing, sticky failed settlement, compiler disposal equality, strict receipt equality. It refuses duplicate intent/settlement/cleanup, stale/crossed references, old failed cleanup substituted after Ready, effect rows before Consumed, and any effect continuation from an in-doubt historical tail. Pure validation can recognize a partial uncertain tail but producer transition refuses any unrelated successor to an uncertain effect/cleanup tail. Only the explicit matching settlement/Recorded and matching cleanup Settled may structurally close pending transitions. A future physical ACK producer must additionally retain the corresponding live one-use holder; the identical historical prefix alone never permits retry/continuation. An unrelated authentic row cannot erase uncertainty.

Inventory retains the checked Ready commitments and exact actual Proposal/State/Decision to check Intent and Recorded through the pure core helper. No caller-produced hash tuple becomes a checked fact. Actual Context runtime/registration/PID/pins stay mandatory. Consistent authenticated rows are historical data only, even if every hash is recomputed; no permit or witness restoration factory is introduced here.

Fold additions live in a new owning child to keep fold.rs below its 1500-line budget. Existing code bodies are not moved or dedented; source audit inventories join each new authority-adjacent child. Tests remain inside their owning harness modules.

### 21.4 Capacity and gates

Before accepting an Intent candidate, reserve serialized maxima for ordinary settlement, target evidence Recorded, compiler Decision cleanup Started/Settled and failure State cleanup plus Stop/Terminal. ReadyPair is no longer a zero-room terminal foundation tail. Every earlier predecessor closure includes this effect closure. Integer seq/turn/attempt widths and actual selected vector/carrier maxima, target evidence maximum and worst failure reason are included. Candidate crossing a byte/row limit refuses before any physical append. This foundation does not yet permit reducer: reserve its later complete grammar separately before enabling production dispatch/public loop. No new path borrows empty future closure assumptions.

This first slice admits the new rows only for inert recovered-data validation. validate_producer_transition rejects every new effect Intent/settlement/Recorded/consumption-cleanup row through the generic candidate route. A subsequent reviewed private candidate overload will consume a non-Clone LiveOwnedEffectAppendObligationV8 produced only by the actual Prepared/Staged/PendingReceipt engine holder and bound to the same held container, authenticated prefix/tail and intended exact row bytes. No caller boolean, raw digest tuple or recovered prefix can construct it. Therefore no recovered uncertain tail is extendable in this slice; pure fold recognition is not producer admission.

Discriminating tests: genuine runtime/store prefix with real target request/evidence, successful/failed actual target settlement; re-MACed wrong target/source grant, request, auth, E/I8, operation, payload/result/accounting/evidence, true combined references and Decision vector; receipt order/duplicates/missing/failed observer; historical uncertain tail recognition vs producer extension refusal; exact boundary capacity and untouched old failure/hash oracle. The actual physical bridge tests remain separate proof of actual drop/ownership. This slice has no production ACK constructors and cannot close R20.

## 22. Private fresh initialization profile

This explicitly selected private v8 successor admits fresh initialization only. It does not admit public Agent execution, authorization ACK factories, owner restoration, replay after partial initialization, or durable terminal delivery. The default ObserveOnly profile and its frozen row/hash rules remain unchanged. Initialization mode is selected from the expected checked Context and actual retained runtime/B initializer proof before decoding; journal rows cannot select or infer this mode.

### 22.1 Exact Task and acknowledged sequence

Before any physical write or ownership admission, the live actor checks the actual typed runtime Task bound into E: nominal declaration, ordered field identities, scalar values and exact Bytes must agree, and the borrowed Task must satisfy its compiled schema. An inert matching carrier alone cannot construct a live owner.

Fresh successful initialization has these true combined sequence positions:

| Sequence | Record | Required basis |
|---|---|---|
| 0 | existing OwnedRunCreated | exact checked Context |
| 1 | ordinary RunOpened | same invocation |
| 2 | ordinary StageReservation | turn0, attempt=null, role=initialize, fuel exactly E.max_steps_per_stage |
| 3 | new owned_initialization_committed | reservation=2, task, task_digest, state, state_digest, consumed |
| 4 | existing owned_state_committed | turn0, state, argument_digest, cleanup_plan_digest |

The new InitializationCommitted body has exactly `kind,reservation,task,task_digest,state,state_digest,consumed`; unknown/duplicate keys refuse. Its reservation is the actual original Initialize reservation, consumed is u64 and at most its exact F, and Task is the exact expected typed Task. Task/state use the existing `semaprax.source-owned-frame-args.v2\0` digest of the canonical record; no new hash, envelope or MAC recipe is introduced. StateCommitted must contain exactly the preceding initialized State and digest, with the existing checked helper cleanup-plan digest. Default ObserveOnly never admits this initialization sequence.

Full F is charged once through the ordinary original Initialize stage reservation, including its original stage count. Recorded consumed is observed source work, not a refund or replacement for F. A smaller or larger reservation allowance refuses before evaluation; that reservation-equality control is distinct from the initializer evaluator's existing fuel-boundary gate.

### 22.2 Physical owner and failure

The actor derives and retains the exact checked initializer. A private non-Clone initialization permit retains the same held store borrower, exact F and cancellation binding. The real reservation ACK precedes source evaluation; Checked-context/store/PID/cancellation checks and complete physical pins surround entry. The actual Task Bytes backing and monotonic allocation provenance move through the checked source initializer into State; source evaluation and the compiler's empty nonresult cleanup proof precede successful commit. The actor records actual consumed steps and exact State from that owner, then checks matching physical initialization and State ACKs. Decoded historical rows cannot create this permit or State owner.

Actual physical append uncertainty or physical guard failure poisons the shared container and quarantines the actual owner. Pure candidate refusal does not itself poison the container; the current actor may still withhold its owner after that refusal, and its broad Uncertain return classification is not evidence of physical poisoning. Failed/partial/provisional initialization has no successful InitializationCommitted/StateCommitted claim, fabricated cleanup receipt, replay permission or Stop publication. The opaque holder retains the container through abandonment; Drop drains backing before the inner foundation's semantic disposer and cannot claim compiler semantic cleanup. Partial initialization cleanup and recovery remain outside this admitted slice.

Owning gates cover actual Task-to-State backing identity and acknowledged F/consumed/stage inventory, prewrite Task/schema/mode/cancellation refusals, before/after persistence failure at all five rows, exact reservation F-minus1/F-plus1 refusals, zero semantic release on abandonment, and unchanged default ObserveOnly seed/inventory behavior.

### 22.3 Actual Observe successor

The same fresh live actor may consume its initialized State through one checked Observe. Its ordinary Observe StageReservation ACK, with turn0, attempt=null and exact E stage allowance F, precedes source evaluation. The permit retains the same store borrower and cancellation binding; no caller replacement State or arbitrary source evaluator is admitted. The checked evaluator borrows the actual State, produces the Copy Observation once, and returns the same State Bytes backing.

The actor then appends the existing ordinary TurnObserved using the actual State/Observation hashes and frozen initial no-feedback recipe. Both original Initialize and Observe reservations count, so the acknowledged reserved fuel is2F and original stage count2. TurnObserved does not record Observe consumed steps; checkpoint or recovery validation must not infer that consumption from the row. No new hash recipe or ordinary schema is introduced.

Failure or a lost reservation/TurnObserved ACK retains the actual owner and same held container; no source evaluation follows a failed reservation ACK. Cancellation before Observe leaves the initialized five-row prefix unchanged. Unpublished wrappers disarm inner semantic disposal before backing release. Helper start/resume, model dispatch, authorization, public loop and recovery remain outside this successor.

### 22.4 Private physical Continue-to-Observe handoff

A separate physical foundation consumes the actual mapped Continue State under an owner-containing transition/next-Observe reservation envelope. Its production constructors are absent; cfg(test) envelopes do not establish journal ACKs or extend recovered v8 histories. Exact next-turn arithmetic, E.max_iterations, E's exact fresh unconsumed stage allowance, the retained container and current cancellation/physical guards precede one checked Observe. Task.budget remains authored source data, not an additional host iteration ceiling.

The State Bytes allocation moves from the mapped Step through Observe without re-admission or replacement. The prior Proposal borrow is retired; the actual Copy Observation may enter the existing prepared helper through its consuming seam, without running Observe again. Guard loss after evaluation quarantines the actual observed/failed owner and suppresses continuation. Unpublished holders drain backing without claiming semantic disposal. Tests use genuine rebuilt E/B/store contexts, including an unchanged-source Task-budget-zero control and a separately rebuilt Complete fixture for Report ownership controls. This admits neither next-turn effect dispatch/cumulative accounting nor terminal delivery.

### 22.5 Actual helper park and same-session checkpoint

The fresh initialized/observed actor may append the existing OwnedWaitCreated and exact Start evaluation reservation, then consume its actual observed State through the checked helper. The Start ACK precedes evaluation. A parked holder retains the same physical borrower, checked helper/B, actual State roots and exclusive allocation witness inventory; serialization validates that live inventory before borrowing the frame. It never reconstructs an owner from snapshot bytes.

The fixed checkpoint producer borrows its key, full scope, context and true Prepared sequence from the same session/container. It preserves frozen v2 checkpoint schema, HMAC domains, canonical bytes and LF. The complete checkpoint is at most65536bytes including LF before hex or append and is checked by the unchanged strict validator. Full reserved fuel comes from acknowledged combined inventory; recorded-consumption lower bound includes acknowledged consumed entries plus this actual Start, without inferring unrecorded Observe work. The Prepared ACK is checked against the actual parked holder.

Initialize, Observe and helper Start each reserveF, totaling3F; original ordinary stage count remains2. Encoding/refusal/uncertain persistence retains the actual holder and container, and cancellation already present when the park actor is entered leaves the observed seven-row prefix unchanged; later cancellation retains any acknowledged reservations/rows. No helper evaluation follows a failed Start ACK. Terminal/parked roots abandon backing without claiming source semantic cleanup. Model dispatch, response/Resume, transfer/authorization, partial-failure recovery and public Agent execution remain outside this slice.

### 22.6 Actual model settlement and charged Resume

The private initialized actor may use its actual parked holder and borrowed checked observation to derive the existing SDK request. It appends the ordinary AttemptIntent through the same held session before dispatch. The one-use dispatch permit binds the actual acknowledged prefix, full PID/pins, scope, E/B and cancellation. Request rendering preserves the legacy SDK prompt bytes; the actor retains the already decoded checked Proposal rather than parsing or reconstructing an owner.

Each external factory, capability, start, poll and clock callback is guarded before subsequent work by held-prefix/cancellation checks. Cancellation may still record its existing failure and bounded usage but cannot Resume; the cancel callback checks held-prefix/store validity while preserving that selected failure. Callback panic persistently quarantines the container and returns the same opaque owner; cancel failure cannot replace an already selected SDK failure. Raw settlement and bounded usage are recorded through the ordinary rows before the exact Resume reservation. The original checked F is charged once at its ACK; callback deadline/pin/cancellation checks precede evaluator entry and follow actual evaluation. The retained bound clock is checked after settlement/usage and at Resume entry/publication.

Only the original parked owner and that reservation may enter Resume. Successful Completed follows the actual terminal helper with its checked Proposal, original State allocation witnesses and true ACK reference. Rejection, malformed response, uncertain persistence or guard loss retains the original owner and any actual provisional terminal result under the same held store. Drop abandons backing only. This holder is private helper completion: Proposal admission, State transfer, Authorize, target dispatch, Reduce, public Agent result delivery, cumulative accounting and durable partial recovery remain separate required successors.

## 23. Authenticated first-turn Reduce proof data

This section defines the closed private proof-data grammar. The admitted implementation checks compiler plans, nominal values, causal references, ordered cleanup observations, original aggregate reservation accounting and phase-specific serialized closure capacity. It does not evaluate Reduce, restore owners, dispatch a target, perform cleanup, deliver an Agent result or mint a production physical ACK from decoded rows. Generic append refuses the seven new owned bodies and ordinary Reduce/Transition/Stop advances of an active Reduce obligation.

The prospective hold, physical producer, physical transfer, cumulative next-turn driver and terminal mapping requirements below are separate authority boundaries from the inert codec/fold. The current private path authenticates a terminal `TerminalSnapshot` after the matching mapped Step and Transition; the row remains proof data and grants no Report delivery or owner reconstruction. Replay Reduce and execution beyond the reached continued turn remain separate gates. A failure Stop records the selected failure after its successful whole receipt without granting terminal delivery.

### 23.1 Admission and start boundary

First implementation is turn0 only, with the actual authenticated Source Agent B, same genuine typed E/runtime, same registered held container, and current checked Target settlement. It never resets TargetAccounting or reuses turn0 proofs to dispatch a later turn. Continue may produce a held next State and the separately checked Observe foundation; production next-turn model/effect remains refused until cumulative accounting/driver joins.

A successful EffectObserved + authenticated OwnedEffectSettlementRecorded + matching whole successful Decision CleanupSettled ends at EffectDecisionReleased (effect_fold.rs:159–195). Only that phase can admit the next ordinary StageReservation(role Reduce, turn0, allowance F==typed E.evaluation_fuel). The original reservation charges full F once and increments original stage count once through the existing ordinary accounting. Owned rows do neither. EffectFailedState/EffectCleanupFailed cannot reserve/evaluate Reduce. No fake Reduce with Outcome reconstructed from result bytes.

Before EffectIntent, the live actor retains one move-only prospective hold for this Reduce F, one original stage slot and full rendered effect→Reduce closure, checked against aggregate ordinary-original/replay plus wait reservation fuel/stage inventory. It is not spent fuel and no refund exists. The matching Reduce reservation ACK consumes the hold exactly once; attempted-write uncertainty quarantines hold and all actual owners under the same lock. An inert record cannot reconstruct the hold or an evaluator allowance. Partial/failed constructors are physical obligations, not snapshots to restore.

### 23.2 Closed bodies

All rows retain exact existing v8 framing/auth/true seq/LF. Digests sha256:lowercase64; coordinates/refs/flags canonical u32; consumed canonical u64 <= the referenced exact F. `plan` equals current B, but exact checked Reduce function/mappings/compiler vectors must additionally match. Unknown/duplicate/missing keys refuse.

| Body kind | Required fields besides kind |
|---|---|
| owned_reduce_staged | turn,attempt,plan,stage_reservation,effect_cleanup_settled,step,step_digest,consumed |
| owned_reduce_cleanup_started | turn,attempt,plan,stage_reservation,effect_cleanup_settled,basis,basis_digest,consumed,operations |
| owned_reduce_cleanup_settled | turn,attempt,started,receipt |
| owned_step_transfer_reserved | turn,attempt,plan,stage_reservation,staged,cleanup,case |
| owned_step_transfer_completed | turn,attempt,reserved,target,transfer_digest |
| owned_effect_failure_state_cleanup_started | turn,attempt,plan,settlement,recorded,decision_cleanup_settled,effect_failure,state_digest,operations |
| owned_effect_failure_state_cleanup_settled | turn,attempt,started,receipt |

**Consumed refinement:** CleanupStarted records actual Reduce consumed also for no-Staged failures. Success must equal its Staged consumed; count the evaluator reservation once, never sum both duplicate observations. This is ACK-observed lower-bound work until complete ordinary stage evidence mapping is checked. It does not convert existing partial inventory into exact terminal total. The closed codec and authenticated fold admit this field.

Step is the exact full nominal Step only after a complete constructor and passed postconditions. Provisional postcondition failure MUST NOT emit Staged. `cleanup` is exactly `{kind:"compiler_empty"}` or `{kind:"observed",started:u32,settled:u32}`. `target` is exactly `{kind:"continue",state:State}`, `{kind:"suspend",state:State}`, `{kind:"complete",report:Report}` or `{kind:"fail",code:i64}`; no nullable fields/extra variants. State/Report/Step use checked nominal IDs and declaration-order fields with existing exact v2 leaf encoding. These are inert data, not owners.

### 23.3 Exact predecessor table

| Current phase | Next legal row | Result / constraints |
|---|---|---|
| EffectDecisionReleased | original Reduce StageReservation | ChargedReduce; observed effect and complete successful Decision receipt required; consumes matching prospective hold |
| ChargedReduce | ReduceStaged | FullStep; exact same reservation/Decision cleanup closure; passed ensures, consumed<=F |
| ChargedReduce | ReduceCleanupStarted(initial/partial/provisional failure) | ReduceCleanupInDoubt; exact actual selected failure + compiler basis/vector; no Staged |
| FullStep | ReduceCleanupStarted(success) | ReduceCleanupInDoubt; only if selected completion vector has active work; basis cites this exact Staged |
| FullStep | TransferReserved(compiler_empty) | TransferInDoubt; independently prove zero active success actions, no fake Cleanup rows |
| ReduceCleanupInDoubt | matching ReduceCleanupSettled | CleanupObserved; exact full ordered receipt, matching Started true seq |
| CleanupObserved(success,whole receipt succeeded) | TransferReserved(observed) | TransferInDoubt; exact Started/Settled/Staged/reservation links |
| CleanupObserved(failure,whole receipt succeeded) | matching ordinary Stop then TerminalSnapshot | FailedTerminal; preserve first selected status; no owner transfer/claim |
| CleanupObserved(unsuccessful observation) | none in this initial producer | Quarantined; preserves existing selected failure if any, no invented cleanup status/Stop/transfer |
| TransferInDoubt | matching TransferCompleted | MappedStep; same actual live map/move once, not generic matching-bytes owner mint |
| MappedStep | matching ordinary Transition | Continue boundary or held terminal boundary; exact frozen mapped-value carrier digest |
| terminal Transition | matching TerminalSnapshot | Terminal held only; complete checked stage/evidence accounting required for success; terminal claim/delivery separately gated |
| Continue Transition | next Observe reservation/TurnObserved | Separate next-turn grammar; cannot admit model/effect until cumulative successor, never retire old K by installing it as new K |
| EffectFailedState | failure State CleanupStarted | FailureStateInDoubt; exact previously selected failed effect + completed Decision receipt; no Reduce |
| FailureStateInDoubt | matching failure State CleanupSettled | FailureStateObserved; receipt against exact active subsequence; Stop/Terminal only if all observations succeeded and selected failure matches |
| FailureStateObserved(unsuccessful observation) | none in this initial producer | Quarantined; no fabricated terminal cleanup success |
| EffectCleanupFailed | none in this initial producer | Quarantined; effect-observation failure does not give State cleanup or ReplaceFailure authority |

The effect Decision receipt references must remain true combined seqs, not projected ordinary indices. New rows never allow legacy owned State cleanup to bypass EffectFailedState. No rearm/replay/effect/transfer/Terminal may cross an unacknowledged cleanup intent. ACKed CleanupStarted recovery is zero-restoration/in-doubt. Any producer Stop before required cleanup ACK is refused; conservative recovered StopInDoubt classification stays separate from producer grammar.

### 23.4 Compiler basis, vectors and failure selection

Closed basis variants (no raw host count):
- `{kind:"initial_failure",status:FailurePair}`: no selected constructor/full Step; exact initial_disposal vector; active flags derive its original vector order.
- `{kind:"partial_failure",status:FailurePair,constructor:ExpressionId,case:DeclarationId,transfer_prefix:[ExpressionId...],active_flags:[u32...]}`: exact compiler constructor under source revision, prefix of its checked field transfer expressions; vector equals failure_by_prefix at that exact prefix. Active flags match the original selected vector. Caller prefix/count cannot attest physical transfer.
- `{kind:"provisional_failure",status:FailurePair,constructor:ExpressionId,case:DeclarationId,active_flags:[u32...]}`: complete actual constructor but failed ensures; vector equals provisional_failure, flags equal completion_live_flags followed by selected result_disposal flags exactly as step.rs:217–238. No successful Staged and no published Step.
- `{kind:"success",staged:u32,constructor:ExpressionId,case:DeclarationId,active_flags:[u32...]}`: exact successful Staged, vector equals completion_cleanup, flags equal compiler completion_live_flags. False guards remain in original vector; do not filter/sort/repair the plan.

FailurePair is frozen `{failure,language_status}`. Nonlanguage closed tags require null language_status. Language failure must match full actual checked NormalizedStatus.to_json on the Reduce proof (including schema/domain/code/class/retryable), not a generic caller status. Cleanup observation failure cannot replace an already selected Reduce/effect/language failure. If cleanup observer fails on otherwise successful Staged, successful transfer is blocked and this initial producer stays quarantined with no Stop/Terminal. There is no existing selected evaluator failure to preserve and no new host-failure tag is invented. A later broader cleanup-failure terminal mapping requires separate contract approval.

Physical prefix/active backing validity must be derived from the sealed live staged obligation and compared to compiler metadata before ACK producer creation. The inert decoder/fold verifies shape/compiler/causal facts only and cannot authorize restoration or release. Pure Context admission requires the closed compiler and causal proof validators. Physical producer and ACK activation additionally require the sealed live lineage; an admitted inert history cannot supply it.

### 23.5 Exact hash/receipt rules

New payloads use recursive lexical JSON-map order, no LF; arrays preserve compiler order:
- Step domain `semaprax.source-agent-owned-reduce.step.v1\0`, payload `{scope,binding:B,plan,turn,attempt,stage_reservation,step}`.
- Basis domain `semaprax.source-agent-owned-reduce.basis.v1\0`, payload `{scope,binding:B,plan,turn,attempt,stage_reservation,basis}`.
- Transfer domain `semaprax.source-agent-owned-reduce.transfer.v1\0`, payload `{scope,binding:B,plan,turn,attempt,reserved,case,mapping,target}`. Mapping is ordered compiler `(source_id,target_id)` pairs; no supplied override.
- Failed-effect `state_digest` is the exact already validated unchanged State commitment retained through Authorize/Ready; failed effect has not run Reduce and cannot replace that digest. Operations are compared in full to the sealed State disposal vector. The authenticated row binds them; these proposed bodies carry no extra operations_digest. Receipt is the exact existing observed receipt JSON, validated against that retained vector and carried directly in the authenticated Settled row (no extra receipt_digest field). No new Operations/Receipt domain or incorrectly shaped reuse of existing RecipeV8::Operations is introduced.

Started.operations is the FULL canonical compiler vector, including false-guard entries, and must equal that original vector independently. Derive the ACTIVE ordered subsequence solely from the sealed compiler basis/actual flags; do not trust a host-selected list. Step settlement skips false guards entirely (step.rs:138–171); it performs no physical operation or observer for them. Settled.receipt uses the existing observed receipt JSON validated against exactly that active subsequence. Its length/order equals the actual active releases, not the full vector; no fake false-guard completed entries are permitted. Capture each actual post-last-owner observer outcome, including mixed panics. The approved physical wrapper catches only observer callback, records that exact outcome, resumes unwind so existing settlement continues; authority checks remain outside both catches. No receipt fabricated from aggregate bool. Missing active operation/outcome or prefix receipt never closes successful cleanup. Unsuccessful observations quarantine both Reduce and failed-effect State cleanup with no fabricated Stop/Terminal success.


### 23.6 Failed-effect status and closure capacity

A failed-effect State cleanup cites the exact authenticated ordinary EffectFailed reason retained at the true combined settlement sequence. HandlerFailed and ResultLimit retain EffectFailed/EffectFailed; no Reduce reservation or phantom evaluator stage is admitted. A producer must separately prove any broader physical failed-effect case before activating it.

For Reduce failures, ReduceFuelExhausted and CallDepthExceeded map to BudgetExhausted/BudgetExhausted. Other admitted checked failures map to Rejected/StageRefused, with a full checked normalized language status where applicable. Cancellation and deadline remain their selected Cancelled/Cancelled or Deadline/Deadline pairs at any future physical producer boundary. No cleanup observation may replace the first selected failure.

Closure room renders exact legal rows from checked vectors and mappings, including authentication, true reference widths and LF. Exclusive branches use maxima; remaining effect-to-Reduce closure propagates backward through authorization and malformed-Proposal retries. The failed-State receipt has no Decision receipt digest. An acknowledged Started row consumes only its own serialized room; the remaining receipt and terminal allowance fit the previously reserved exact byte/row caps. Prospective room does not spend evaluator fuel; the matching original Reduce reservation is charged once by ordinary accounting.

## 24. Actual Ready-only append boundary

The private source actor may derive a live Ready obligation only from its actual successfully Staged Authorize owner, unchanged State, checked Proposal and full Granted Decision. Current policy must admit the exact checked effect operation. The bounded producer requires the compiler's single active Granted seal operation; its source Ready grant and physical target grant use their distinct frozen commitments. Refused and cancelled actors retain their actual staged owner without a Ready append.

The obligation owns the original actor and selected existing OwnedAuthorizationReady row. It binds the actual journal, E/B/scope/generation, original Staged ACK and current pre-Ready prefix. Generic inert Ready metadata remains admitted; it supplies no owner and an intervening append invalidates the live obligation.

The fixed append adapter accepts only this consuming obligation and a session at its exact predecessor. The existing same-FD append, persistence, reread and postguards precede construction of a non-Clone successor witness. The opaque envelope retains the unchanged obligation, actual successor session, selected row, and exact predecessor/successor sequence, byte length and authentication tail. No caller-controlled cursor update, owner replacement, detached ACK or witness extraction is admitted.

Post-append validation uses that sealed successor witness. It rechecks the actual roots, commitments, policy, caught bound clock, cancellation, physical pins and exact successor prefix while the owner's original session remains unchanged. A postwrite refusal quarantines the same container and retains the same owner and successor witness. Earlier failure branches retain the original obligation under the existing append failure classification. Drop drains backing under the held lifetime and supplies no semantic receipt.

This boundary does not promote the actual owner to a physical Ready grant, emit AuthorizationConsumed or EffectIntent, enter a handler, release a seal or publish Outcome. Those successors require the shared container's exclusive prospective Reduce reservation and their own physical ACK/cleanup gates. No decoded history can construct the live obligation or its successor envelope.


## 25. Consuming the actual Ready envelope (private integration boundary)

The fixed Ready append envelope has a consuming private `advance_ready` method. It moves its retained actual obligation, post-Ready session and sealed successor witness directly into the owner phase consumer. It exposes no detached witness, raw owner or tuple of parts. The consumer checks the exact authenticated successor and the same live policy, clock, cancellation, store and original owned State/Decision/K. Only this successful check mints the private one-use Ready promotion permit.

Promotion uses the existing checked Authorize settlement with its compiler-proved empty successful cleanup vector. It moves the same Staged State/Decision roots to the existing physical Ready wrapper without another source evaluation, host call or semantic release. A failure after that move retains the actual Ready wrapper and permanently quarantines the held container; it does not restore Staged or retry settlement.

The resulting move-only obligation selects the existing ordinary AuthorizationConsumed row with the source Ready grant. The distinct physical target grant remains separately pinned. Every later guard failure quarantines the container. This boundary alone supplies no Consumed physical ACK, Prepared effect, prospective Reduce hold, Intent, target entry, Reduce evaluation, public Agent execution or recovery materialization. Those require their own actual owner consumers and executable gates.


## 26. Actual fixed AuthorizationConsumed append (private integration boundary)

The fixed Consumed adapter accepts only the actual move-only obligation from section 25. Its selected ordinary row must match the same journal and unchanged Ready successor sequence and bytes. Candidate validation and the common same-FD append, sync, reread and postguards precede construction of the private Consumed successor witness. The successful envelope retains the unchanged actual obligation first, the actual post-Consumed session and the sealed witness; failures retain the actual owner once.

The core successor validator authenticates that witness against the original Ready lineage and checks the new current Consumed prefix before and after caught clock, cancellation, current policy and actual State/Decision/K checks. It does not use the stale pre-Consumed Ready prefix as a freshness check. Any later validation failure permanently quarantines the held container. A wrong-container preflight performs no append and does not poison either healthy container; same-container stale lineage retires authority.

This boundary provides an actual Consumed ACK with its retained owner. It does not construct a Prepared effect or expose a detached ACK, owner, grant or witness. Exclusive prospective Reduce funding, Intent authority and later owner phase consumers remain required before target entry.

## 27. Exclusive prospective Reduce hold (private integration boundary)

Only the actual verified Consumed envelope may reserve the future Reduce stage. Acquisition authenticates the current physical prefix and binds its exact sequence, byte length, authentication tail, turn and attempt. It checks the immutable evaluation fuel against the stage limit, adds that fuel to the existing reserved total without charging wait fuel again, checks one remaining stage, and checks the remaining closed branch byte and row allowance. No evaluator stage is spent by this prospective check.

The shared journal retains one private reservation identity. All callbacks precede the final callback-free insertion, which rechecks vacancy, append activity and checked identity advancement. The move-only held envelope retains its actual owner before the same reservation guard. Generic advancing writes are refused before candidate creation and at the physical append guards while the reservation exists; read-only sessions remain permitted. This packet adds no reservation-aware write route.

Validation checks container pointer identity before reading a foreign container, then authenticates fresh physical history and the exact retained registry identity, prefix, fuel and coordinates. It rechecks funding and closure room after owner callbacks. Every validation failure permanently quarantines the owning container. Dropping the reservation also quarantines it and never refunds, clears or replaces its credit. Restoring truncated bytes cannot revive authority.

These checks do not construct a Prepared effect, emit Intent, enter a target or perform Reduce. A later consuming owner transition must retain this exact guard on every success and failure path. Matching phase ACKs and the original durable Reduce ACK require their own closed advancement and single-debit gates. Inert histories and registry metadata cannot manufacture a live reservation or owner.

## 28. Consuming held authorization into Prepared (private integration boundary)

The held Consumed envelope has one private consuming delegate. It moves the actual authorization owner, successor session, sealed Consumed witness and same prospective Reduce hold directly into the fixed owner consumer. No unheld envelope, decoded row, detached ACK or tuple-of-parts accessor supplies this route.

The consumer checks actual session container identity and authenticated Consumed lineage, same immutable execution and binding, checked Proposal, distinct source and physical target grant commitments, exact true combined references, current policy, caught bound clock, cancellation and fresh reservation guard. Only that live consumer constructs the private borrowed authorization permit. The interpreter constructs its authorization ACK from the actual Ready owner and this sealed permit; adjacent numeric references alone carry no authority. Preparation reuses the existing checked owner transition without another source evaluation, target call or semantic release.

Every result retains the same reservation after its actual owners and backing. Pre-handoff failure retains the original authorization obligation; interpreter preparation failure retains its actual Ready or Prepared rejection owner. A failure during post-preparation guards retains Prepared, permanently quarantines the container and cannot rewind Ready. Successful Prepared publication repeats fresh owner, clock, prefix and reservation checks; any later failure also quarantines. The owning gate injects cancellation, deadline and panic during the actual post-preparation handoff, independently of later published-owner failures.

This route remains private and provides no EffectIntent ACK, target entry, settlement, Reduce evaluation, terminal cleanup, public Agent route or recovery materialization. Those require consuming physical ACK boundaries and continued retention of this exact reservation.

## 29. Actual Intent selection and borrowed preflight (private integration boundary)

The actual Prepared wrapper may consume itself to select the existing ordinary EffectIntent row from its genuine checked operation and physical request digest. Selection compares the existing typed request renderer and retains the same State, Decision, namespace, Proposal, distinct grants, journal and exclusive Reduce hold. It performs no append, target call, evaluator work or semantic release. Selection or live-guard failure retains that actual Prepared owner and hold under quarantine.

Only borrowing this actual obligation constructs the private fixed append permit. Its full preflight checks the retained live lineage outside any physical lease borrow. Its separate prefix check borrows a typed inventory, excludes synthetic contexts, matches actual container and immutable context identity, original Consumed registry identity, cursor, authentication, fuel and coordinates, and rechecks funding and remaining room without file reads, clock callbacks or lease reborrows. Operation and request authority still come from the actual selected obligation and checked candidate path. This pure helper grants no generic write eligibility or ACK.

The existing engine dispatcher now shares its original ACK checks and owner conversion through a zero-target activation helper. Its existing entry guard, handler, accounting and accepted projection remain in their original order. Test-only ACK fixtures retain their compatibility role and supply no source physical ACK authority. The actual fixed Intent append, authenticated registry advance and physical source activation remain required before the selected source obligation may enter a target.

## 30. Actual held Intent append and zero-call activation

The fixed Intent adapter consumes the actual Prepared owner's selected obligation and the same exclusive future-Reduce hold. Its common physical funnel retains Pending, verified append bytes, and the armed append marker together. The private Intent successor is constructed only after same-file durable verification and Pending acknowledgement. Actual-session comparison and the exact hold registry transition run without callbacks while the marker remains armed; fresh owner, policy, clock, and hold guards run after marker completion. Generic appends remain denied while that hold exists.

The consuming post-Intent route validates the newly acknowledged session and successor, then hands the actual Prepared owner into effect activation with closed Consumed/Intent references. Activation performs no target call or physical release. Rejections retain the actual owner before the same hold; uncertain or lost-authority lineage is quarantined without retry or restoration of the old phase. The owning tests exercise the real append/ACK route. Target dispatch, settlement, recovery, and the public Agent migration remain separate unfinished requirements.

## 31. Actual consuming dispatch and retained initial accounting

The consuming live Activated route borrows its sealed current Intent permit internally and moves the actual Staged owner, all four accounting dimensions, and the same Intent lineage and exclusive hold into the dispatched successor. No caller supplies an accounting ledger or a guard closure. Successful actual first-Intent activation establishes the private initial ledger once, after checking the genuine turn-zero prefix; it is never reset at dispatch. Future turns require that ledger to move through Continue and cumulative authenticated settlement verification before first-effect admission is expanded.

The legacy ACK dispatcher and the actual live adapter call one shared dispatcher body. Entry guards precede the host call, the target primary failure precedes exit-guard failure selection, and accepted result projection follows the successful exit guard. Host failure, panic, cancellation, or later authority loss retains the actual owner, evidence, accounting charges, and same hold under quarantine. Dispatch grants no Decision release, successful Outcome, Reduce debit, terminal delivery, or recovery authority.

## 32. Actual target settlement and Recorded ACKs

The consuming dispatched successor selects its ordinary EffectObserved or admitted EffectFailed row from the actual target observation and sticky failure. The fixed settlement adapter retains the actual Staged owner, all four accounting dimensions, and the same exclusive hold through the verified durable ACK. Its consuming Recorded selector uses the existing checked mapper to bind exact live evidence and result bytes, operation, request, authorization, and true Intent/settlement references. Only successful fixed ACKs advance the same registry into Settlement and Recorded phases; these transitions reserve no new credit and debit no future-Reduce credit.

Each private successor witness is constructed only after same-file append verification and Pending acknowledgement. Callback-free phase comparison and advancement occur under the append marker; fresh phase-specific owner checks run after it clears. Uncertainty or stale same-container lineage retains the actual owner and accounting under poison, and no phase restoration enables retry. These adapters do not authorize Decision cleanup, mint Outcome, activate Reduce, deliver a terminal result, or restore owners from history. Those consuming routes remain required successors.

## 33. Authenticated accounting prefix foundation

The authenticated inventory route builds an incremental checked accounting prefix bound to the borrowed exact Context, generation, source bindings, true causal references, prefix length, and MAC boundary. Each Recorded exchange is checked against preceding checked exchanges before it extends the prefix. The final proof is available only after complete fold validation and the lease postguard; pure entry checking publishes no authenticated accounting history. This proof carries no execution, host, cleanup, or owner-restoration authority.

The target accounting checker reuses actual request reservation, result charging, and remaining-total calculations for all four dimensions. Current production admission still permits only the first effect in turn zero. Two real target exchanges in component tests do not establish cumulative source execution. In particular, a post-host cumulative result-budget failure whose evidence lacks a verifiable raw-byte charge is refused; accounting deltas cannot supply that missing commitment. Actual ledger transfer through Continue, later-turn grammar, and complete cumulative support remain required work.


## 34. Inert facts from the actual reducer stage

The existing physical reducer stage retains the evaluator allowance and exact consumed fuel captured at its sole evaluator call. Rejected physical cleanup preserves those observations with the same actual holder; no reset, refund, or caller override is supplied.

A read-only stage borrower checks the same helper, binding, process/store authority, physical root provenance, canonical pending actions, and liveness flags before projecting descriptive facts. A full Step is available only after actual successful constructor and postcondition evaluation. Initial, partial-transfer, and provisional failures retain their actual selected status and compiler cleanup basis without becoming a full Step. Descriptive staged coordinates carry no ACK authority. These facts neither expose an owner nor permit physical release, original Reduce admission, Step transfer, terminal publication, or recovery. The actual joined producer routes and their executable gates remain required.


## 35. Actual Decision cleanup and Outcome handoff

The consuming Recorded obligation retains its actual Staged effect first, the invocation accounting ledger, and the same exclusive future-Reduce hold. Its fixed CleanupStarted and CleanupSettled append routes require exact selected rows, causal references, authenticated predecessor bytes, and the same physical container. Only the verified same-file append, sync, reread, and Pending ACK permit private witness construction and callback-free registry advancement under the append marker. Witnesses cannot be detached or constructed from replayed rows.

Before CleanupStarted, clock, deadline, cancellation, current policy, process, and owner checks all apply. Immediately after its true ACK, incurred physical Decision cleanup and receipt settlement continue with the exact phase, registry, process, policy, and owner checks, without a fresh clock/deadline callback or cancellation rejection. This narrow exception preserves the existing physical cleanup contract; it does not admit another effect, Outcome, or Reduce. Canonical physical operations and actual observation receipts remain unchanged, and a selected failure remains sticky.

Outcome handoff restores the full clock, deadline, and cancellation guards before and after its one-use mint. Failure before mint retains the actual PendingReceipt; failure after mint retains the actual Executed State and Outcome. Neither path rewinds to a Released predecessor. Failed target or cleanup-observer paths retain the actual failed-State obligation for required later State cleanup; they cannot activate Reduce.

Successful handoff supplies an actual Executed owner with the same ledger and hold. Original Reduce reservation and evaluation, physical Step settlement and transfer, cumulative turns, terminal cleanup and publication, and durable recovery remain unfinished successors. This private route is not public Agent lifecycle acceptance.


## 36. Actual original Reduce reservation and evaluation

The consuming Executed State and Outcome select the original full-F Reduce reservation from the same invocation Context and authentic successful Decision cleanup tail. The fixed append route keeps those actual roots, all four accounting dimensions, and the exclusive prospective Reduce hold together. It admits no caller fuel override, generic stage reservation, owner getter, reconstruction, or second host call.

Only same-file durable verification and Pending acknowledgement construct the private reservation successor. Callback-free acknowledged-session comparison spends the same exclusive hold under the append marker, matching exact R+F and S+1. After marker completion, a fresh charged-prefix guard checks current authority before entering the existing source reducer with exactly F. Replay and prior wait fuel stay reserved; no credit is refunded or reset.

The sole existing reducer evaluation returns the actual Staged holder, its observed consumed fuel and descriptive full-Step or selected-failure facts, with the same accounting ledger and Reduce lineage. Pre-prepare rejection retains Executed; post-prepare guard failure retains Prepared; evaluator or post-evaluation failure retains Staged. An actual language failure remains selected while authority errors are reported separately. No failed ACK enables evaluation or retry.

Focused tests compare successful Continue and Complete values and consumed fuel with independent ordinary evaluation, preserve actual backing ownership, inject all four physical ACK failures, execute real source fuel/arithmetic/postcondition failures, and check owner retention at each guard boundary. These private foundations grant no physical Step release or transfer, cumulative execution, terminal result, recovery, or public Agent acceptance; those successors and integrated verification remain required.


## 37. Actual Step cleanup, field transfer and frozen Transition

The successful actual Staged Reduce owner selects its compiler cleanup basis and complete canonical operation vector. The fixed same-file Started ACK enables only those physical active operations; the existing physical release produces ordered actual observations. Settled, TransferReserved, TransferCompleted and ordinary Transition each require an exact selected predecessor and retain the same invocation accounting and charged Reduce hold without another debit.

A compiler-empty successful basis skips Started and Settled. Its distinct sealed origin binds the actual Staged predecessor; it cannot invent an observed cleanup or a zero sequence. Transfer moves the existing Step fields into the mapped State or Report, preserving their actual Bytes backings, and publishes only the frozen ordinary Transition carrier after the complete typed transfer has passed verification.

Incurred cleanup and its receipt use the narrow cancellation/deadline exception; field transfer and Transition restore full current guards. A selected language failure remains sticky, and every rejected append or interrupted operation retains its actual boundary owner. Failed source evaluation does not fabricate a full Step. Genuine owning tests cover Continue, Complete, compiler-empty Complete, ordered physical observations, guards and physical persistence faults. Integrated runtime verification remains required; these private joins do not implement cumulative execution, terminal result publication, durable recovery or public Agent acceptance.


## 38. Actual failed-target State cleanup

After an actual failed target and completed Decision cleanup receipt, the retained failed owner selects its compiler-derived State result disposal vector. Only its fixed same-file Started ACK authorizes the existing physical release; the ordered actual receipt selects Settled and the sticky ordinary EffectFailed Stop. Observer failure cannot enter this target-failure route. No Outcome, Reduce, retry or new target dispatch is admitted.

The same actual State, accounting ledger and charged Reduce hold remain bound throughout. All four accounting dimensions remain unchanged. Failed or uncertain ACKs quarantine the owner; unsuccessful physical receipts cannot publish Stop. Genuine owning tests cover handler failure, result limit, ordered release, cancellation, wrong binding and persistence faults. Integrated runtime verification remains required; public lifecycle acceptance and terminal publication remain unfinished.

Immutable checked Reduce proof is retained once per invocation Context. Its accessor verifies the exact binding and helper identity. Capacity and reservation borrow that proof while recomputing current prefix and physical guards; no physical authority or current-prefix capacity verdict is cached. Unsupported proof errors retain their original deferred failure boundary.

## 39. Explicit cumulative profile and descriptive turn carry

The private cumulative initialized profile requires both an independently selected immutable checked Context and one exact `OwnedContinuationProfileSelected` row immediately after `OwnedRunCreated`. Its fixed version is `semaprax.source-agent-owned-wait.cumulative.v1`; the row repeats the existing authenticated invocation iteration ceiling. Missing, duplicate, misplaced, unknown, mismatched, or default-context profile selection refuses. No failed cumulative parse falls back to the frozen first-turn grammar.

A descriptive next State commitment is admitted only after the same turn's exact successful Step transfer and frozen Continue Transition. It must be the immediately next combined sequence, increment the checked turn by exactly one below the authenticated ceiling, and equal the compiler-mapped State with its exact argument and cleanup commitments. Only then are old wait, proposal, Decision, effect, original/replay stage bases and per-turn observations retired. Total R, S, recorded consumption, wait fuel and complete ordinary history remain cumulative. Generic producer append cannot grant this next-State authority. Actual owner transfer requires its separate fixed live producer.

The authenticated accounting builder extends the existing four-dimension exchange chain only in the selected cumulative Context; final complete fold validation also checks the profile and Continue lineage before exporting its proof. Pure carrier checking uses preceding checked exchange facts for arithmetic validation but exports no authenticated history. Later target exchange, original Reduce and failure cleanup coordinates use the current admitted turn; the legacy first-turn constructors retain their turn-zero refusal.

Capacity forecasting includes the profile row, checked maximal closure of every remaining turn and retained State cleanup at the iteration ceiling, with checked byte/row multiplication and no increase to frozen global limits. Known terminal or failed branches carry no future turn room. All new positive-turn Reduce templates use the authenticated maximal coordinate width; default capacity templates remain unchanged.

The cumulative forecast retains only the immutable maximal fresh-turn byte/row pair. Its key binds every context input used by that forecast and the exact checked Agent and Reduce proof objects; the checked Reduce identity is revalidated on each access. Initialization and cumulative-profile selection clear the cache. Each invocation still recomputes remaining turns, checked multiplication, retained State closure, current-prefix capacity and physical guards. No inventory, ACK, remaining allowance or successful capacity verdict is reused. Uncached-parity, capacity-edge, changed-bound, crossed-proof and profile-reset tests are supplied; execution and any speedup remain unverified.

Effect capacity also retains three constant byte/row aggregates for the fixed Intent, maximal settlement alternative and Recorded rows. These are computed once through the unchanged canonical serializers from frozen schema constants and maximal coordinate/payload widths. The retained values contain no input, inventory, physical proof or capacity verdict. Each call still derives and validates the compiler-dependent Started row and receipt, adds the current after-effect closure, and checks the actual document and entry counts. The fixed-prefix parity and dynamic-cleanup refusal regression is supplied but not yet run; no timing improvement is claimed.

This is profile and inert admission machinery plus the actual initializer's profile ACK. It supplies no physical continuation from JSON, owner reconstruction, second live Intent, terminal delivery, public Agent route or crash recovery. Complete actual ledger/owner carry, Observe consumed evidence and public execution remain required before acceptance.


## 40. Actual Continue State and original Observe handoff

The actual compiler-mapped Continue State selects the next canonical State commitment through a fixed same-file ACK. The same accounting ledger and exclusive spent Reduce hold remain attached; the commitment preserves R/S and advances the checked turn below the invocation iteration ceiling. Authored Task.budget is language data, not a host iteration ceiling.

The next original Observe reservation has exactly the invocation allowance. Its authentic ACK adds that F and one stage once to the same hold registry, before the sole consuming Observe evaluation. The actual observed or failed holder retains the observed budget consumption and original State backing. Every rejected append retains its actual phase owner; uncertain persistence and post-ACK guard failures quarantine without evaluation retry. Genuine owning tests compare actual Copy results and consumption against independent ordinary Observe and prove failed ACKs never enter the evaluator. This handoff stops at the opaque actual observed/failed holder. It does not implement a second target exchange, terminal publication or public lifecycle acceptance.

## 41. Irreversible observer cleanup seal foundation

A complete actual failed Decision observer receipt ACK, while ordinary authority is still healthy, can atomically install a private cleanup-only seal bound to the actual retained State, journal identity, cursor/MAC, policy, invocation ledger and same Reduce hold. Partial Decision release cannot create it. Ordinary poison remains permanent and cannot be cleared.

The seal has separate monotonic retirement. Expected ordinary Poisoned refusal does not retire it; any later actual fault, pin/prefix loss or seal drop retires it permanently, even if bytes or namespace pins are restored. All physical fault writes pass through a retirement-aware private poison carrier with no raw setter. Owning controls exercise actual receipt installation, ordinary refusal and pin-loss restoration with unchanged prefix and State backing. This foundation supplies no State cleanup writer, Stop or completion route.


## 42. Actual cumulative Observe settlement producer

The explicit cumulative profile requires OwnedObserveSettled immediately after its original Observe reservation, before the matching ordinary TurnObserved. The closed observed branch binds the actual Copy Observation, State commitment and sole evaluator consumption. The failed branch binds the actual failed State, compiler phase and nullable language status. Consumption is added once within the original allowance. Default profile rows remain unchanged.

Initial and continued producers retain the actual engine holder, validate its root/schema/exclusive provenance and acknowledge both successful rows through fixed append witnesses. The same accounting and Reduce hold survive continuation; settlement adds no new reservation or stage. A failed append or current guard refusal retains the actual boundary holder and irreversibly quarantines ordinary authority. Successful settlement renews the registry cursor only after the authentic ACK. Generic producer admission remains closed.

The failed grammar requires canonical State cleanup and the exact full original Observe failure receipt before its sticky Stop; it rejects host receipts and skipped consumption. This packet retains the failed physical holder; section 45 defines its subsequent State cleanup writer. Owning tests compare real results, language failure and consumed charge with independent ordinary Observe, and exercise all persistence boundaries and post-ACK cancellation. Runtime verification is pending. No second exchange, terminal publication, crash recovery or public lifecycle acceptance is claimed.


## 43. Actual Decision-observer terminal State cleanup

The capacity phase regression distinguishes the successful Decision cleanup's
Reduce reserve from the failed Decision observer's own State cleanup reserve.
The historical equality between them predates this separate closure. Its
replacement pins both fixture bounds, checks the observer bound against typed
authenticated Started/Settled widths plus terminal allowance, and exercises
legal cleanup edges, premature or wrong-status Stop refusal, and a failed State
observer's zero continuation room. This strengthened regression is unrun; no
capacity formula or production authority is changed by the correction.

The private cleanup seal from a complete failed Decision observation receipt can select a distinct State cleanup Started row, bound to that actual receipt and retained State. Its fixed durable ACK authorizes the original compiler State disposal vector. The actual ordered physical receipt selects Settled; only successful complete observations and its true ACK can select the original sticky Stop. Failed target causes retain EffectFailed; a successful target with failed Decision observation uses the distinct observer cause and Rejected/StageRefused.

The same State, four-dimensional ledger and Reduce hold remain attached. Normal poison stays permanent; only the cleanup seal advances, and all later faults retire it permanently. Incurred release/receipt may finish under the narrow cancellation/deadline exception, preserving store/PID/binding/policy checks; before Started and Stop all full guards apply. Partial or failed State observation cannot publish Stop. Actual owning tests include all sticky target causes, guard loss, persistence faults, wrong journal, reminted joins and pin restoration. Integrated runtime verification remains pending; no public terminal publication or recovery is supplied.

## 44. Retained successful continuation preparation foundations

An actual successful continued Observe holder and both authentic settlement/TurnObserved ACKs can retain an opaque next-wait boundary with unchanged State, accounting, same hold and authenticated lineage. Failed or incomplete settlement retains its actual owner for cleanup. The engine foundation can move that exact observed State into the existing helper preparation, retaining original allocation provenance and causal context. This preparation runs no source evaluator, model, target or grant. Actual source-to-engine handoff, fixed next-Start/Prepared ACK transport and cumulative authorization/dispatch remain required. Owning foundation tests are unrun until the central gate; public multi-exchange acceptance remains unfinished.


## 45. Actual failed-Observe State cleanup writer

Only an actual failed initial or continued Observe holder with its authentic OwnedObserveSettled ACK can select State cleanup. Started is bound to the actual failure, State provenance, original full Observe failure-cleanup vector and exact retained prefix. Its fixed durable ACK incurs one physical release through the existing failed-Observe settlement primitive. Preallocated canonical slots capture each actual observer outcome; the returned physical receipt and all action outcomes must agree. Partial release, capture mismatch or later guard loss retains the actual boundary holder and permanently quarantines it. No retry, synthetic aggregate receipt or restored owner is admitted.

The receipt row references the actual Started row index. After release, guards use the actual released engine holder and descriptive facts cached before release, never the old State root. Only a full successful receipt and its true Settled ACK can select the sticky Stop: Fuel/Depth exhaustion keeps BudgetExhausted; other admitted Observe failures select Rejected/StageRefused. Full current guards apply before Started and Stop. Incurred release and receipt may finish after cancellation or clock expiry while retaining physical store, PID, schema, exclusivity and applicable policy checks. Initial cleanup adds no policy/clock or accounting authority; continued cleanup retains the unchanged ledger and same Reduce hold without debit, refund or reservation reset.

The generic append classifier denies all State cleanup and Stop spellings at a checked failed cumulative Observe prefix. Only sealed actual-owner permits can advance this writer.

The capture and actual initial/continued writer harnesses include canonical mixed observer outcomes, persistence faults at all three row boundaries, cancellation, pin loss/restore, reminted history and owner lifetime. Their central runtime gate remains pending. This private writer supplies no public terminal publication, recovery delivery or second exchange. Source integration and review are separate from runtime acceptance.


## 46. Actual continued Start and Prepared transport

A successful continued Observe owner and both authentic settlement ACKs may prepare the next helper using the same actual State, four-dimensional ledger, Reduce hold and authenticated lineage. Only fixed Created and full-F Start reservation ACKs permit entry into the existing next-wait evaluator. Entry moves the actual owner once; entered Parked or Terminal results retain real evaluator consumption and accounting. Refusal before evaluator entry retains its owner and accounting with no observed evaluator consumption. Cancellation before either ACK boundary cannot enter or replay source work.

The checkpoint encoder borrows the actual retained container through its matching permit. Prepared is derived from the actual parked observation and original Start allowance. Its row uses the actual reservation index and next sequence; a sealed successor exists only after the same-descriptor write, synchronization and reread ACK. Registry advancement preserves the same hold identity and accounting, retires the previous reservation guard, and installs the actual Prepared prefix only after that ACK. Wrong containers, persistence faults and stale guards retain their actual owner and permanently retire ordinary authority where incurred.

Owning tests cover genuine parked results and consumption against the ordinary evaluator, cancellation around the durable boundaries, all Prepared persistence faults and no premature evaluation or replay. Central runtime verification remains pending. This private route supplies no next SDK exchange, target authorization, public multi-exchange acceptance or recovery delivery.

## 47. Actual continued model settlement and Resume transport

Only the actual continued Prepared carrier may select the next model request. Its sealed origin binds the retained helper, current authenticated prefix and the same spent Reduce token. The fixed AttemptIntent ACK precedes entry into the existing SDK dispatcher. The initial request keeps its frozen turn-zero recipe; continued requests retain their actual turn and observation without copying an owned State root.

The SDK dispatcher checks current physical provenance, journal witness and registry phase before work and after every external callback. Cancellation and clock admission apply to new provider and Resume work. An incurred failed SDK settlement retains its selected primary failure and usage through narrower physical, process, execution, policy and parked-helper checks; cancellation cannot erase its failed settlement record. Panic catches retain the actual owner outside the catch boundary.

Actual AttemptSettled/AttemptFailed and AttemptUsage ACKs precede binding decoded proposal data. That data is inert and cannot manufacture an owner, accounting proof or execution permit. Accounting comes from the existing authenticated inventory builder and binds exact Context, execution, binding, generation, prefix bytes, row count and MAC. A candidate's successor proof becomes current only at the true durable ACK. Synthetic inventories carry no proof. All four carried accounting dimensions agree with the prior authenticated settlement.

The original full-F Resume reservation ACK precedes the sole existing helper Resume evaluation. A successful actual terminal State selects Completed through its fixed ACK. Refusal, partial failure, terminal failure or guard loss retains the actual boundary owner, consumption, accounting and same spent token. This route introduces no Answered wire row, new authorization, target dispatch, token renewal, public loop, terminal publication or recovery owner reconstruction.

Six owning tests cover the real offline SDK/Resume/Completed route, persistence uncertainty at all five ACK phases and all four physical windows, callback panic and post-poll cancellation, combined cancellation with physical or policy loss, and exact accounting-proof binding. Physical fault controls require the retained InDoubt variant rather than accepting a precursor refusal. Central runtime verification remains pending; the second target exchange and full public lifecycle criterion remain unfinished.

## 48. Actual continued Transfer and Authorize transport

Only an actual successful continued Completed owner may select ProposalAdmitted and State transfer. The fixed TransferReserved ACK precedes the existing consuming physical move; TransferCompleted describes that actual move. The original full-F Authorize reservation ACK precedes the sole checked authorization evaluation. Only that reservation debits F and one stage reservation; all four carried accounting dimensions and the same spent registry token remain intact.

The actual full Granted or Refused Decision selects OwnedAuthorizationStaged through its fixed durable ACK. A complete Refused outcome has no source failure. Partial or failed source evaluation retains its actual resulting holder, selected cause, measured consumption and accounting under permanent quarantine. No DTO or ordinary evaluator result may reconstruct the live holder. The ordinary evaluator is used only as an independent result, status and consumption oracle.

New phase-specific witnesses and guards replace the old model cursor after each true ACK. Initial and continued authorization reuse the existing evaluator and transfer protocol. Strict cancellation, deadline, physical provenance, policy and callback checks remain necessary before new source work; incurred source failure retains actual custody rather than granting a retry.

Four owning tests cover full Granted/Refused results and exact consumption, all five ACK phases at four physical fault windows, Requires/Ensures/Arithmetic/Fuel failures after actual source entry, and strict pre-entry plus post-evaluation guards. They remain unrun until the central gate. This private route supplies no Ready, token renewal, second target dispatch, Refused cleanup tail, public terminal publication or recovery reconstruction; those remain required for the full lifecycle criterion.


## 49. Actual continued Decision cleanup

An actual continued Recorded holder can select the next Decision cleanup only
by borrowing its own retained Decision through the current settlement permit.
The selected row uses the current positive turn, exact Staged/Ready/Consumed,
Intent/settlement/Recorded references, and the unchanged compiler disposal
vector and digest recipes. Preparation moves the same live holder into an
opaque obligation; snapshots and evidence cannot construct it.

The existing fixed cleanup append adapter now admits that obligation. Only its
actual same-descriptor Started ACK advances the same prospective Reduce hold
from the cumulative Recorded phase. It neither refunds nor debits fuel, stage
reservations or target accounting. The old settlement cursor becomes historical.
That fixed ACK permits the existing physical Decision release once. Its actual
receipt selects the unchanged Settled row through the same adapter. The State,
accounting ledger and same hold remain retained throughout, including every
failed append, partial release and failed observer result. Release failure
permanently quarantines the holder; it cannot return to dispatch or retry release.

New work before Started requires the existing full settlement guards. Incurred
release and receipt retain store/PID/source/policy checks while allowing
cancellation or deadline expiry. An observer panic is captured by the existing
ordered release primitive and recorded as a failed receipt, never a successful
Outcome. These checks add no recovered-owner constructor or second journal.

Focused owning tests cover genuine second-target success, host failure and
result-budget failure, exact compiler vectors and coordinates, cancellation,
observer panic, and both cleanup ACK rows at all four physical fault windows.
Verification remains incomplete. The focused selector compiled at `f16519438`
with the default test-thread stack, one build job, debug information disabled
and incremental compilation disabled. Before the cumulative holders were boxed,
the first test aborted with stack overflow. After boxing, the six-test selector
was interrupted after 12 minutes of runtime without a completed test; the exact
single-fixture cancellation-refusal test was then interrupted after 9 minutes
without a result. No behavioral gate is claimed passed. The full quality profile
was not run under the bounded-test instruction.

The private driver now composes this exact pair from a real continued
Recorded holder: it writes Started, invokes the explicit cleanup observer once,
and writes Settled. Its failure carrier keeps the Prepared, Started, released,
or append owner that actually reached the failed boundary. A post-release
failure therefore has no route to invoke cleanup again. The focused real-chain
selector passed 1/1 locally, asserting one physical cleanup observation and both
durable rows. This supplies neither a public entry nor restart path.

The actual successful Settled ACK now also binds a one-use physical
State/Outcome handoff. A live permit checks the exact continued turn, append
session and Settled witness against the retained source hold, runtime,
execution, policy, clock and cancellation. The existing consuming effect
cleanup ACK transfers the pending holder into its executed turn only after
those checks. The reached owner stays quarantined on failure, with no route
back to Decision release. The focused Outcome selector passed 1/1 locally for
success and fresh cancellation, with one physical release and unchanged
journal bytes. This is private transport, not public recovery authority.

The private continued Reduce sibling requires that minted Outcome on the exact
Settled holder before selecting one
positive-turn `StageReservation(role Reduce)` with the retained typed fuel. Its
fixed same-file ACK moves the existing hold from `CleanupSettled` to
`SpentReduce` exactly once, preserving the turn-1 lineage, owner, MAC cursor
and accounting. Prewrite, ACK and post-ACK failures retain their unique reached
owner and quarantine uncertainty; receipt bytes never grant this authority.
The reserved holder now consumes the actual executed Outcome once into the
existing checked reducer. A fresh spent-hold, source, policy, clock, and
cancellation guard remains valid after that physical owner moves. Pre-entry,
evaluation, and post-entry failures retain their reached owners and quarantine
uncertainty. The private real-chain selector passed 1/1 locally, reaching full
Step facts with unchanged accounting, one durable Reduce reservation row, and
no second host call or cleanup observation.

The evaluated turn-1 holder can now select the exact Step facts and append the
first durable Step ACK through the fixed Step permit. The successor retains the
same physical owner and spent Reduce hold; the row gives no cleanup, result
move, or public continuation authority. A prewrite fault retains that owner
and quarantines the append path. The focused real-chain success and prewrite
selectors each exercise one actual continued turn locally.

The acknowledged staged owner can next select `OwnedReduceCleanupStarted`
from the same evaluated cleanup basis and append it through the fixed Step
permit. Its ACK marks cleanup as incurred and retains the owner and spent
hold. The post-ACK guard still checks physical provenance, source, plan, and
policy; cancellation or clock expiry can no longer erase incurred cleanup.
This row does not execute physical cleanup or publish a result. The focused
real-chain success and prewrite-refusal selectors each passed locally; the
fault retains the staged owner and quarantines the append lease.

From that actual incurred Started holder, the private continued Step now
passes the existing physical cleanup engine its sealed owner-bound permit,
releases the compiler's selected cleanup vector once, and appends the actual
`OwnedReduceCleanupSettled` receipt. A prewrite fault on the receipt append
retains the released owner and cannot repeat cleanup. The focused success and
receipt-prewrite real-chain selectors each passed locally. Result transfer,
terminal publication, public entry, and restart recovery remain separate.

The same receipt owner now reserves and physically moves the real turn-one
Step fields, then ACKs `OwnedStepTransferCompleted` and the matching terminal
`Transition`. The real-chain transfer selector passed locally. For a terminal
mapped Step, a private successor derives `TerminalSnapshot` from the
authenticated ordinary execution projection and the actual Step carrier;
the fixed Step append retains the mapped owner through its ACK. The fold
accepts this row only after the matching terminal Transition and checks its
turn, status, carrier digest, committed accounting and canonical evidence.
The terminal success and prewrite-refusal selectors passed the central gate.
Evidence input remains descriptive and cannot mint an owner or ACK. A separate
read-only projection reopens the same registered store and authenticates its
terminal prefix before returning checked status, evidence and carrier bytes;
the actual store-reopen success and prewrite-refusal selectors passed. This
projection returns no physical State or Report owner.

A separate later-Continue holder takes the actual mapped turn-one Step after
its Continue Transition ACK. It selects turn-two `OwnedStateCommitted` from
that State, then ACKs the exact turn-two Observe reservation before the
physical Observe engine runs. The fixed append shares the existing candidate,
pending and same-file write/reread path; its spent hold registry remains
cumulative, and failures retain the phase owner. The owning three-turn success
and State-prewrite refusal selectors passed the central gate.

The private Complete terminal owner now has a consuming Report claim after the
exact `TerminalSnapshot` ACK. Claim verifies the current registered prefix,
Complete status, turn, carrier bytes and digest against the live mapped Step,
then moves the original physical Report into a claimed owner retaining its
store borrower. A borrowed delivery projection is checked again against the
terminal carrier; stale store authority drains backing without asserting
semantic result disposal. Transition alone returns the same unclaimed owner.
The owning success and terminal-prewrite refusal selectors passed locally;
this route remains private, and
the recovered terminal evidence still cannot recreate a physical Report.

The later physical Observe owner can select `OwnedObserveSettled` from its
actual State and Copy result, then append `TurnObserved` through the fixed
Observe settlement writer and cumulative hold registry. A rejected append
keeps the physical Observe owner and its exact reservation under quarantine.
The focused turn-two success and prewrite-refusal selectors passed locally.
The actual two-ACK observed owner can then carry the physical State and checked
Copy observation into turn-two `OwnedWaitCreated` and original Start
reservation. Both rows use the fixed ContinuedStart candidate, pending writer,
same-file reread and cumulative hold registry; a refused append retains the
later owner. The focused Start success and prewrite-refusal selectors passed
locally. A later two-ACK Start owner now has a consuming source-entry join:
it validates the current original Start reservation, cumulative hold, policy
and clock, then moves the same physical State through the existing continued
wait preparation and source helper. The returned private owner retains the
actual Parked outcome and original observation and ACKs. This source-entry
join passed its owning success and Start-prewrite refusal selectors locally.
The later Parked owner now selects a checkpoint from its physical State and
request under the original Start ACK, then offers the exact
`OwnedWaitPrepared` row to the existing fixed writer and cumulative hold.
The later Prepared ACK retains the same Parked owner and accounting; a
prewrite refusal keeps that owner under quarantine. Its owning success and
refusal regressions passed locally. The acknowledged later Prepared owner
now supplies a borrowed physical State, request, observation and exact
ordinal to the checked Model request builder. Its ordinary `AttemptIntent`
uses the existing fixed Model writer and cumulative hold; the retained owner
validates the exact ACK before any SDK dispatch. The owning Intent success
and prewrite-refusal regressions passed locally. The same acknowledged
turn-two owner now dispatches the checked request through the existing SDK
guard, selecting one ordinary `AttemptSettled` or `AttemptFailed` row from
the actual response. Its fixed writer ACKs that row under the cumulative
hold, while prewrite refusal retains the dispatched owner. The owning
settlement success and refusal gates passed locally. The same physical
turn-two owner can now select the SDK's reported `AttemptUsage`, ACK it through
the fixed Model writer, bind the decoded Proposal, and ACK one full-fuel
`OwnedWaitReserved` Resume row. Prewrite refusals retain the owner at each
boundary. The owning Usage and Resume reservation success and prewrite-refusal
gates passed locally. The later Resume ACK now has a consuming physical join:
a closed lineage permit checks the exact original full-fuel reservation and
current registered store before the shared interpreter resumes its actual
Parked owner. Refused, terminal and guard-lost outcomes retain their physical
owners. Only successful Resumed State can select `OwnedWaitCompleted`, using
its observed consumption and State-bound result digest. The fixed Model writer
retains that same State after its Completed ACK or prewrite refusal. The owning
physical Resume success and Completed-prewrite refusal selectors passed locally
(1/1 each). The later Effect/Reduce/Step join is specified in section 50;
later failed-Observe cleanup still requires its owner join.

Public multi-turn entry, public Report delivery and physical owner restoration
after restart remain
unfinished. This bounded local result is not completion of the
public owned-Agent lifecycle criterion.


## 50. Private later Completed owner join (#330)

The turn-two Completed holder has one consuming join into the existing
continued authorization, effect, Decision cleanup, Reduce and staged Step
pipeline. Its live successful Resume owner moves with the checked Proposal,
actual request and ordinal, Prepared/Observe/Start history, and all five Model
acknowledgements. The join does not append a row or reenter Resume.

Before moving, the join validates the retained Resume witness against its
acknowledged session, the Completed witness against that exact predecessor's
cursor and authentication, and the current Completed prefix. It recomputes the
Completed row from the actual Resumed State, checked Proposal, wait identity
and observed fuel, then compares the cumulative accounting. A mismatched wait
or other provenance failure retains the reached owner and quarantines the
journal. History and matching bytes cannot construct the physical owner.

The continuation lineage distinguishes the first Step from the retained actual
later Step. Its borrowed origin selects that owner's physical Reduce hold,
policy, cancellation and clock; it cannot substitute the first turn's hold or
reset accounting. The existing fixed authorization and effect writers retain
all reservation, ACK, transfer and host-entry guards. First authorization ACK
failure retains the same State, and successful advancement reaches one physical
effect dispatch, one Decision cleanup, one original Reduce reservation and the
actual owned Step ACK. The later Step terminal successor and its separate
executable gate are specified in section 51.

The focused gate is:

```sh
cargo test --locked -p semaprax --lib owned_continued_step_turn_two -- --nocapture
```

It includes physical Resume/Completed preservation, the actual turn-two
Effect/Reduce/Step chain, a foreign-wait refusal with unchanged persisted bytes,
and first-authorization prewrite refusal retaining State. These new join cases
have focused local success and foreign-wait refusal passes (1/1 each, 817.26
and 511.94 seconds). The authorization-prewrite refusal remains pending
execution. Public multi-turn entry, Report
delivery, restart restoration, broader iteration and native/Wasm owned-wait
support remain separate completion requirements.


## 51. Private later Complete terminal and Report closure (#330)

The physical turn-two Complete Step has a private consuming closure through
its existing six fixed ACK boundaries: compiler cleanup Started, actual
cleanup receipt, field-transfer reservation, completed field transfer,
ordinary Transition, and TerminalSnapshot. Entry checks the same journal,
current live owner and full Complete Step before the first append. Each
successor retains the same spent Reduce hold, cumulative accounting and
original physical fields. The closure does not supply fresh fuel or repeat
Model, Resume, authorization, effect, or Reduce execution.

The cleanup engine visits the compiler's canonical vector once. A failed
receipt cannot yield Ready or result transfer. Every failed selection, session,
physical append, ACK advancement, cleanup or field move returns its actual
reached owner, with its selected failure and already incurred release intact.
There is no driver retry path. In particular, receipt prewrite refusal retains
the released Step, and terminal prewrite refusal retains the mapped Report;
neither can claim delivery or derive a replacement physical owner from rows.

Only the authentic terminal ACK enables the existing consuming Complete Report
claim. Its borrowed delivery projection checks the live Report against that
exact terminal carrier. The success fixture also reopens the registered store
and authenticates terminal evidence after dropping the physical holder; this
recovered evidence remains descriptive and cannot restore a Report owner.

The focused gate is:

```sh
cargo test --locked -p semaprax --lib owned_continued_step_turn_two_terminal_report -- --test-threads=1
```

The three owning cases cover original Report retention and claim, cleanup
receipt prewrite refusal, and terminal prewrite refusal. They assert one
physical cleanup, exact cumulative funding and turn, no terminal evidence on
refusal, and no cleanup retry on drop. Focused local success and terminal
prewrite refusal passed 1/1 each on 2 October 2026 (811.35 and 808.97
seconds). The nominal-case mapping regression passed 1/1. The receipt
prewrite case and repository full quality profile remain unrun. This is a
private Complete successor, not public multi-turn
entry, public Report delivery, failed-Observe cleanup, or physical owner
restoration after restart. Broader iteration and native/Wasm owned-wait support
remain separate completion requirements.
