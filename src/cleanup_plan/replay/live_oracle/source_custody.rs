//! Whole-source inverse guards; no excluded original interior.
fn undo(source: &mut String, new: &str, old: &str) {
    assert_eq!(
        source.matches(new).count(),
        1,
        "exact unique receiving edit"
    );
    *source = source.replacen(new, old, 1);
}
mod a;
mod b;
mod current_view;
