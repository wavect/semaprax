# package_lock/subject.rs

- parse_subject · function · L3-L174 — pub(super) fn parse_subject(
- parse_wrapper · function · L176-L220 — pub(super) fn parse_wrapper<'a>(
- MARKER · constant · L187-L187 — const MARKER: &str = "\"payload\":";
- validate_package_report_wire · function · L222-L349 — fn validate_package_report_wire(report: &str) -> Result<(), Diagnostic>
- PAYLOAD_MARKER · constant · L236-L236 — const PAYLOAD_MARKER: &str = "\"payload\":";
- parse_dependencies · function · L351-L375 — fn parse_dependencies(value: &Value) -> Result<Vec<Coordinate>, Diagnostic>
- parse_targets · function · L377-L395 — fn parse_targets(value: &Value) -> Result<Vec<TargetFact>, Diagnostic>
- parse_sorted_strings · function · L397-L421 — fn parse_sorted_strings(
- parse_provenance · function · L423-L451 — fn parse_provenance(value: &Value) -> Result<Vec<ProvenanceFact>, Diagnostic>
- validate_json_wire · function · L453-L506 — pub(super) fn validate_json_wire(value: &str, label: &str) -> Result<(), Diagnostic>
- top_level_object_keys · function · L508-L570 — fn top_level_object_keys(value: &str) -> Result<Vec<String>, Diagnostic>
