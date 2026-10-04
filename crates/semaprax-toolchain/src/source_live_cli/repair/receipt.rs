use super::*;

const EFFECT_ACCOUNTING_SCHEMA: &str = "semaprax.source-live-cli.repair-effect-accounting.v2";
const PATCH_RECEIPT_POLICY_SCHEMA: &str = "semaprax.patch-receipt-policy.v1";

/// Projects terminal execution facts the source-journal validator has already
/// bound to this invocation. The journal remains the accounting owner.
pub(super) fn effect_accounting(
    checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    model_dispatches: u32,
    effect_dispatches: u32,
    live: Option<&semaprax::agent_lifecycle::iterative::driver::EffectAccounting>,
) -> Result<Value, CliError> {
    let terminal = checkpoint.terminal_snapshot().ok_or(CliError::refused(
        "repair checkpoint has no terminal snapshot",
    ))?;
    let evidence: Value = serde_json::from_slice(terminal.evidence())
        .map_err(|_| CliError::refused("repair terminal accounting evidence is malformed"))?;
    let object = evidence.as_object().ok_or(CliError::refused(
        "repair terminal accounting evidence is not an object",
    ))?;
    let number = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or(CliError::refused(
                "repair terminal accounting evidence has an invalid counter",
            ))
    };
    if object.get("schema").and_then(Value::as_str)
        != Some("semaprax.agent-source-terminal-evidence.v2")
        || object.get("invocation").and_then(Value::as_str) != Some(checkpoint.invocation())
        || object.get("status").and_then(Value::as_str) != Some(terminal.status().as_str())
    {
        return Err(CliError::refused(
            "repair terminal accounting evidence has an unexpected binding",
        ));
    }
    let effects = number("effects")?;
    let attempts = number("attempts")?;
    let stages = number("stages")?;
    let committed_stage_fuel = number("committed_stage_fuel")?;
    let committed_model_units = object
        .get("committed_model_units")
        .and_then(Value::as_i64)
        .ok_or(CliError::refused(
            "repair terminal accounting evidence has an invalid committed budget",
        ))?;
    if effects < u64::from(effect_dispatches) || attempts < u64::from(model_dispatches) {
        return Err(CliError::refused(
            "repair dispatch counters exceed validated terminal accounting",
        ));
    }
    let accounting = match live {
        Some(live) => {
            let dispatched = u64::from(live.dispatched_calls);
            let replayed = u64::from(live.replayed_calls);
            let invocation_calls = dispatched.checked_sub(replayed).ok_or(CliError::refused(
                "repair effect accounting replay calls exceed total calls",
            ))?;
            let invocation_arguments = live
                .argument_bytes
                .checked_sub(live.replayed_argument_bytes)
                .ok_or(CliError::refused(
                    "repair effect accounting replay arguments exceed total arguments",
                ))?;
            let invocation_results = live
                .result_bytes
                .checked_sub(live.replayed_result_bytes)
                .ok_or(CliError::refused(
                    "repair effect accounting replay results exceed total results",
                ))?;
            let total_bytes =
                live.argument_bytes
                    .checked_add(live.result_bytes)
                    .ok_or(CliError::refused(
                        "repair effect accounting total byte count overflowed",
                    ))?;
            let invocation_total =
                invocation_arguments
                    .checked_add(invocation_results)
                    .ok_or(CliError::refused(
                        "repair invocation effect byte count overflowed",
                    ))?;
            let replayed_total = live
                .replayed_argument_bytes
                .checked_add(live.replayed_result_bytes)
                .ok_or(CliError::refused(
                    "repair replay effect byte count overflowed",
                ))?;
            if dispatched != effects
                || invocation_calls != u64::from(effect_dispatches)
                || total_bytes > live.max_total_bytes
                || live.replayed_argument_bytes > live.argument_bytes
                || live.replayed_result_bytes > live.result_bytes
            {
                return Err(CliError::refused(
                    "repair effect accounting does not match the validated terminal journal",
                ));
            }
            json!({
                "status": "complete",
                "effective_limits": {
                    "dispatched_calls": live.max_calls,
                    "argument_bytes_per_call": live.max_argument_bytes,
                    "result_bytes_per_call": live.max_result_bytes,
                    "total_charged_bytes": live.max_total_bytes,
                },
                "cumulative_terminal_journal": {
                    "dispatched_calls": dispatched,
                    "charged_argument_bytes": live.argument_bytes,
                    "charged_result_bytes": live.result_bytes,
                    "charged_total_bytes": total_bytes,
                },
                "this_invocation": {
                    "dispatched_calls": invocation_calls,
                    "charged_argument_bytes": invocation_arguments,
                    "charged_result_bytes": invocation_results,
                    "charged_total_bytes": invocation_total,
                },
                "historical_replay": {
                    "dispatched_calls": replayed,
                    "charged_argument_bytes": live.replayed_argument_bytes,
                    "charged_result_bytes": live.replayed_result_bytes,
                    "charged_total_bytes": replayed_total,
                },
                "terminal_disposition": if live.failure.is_some() { "failure" } else { "settled" },
                "failure_reason": live.failure,
                "uncertain": false,
            })
        }
        None => json!({
            "status": "absent",
            "reason": "terminal_checkpoint_predates_exact_effect_byte_accounting",
            "uncertain": false,
        }),
    };
    Ok(json!({
        "schema": EFFECT_ACCOUNTING_SCHEMA,
        "status": "validated_terminal_journal_projection",
        "terminal_status": terminal.status().as_str(),
        "total_effect_dispatches": effects,
        "total_model_attempts": attempts,
        "total_stages": stages,
        "committed_model_units": committed_model_units,
        "committed_stage_fuel": committed_stage_fuel,
        "this_invocation_model_dispatches": model_dispatches,
        "this_invocation_effect_dispatches": effect_dispatches,
        "replayed_without_dispatch": model_dispatches == 0 && effect_dispatches == 0,
        "effect_budget": accounting,
        "nonclaims": [
            "journal_accounting_is_not_provider_delivery_or_cost_proof",
            "effect_counts_do_not_describe_external_side_effect_completion",
            "receipt_projection_grants_no_effect_or_publication_authority",
        ],
    }))
}

