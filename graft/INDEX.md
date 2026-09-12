<!-- graft graph: built from src/ ONLY, at commit 1fc844643ee87c584e7b50ac65f54fdd37e93a90. Anything landed after that SHA is invisible here. Not a substitute for rg on tests/, docs/, std/ or examples/. Regenerate with: graft build src (then move src/graft to ./graft). -->

# graft — repo map

Small markdown nodes summarising this repo. `grep` any term, symbol, or
filename here, or run `graft ask "<task>"`. Each node carries prose plus exact
`file:line`; open a source file only to edit the named span.

The same graph is queryable as MCP tools (`graft_find_code`, `graft_find_all`,
`graft_trace_calls`, `graft_file_api`, `graft_repo_map`) where a host exposes them, and
as the `graft` CLI everywhere else. Edges — who calls what — live only in the
graph, not in these files: `graft callers <symbol>` is the only way to read them.

## Concepts

- [abi_report](abi_report.md) — abi_report
- [agent_definition](agent_definition.md) — agent_definition
- [agent_deployment](agent_deployment.md) — agent_deployment
- [agent_economics](agent_economics.md) — agent_economics
- [agent_harness](agent_harness.md) — agent_harness
- [agent_interaction_schema](agent_interaction_schema.md) — agent_interaction_schema
- [agent_lifecycle](agent_lifecycle.md) — agent_lifecycle
- [agent_lifecycle_typed_carrier](agent_lifecycle_typed_carrier.md) — agent_lifecycle_typed_carrier
- [agent_observation](agent_observation.md) — agent_observation
- [agent_proposal](agent_proposal.md) — agent_proposal
- [agent_runtime](agent_runtime.md) — agent_runtime
- [agent_runtime_v2](agent_runtime_v2.md) — agent_runtime_v2
- [agent_skill_bundle](agent_skill_bundle.md) — agent_skill_bundle
- [agent_transcript](agent_transcript.md) — agent_transcript
- [agent_transport](agent_transport.md) — agent_transport
- [aggregate_layout](aggregate_layout.md) — aggregate_layout
- [arc_zones](arc_zones.md) — arc_zones
- [architecture_claims](architecture_claims.md) — architecture_claims
- [assurance_manifest](assurance_manifest.md) — assurance_manifest
- [ast](ast.md) — ast
- [bounded_output](bounded_output.md) — bounded_output
- [box_ops](box_ops.md) — box_ops
- [byte_data_capacity](byte_data_capacity.md) — byte_data_capacity
- [byte_ops](byte_ops.md) — byte_ops
- [c_header](c_header.md) — c_header
- [cache_codec](cache_codec.md) — cache_codec
- [call_index](call_index.md) — call_index
- [candidate_archive_store](candidate_archive_store.md) — candidate_archive_store
- [capability_manifest](capability_manifest.md) — capability_manifest
- [cleanup](cleanup.md) — cleanup
- [cleanup_plan](cleanup_plan.md) — cleanup_plan
- [cli_driver](cli_driver.md) — cli_driver
- [codegen](codegen.md) — codegen
- [command_io_ops](command_io_ops.md) — command_io_ops
- [command_profile](command_profile.md) — command_profile
- [conformance](conformance.md) — conformance
- [cxx_shim](cxx_shim.md) — cxx_shim
- [database_fixture](database_fixture.md) — database_fixture
- [diagnostic](diagnostic.md) — diagnostic
- [digest_hex](digest_hex.md) — digest_hex
- [doc](doc.md) — doc
- [doctor](doctor.md) — doctor
- [economic_agent](economic_agent.md) — economic_agent
- [environment_ops](environment_ops.md) — environment_ops
- [environment_snapshot](environment_snapshot.md) — environment_snapshot
- [execution_revision](execution_revision.md) — execution_revision
- [filesystem_ops](filesystem_ops.md) — filesystem_ops
- [filesystem_provider](filesystem_provider.md) — filesystem_provider
- [format](format.md) — format
- [freestanding_object](freestanding_object.md) — freestanding_object
- [graph](graph.md) — graph
- [graph_cleanup](graph_cleanup.md) — graph_cleanup
- [graph_loan](graph_loan.md) — graph_loan
- [hir](hir.md) — hir
- [host_io_ops](host_io_ops.md) — host_io_ops
- [host_ownership](host_ownership.md) — host_ownership
- [hosted_interpreter](hosted_interpreter.md) — hosted_interpreter
- [https_client](https_client.md) — https_client
- [hygienic](hygienic.md) — hygienic
- [image_transport](image_transport.md) — image_transport
- [impact](impact.md) — impact
- [installed_diagnostics](installed_diagnostics.md) — installed_diagnostics
- [installed_fix_plan](installed_fix_plan.md) — installed_fix_plan
- [installed_guidance](installed_guidance.md) — installed_guidance
- [interpreter](interpreter.md) — interpreter
- [iterator_ops](iterator_ops.md) — iterator_ops
- [job_fixture](job_fixture.md) — job_fixture
- [lexer](lexer.md) — lexer
- [lib](lib.md) — lib
- [live_invocation](live_invocation.md) — live_invocation
- [loan_plan](loan_plan.md) — loan_plan
- [main](main.md) — main
- [native_scratch](native_scratch.md) — native_scratch
- [native_settlement](native_settlement.md) — native_settlement
- [network_io_ops](network_io_ops.md) — network_io_ops
- [network_provider](network_provider.md) — network_provider
- [openapi](openapi.md) — openapi
- [owned_resource_corpus](owned_resource_corpus.md) — owned_resource_corpus
- [package_build](package_build.md) — package_build
- [package_build_v2](package_build_v2.md) — package_build_v2
- [package_compatibility](package_compatibility.md) — package_compatibility
- [package_lock](package_lock.md) — package_lock
- [package_lock_v2](package_lock_v2.md) — package_lock_v2
- [package_lock_v3](package_lock_v3.md) — package_lock_v3
- [package_range](package_range.md) — package_range
- [package_report](package_report.md) — package_report
- [package_report_v2](package_report_v2.md) — package_report_v2
- [package_resolution_snapshot](package_resolution_snapshot.md) — package_resolution_snapshot
- [package_resolver](package_resolver.md) — package_resolver
- [package_resolver_v2](package_resolver_v2.md) — package_resolver_v2
- [package_semantic_graph](package_semantic_graph.md) — package_semantic_graph
- [package_source_capsule](package_source_capsule.md) — package_source_capsule
- [parser](parser.md) — parser
- [patch](patch.md) — patch
- [patch_evidence](patch_evidence.md) — patch_evidence
- [plugin_manifest](plugin_manifest.md) — plugin_manifest
- [prelude](prelude.md) — prelude
- [private_capacity_contract](private_capacity_contract.md) — private_capacity_contract
- [process_ops](process_ops.md) — process_ops
- [process_provider](process_provider.md) — process_provider
- [project_revision_store](project_revision_store.md) — project_revision_store
- [properties](properties.md) — properties
- [protocol_check](protocol_check.md) — protocol_check
- [public_generic_abi](public_generic_abi.md) — public_generic_abi
- [public_generic_consumer](public_generic_consumer.md) — public_generic_consumer
- [public_generic_settlement](public_generic_settlement.md) — public_generic_settlement
- [public_generic_surface](public_generic_surface.md) — public_generic_surface
- [public_generic_type](public_generic_type.md) — public_generic_type
- [quality_route](quality_route.md) — quality_route
- [query](query.md) — query
- [region_report](region_report.md) — region_report
- [repair](repair.md) — repair
- [requirement_traceability](requirement_traceability.md) — requirement_traceability
- [review](review.md) — review
- [runtime_status](runtime_status.md) — runtime_status
- [scoped_tasks](scoped_tasks.md) — scoped_tasks
- [semantic_cache_store](semantic_cache_store.md) — semantic_cache_store
- [semantic_discovery](semantic_discovery.md) — semantic_discovery
- [semantic_embedding](semantic_embedding.md) — semantic_embedding
- [semantic_retention](semantic_retention.md) — semantic_retention
- [semantic_retention_lifecycle](semantic_retention_lifecycle.md) — semantic_retention_lifecycle
- [semantic_retention_registry](semantic_retention_registry.md) — semantic_retention_registry
- [semantic_retention_store](semantic_retention_store.md) — semantic_retention_store
- [semantic_service_mcp](semantic_service_mcp.md) — semantic_service_mcp
- [semantic_service_transport](semantic_service_transport.md) — semantic_service_transport
- [semantic_trace](semantic_trace.md) — semantic_trace
- [semantic_workspace](semantic_workspace.md) — semantic_workspace
- [semantic_workspace_change](semantic_workspace_change.md) — semantic_workspace_change
- [semantic_workspace_operations](semantic_workspace_operations.md) — semantic_workspace_operations
- [semantic_workspace_structural_change](semantic_workspace_structural_change.md) — semantic_workspace_structural_change
- [simd_report](simd_report.md) — simd_report
- [source_verify](source_verify.md) — source_verify
- [static_protocol](static_protocol.md) — static_protocol
- [str_ops](str_ops.md) — str_ops
- [string_ops](string_ops.md) — string_ops
- [structured_tasks](structured_tasks.md) — structured_tasks
- [target_evidence](target_evidence.md) — target_evidence
- [trace_path_certificate](trace_path_certificate.md) — trace_path_certificate
- [ui_schema](ui_schema.md) — ui_schema
- [variant_layout](variant_layout.md) — variant_layout
- [vec_ops](vec_ops.md) — vec_ops
- [verify](verify.md) — verify
- [wasm](wasm.md) — wasm
- [wit_component](wit_component.md) — wit_component
- [workspace](workspace.md) — workspace
- [workspace_analysis](workspace_analysis.md) — workspace_analysis
- [workspace_graph](workspace_graph.md) — workspace_graph
- [workspace_patch_evidence](workspace_patch_evidence.md) — workspace_patch_evidence

## Files

1270 per-file wiring cards mirror the source tree under `graft/` (1270 carry extracted symbols). They are deliberately not enumerated here —
`grep` a symbol or `find`/`ls` a filename under `graft/` to land on the card for that file.
