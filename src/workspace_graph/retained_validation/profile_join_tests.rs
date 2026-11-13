use super::project_type_imports_admitted;
use crate::project::ProjectProfile;
#[test]
fn type_import_join_preserves_frozen_scalar_and_stream_profiles() {
    for profile in [
        ProjectProfile::StdinStreamDataCommandIoV2,
        ProjectProfile::StdinStreamOwnedDataCommandIoV1,
        ProjectProfile::StdinStreamCollectionRecordCommandIoV1,
        ProjectProfile::StdinStreamNestedOutcomeCommandIoV1,
    ] {
        assert!(project_type_imports_admitted(profile));
    }
    assert!(!project_type_imports_admitted(
        ProjectProfile::StdinStreamDataCommandIoV1
    ));
    assert!(!project_type_imports_admitted(
        ProjectProfile::StdinStreamTextCommandIoV1
    ));
}

#[test]
fn scalar_web_entrypoint_borrows_the_lookup_and_shares_the_selected_key() {
    use super::scalar_web_entrypoint;
    use crate::hir::{self, IdentityOrigin, LinkedScalarFunction};
    use crate::test_allocation_observer::measure;

    let source = r#"module app;
@id("app.main") fn main() -> i64 { 0 }
@id("app.other") fn other() -> i64 { 1 }
@id("app.params") fn params(x: i64) -> i64 { x }
@id("app.flag") fn flag() -> bool { true }"#;
    let checked = crate::check(source, "web-entrypoint.spx").unwrap();
    let resolved = hir::resolve(&checked).unwrap();
    let available = resolved
        .functions
        .into_iter()
        .map(|function| {
            (
                function.id.clone(),
                LinkedScalarFunction {
                    function,
                    origin: IdentityOrigin::Explicit,
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let (ordinary, _) = available.get_key_value("app.main").unwrap();
    let (other, _) = available.get_key_value("app.other").unwrap();
    let (results, allocation) = measure(|| {
        [
            scalar_web_entrypoint(ProjectProfile::ScalarV1, &available, ordinary, "app.other"),
            scalar_web_entrypoint(
                ProjectProfile::ScalarV1,
                &available,
                ordinary,
                "app.missing",
            ),
            scalar_web_entrypoint(ProjectProfile::ScalarV1, &available, ordinary, "app.params"),
            scalar_web_entrypoint(ProjectProfile::ScalarV1, &available, ordinary, "app.flag"),
            scalar_web_entrypoint(
                ProjectProfile::StdinStreamTextCommandIoV1,
                &available,
                ordinary,
                "app.other",
            ),
        ]
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, 0);
    for (actual, expected) in results
        .iter()
        .zip([other, ordinary, ordinary, ordinary, ordinary])
    {
        assert_eq!(actual, expected);
        assert_eq!(
            actual.shared_allocation_key(),
            expected.shared_allocation_key()
        );
    }
}
