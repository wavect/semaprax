mod candidate;
#[cfg(test)]
mod hidden_tests {
    use super::candidate::validate;
    #[test]
    fn compound_invalid_records_preserve_first_error_precedence() {
        assert_eq!(validate(6, 2, 0), 1);
        assert_eq!(validate(7, 2, 0), 2);
        assert_eq!(validate(7, 1, -1), 3);
        assert_eq!(validate(7, 1, 65), 3);
        assert_eq!(validate(7, 1, 64), 0);
        assert_eq!(validate(7, 1, 1), 0);
    }
}
fn main() {}
