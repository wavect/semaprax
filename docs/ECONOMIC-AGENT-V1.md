# Economic Agent v1

Private Economic Agent v1 A+B is exact-head hosted green at fe75c38d898b71e3ed5c57411fb46d0dbd4fc34b in run 31611748969, including both Economic gates on Ubuntu, macOS, and Windows. Public C local evidence is green; exact-head hosted promotion is pending.
This changes none of the 38 Partial/18 Missing totals.

## Authority boundary

Economic Agent v1 consumes only a completed, already-replayed Agent Runtime
result whose untrusted final message is a canonical Payment Intent. The model
cannot approve, sign, broadcast, widen policy, or mint wallet authority. A
separate injected approver binds the exact Policy, Intent, Plan, Simulation,
and Approval Request. Opaque injected custody receives only the approved
unsigned transaction and digest bindings; keys and credentials never enter the
runtime, Trace, Evidence, or diagnostics.

The public injected-host API admits only native assets on Sepolia EIP-1559 type-2,
Solana devnet System Program transfers, Bitcoin regtest P2WPKH PSBT v2, and an
x402 invoice overlay over one of those rails. It includes no built-in HTTP,
DNS, chain node, journal, approver, custody, signing key, filesystem, process,
environment, mainnet, token, contract, arbitrary program/script, swap, bridge,
refund, or automatic rebroadcast authority.

## Canonical state and execution

The exact v1 documents are Policy, Payment Intent, x402 Invoice, Chain
Snapshot, Payment Plan, Simulation, Approval Request, Approval, Journal,
Broadcast Receipt, Reconciliation, Trace, and Evidence. Each is compact
canonical UTF-8 JSON with one terminal LF, exact key order, closed types,
bounded depth, checked decimal integers, exact domain-separated digests, and
independent replay. Journal broadcast and reconciliation fields retain bounded
`schema,digest,bytes,document` capsules so restart reconciliation can recover
the exact transaction identity without another authority read.

Execution loads the journal once, atomically reserves the exact rolling-24h
policy window with the first Journal CAS, obtains a frozen rail snapshot,
builds and independently decodes the unsigned transaction, simulates, obtains
separate approval, signs once, persists the signed binding before broadcast,
broadcasts at most once, and reconciles one observation. Uncertain broadcast is
persisted and is never retried automatically. Standalone reconciliation
requires the same sealed Agent result and can never sign or broadcast.

The one decreasing 64 MiB builder budget covers canonical inputs, retained
state, adapter sinks, journal candidates, Trace, Evidence, and replay. Before
each injected authority call, cancellation, deadline, policy, output,
continuation, Journal, Trace, Evidence, and builder capacity are required to
fit. Once an external effect is attempted, every operational exit must return
canonical replayed Trace/Evidence and the admitted Journal transition is
attempted according to the state machine.

## Evidence gates

The configured focused gate is:

```sh
cargo test --locked -p semaprax --lib economic_agent::tests -- --nocapture
```

It must pass with deterministic fake journals, adapters, approvers, and custody
on Ubuntu, macOS, and Windows before the private hosted claim. Required gates
also include every document KAT and mutation, independent chain byte vectors,
x402 SSRF/path hostiles, exact/+1 limits, rolling-window concurrency, every
adapter disposition, cancellation/deadline boundaries, no-retry restart and
process-termination evidence, replay mutation, secret/no-write inventory,
full workspace tests, strict Clippy, rustdoc, formatting, and package/external
consumer checks. Test-network names are encoding namespaces; CI performs no
live node, faucet, credential, or external-network request. Process-kill gates
prove OS process termination and journal replay, not power-loss durability.

The additive public C surface now exposes only the opaque injected-host dialect
documented below. Its local evidence is green; hosted promotion remains pending
an exact-head 12/12 run with the public gate on all three host operating systems.

## Exact canonical wire ledger

All documents use compact UTF-8 JSON followed by exactly one LF. Objects are
closed and preserve the following key order; arrays have the documented
semantic order and decimal integers are JSON `u64` without signs, fractions,
or exponents.

- Policy, `semaprax.economic-agent-policy.v1`:
  `schema,economic_agent_id,wallet_id,network_policies,x402_origins,limits,nonclaims`.