/// Rebinds checkpoint-authenticated terminal charges to the invocation that
/// is reading them. A terminal replay owns no dispatches, so all retained
/// charges are historical even though the original producing invocation was
/// live. The retained sidecar supplies exact bytes; this adapter only changes
/// the descriptive current/historical partition after checking journal facts.
fn retained_effect_accounting_for_invocation(
    checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    model_dispatches: u32,
    effect_dispatches: u32,
    retained: &Value,
) -> Result<Value, CliError> {
    let baseline = effect_accounting(checkpoint, model_dispatches, effect_dispatches, None)?;
    for key in [
        "schema",
        "terminal_status",
        "total_effect_dispatches",
        "total_model_attempts",
        "total_stages",
        "committed_model_units",
        "committed_stage_fuel",
    ] {
        if retained.get(key) != baseline.get(key) {
            return Err(CliError::refused(
                "retained repair effect accounting does not match terminal journal",
            ));
        }
    }
    let mut rebound = retained.clone();
    let budget = rebound
        .get_mut("effect_budget")
        .and_then(Value::as_object_mut)
        .ok_or(CliError::refused(
            "retained repair effect accounting has no effect budget",
        ))?;
    if budget.get("status").and_then(Value::as_str) != Some("complete") {
        return Err(CliError::refused(
            "retained repair effect accounting is not exact",
        ));
    }
    let cumulative =
        budget
            .get("cumulative_terminal_journal")
            .cloned()
            .ok_or(CliError::refused(
                "retained repair effect accounting has no cumulative charges",
            ))?;
    let counters = [
        "dispatched_calls",
        "charged_argument_bytes",
        "charged_result_bytes",
        "charged_total_bytes",
    ];
    if !counters
        .iter()
        .all(|key| cumulative.get(*key).and_then(Value::as_u64).is_some())
    {
        return Err(CliError::refused(
            "retained repair effect accounting has invalid cumulative charges",
        ));
    }
    if effect_dispatches == 0 {
        budget.insert(
            "this_invocation".to_owned(),
            json!({
                "dispatched_calls": 0,
                "charged_argument_bytes": 0,
                "charged_result_bytes": 0,
                "charged_total_bytes": 0,
            }),
        );
        budget.insert("historical_replay".to_owned(), cumulative);
    }
    let object = rebound.as_object_mut().ok_or(CliError::refused(
        "retained repair effect accounting is malformed",
    ))?;
    object.insert(
        "this_invocation_model_dispatches".to_owned(),
        json!(model_dispatches),
    );
    object.insert(
        "this_invocation_effect_dispatches".to_owned(),
        json!(effect_dispatches),
    );
    object.insert(
        "replayed_without_dispatch".to_owned(),
        json!(model_dispatches == 0 && effect_dispatches == 0),
    );
    Ok(rebound)
}

