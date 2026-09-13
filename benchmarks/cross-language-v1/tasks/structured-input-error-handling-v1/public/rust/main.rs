mod candidate;
#[cfg(test)]
mod public_tests {
    use super::candidate::validate;
    #[test]
    fn valid_boundaries_and_single_field_errors() {
        assert_eq!(validate(7, 1, 1), 0);
        assert_eq!(validate(7, 1, 64), 0);
        assert_eq!(validate(6, 1, 10), 1);
        assert_eq!(validate(7, 2, 10), 2);
        assert_eq!(validate(7, 1, 0), 3);
        assert_eq!(validate(7, 1, 65), 3);
    }
}
fn main() {}
