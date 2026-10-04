use super::*;

const EFFECT_ACCOUNTING_SCHEMA: &str = "semaprax.source-live-cli.repair-effect-accounting.v1";

/// Projects terminal execution facts the source-journal validator has already
/// bound to this invocation. The journal remains the accounting owner.
fn effect_accounting(
    checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    model_dispatches: u32,
    effect_dispatches: u32,
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
        "nonclaims": [
            "journal_accounting_is_not_provider_delivery_or_cost_proof",
            "effect_counts_do_not_describe_external_side_effect_completion",
            "receipt_projection_grants_no_effect_or_publication_authority",
        ],
    }))
}

pub(super) fn receipt(
    config: &RepairConfig,
    preview: Option<&semaprax::agent_runtime_v2::OfflineRepairPreview>,
    candidate_test_evidence: Option<&CandidateTestEvidence>,
    replayed_candidate_test_evidence: Option<ReplayedCandidateTestEvidence>,
    candidate_test_selected: bool,
    receipt_context: Option<&RepairReceiptContext>,
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
        report["runtime_effect_accounting"] =
            effect_accounting(checkpoint, model_dispatches, effect_dispatches)?;
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