- Payment Intent, `semaprax.economic-agent-payment-intent.v1`:
  `schema,intent_id,wallet_id,rail,idempotency_key,created_at_ms,expires_at_ms,memo,payment`.
  EVM payment keys are `kind,network,asset,recipient,amount_atomic,max_fee_atomic`;
  Solana adds `max_compute_units,max_priority_fee_atomic`; Bitcoin adds
  `confirmation_target`; x402 uses
  `kind,origin,method,resource,invoice_digest,payee,settlement_rail,network,asset,amount_atomic,max_fee_atomic,invoice_expires_at_ms,invoice_nonce`.
- x402 Invoice: `schema,origin,method,resource,invoice_id,payee,settlement_rail,network,asset,amount_atomic,max_fee_atomic,expires_at_ms,nonce,idempotency_key`.
- Chain Snapshot: `schema,rail,network,observed_at_ms,expires_at_ms,state`.
  The EVM state is `chain_id,from,nonce,base_fee_per_gas,max_priority_fee_per_gas,gas_limit`;
  Solana is `fee_payer,recent_blockhash,last_valid_block_height,lamports_per_signature`;
  Bitcoin is `wallet_script_pubkey,height,fee_rate_sat_vbyte,utxos`, with each
  sorted UTXO `txid,vout,value_atomic,script_pubkey,confirmations`.
- Payment Plan: `schema,run_id,source_agent_evidence,policy,intent,x402_invoice,chain_snapshot,rail,network,asset,wallet_id,recipient,amount_atomic,max_fee_atomic,unsigned_transaction,expires_at_ms`.
- Simulation: `schema,plan,success,fee_atomic,balance_before_atomic,balance_after_atomic,allowance_atomic,units,expires_at_ms`.
- Approval Request: `schema,run_id,wallet_id,rail,network,asset,recipient,amount_atomic,max_fee_atomic,origin,method,resource,policy,intent,plan,simulation,expires_at_ms`.
- Approval: `schema,approval_id,approver_id,policy,intent,plan,simulation,approval_request,decision,approved_amount_atomic,approved_fee_atomic,expires_at_ms`.
- Journal: `schema,idempotency_key,version,policy,intent,run_id,state,reserved_amount_atomic,reserved_fee_atomic,plan,simulation,approval,unsigned_transaction,signed_transaction,broadcast,reconciliation,updated_at_ms`.
- Broadcast Receipt: `schema,rail,network,signed_transaction_digest,transaction_id,disposition,observed_at_ms`.
- Reconciliation: `schema,rail,network,transaction_id,status,observed_at_ms,observed_height,confirmations,canonical_block_id`.
- Trace: `schema,run_id,source_agent_evidence_digest,policy_digest,intent_digest,events,result,nonclaims`.
  Events are `index,kind,rail,input_digest,output_digest,status,usage`; usage is
  `journal_reads,journal_writes,invoice_reads,snapshot_reads,simulations,approvals,signatures,broadcasts,reconciliations,input_bytes,output_bytes,elapsed_ms`.
- Evidence: `schema,run_id,source_agent,policy,intent,x402_invoice,plan,simulation,approval,journal,broadcast,reconciliation,trace,result,limits,budget,nonclaims`.

Document digest domains are
`semaprax.economic-agent.{policy|payment-intent|x402-invoice|chain-snapshot|payment-plan|simulation|approval-request|approval|journal|broadcast-receipt|reconciliation|trace|evidence}-digest.v1\0`.
Unsigned and signed transaction domains end in
`unsigned-transaction-digest.v1\0` and `signed-transaction-digest.v1\0`.
The run-ID domain is `semaprax.economic-agent.run-id.v1\0` and binds the
source Agent Evidence digest, Policy digest, Intent digest, and exact
idempotency bytes.

## Exact limits, budget, and durable topology

