---
covers: []
---
# quality_route.rs

- SCHEMA · constant · L7-L7 — const SCHEMA: &str = "semaprax.quality-route.v2";
- PathRow · type · L8-L8 — type PathRow = (String, &'static str, &'static str);
- plan · function · L15-L21 — pub fn plan(
- plan_with_base · function · L29-L87 — pub fn plan_with_base(
- changed_plan · function · L89-L137 — fn changed_plan(
- discover_changes · function · L139-L187 — fn discover_changes(root: &Path, base: &str) -> Result<BTreeSet<String>, String>
- validate_alias · function · L189-L219 — fn validate_alias(path: &str) -> Result<(), String>
- portable_windows_component · function · L221-L250 — fn portable_windows_component(component: &str) -> bool
- validate_worktree_path · function · L252-L299 — fn validate_worktree_path(root: &Path, base: &str, path: &str) -> Result<(), String>
- resolve_base · function · L301-L364 — fn resolve_base(root: &Path, explicit: Option<&str>) -> Result<String, String>
- utf8_environment · function · L366-L374 — fn utf8_environment(name: &str) -> Result<Option<String>, String>
- validate_target_ref · function · L376-L385 — fn validate_target_ref(root: &Path, target_ref: &str) -> Result<(), String>
- validate_object_id · function · L387-L396 — fn validate_object_id(base: &str) -> Result<(), String>
- tracked_in_index_head_or_base · function · L398-L419 — fn tracked_in_index_head_or_base(root: &Path, base: &str, path: &str) -> Result<bool, String>
- mapping · function · L421-L458 — fn mapping(path: &str) -> (&'static str, &'static str, bool)
- surface_gates · function · L463-L477 — fn surface_gates(profile: &str, paths: &[PathRow]) -> Vec<&'static str>
- gates · function · L479-L512 — fn gates(profile: &str) -> &'static [&'static str]
- git · function · L514-L527 — fn git(root: &Path, arguments: &[&str]) -> Result<Output, String>
- nul_fields · function · L529-L540 — fn nul_fields<'a>(bytes: &'a [u8], label: &str) -> Result<Vec<&'a str>, String>
- utf8 · function · L542-L544 — fn utf8<'a>(bytes: &'a [u8], label: &str) -> Result<&'a str, String>
- display_paths · function · L546-L552 — fn display_paths(paths: &[String]) -> String
