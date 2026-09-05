use super::*;

#[test]
fn every_legacy_marker_reports_one_exact_closed_profile() {
    let legacy = [
        (
            PreparedProjectAdmission::UsefulTextConsumerV1,
            ProjectProfile::UsefulTextConsumerV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataV1,
            ProjectProfile::UsefulDataV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataCommandV1,
            ProjectProfile::UsefulDataCommandV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataCommandV2,
            ProjectProfile::UsefulDataCommandV2,
        ),
        (
            PreparedProjectAdmission::LanguageCommandIoV1,
            ProjectProfile::LanguageCommandIoV1,
        ),
        (
            PreparedProjectAdmission::LineCommandIoV1,
            ProjectProfile::LineCommandIoV1,
        ),
        (
            PreparedProjectAdmission::NetworkCommandIoV1,
            ProjectProfile::NetworkCommandIoV1,
        ),
        (
            PreparedProjectAdmission::HttpsCommandIoV1,
            ProjectProfile::HttpsCommandIoV1,
        ),
    ];
    for (prepared, expected) in legacy {
        assert_eq!(prepared.profile(), expected);
        assert!(prepared.owned_descriptor().is_none());
        assert!(prepared.flat_record_descriptor().is_none());
    }
}
