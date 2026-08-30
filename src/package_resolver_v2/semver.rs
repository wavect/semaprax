use super::wire;
use crate::diagnostic::Diagnostic;
pub(super) use crate::package_range::{Range, Version};
use std::cmp::Ordering;

pub(super) fn parse_version(value: &str) -> Result<Version, Diagnostic> {
    crate::package_range::parse_version(value, input_error)
}
pub(super) fn parse_range(value: &str) -> Result<Range, Diagnostic> {
    crate::package_range::parse_range(value, input_error)
}
fn input_error(message: String) -> Diagnostic {
    wire::input_error(message)
}
pub(super) fn compare_coordinates(
    left_package: &str,
    left_version: Version,
    right_package: &str,
    right_version: Version,
) -> Ordering {
    left_package
        .cmp(right_package)
        .then_with(|| left_version.cmp(&right_version))
}