Limits occur in this order:
`max_policy_bytes,max_intent_bytes,max_invoice_bytes,max_snapshot_bytes,max_plan_bytes,max_simulation_bytes,max_approval_request_bytes,max_approval_bytes,max_journal_bytes,max_unsigned_transaction_bytes,max_signed_transaction_bytes,max_broadcast_receipt_bytes,max_reconciliation_bytes,max_trace_events,max_trace_bytes,max_evidence_bytes,max_builder_bytes,max_json_depth,max_identifier_bytes,max_memo_bytes,max_recipients,max_network_policies,max_x402_origins,max_utxos,max_reconciliations,max_elapsed_ms,max_amount_atomic,max_fee_atomic,max_compute_units,max_confirmation_target,max_concurrency,max_unexpected_authority_calls`.
Production maxima are respectively
`1048576,1048576,1048576,1048576,1048576,1048576,1048576,65536,8388608,1048576,2097152,1048576,1048576,1024,8388608,16777216,67108864,16,128,1024,128,16,32,100,64,600000,1000000000000000000,1000000000000000,200000,144,1,0`.

Budget keys are
`used_policy_bytes,used_intent_bytes,used_invoice_bytes,used_snapshot_bytes,used_plan_bytes,used_simulation_bytes,used_approval_request_bytes,used_approval_bytes,used_journal_bytes,used_unsigned_transaction_bytes,used_signed_transaction_bytes,used_broadcast_receipt_bytes,used_reconciliation_bytes,used_trace_events,used_trace_bytes,used_evidence_bytes,used_builder_bytes,used_recipients,used_network_policies,used_x402_origins,used_utxos,used_reconciliations,used_elapsed_ms,used_concurrency,used_unexpected_authority_calls`.

Fresh durable versions are exact: v1 Reserved, v2 Prepared, v3 Approved,
v4 Approved as the durable sign-attempt marker, v5 Signed, v6
BroadcastUnknown with the runtime provisional receipt (`unknown`, observed
time zero), and v7 an actual adapter receipt with positive observed time.
Custody and broadcast are never retried after their markers. Reconciliation
uses base B=6 for a provisional receipt and B=7 for an actual receipt. For
`offset=version-B`, persisted attempts are `(offset+1)/2`; even offsets are
between attempts and odd offsets are durable attempt markers. An odd marker
surviving termination consumes its attempt. v1 may resume without reserving
again; v2-v5 fail closed without further authority; v6 and later are
reconcile-only. The configured maximum of 64 observations is cumulative over
restarts.

## Exact nonclaims

Policy, Trace, and Evidence carry this ordered list:

1. `no_model_output_payment_authority`
2. `no_model_self_approval_or_policy_expansion`
3. `no_seed_private_key_credential_or_signing_material_input`
4. `no_secret_prompt_trace_evidence_log_or_diagnostic_exposure`
5. `no_builtin_network_http_dns_custody_or_chain_authority`
6. `no_mainnet_authority`
7. `no_wildcard_network_asset_recipient_origin_or_resource`
8. `no_token_contract_program_script_swap_bridge_or_unlimited_approval`
9. `no_raw_signing_or_signed_transaction_export`
10. `no_exactly_once_signing_broadcast_or_payment`
11. `no_automatic_uncertain_broadcast_retry`
12. `no_guaranteed_confirmation_finality_or_reorg_freedom`
13. `no_compromised_wallet_approver_adapter_provider_or_chain_recovery`
14. `no_power_loss_durability_without_host_journal_contract`
15. `no_cross_process_or_distributed_concurrency_guarantee`
16. `no_live_price_exchange_rate_fee_or_cost_accuracy`
17. `no_balance_allowance_or_simulation_truth_beyond_adapter`
18. `no_human_identity_intent_approval_provenance_or_nonrepudiation`
19. `no_signature_attestation_or_custody_provenance`
20. `no_tax_accounting_legal_regulatory_sanctions_or_compliance_correctness`
21. `no_privacy_data_residency_or_unlinkability_guarantee`
22. `no_x402_redirect_ssrf_private_network_or_server_honesty_guarantee_beyond_admitted_adapter_contract`
23. `no_automatic_refund_chargeback_replacement_or_fee_bumping`
24. `no_wallet_recovery_rotation_backup_or_inheritance`
25. `no_general_payment_sdk_or_production_readiness`
26. `no_language_graph_cleanup_backend_or_workspace_atomicity_semantics`
27. `no_current_agent_runtime_schema_api_or_kat_modification`
28. `no_completion_matrix_status_promotion`

## Schema literals and diagnostics

The 13 schema literals, in document order, are:

