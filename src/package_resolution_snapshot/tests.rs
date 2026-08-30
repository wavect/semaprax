#[test]
fn frozen_framing_derivation_is_exact() {
    assert_eq!(super::wire::fixed_framing_fixture_bytes(), 1_114);
    assert_eq!(super::MAX_REQUIREMENT_FRAMING_BYTES, 1_255);
    assert_eq!(super::MAX_CAPABILITY_FRAMING_BYTES, 66_047);
    assert_eq!(super::MAX_SUBJECT_DELIMITER_BYTES, 63);
    assert_eq!(super::MAX_INPUT_FRAMING_BYTES, 68_479);
}

#[test]
fn component_and_cumulative_length_guards_are_exact_without_large_allocations() {
    let maxima = [
        super::MAX_INPUT_BYTES,
        crate::package_resolver::MAX_OUTPUT_BYTES,
        crate::package_lock_v2::MAX_OUTPUT_BYTES,
    ];
    assert_eq!(maxima.iter().sum::<usize>(), super::MAX_SNAPSHOT_BYTES);
    super::model::validate_lengths(maxima[0], maxima[1], maxima[2]).unwrap();
    for index in 0..3 {
        let mut lengths = maxima;
        lengths[index] += 1;
        assert_eq!(
            super::model::validate_lengths(lengths[0], lengths[1], lengths[2])
                .unwrap_err()
                .code,
            "SPX-PK503"
        );
    }
    assert_eq!(
        super::model::validate_lengths(usize::MAX, 1, 0)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
}

#[test]
fn raw_subject_preallocation_count_and_byte_guards_are_exact() {
    use crate::package_resolver::{MAX_SUBJECTS, MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES};
    super::model::admit_subject_slot(MAX_SUBJECTS - 1).unwrap();
    assert_eq!(
        super::model::admit_subject_slot(MAX_SUBJECTS)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
    assert_eq!(
        super::model::add_subject_bytes(0, MAX_SUBJECT_BYTES).unwrap(),
        MAX_SUBJECT_BYTES
    );
    assert_eq!(
        super::model::add_subject_bytes(0, MAX_SUBJECT_BYTES + 1)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
    assert_eq!(
        super::model::add_subject_bytes(
            MAX_TOTAL_SUBJECT_BYTES - MAX_SUBJECT_BYTES,
            MAX_SUBJECT_BYTES
        )
        .unwrap(),
        MAX_TOTAL_SUBJECT_BYTES
    );
    assert_eq!(
        super::model::add_subject_bytes(MAX_TOTAL_SUBJECT_BYTES, 1)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
    assert_eq!(
        super::model::add_subject_bytes(usize::MAX, 1)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
}
