use super::*;

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
