//! Closed physical carrier for a selected `Result<Owner, i64>` constructor
//! followed by one shared `&str` method call.  The Project route supplies
//! identity and authority; this module never discovers packages or tools.
use crate::diagnostic::Diagnostic;

pub(super) fn render_regex_result_owner_carrier(
    owner: &str,
    constructor: &str,
    method: &str,
    error: &str,
) -> Result<String, Diagnostic> {
    for path in [owner, constructor, method, error] {
        if semaprax::native_rust_binding::rust_api_path_tokens(path).as_deref() != Some(path) {
            return Err(Diagnostic::io(
                "SPX-B143",
                "selected Regex Result carrier path cannot be emitted",
            ));
        }
    }
    let expected_constructor = format!("{owner}::new");
    let expected_method = format!("{owner}::is_match");
    if constructor != expected_constructor || method != expected_method {
        return Err(Diagnostic::io(
            "SPX-B145",
            "selected Result owner carrier requires Regex::new and Regex::is_match",
        ));
    }
    Ok(include_str!("owner_borrowed_result.rs.txt")
        .replace("@OWNER@", owner)
        .replace("@CONSTRUCTOR@", constructor)
        .replace("@METHOD@", method)
        .replace("@ERROR@", error))
}
