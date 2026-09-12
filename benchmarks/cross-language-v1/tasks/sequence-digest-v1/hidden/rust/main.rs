// Hidden overlay: replaces the public `main.rs` verbatim (same relative
// path) for the scoring phase only, adding a hidden test module a solver
// never sees. Implementation functions are unchanged from the public file.

fn is_even(value: i64) -> i64 {
    if value % 2 == 0 {
        1
    } else {
        0
    }
}

fn is_negative(value: i64) -> i64 {
    if value < 0 {
        1
    } else {
        0
    }
}

fn max2(left: i64, right: i64) -> i64 {
    if left > right {
        left
    } else {
        right
    }
}

fn sum(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    a + b + c + d + e
}

fn count_even(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    is_even(a) + is_even(b) + is_even(c) + is_even(d) + is_even(e)
}

fn count_negative(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    is_negative(a) + is_negative(b) + is_negative(c) + is_negative(d) + is_negative(e)
}

fn max(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    max2(max2(max2(a, b), max2(c, d)), e)
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_vector() {
        assert_eq!(sum(1, 2, 3, 4, 5), 15);
        assert_eq!(count_even(1, 2, 3, 4, 5), 2);
        assert_eq!(count_negative(1, 2, 3, 4, 5), 0);
        assert_eq!(max(1, 2, 3, 4, 5), 5);
    }

    #[test]
    fn negative_vector() {
        assert_eq!(sum(-1, -2, -3, -4, -5), -15);
        assert_eq!(count_even(-1, -2, -3, -4, -5), 2);
        assert_eq!(count_negative(-1, -2, -3, -4, -5), 5);
        assert_eq!(max(-1, -2, -3, -4, -5), -1);
    }
}

#[cfg(test)]
mod hidden_tests {
    use super::*;

    #[test]
    fn zero_vector() {
        assert_eq!(sum(0, 0, 0, 0, 0), 0);
        assert_eq!(count_even(0, 0, 0, 0, 0), 5);
        assert_eq!(count_negative(0, 0, 0, 0, 0), 0);
        assert_eq!(max(0, 0, 0, 0, 0), 0);
    }

    #[test]
    fn late_maximum_vector() {
        assert_eq!(sum(-100, 7, 7, 7, 100), 21);
        assert_eq!(count_even(-100, 7, 7, 7, 100), 2);
        assert_eq!(count_negative(-100, 7, 7, 7, 100), 1);
        assert_eq!(max(-100, 7, 7, 7, 100), 100);
    }
}