```text
semaprax.economic-agent-policy.v1
semaprax.economic-agent-payment-intent.v1
semaprax.economic-agent-x402-invoice.v1
semaprax.economic-agent-chain-snapshot.v1
semaprax.economic-agent-payment-plan.v1
semaprax.economic-agent-simulation.v1
semaprax.economic-agent-approval-request.v1
semaprax.economic-agent-approval.v1
semaprax.economic-agent-journal.v1
semaprax.economic-agent-broadcast-receipt.v1
semaprax.economic-agent-reconciliation.v1
semaprax.economic-agent-trace.v1
semaprax.economic-agent-evidence.v1
```

Exact diagnostic ownership is:

- `SPX-G210`: `Economic Agent {document} is not canonical {schema} JSON`.
- `SPX-G211`: `Economic Agent policy invariant failed: {field}`.
- `SPX-G212`: `Economic Agent payment intent was rejected: {reason}`,
  where reason is one of `agent run not completed`, `wallet mismatch`,
  `rail/network/asset not allowed`, `recipient not allowed`,
  `origin/method/resource not allowed`, `expired`,
  `amount or fee not allowed`, or `idempotency already bound`.
- `SPX-G213`: `Economic Agent prepared transaction or simulation disagrees with the admitted intent`.
- `SPX-G214`: `Economic Agent approval is absent, expired, rejected, or digest-mismatched`.
- `SPX-G215`: `Economic Agent journal state or idempotency replay disagrees with the admitted operation`.
- `SPX-G216`: `{field} exceeds {maximum}`.
- `SPX-G217`: `Economic Agent Trace or Evidence disagrees with the replayed state machine`.
- `SPX-I222`: `Economic Agent journal adapter failed`.
- `SPX-I223`: `Economic Agent chain adapter failed`.
- `SPX-I224`: `Economic Agent approval adapter failed`.
- `SPX-I225`: `Economic Agent custody adapter failed`.
- `SPX-I226`: `Economic Agent broadcast outcome is uncertain`.
- `SPX-I227`: `Economic Agent reconciliation adapter failed`.
- `SPX-I228`: `Economic Agent run was cancelled`.
- `SPX-I229`: `Economic Agent deadline was exceeded`.

## Public injected contract

The public C surface is a visibility-only promotion over the hosted A+B core.
Its exact entry points are
`EconomicAgent::new(policy:&str,host:H,cancellation:AgentCancellation)`,
`execute(&mut self,source:&AgentRun)`, and
`reconcile(&mut self,idempotency_key:&str,source:&AgentRun)`.
`EconomicAgentHost` is the supertrait of `PaymentJournal`,
`X402InvoiceAdapter`, the EVM/Solana/Bitcoin payment adapters,
`PaymentApprover`, and `WalletCustody`, and supplies the pure
`boundary_probe()->Box<dyn EconomicBoundaryProbe>` observation.

`PaymentJournal::load(idempotency_key,sink)` returns exactly `Missing`,
`Present`, `DefinitelyNotStarted`, or `FailedUncertain`.
`compare_and_swap(idempotency_key,expected_version,journal,rolling)` and all
other adapters return `Succeeded`, `DefinitelyNotStarted`, `FailedUncertain`,
or `PolicyRejected`. Rolling is exactly `Reserve(&reservation)`, `Retain`, or
`Release`; the reservation getters expose wallet, rail, network, asset,
requested time, amount, and maximum rolling-24h amount. The journal host
atomically samples its trusted nondecreasing clock, validates requested-time
freshness, expires rows at `now-admitted_at >= 86400000`, checked-sums the exact
wallet/rail/network/asset tuple, and binds the admitted row to the idempotency
key. Release is legal only before any possible custody or broadcast attempt.

Each rail adapter has exact methods
`{rail}_snapshot(intent,sink)`,
`{rail}_simulate(plan,unsigned_transaction,sink)`,
`{rail}_broadcast(signed_transaction,sink)`, and
`{rail}_reconcile(transaction_id,sink)`.
The invoice adapter receives only `origin,method,resource`; the approver only
the canonical Approval Request; custody receives only
`wallet_id,rail,unsigned_transaction_digest,unsigned_transaction,approval_digest`.
Document and byte sinks expose only sticky `push`; they have no public
constructor, content readback, or rejection-reason channel.

