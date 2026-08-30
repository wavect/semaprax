#[test]
fn frozen_framing_derivation_is_exact() {
    assert_eq!(super::wire::fixed_framing_fixture_bytes(), 1_114);
    assert_eq!(super::MAX_REQUIREMENT_FRAMING_BYTES, 1_255);
    assert_eq!(super::MAX_CAPABILITY_FRAMING_BYTES, 66_047);
    assert_eq!(super::MAX_SUBJECT_DELIMITER_BYTES, 63);
    assert_eq!(super::MAX_INPUT_FRAMING_BYTES, 68_479);
}
