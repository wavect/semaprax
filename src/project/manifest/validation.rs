//! Exact manifest identity and grammar helpers.

use super::*;

pub(super) fn valid_stable_id(value: &str) -> bool {
    (1..=MAX_STABLE_ID_BYTES).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

/// The exact line shape of a Project v1 manifest, for the reader who wrote
/// the keys in another order or left one out.
pub(super) const V1_SHAPE_HELP: &str =
    "write exactly these six lines in this order, then one final newline: \
                             `schema = \"semaprax.project.v1\"`, `name = \"…\"`, \
                             `entry = \"module.with.main\"`, `sources = [\"src/….spx\", …]`, \
                             `web_exports = [\"stable.id\", …]` (byte-sorted), \
                             `tests = [\"module.tests\"]`";

pub(super) fn grammar_with_help(message: impl Into<String>, help: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J100", message).with_help(help)]
}
