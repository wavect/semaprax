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
