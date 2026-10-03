//! Safe generated runtime for a private C-compatible callback body.
//! Rust closure values are constructed normally; no unstable Fn trait impls,
//! Rust trait-object layout assumptions, unsafe Send/Sync, or raw handle API.
pub(super) fn render(
    snapshot: &str,
    transition: &str,
    trait_path: &str,
    method: &str,
    error: &str,
) -> String {
    include_str!("callback_runtime.template")
        .replace("$SNAPSHOT", snapshot)
        .replace("$TRANSITION", transition)
        .replace("$TRAIT", trait_path)
        .replace("$METHOD", method)
        .replace("$ERROR", error)
}
