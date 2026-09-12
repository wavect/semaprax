# agent_skill_bundle/tests.rs

- COMMITTED_BUNDLE · constant · L14-L14 — const COMMITTED_BUNDLE: &str = include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json");
- REQUIRED_VERBS · constant · L16-L19 — const REQUIRED_VERBS: &[&str] = &[
- generation_is_byte_identical_on_repetition · function · L22-L26 — fn generation_is_byte_identical_on_repetition()
- committed_bundle_is_pinned_and_regenerates_byte_identical · function · L29-L36 — fn committed_bundle_is_pinned_and_regenerates_byte_identical()
- regenerate_committed_bundle · function · L40-L44 — fn regenerate_committed_bundle()
- exact_payload_text · function · L52-L57 — fn exact_payload_text(envelope: &str) -> &str
- PAYLOAD_KEY · constant · L53-L53 — const PAYLOAD_KEY: &str = "\"payload\":";
- envelope_is_well_formed_and_bounded · function · L60-L81 — fn envelope_is_well_formed_and_bounded()
- public_workflow_is_sorted_by_verb_and_covers_the_required_ten_verbs · function · L84-L99 — fn public_workflow_is_sorted_by_verb_and_covers_the_required_ten_verbs()
- authority_classes_cover_exactly_the_used_set_and_are_sorted · function · L102-L120 — fn authority_classes_cover_exactly_the_used_set_and_are_sorted()
- target_profiles_are_sorted_and_closed · function · L123-L128 — fn target_profiles_are_sorted_and_closed()
- every_public_workflow_command_names_an_existing_top_level_cli_surface · function · L131-L156 — fn every_public_workflow_command_names_an_existing_top_level_cli_surface()
- package_status_matches_the_bundled_standard_library_catalog · function · L159-L171 — fn package_status_matches_the_bundled_standard_library_catalog()
- negotiate_agent_skill_schema_accepts_exact_match_and_rejects_drift · function · L174-L181 — fn negotiate_agent_skill_schema_accepts_exact_match_and_rejects_drift()
- discovery_operations_embed_the_full_live_catalog_including_this_bundle_s_own_entry · function · L184-L201 — fn discovery_operations_embed_the_full_live_catalog_including_this_bundle_s_own_entry()
