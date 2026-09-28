# Source Agent owned wait v1 — combined SourceLive v8 contract

Status: **reviewed bounded design; implementation in progress; no runtime or R20 completion claim**. Audience: compiler, interpreter, journal, SDK and typed-runtime implementers and reviewers. The [embedded source Agent syntax](LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md#additive-embedded-execution-metadata-v070-candidate) owns the frontend association. Existing owned-frame v1, Copy model-wait v1/source journal v7, public ordinary event enums and all predecessor bytes remain frozen. Sections 8–14 are normative refinements and supersede conflicting shorthand in earlier sections.

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
