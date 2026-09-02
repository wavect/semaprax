use super::*;
fn selected(policy: VNextPolicy) -> Value {
    let methods = super::super::methods(&policy, false);
    let capabilities = capabilities(&methods, &policy, false);
    bundle(
        &methods
            .iter()
            .map(|method| descriptor(method, &policy))
            .collect::<Vec<_>>(),
        &capabilities,
    )
    .unwrap()
}
#[test]
fn dependency_query_is_read_only_with_closed_chunks_and_opaque_facts() {
    let bundle = selected(VNextPolicy::default());
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "image/dependencies")
        .unwrap();
    assert_eq!(method["capability"], "semantic_read");
    assert_eq!(method["query"], true);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
    let chunk = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.image-declaration-dependencies-chunk.v1")
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(chunk["properties"]["source_authority"]["const"], false);
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .contains(&json!("target")));
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .contains(&json!("image_revision")));
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-declaration-dependencies.v1")));
}
#[test]
fn cleanup_dependencies_are_v5_read_only_with_closed_chunks_and_generated_clients() {
    let bundle = selected(VNextPolicy::default());
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "image/cleanup-dependencies")
        .unwrap();
    assert_eq!(method["capability"], "semantic_read");
    assert_eq!(method["query"], true);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 4);
    assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
    assert_eq!(params["required"], json!(["image_revision", "target"]));
    let chunk = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.image-cleanup-dependencies-chunk.v1")
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(chunk["properties"]["source_authority"]["const"], false);
    assert_eq!(
        chunk["properties"]["report_schema"]["const"],
        crate::project::IMAGE_CLEANUP_DEPENDENCIES_SCHEMA
    );
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .contains(&json!("image_revision")));
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .contains(&json!("target")));
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-cleanup-dependencies.v1")));
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("image/cleanup-dependencies"));
        assert!(source.contains("semaprax.image-cleanup-dependencies-chunk.v1"));
    }
    for test_enabled in [false, true] {
        assert!(
            !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                .iter()
                .any(|method| method.name == "image/cleanup-dependencies")
        );
    }
}
#[test]
fn candidate_cleanup_dependencies_require_candidate_grant_and_exact_target_binding() {
    let readonly = selected(VNextPolicy::default());
    assert!(!readonly["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"] == "candidate/cleanup-dependencies"));
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    });
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/cleanup-dependencies")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], true);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 5);
    assert_eq!(
        params["required"],
        json!(["image_revision", "candidate_revision", "target"])
    );
    assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
    let chunk = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| {
            document["$id"] == "urn:semaprax.image-candidate-cleanup-dependencies-chunk.v1"
        })
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(chunk["properties"]["source_authority"]["const"], false);
    assert_eq!(
        chunk["properties"]["report_schema"]["const"],
        crate::project::PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA
    );
    for field in ["image_revision", "candidate_revision", "target"] {
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!(field)));
    }
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "urn:semaprax.project-candidate-cleanup-dependencies.v1"
        )));
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("candidate/cleanup-dependencies"));
        assert!(source.contains("semaprax.image-candidate-cleanup-dependencies-chunk.v1"));
    }
    for test_enabled in [false, true] {
        assert!(
            !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                .iter()
                .any(|method| method.name == "candidate/cleanup-dependencies")
        );
    }
}
#[test]
fn draft_archives_keep_explicit_selectors_closed_strings_and_candidate_grant() {
    let readonly = selected(VNextPolicy::default());
    assert!(!readonly["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"]
            .as_str()
            .unwrap()
            .starts_with("hole/archive-")));
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    });
    for name in ["hole/archive-export", "hole/archive-restore"] {
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "candidate_prepare");
        assert_eq!(method["query"], name == "hole/archive-export");
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"].as_object().unwrap().len(), 4);
        if name == "hole/archive-restore" {
            assert_eq!(
                params["required"],
                json!([
                    "image_revision",
                    "archive",
                    "archive_revision",
                    "draft_revision"
                ])
            );
            assert_eq!(
                params["properties"]["archive"]["$ref"],
                format!(
                    "urn:{}",
                    crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA
                )
            );
        } else {
            assert_eq!(params["properties"]["offset"]["maximum"], 128 * 1024 * 1024);
        }
    }
    let archive = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| {
            document["$id"]
                == format!(
                    "urn:{}",
                    crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA
                )
        })
        .unwrap();
    assert_eq!(archive["additionalProperties"], false);
    assert_eq!(archive["properties"].as_object().unwrap().len(), 12);
    for field in ["candidate_archive", "draft_recovery_capsule"] {
        assert_eq!(archive["properties"][field]["type"], "string");
    }
    for field in ["source_authority", "approval_authority", "trusted_hir"] {
        assert_eq!(archive["properties"][field]["const"], false);
    }
    let chunk = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.image-draft-archive-chunk.v1")
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    for field in ["image_revision", "archive_revision", "draft_revision"] {
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!(field)));
    }
    assert_eq!(chunk["properties"]["materializable"]["const"], false);
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(format!(
            "urn:{}",
            crate::project::PROJECT_CANDIDATE_ARCHIVE_SCHEMA
        ))));
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("request_hole_archive_export"));
        assert!(source.contains("request_hole_archive_restore"));
        assert!(source.contains("archive_revision"));
    }
    for test_enabled in [false, true] {
        assert!(
            !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                .iter()
                .any(|method| method.name.starts_with("hole/archive-"))
        );
    }
}
#[test]
fn draft_rebase_has_exact_selectors_closed_reports_and_no_extra_grant() {
    let readonly = selected(VNextPolicy::default());
    assert!(!readonly["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"] == "hole/rebase"));
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    });
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "hole/rebase")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], false);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 3);
    assert_eq!(
        params["required"],
        json!([
            "image_revision",
            "draft_revision",
            "new_base_candidate_revision"
        ])
    );
    let documents = bundle["documents"].as_array().unwrap();
    let wrapper = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.image-draft-rebase.v1")
        .unwrap();
    assert_eq!(wrapper["additionalProperties"], false);
    assert_eq!(wrapper["required"].as_array().unwrap().len(), 4);
    assert_eq!(
        wrapper["properties"]["draft"]["$ref"],
        "urn:semaprax.image-draft-handle.v1"
    );
    let report = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-candidate-draft-rebase.v1")
        .unwrap();
    assert_eq!(report["additionalProperties"], false);
    assert_eq!(report["required"].as_array().unwrap().len(), 12);
    assert_eq!(
        report["properties"]["holes"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(report["properties"]["materializable"]["const"], false);
    assert_eq!(report["properties"]["source_authority"]["const"], false);
    assert_eq!(
        report["properties"]["last_valid_rebase"]["$ref"],
        "urn:semaprax.project-candidate-rebase.v1"
    );
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("request_hole_rebase"));
        assert!(source.contains("new_base_candidate_revision"));
    }
    for test_enabled in [false, true] {
        assert!(
            !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                .iter()
                .any(|method| method.name == "hole/rebase")
        );
    }
}
#[test]
fn draft_merge_has_two_exact_parents_and_closed_bounded_report() {
    let readonly = selected(VNextPolicy::default());
    assert!(!readonly["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"] == "hole/merge"));
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    });
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "hole/merge")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], false);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 3);
    assert_eq!(
        params["required"],
        json!(["image_revision", "draft_revision", "other_draft_revision"])
    );
    let documents = bundle["documents"].as_array().unwrap();
    let wrapper = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.image-draft-merge.v1")
        .unwrap();
    assert_eq!(wrapper["additionalProperties"], false);
    assert_eq!(wrapper["required"].as_array().unwrap().len(), 5);
    assert_eq!(
        wrapper["properties"]["draft"]["$ref"],
        "urn:semaprax.image-draft-handle.v1"
    );
    let report = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-candidate-draft-merge.v1")
        .unwrap();
    assert_eq!(report["additionalProperties"], false);
    assert_eq!(report["required"].as_array().unwrap().len(), 14);
    for field in ["left_holes", "right_holes", "holes"] {
        assert_eq!(report["properties"][field]["maxItems"], 16);
        assert_eq!(
            report["properties"][field]["items"]["additionalProperties"],
            false
        );
    }
    assert_eq!(
        report["properties"]["holes"]["items"]["properties"]["parents"]["enum"],
        json!([["left"], ["right"], ["left", "right"]])
    );
    assert_eq!(report["properties"]["materializable"]["const"], false);
    assert_eq!(report["properties"]["source_authority"]["const"], false);
    assert_eq!(
        report["properties"]["last_valid_merge"]["$ref"],
        "urn:semaprax.project-candidate-rebase.v1"
    );
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("request_hole_merge"));
        assert!(source.contains("other_draft_revision"));
    }
    for test_enabled in [false, true] {
        assert!(
            !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                .iter()
                .any(|method| method.name == "hole/merge")
        );
    }
}
#[test]
fn draft_recovery_is_v5_candidate_only_with_closed_replay_schema() {
    let readonly = selected(VNextPolicy::default());
    assert!(!readonly["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"]
            .as_str()
            .unwrap()
            .starts_with("hole/recovery-")));
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        ..VNextPolicy::default()
    });
    for name in ["hole/recovery-export", "hole/recovery-restore"] {
        let descriptor = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(descriptor["capability"], "candidate_prepare");
        assert_eq!(descriptor["query"], name == "hole/recovery-export");
    }
    let capsule_id = format!(
        "urn:{}",
        crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
    );
    let capsule = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == capsule_id)
        .unwrap();
    assert_eq!(capsule["additionalProperties"], false);
    assert_eq!(
        capsule["properties"]["compiler"]["const"]["compatibility"],
        crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY
    );
    assert_eq!(
        capsule["required"],
        json!([
            "schema",
            "compiler",
            "base_revision",
            "draft_schema",
            "candidate_recovery",
            "holes",
            "draft_digest",
            "capsule_digest"
        ])
    );
    assert_eq!(
        capsule["properties"]["candidate_recovery"]["$ref"],
        "urn:semaprax.project-candidate-recovery.v1"
    );
    let holes = &capsule["properties"]["holes"];
    assert_eq!(holes["maxItems"], 16);
    let kinds = holes["items"]["oneOf"].as_array().unwrap();
    assert_eq!(kinds.len(), 3);
    assert_eq!(kinds[0]["required"], json!(["kind", "hole_id", "target"]));
    assert_eq!(
        kinds[1]["required"],
        json!(["kind", "hole_id", "target", "expression_id"])
    );
    assert_eq!(
        kinds[2]["required"],
        json!(["kind", "hole_id", "target", "expression_id"])
    );
    assert_eq!(
        kinds[2]["properties"]["kind"]["const"],
        "contract_expression"
    );
    for kind in kinds {
        assert_eq!(kind["additionalProperties"], false);
    }
    assert!(!bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(capsule_id)));
    for test_enabled in [false, true] {
        assert!(!crate::image_transport::candidates::methods(test_enabled)
            .iter()
            .any(|method| method.name.starts_with("hole/recovery-")));
    }
}
#[test]
fn selected_bundle_resolves_constructor_requests_and_marks_opaque_reports() {
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        build_enabled: true,
        ..VNextPolicy::default()
    });
    let ids = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["$id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(ids.contains("urn:semaprax.semantic-change-intent.v1"));
    for id in [
        "urn:semaprax.image-workspace-refresh-preview.v1",
        "urn:semaprax.image-workspace-refresh.v1",
        "urn:semaprax.image-artifact-projection-chunk.v1",
        "urn:semaprax.image-interface-delta-chunk.v1",
        "urn:semaprax.image-symbol-diagnostics-chunk.v1",
    ] {
        assert!(ids.contains(id));
    }
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|schema| schema == "urn:semaprax.image-artifact-projection.v1"));
    for report in [
        "urn:semaprax.project-candidate-interface-delta.v1",
        "urn:semaprax.project-candidate-symbol-diagnostics.v1",
    ] {
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(report)));
    }
    let diagnostics = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/symbol-diagnostics")
        .unwrap();
    assert_eq!(diagnostics["capability"], "candidate_diagnostics");
    let interface = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/interface-delta")
        .unwrap();
    assert_eq!(interface["capability"], "candidate_prepare");
    let params = &diagnostics["request_schema"]["properties"]["params"];
    assert!(!params["required"]
        .as_array()
        .unwrap()
        .contains(&json!("expected_report_revision")));
    assert_eq!(
        params["properties"]["expected_report_revision"]["type"],
        "string"
    );
    let apply = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/apply-intent")
        .unwrap();
    assert_eq!(
        apply["request_schema"]["properties"]["params"]["additionalProperties"],
        false
    );
    let control = apply["request_schema"]["properties"]["id"]["oneOf"][1]["pattern"]
        .as_str()
        .unwrap();
    assert_eq!(control, r"^[^\u0000-\u001f\u007f-\u009f]+$");
}
#[test]
fn optional_and_nullable_fields_remain_distinct_and_capabilities_do_not_expand() {
    let bundle = selected(VNextPolicy::default());
    assert!(!bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"] == "candidate/commit"
            || method["method"] == "candidate/apply-intent"));
    let context = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "image/context")
        .unwrap();
    let params = &context["request_schema"]["properties"]["params"];
    assert!(!params["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|name| name == "max_bytes"));
    assert_eq!(params["properties"]["max_bytes"]["type"], "integer");
    let chunk = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.image-target-admission-chunk.v1")
        .unwrap();
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|name| name == "candidate_revision"));
    assert_eq!(
        chunk["properties"]["candidate_revision"]["anyOf"][1]["type"],
        "null"
    );
}
#[test]
fn artifact_kinds_use_existing_build_grant_and_closed_client_schemas() {
    for policy in [
        VNextPolicy::default(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
    ] {
        let bundle = selected(policy);
        for name in [
            "candidate/build",
            "candidate/artifact-delta",
            "candidate/analysis-artifact-evidence",
        ] {
            assert!(!bundle["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method["method"] == name));
        }
    }
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        build_enabled: true,
        ..VNextPolicy::default()
    });
    for name in [
        "candidate/build",
        "candidate/artifact-delta",
        "candidate/analysis-artifact-evidence",
    ] {
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "candidate_build");
        assert_eq!(method["query"], false);
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(
            params["properties"]["kind"]["enum"],
            json!(["web", "npm", "openapi", "c"])
        );
        assert_eq!(params["properties"].as_object().unwrap().len(), 5);
        assert!(params["properties"].get("max_build_bytes").is_none());
        assert!(params["properties"].get("path").is_none());
    }
    for id in [
        "urn:semaprax.image-artifact-projection-chunk.v1",
        "urn:semaprax.image-artifact-delta-chunk.v1",
        "urn:semaprax.image-analysis-artifact-evidence-chunk.v1",
    ] {
        let chunk = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| document["$id"] == id)
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        let kind = &chunk["properties"]["kind"];
        let choices = kind
            .get("enum")
            .or_else(|| kind["anyOf"][0].get("enum"))
            .unwrap();
        assert_eq!(choices, &json!(["web", "npm", "openapi", "c"]));
        for field in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk["properties"][field]["const"], false);
        }
    }
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("openapi"));
        assert!(source.contains("request_candidate_build"));
        assert!(source.contains("request_candidate_artifact_delta"));
        assert!(source.contains("request_candidate_analysis_artifact_evidence"));
    }
    for test_enabled in [false, true] {
        for method in crate::image_transport::candidates::diagnostics::methods(test_enabled) {
            assert!(!serde_json::to_string(&method_description(method))
                .unwrap()
                .contains("openapi"));
        }
    }
}
#[test]
fn generated_clients_have_typed_builders_bounds_and_actual_lf_escapes() {
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        build_enabled: true,
        ..VNextPolicy::default()
    });
    for language in ["typescript", "python", "rust"] {
        let source = clients::generate(language, &bundle).unwrap();
        assert!(source.contains("WorkspaceRefreshParams"));
        assert!(source.contains("request_workspace_refresh"));
        assert!(source.contains("request_candidate_interface_delta"));
        assert!(source.contains("request_candidate_contract_delta"));
        assert!(source.contains("request_candidate_ownership_delta"));
        assert!(source.contains("request_candidate_artifact_delta"));
        assert!(source.contains("request_candidate_contract_expression_catalog"));
        assert!(source.contains("request_hole_open_contract_expression"));
        assert!(source.contains("request_candidate_symbol_diagnostics"));
        assert!(source.contains("expected_report_revision"));
        assert!(source.contains("decode_request_candidate_apply_intent"));
        assert!(source.contains("expected_new_project_revision"));
        assert!(source.contains("request byte bound"));
        assert!(!source.contains("request_candidate_commit("));
        assert!(source.len() < MAX_DISCOVERY_BYTES);
        match language {
            "typescript" => {
                assert!(source.contains(r"return line+'\n';"));
                assert!(source.contains("Number.isSafeInteger"));
            }
            "python" => {
                assert!(source.contains(r"return line + '\n'"));
                assert!(source.contains("NotRequired[int]"));
            }
            _ => {
                assert!(source.contains(r#"Ok(line+"\n")"#));
                assert!(source.contains("pub r#expected_new_project_revision: String"));
            }
        }
    }
}

#[test]
fn candidate_payloads_and_optional_frontend_work_are_concrete() {
    let bundle = selected(VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        ..VNextPolicy::default()
    });
    let documents = bundle["documents"].as_array().unwrap();
    for id in [
        "semaprax.project-change-catalog.v1",
        "semaprax.project-candidate-comparison.v1",
        "semaprax.image-candidate-reconciliation.v1",
        "semaprax.project-candidate-rebase.v1",
        "semaprax.image-validation-catalog.v1",
        "semaprax.image-validation-catalog.v2",
        "semaprax.project-candidate-semantic-delta-catalog.v1",
        "semaprax.project-candidate-test-plan.v1",
    ] {
        let uri = format!("urn:{id}");
        let schema = documents.iter().find(|doc| doc["$id"] == uri).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["schema"]["const"], id);
        assert!(!bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == &uri));
    }
    for id in [
        "urn:semaprax.image-workspace-refresh.v1",
        "urn:semaprax.image-workspace-refresh-preview.v1",
    ] {
        let schema = documents.iter().find(|doc| doc["$id"] == id).unwrap();
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "frontend_work"));
        assert_eq!(
            schema["properties"]["frontend_work"],
            json!({"oneOf":[
                {"$ref":"urn:semaprax.project-frontend-cache-work.v1"},
                {"$ref":"urn:semaprax.project-semantic-cache-work.v1"}
            ]})
        );
    }
    let frontend = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-frontend-cache-work.v1")
        .unwrap();
    assert_eq!(
        frontend["properties"]["invalidated_sources"]["type"],
        "array"
    );
    assert_eq!(
        frontend["properties"]["work"]["properties"]["checked_HIR_reused"],
        json!({"const":0})
    );
    let semantic = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-semantic-cache-work.v1")
        .unwrap();
    assert_eq!(
        semantic["properties"]["schema"]["const"],
        "semaprax.project-semantic-cache-work.v1"
    );
    assert_eq!(
        semantic["properties"]["work"]["properties"]["checked_HIR_reused"],
        json!({"type":"integer","minimum":0,"maximum":16})
    );
    let catalog = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-change-catalog.v1")
        .unwrap();
    let operations = catalog["properties"]["operations"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(operations.len(), 13);
    let repairs = operations
        .iter()
        .filter(|schema| schema["properties"]["kind"]["const"] == "repair_diagnostic")
        .collect::<Vec<_>>();
    assert_eq!(repairs.len(), 2);
    for (repair, class, selector) in [
        (
            repairs[0],
            "borrow_owned_byte_field_without_staging",
            "attempt/repair-catalog",
        ),
        (
            repairs[1],
            "retag_integer_literal_to_retained_return_type",
            "candidate-attempt/repair-catalog",
        ),
    ] {
        assert_eq!(repair["additionalProperties"], false);
        assert_eq!(repair["properties"]["repair_class"]["const"], class);
        assert_eq!(repair["properties"]["selector_source"]["const"], selector);
        assert_eq!(
            repair["properties"]["rejected_kind"]["const"],
            "replace_function_body"
        );
    }
    for kind in [
        "rename_declaration",
        "change_function_signature",
        "replace_function_body",
        "repair_diagnostic",
        "replace_expression",
        "replace_contract_expression",
        "add_contract",
        "add_declaration",
        "extract_function",
        "move_declaration",
        "add_record_field",
        "implement_interface",
    ] {
        let operation = operations
            .iter()
            .find(|schema| schema["properties"]["kind"]["const"] == kind)
            .unwrap();
        assert_eq!(operation["additionalProperties"], false);
    }
}
