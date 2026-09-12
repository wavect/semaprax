// Public implementation: four independent scalar digests over five signed
// 64-bit inputs. See ../../EQUIVALENCE.md for the exact input/output/boundary
// contract every language implementation of this task must meet.
//
// Compiled and tested with bare `rustc --test` (no Cargo project): the task
// has no dependencies, so `cargo`'s build graph and lockfile add nothing this
// snapshot needs, and staying off Cargo keeps this fixture out of the
// repository's own Cargo workspace.

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
