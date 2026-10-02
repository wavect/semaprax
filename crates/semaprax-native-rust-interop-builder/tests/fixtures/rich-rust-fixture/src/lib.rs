//! Ordinary Rust fixture input for RI-01.  It has no SEMAPRAX annotations or
//! FFI export; the generated adapter owns the C-compatible boundary.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DivisionError {
    Zero,
}

pub fn add(left: i64, right: i64) -> i64 {
    left + right
}

pub fn checked_div(left: i64, right: i64) -> Result<i64, DivisionError> {
    left.checked_div(right).ok_or(DivisionError::Zero)
}
