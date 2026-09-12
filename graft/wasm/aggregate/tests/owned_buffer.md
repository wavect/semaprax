# wasm/aggregate/tests/owned_buffer.rs

- Fixture · struct · L5-L5 — struct Fixture(std::path::PathBuf);
- drop · function · L8-L10 — fn drop(&mut self)
- SUCCESS · constant · L13-L37 — const SUCCESS: &str = r#"
- owned_buffer_host_probe_module · function · L39-L102 — fn owned_buffer_host_probe_module() -> Vec<u8>
- owned_buffer_host_boundary_rejects_forged_inputs_without_poisoning_reentry · function · L105-L161 — fn owned_buffer_host_boundary_rejects_forged_inputs_without_poisoning_reentry()
- COMPUTED · constant · L166-L193 — const COMPUTED: &str = r#"
- COMPUTED_OUT_OF_RANGE · constant · L199-L214 — const COMPUTED_OUT_OF_RANGE: &str = r#"
- LOOP_FILL · constant · L221-L249 — const LOOP_FILL: &str = r#"
- LOOP_PAST_END · constant · L255-L273 — const LOOP_PAST_END: &str = r#"
- DECODED_STRING_BUFFER · constant · L281-L330 — const DECODED_STRING_BUFFER: &str = r#"
- FAILURE · constant · L332-L347 — const FAILURE: &str = r#"
- owned_bounded_byte_buffer_executes_and_reenters_without_memory_copy · function · L350-L448 — fn owned_bounded_byte_buffer_executes_and_reenters_without_memory_copy()
- CONTRACT_FAILURE · constant · L355-L355 — const CONTRACT_FAILURE: &str = "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.contract.v1'||status.code!==1||error.message!=='SEMAPRAX contract failure')throw error;failed=true;}if(!failed)throw Error('missing owned buffer failure');";
- BOUND_FAILURE · constant · L359-L359 — const BOUND_FAILURE: &str = "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.byte-buffer.v1'||status.code!==1)throw error;failed=true;}if(!failed)throw Error('missing owned buffer element-bound failure');";
- RETURNS_SEVEN · constant · L360-L361 — const RETURNS_SEVEN: &str =
