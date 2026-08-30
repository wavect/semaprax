#[test]
fn caller_requirement_lengths_reject_before_resolver_replay_or_rendering() {
    use crate::package_resolver::{Requirement, ResolutionInput, ResolutionOptions};
    let mut input = ResolutionInput {
        requirements: vec![Requirement {
            package: "fixture".to_owned(),
            range: "=4294967295.4294967295.4294967295".to_owned(),
        }],
        subjects: vec![],
        target: "native64".to_owned(),
        allowed_capabilities: vec![],
    };
    assert_eq!(input.requirements[0].range.len(), 33);
    super::model::preflight_requirements(&input.requirements).unwrap();
    super::model::validate_requirement_count(4).unwrap();
    for count in [0, 5] {
        assert_eq!(
            super::model::validate_requirement_count(count)
                .unwrap_err()
                .code,
            "SPX-PK501"
        );
    }
    for range in [
        format!("{}0", input.requirements[0].range),
        "=".to_owned() + &"1".repeat(1024 * 1024),
    ] {
        input.requirements[0].range = range;
        // Invalid evidence would otherwise be rejected by Resolver-v1 wire
        // admission; PK501 proves the borrowing input guard takes precedence.
        assert_eq!(
            super::generate(&input, &ResolutionOptions::default(), "not evidence")
                .unwrap_err()
                .code,
            "SPX-PK501"
        );
        assert_eq!(
            super::wire::render_input(&input, &ResolutionOptions::default())
                .unwrap_err()
                .code,
            "SPX-PK501"
        );
    }
}

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