/// Describes only observations already admitted by the authorized repair
/// runtime. Rendering this policy neither runs a candidate test nor invokes an
/// effect; the terminal journal remains the cumulative accounting owner.
fn runtime_receipt_policy(
    accounting: &Value,
    candidate_test_evidence: Option<&CandidateTestEvidence>,
    replayed_candidate_test_evidence: Option<ReplayedCandidateTestEvidence>,
    candidate_test_selected: bool,
) -> Value {
    let candidate_test_execution = match (candidate_test_evidence, replayed_candidate_test_evidence)
    {
        (Some(evidence), None) => json!({
            "status": evidence.status.text(),
            "coverage": "partial_authorized_candidate_test_observation",
            "observation": "present_in_this_invocation_receipt",
        }),
        (None, Some(evidence)) => json!({
            "status": evidence.status.text(),
            "coverage": "partial_replayed_candidate_test_feedback_only",
            "observation": "not_retained_in_terminal_journal",
        }),
        (None, None) => json!({
            "status": "absent",
            "coverage": "not_observed",
            "reason": if candidate_test_selected {
                "no_settled_candidate_test_observation_is_available"
            } else {
                "this_repair_host_has_no_candidate_test_execution_authority"
            },
        }),
        (Some(_), Some(_)) => json!({
            "status": "conflicting",
            "coverage": "invalid",
        }),
    };
    json!({
        "schema": PATCH_RECEIPT_POLICY_SCHEMA,
        "check_profile": "authorized_repair_runtime_observations",
        "evidence_selection_scope": "settled_candidate_test_observation_and_validated_terminal_journal",
        "effect_accounting_scope": "validated_terminal_journal",
        "coverage": {
            "candidate_test_execution": candidate_test_execution,
            "runtime_effects": {
                "status": accounting["effect_budget"]["status"].clone(),
                "coverage": "validated_dispatch_and_charged_byte_accounting_not_effect_completion",
                "this_invocation": {
                    "model_dispatches": accounting["this_invocation_model_dispatches"].clone(),
                    "effect_dispatches": accounting["this_invocation_effect_dispatches"].clone(),
                },
                "cumulative_terminal_journal": {
                    "model_attempts": accounting["total_model_attempts"].clone(),
                    "effect_dispatches": accounting["total_effect_dispatches"].clone(),
                },
                "replayed_without_dispatch": accounting["replayed_without_dispatch"].clone(),
                "budget_and_charges": accounting["effect_budget"].clone(),
            },
        },
        "execution": false,
        "source_authority": false,
        "publication_authority": false,
        "nonclaims": [
            "receipt_generation_did_not_execute_candidate_tests_or_effects",
            "dispatch_accounting_is_not_external_effect_completion_or_provider_delivery_proof",
            "receipt_policy_grants_no_test_effect_or_publication_authority",
        ],
    })
}

