#[derive(serde::Deserialize)]
struct Edit {
    before: String,
    after: String,
}
#[derive(serde::Deserialize)]
struct Projection {
    source: String,
    edits: Vec<Edit>,
}
#[test]
fn complete_current_call_owners_are_exact_original_with_only_declared_held_profile_hooks() {
    let projections: Vec<Projection> = serde_json::from_str(ci_str!(
        "../../../../docs/evidence/held-generic-call-profiles-812/exact-current-projections.json"
    ))
    .unwrap();
    assert_eq!(projections.len(), 3);
    for (projection, (path, current, original)) in projections.iter().zip([
        ("src/hir/validation.rs", ci_str!("../../validation.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation.rs.before.txt")),
        ("src/hir/validation/call_parameters.rs", ci_str!("../call_parameters.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__call_parameters.rs.before.txt")),
        ("src/hir/validation/executable_profiles.rs", ci_str!("../executable_profiles.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__executable_profiles.rs.before.txt")),
    ]) {
        assert_eq!(projection.source, path);
        let mut projected = current.to_owned();
        for edit in &projection.edits {
            assert_eq!(projected.matches(&edit.after).count(), 1);
            projected = projected.replacen(&edit.after, &edit.before, 1);
        }
        assert_eq!(projected, original);
    }
}
#[test]
fn complete_saved_lookup_and_argument_receivers_and_shared_type_prefix_remain_byte_exact() {
    let original = ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__executable_profiles.rs.before.txt");
    assert_eq!(
        ci_str!("../executable_profiles/before_812.rs"),
        original.replacen("#[cfg(test)]\nmod tests;\n", "", 1)
    );
    let original = ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__call_parameters.rs.before.txt");
    let start = original
        .find("    pub(super) fn validate_argument_ownership_view(")
        .unwrap();
    let end = original[start..]
        .find("\n}\n\n#[cfg(test)]\nmod tests;")
        .unwrap()
        + start;
    let saved = format!(
        "use super::*;\nimpl HirValidator<'_> {{\n{}\n}}\n",
        &original[start..end]
    );
    assert_eq!(
        ci_str!("../call_parameters/before_812.rs").replace(
            "fn validate_argument_ownership_view_saved_812(",
            "fn validate_argument_ownership_view("
        ),
        saved
    );
    assert_eq!(ci_str!("../type_profiles.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__type_profiles.rs.before.txt"));
    assert_eq!(ci_str!("../../byte_capacity.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__byte_capacity.rs.before.txt"));
    assert_eq!(ci_str!("../type_requirement.rs"), ci_str!("../../../../docs/evidence/held-generic-call-profiles-812/src__hir__validation__type_requirement.rs.before.txt"));
}
