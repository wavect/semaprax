//! Fixed complete before/current witness for the retained-tail mechanism.
//! This is an explicit semantic transformation witness, separate from formatting.
pub(super) fn prepared_owner_stage(current: &str) -> String {
    crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
        "41cb2cf4267b055c3aee09c6a6bb143da182c28a6b583524ca7608e590133c10",
        current,
    )
}
pub(super) fn prepared_tail_stage(current: &str) -> String {
    crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source.rs",
        "1b4fe517fc67c10d948bde00946cb17bf88c84b0f6c333d2b365851b28f22d37",
        current,
    )
}

fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(
        current.len(),
        length,
        "unknown retained-tail subject extent"
    );
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("retained-tail inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(
            &result[at..end],
            after,
            "unknown retained-tail source change"
        );
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
mod prepared_links;
fn previous_stage(current: &str) -> String {
    restore(current, 6431, MACRO)
}
pub(super) fn previous(current: &str) -> String {
    previous_stage(&prepared_links::previous_macro(current))
}
pub(super) fn previous_owner(current: &str) -> String {
    restore(
        &prepared_links::previous_owner(&prepared_owner_stage(current)),
        21817,
        OWNER,
    )
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (6367, "}\n                []", "None"), (6350, "", ","),
    (6338, "ad", ""), (6336, "h", "valu"),
    (6306, "", "     match value {\n                Some(value) =>"), (6299, "", "}\n"),
    (6168, "                [head, rest @ ..] => {\n                    $tail = rest;\n    ", ""), (6155, "atch $tail", "e()"),
    (6154, "", "if value.is_so"), (6139, "", "$owner.$cursor.item"),
    (6138, "", ".get"), (6131, "us", "tory"),
    (6129, "n", "t value = hi"), (6126, "$c", "l"),
    (6111, ")]", "ory = &$history;"), (6108, "e", " hi"),
    (6101, "#[cfg(", "le"), (4416, "nt, $census:ident", "xpr"),
    (4413, "id", ""), (4409, "ail", "ory"),
    (4408, "", "his"), (4285, " + 4 * TAIL_VIEW", ""),
    (3240, ">()\n    + size_of::<usize", ""), (3050, ">()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]", ""),
    (3014, "=", "   + 3 * size_of::<Option<&Rc<SkeletonObservation>>>()\n    + 2 * size_of::<&Option<&Rc<SkeletonObservation>>>()\n    + size_of::<bool>()\n    + size_of::<usize>()\n    +"), (3013, "", ">()\n"),
    (3007, " ", ":<"), (2998, "nst TAIL", "f"),
    (2993, "r) c", "_"), (2990, "up", "iz"),
    (2985, "pub(", "    + 2 * "), (2983, ";", ""),
    (2940, "2", "3"), (2937, "", "\n   "),
    (2890, "", " +"), (2886, "", " size_of::<&History>()"),
    (2872, "_VIEW", ""), (2850, "\n", ""),
    (2723, " below", ""), (2687, " cursor item increment is still", "y"),
    (2601, ", assignment, as_ref output,\n                         // Option transport and origina", ""), (2557, " immutable Vec slice; each yielded head/re", ""),
    (2520, "forms\n                         // ", "h"), (2519, "", "main"),
    (2514, "scop", "r"), (2496, " above. Eac", ""),
    (2491, "main", ""), (2489, "", "and f"),
];
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (20919, "us", "tory"), (20888, "l,\n                        cen", ""),
    (20886, "a", "h.h"), (20883, "n_", ""),
    (20881, "l", ""), (19054, "us", "tory"),
    (19023, "l,\n                        cen", ""), (19021, "a", "h.h"),
    (19018, "n_", ""), (19016, "l", ""),
    (17925, ";\n        let mut plan_tail = path.history.tail.as_slice()", ""), (17653, " + lexical_cursor::TAIL_VIEW", ""),
    (14624, "us", "tory"), (14589, "l,\n                            cen", ""),
    (14587, "a", "h.h"), (14586, "", "pa"),
    (14160, "us", "tory"), (14125, "l,\n                            cen", ""),
    (14123, "a", "h.h"), (14122, "", "pa"),
    (13452, ";\n                    let mut left_tail = left_path.history.tail.as_slice();\n                    let mut right_tail = right_path.history.tail.as_slice()", ""), (13193, " + 2 * lexical_cursor::TAIL_VIEW", ""),
];

#[cfg(test)]
mod tests {
    use super::*;
    const CURRENT: &str = include_str!("../lexical_cursor.rs");
    const BEFORE: &str = include_str!("../before_pair_scope/lexical_cursor.rs");
    #[test]
    fn retained_tail_witness_covers_the_whole_macro_and_owner() {
        assert_eq!(previous(CURRENT), BEFORE);
        assert_eq!(
            previous_owner(include_str!("../../comparison_owner.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/retained-tail-7fb0-20261018/before-owner.rs.txt"
            ))
        );
    }
    #[test]
    fn retained_tail_witness_rejects_span_extent_and_unrecognized_body_drift() {
        let stage = prepared_links::previous_macro(CURRENT);
        let current = stage.as_str();
        let previous = previous_stage;
        let &(at, after, _) = MACRO
            .iter()
            .find(|(_, after, _)| !after.is_empty())
            .unwrap();
        let mut changed = current.to_owned();
        let end = at + after.chars().next().unwrap().len_utf8();
        let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
        changed.replace_range(at..end, replacement);
        assert!(std::panic::catch_unwind(|| previous(&changed)).is_err());
        let changed = format!("{current} ");
        assert!(std::panic::catch_unwind(|| previous(&changed)).is_err());
        let changed = current.replacen("macro_rules!", "macro_rulex!", 1);
        assert_ne!(changed, current);
        let result = std::panic::catch_unwind(|| previous(&changed));
        assert!(result.is_err() || result.unwrap() != BEFORE);
    }
}