pub(super) fn receipt(
    config: &RepairConfig,
    preview: Option<&semaprax::agent_runtime_v2::OfflineRepairPreview>,
    candidate_test_evidence: Option<&CandidateTestEvidence>,
    replayed_candidate_test_evidence: Option<ReplayedCandidateTestEvidence>,
    candidate_test_selected: bool,
    receipt_context: Option<&RepairReceiptContext>,
    terminal_patch_receipt: Option<&TerminalPatchReceipt>,
    checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    model_dispatches: u32,
    effect_dispatches: u32,
) -> Result<String, CliError> {
    let terminal = checkpoint.terminal_snapshot().ok_or(CliError::refused(
        "repair checkpoint has no terminal snapshot",
    ))?;
    let mut report = json!({
        "schema": RECEIPT_SCHEMA_V1,
        "target": config.target,
        "status": terminal.status().as_str(),
        "generation": checkpoint.generation(),
        "model_dispatches": model_dispatches,
        "effect_dispatches": effect_dispatches,
        "source_mutation": false,
        "publication_authority": false,
    });
    if matches!(
        &config.provider,
        RepairProvider::OpenCode | RepairProvider::Claude
    ) {
        let receipt_context = receipt_context.ok_or(CliError::refused(
            "repair V2 receipt has no checked profile context",
        ))?;
        report["schema"] = json!(if matches!(config.provider, RepairProvider::Claude) {
            RECEIPT_SCHEMA_V3
        } else {
            RECEIPT_SCHEMA_V2
        });
        report["selected_profile"] = json!({
            "config_schema": if matches!(config.provider, RepairProvider::Claude) { CONFIG_SCHEMA_V3 } else { CONFIG_SCHEMA_V2 },
            "provider_id": receipt_context.provider_id.as_str(),
            "model_id": receipt_context.model_id.as_str(),
            "adapter_identity": receipt_context.adapter_identity.as_str(),
            "adapter_version": receipt_context.adapter_version.as_str(),
            "provider_profile": receipt_context.provider_profile.as_str(),
        });
        report["checked_prerequisites"] = json!({
            "program_root": receipt_context.program_root.as_str(),
            "source_revision": receipt_context.source_revision.as_str(),
            "proposal_schema_digest": receipt_context.proposal_schema_digest.as_str(),
            "deployment_binding": receipt_context.deployment_binding.as_str(),
        });
        report["model_attempts"] = Value::Array(
            semaprax::model_call_receipt::source_projection::project_source_calls(checkpoint)
                .map_err(|_| CliError::refused("repair model-attempt projection refused"))?
                .into_iter()
                .map(|attempt| {
                    checked_value(
                        &attempt.render(),
                        "repair model-attempt receipt projection refused",
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        report["journal_binding"] = json!({
            "invocation": checkpoint.invocation(),
            "chain": checkpoint.chain(),
            "generation": checkpoint.generation(),
        });
        let runtime_effect_accounting = terminal_patch_receipt
            .and_then(TerminalPatchReceipt::runtime_effect_accounting)
            .map(|retained| {
                retained_effect_accounting_for_invocation(
                    checkpoint,
                    model_dispatches,
                    effect_dispatches,
                    retained,
                )
            })
            .transpose()?
            .unwrap_or(effect_accounting(
                checkpoint,
                model_dispatches,
                effect_dispatches,
                None,
            )?);
        report["runtime_effect_accounting"] = runtime_effect_accounting.clone();
        report["receipt_policy"] = runtime_receipt_policy(
            &runtime_effect_accounting,
            candidate_test_evidence,
            replayed_candidate_test_evidence,
            candidate_test_selected,
        );
        report["patch_receipt"] = terminal_patch_receipt
            .map(TerminalPatchReceipt::value)
            .transpose()?
            .unwrap_or(Value::Null);
        report["candidate_test_execution"] =
            match (candidate_test_evidence, replayed_candidate_test_evidence) {
                (Some(evidence), None) => json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "status": evidence.status.text(),
                    "feedback_code": evidence.feedback_code,
                    "replayed": false,
                    "observation": checked_value(
                        &evidence.canonical,
                        "repair candidate-test observation refused",
                    )?,
                }),
                (None, Some(evidence)) => json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "status": evidence.status.text(),
                    "feedback_code": evidence.feedback_code,
                    "replayed": true,
                    "observation": Value::Null,
                }),
                (None, None) => json!({
                    "status": "not_run",
                    "reason": if candidate_test_selected {
                        "no settled candidate-test observation is available"
                    } else {
                        "this repair host has no candidate test-execution authority"
                    },
                }),
                (Some(_), Some(_)) => {
                    return Err(CliError::refused(
                        "repair candidate-test receipt has conflicting observations",
                    ))
                }
            };
    }
    if let Some(preview) = preview {
        report["candidate_digest"] = json!(preview.candidate().candidate_digest());
        report["source_review"] =
            checked_value(preview.source_review(), "repair source review refused")?;
        report["semantic_delta"] =
            checked_value(preview.semantic_delta(), "repair semantic delta refused")?;
        report["impact_summary"] =
            checked_value(preview.impact_summary(), "repair impact summary refused")?;
        if matches!(
            &config.provider,
            RepairProvider::OpenCode | RepairProvider::Claude
        ) {
            let candidate_test_ran =
                candidate_test_evidence.is_some() || replayed_candidate_test_evidence.is_some();
            let mut blind_spots = vec![
                Value::String(
                    "no publication, Git mutation, or physical delivery is authorized by this receipt"
                        .to_owned(),
                ),
                Value::String(
                    "provider usage is an observation, not cost or delivery proof".to_owned(),
                ),
            ];
            if !candidate_test_ran {
                blind_spots.insert(0, Value::String(if candidate_test_selected {
                    "candidate tests were not observed: no settled candidate-test observation is available"
                        .to_owned()
                } else {
                    "candidate tests were not executed: this host has no test-execution authority"
                        .to_owned()
                }));
            } else if replayed_candidate_test_evidence.is_some() {
                blind_spots.insert(
                    0,
                    Value::String(
                        "candidate-test observation is replayed from the durable journal; no new test was executed"
                            .to_owned(),
                    ),
                );
            }
            report["analysis"] = json!({
                "coverage": {
                    "source_review": true,
                    "semantic_delta": true,
                    "impact_summary": true,
                    "candidate_test_execution": candidate_test_ran,
                },
                "blind_spots": blind_spots,
            });
        }
    } else {
        report["candidate_digest"] = Value::Null;
        report["source_review"] = Value::Null;
        report["semantic_delta"] = Value::Null;
        report["impact_summary"] = Value::Null;
        if matches!(
            &config.provider,
            RepairProvider::OpenCode | RepairProvider::Claude
        ) {
            let replayed_candidate_test = replayed_candidate_test_evidence.is_some();
            report["analysis"] = json!({
                "coverage": {
                    "source_review": false,
                    "semantic_delta": false,
                    "impact_summary": false,
                    "candidate_test_execution": replayed_candidate_test,
                },
                "blind_spots": [
                    "terminal checkpoint replay did not create or revalidate a candidate",
                    if replayed_candidate_test_evidence.is_some() {
                        "candidate-test observation is replayed from the durable journal; no new test was executed"
                    } else if candidate_test_selected {
                        "no settled candidate-test observation is available"
                    } else {
                        "candidate tests were not executed: this host has no test-execution authority"
                    },
                    "no publication, Git mutation, or physical delivery is authorized by this receipt",
                ],
            });
        }
    }
    serde_json::to_string(&report)
        .map(|report| format!("{report}\n"))
        .map_err(|_| CliError::refused("repair report cannot be rendered"))
}
