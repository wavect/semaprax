//! Whole-source inverse guards; no excluded original interior.
#[track_caller]
fn undo(source: &mut String, new: &str, old: &str) {
    assert_eq!(
        source.matches(new).count(),
        1,
        "exact unique receiving edit: {:?}",
        new.chars().take(160).collect::<String>()
    );
    *source = source.replacen(new, old, 1);
}
mod a;
mod b;
mod current_view;
mod presence;
mod presence_tests;