The authority sequence is sealed Agent replay, Policy/Intent admission,
one Journal load, v1 rolling Reserve CAS, optional invoice, snapshot, core
build plus independent unsigned decode, simulation and v2 CAS, approval and v3
CAS, v4 sign-attempt CAS, one custody call, v5 Signed CAS, v6 provisional
broadcast CAS, one broadcast call, optional v7 actual-receipt CAS, durable odd
reconcile-attempt CAS, one reconcile call, even completion CAS, and independent
Trace/Evidence replay. Capacity, cancellation, deadline, freshness, and policy
are rechecked immediately before every external call and after every durable
pre-attempt marker. Definitely-not-started is never automatically retried;
uncertainty seals the corresponding boundary. A killed process may leave only
the exact authenticated old state or admitted next state; restart never signs
or broadcasts twice.

## Frozen 13-document fixture ledger

Each row is `document | SHA-256(raw canonical bytes) | domain digest | bytes`:

```text
policy | 57ce4d3844f49c9102eb1a2c17f1946305c623e587d4f52a31744ba96ff6114a | sha256:ee623062817928e0088f24b8215705f9aad8e19a52861db6d6051679889c0b53 | 2987
intent | bfe0695c7e2a5bdfd545b264fb79777cfdadaa449d9089c59753ae3739e36d86 | sha256:2a13c2a14cfafba6b4087e647de9e5609c8bb65ddad25c305aa8f5bc28091e2c | 670
invoice | 38b5b00511f2e461f8df0fe1a830e89109376c893e3c52cea5d23a8d36d8733b | sha256:24cb1025c6beb2a081a05ab504f7d7f6cbb37b27e003da35cbbea003a52ac095 | 417
snapshot | d005d0f573f337d804d80b8489b63a9f6b03099837b230af69e18a4692b4b9eb | sha256:4123e22e7449e4bbcef812af71337f2e3e5390b4cce20e59f7080b74eeb727d0 | 309
plan | 75418fad0967fa4791d9f146f6997af67b3e76f51dcb2320bfbe2211814bde45 | sha256:81dad7aa8e82bdf8ef7b02e2b5a94b899715c86e7d82895c7cdb18c5e7ed28d8 | 1391
simulation | b3508d24fd29028a9fad89703ba72ade9f4e620eec30f0dd2017b711b96db483 | sha256:3a1ac9d741be20bf0d5e35a78e475369a5ccad5c3662bbc5f5365f123df81f1d | 369
approval_request | fc932dcef1eb518ba05f463df9e3dd7193ce96df408edb60f3aaa9214a3f19b9 | sha256:0833d896f4be4e4feb08d0558e43e7512d589a6c96c368c6ce73ef3a8435adf1 | 1056
approval | 48f716162ae5ec67b28303c5e5c09b641a16a1711b7b83bd4d16a1be6094a56c | sha256:63f3e81facdc9e0ece43b28cbd47310b1a7898662cd4afe3abaae24d06eab8db | 1022
journal | f65a8f115c405b086d9a6edb1366a594c87b9f295be8739ea2a56724297f69c9 | sha256:9f3d1f568a090c280cfe645536e912d4eb0c18c740bb7309d434ebfd1d1cb169 | 2394
broadcast | 51479da80d60c4e4c363302963010a6675278e53958ce563dafb2892da3c537f | sha256:64882648d5bac5fb58a7408d38e5fa737ba314d27decba34e2106c2310c651a6 | 335
reconciliation | b1b449375018c27465332384d67205438bea8a2660d3144c417e5de5d5198ba1 | sha256:e26e7655d758b53867228950de241c3265675f18687110550fc89ecdc46f2b4a | 301
trace | a388543ab6c1a57a0b7798fbd0c5d721bb33c0ab7f7123f3bb8f24c4c965db58 | sha256:f28c44894b93948068381bb9047fedade3b855cdd1831a992466a08fa97f6f11 | 11023
evidence | 2d4d4164476bd4fdd037f138b264d0a72728b125d6819baae87da165242788b0 | sha256:9dd80e5a13aaaa02b5b854cee0f68870ac22dfdabca14e79d32857ae35980cc6 | 17399
```
