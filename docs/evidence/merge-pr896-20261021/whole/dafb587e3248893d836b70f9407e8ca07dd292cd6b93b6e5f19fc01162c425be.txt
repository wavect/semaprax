//! Exact complete-source witness for consuming prepared PLAN links.
//! This semantic transformation is separate from formatting and retained tails.
fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(
        current.len(),
        length,
        "unknown prepared-link subject extent"
    );
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("prepared-link inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(&result[at..end], after, "unknown prepared-link source span");
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
mod reachable_primitives;
fn previous_macro_stage(current: &str) -> String {
    restore(current, 6457, MACRO)
}
pub(super) fn previous_macro(current: &str) -> String {
    previous_macro_stage(&reachable_primitives::previous_macro(current))
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (5160, "                    #[cfg(test)]\n                    $census.load();\n                    let Some(link) = $owner.$cursor.links.pop() else {\n                        unreachable!(\"prepared ordinary plan links\");\n                    };\n", "                    let links = $owner.$cursor.links.as_slice();\n                    let link = links[links.len() - $owner.$cursor.segment - 1];\n"),
    (4701, "                #[cfg(test)]\n                $census.step();\n                if $owner.$cursor.segment >= $owner.$limit {\n", "                if $owner.$cursor.segment >= $owner.$cursor.links.len() {\n"),
    (4215, "    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,\n", "    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident,\n"),
    (1674, "pub(super) const LIMIT: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<usize>();\n// Prepared backward links are consumed from the back. Their original extent\n// remains in the comparison owner; the cursor segment/item counters stay exact.\npub(super) const LOAD: usize = 2 * size_of::<&mut Vec<usize>>()\n    + size_of::<Option<usize>>()\n    + size_of::<usize>()\n    + size_of::<bool>()\n    + 2 * size_of::<&[Segment]>()\n    + 2 * size_of::<usize>()\n    + size_of::<&Segment>()\n    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n", "pub(super) const LOAD: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>() // as_slice actual/formal/return/local\n    + 2 * size_of::<&[usize]>() + size_of::<usize>() // len actual/formal/return\n    + 2 * size_of::<usize>() // subtraction intermediates\n    + 2 * size_of::<&[usize]>() + 2 * size_of::<usize>() // slice index actual/formal\n    + size_of::<&usize>() + size_of::<usize>() // returned reference and copied link\n    + 2 * size_of::<&[Segment]>() + 2 * size_of::<usize>() + size_of::<&Segment>()\n    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>() // items as_slice actual/formal\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // as_slice return and stored field copy\n"),
    (983, "pub(super) const STEP: usize = 2 * size_of::<usize>() + size_of::<bool>() // retained segment extent and condition\n", "pub(super) const STEP: usize = 2 * size_of::<&Vec<usize>>() + size_of::<usize>() + size_of::<bool>() // unchanged len/condition\n"),
];
pub(super) fn previous_owner(current: &str) -> String {
    restore(current, 22474, OWNER)
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (21474, "                        left_segments,\n", ""),
    (19570, "                        left_segments,\n", ""),
    (18378, "        owner.left_segments = owner.left_cursor.links.len();\n", ""),
    (18051, "            PREPARE_CALL\n                + lexical_cursor::RESET\n                + lexical_cursor::TAIL_VIEW\n                + lexical_cursor::LIMIT,\n", "            PREPARE_CALL + lexical_cursor::RESET + lexical_cursor::TAIL_VIEW,\n"),
    (14954, "                            right_segments,\n", ""),
    (14448, "                            left_segments,\n", ""),
    (12824, "                owner.right_segments = owner.right_cursor.links.len();\n", ""),
    (12431, "                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,\n", "                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET,\n"),
    (12198, "                owner.left_segments = owner.left_cursor.links.len();\n", ""),
    (11808, "                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET + lexical_cursor::LIMIT,\n", "                    PREPARE_CALL + PATH_INDEX + lexical_cursor::RESET,\n"),
    (8899, "        left_segments: 0,\n        right_segments: 0,\n", ""),
    (7503, "    + 2 * size_of::<usize>() // initial retained segment extents\n", ""),
    (640, "    left_segments: usize,\n    right_segments: usize,\n", ""),
];
pub(super) fn previous_tail_source(current: &str) -> String {
    restore(current, 5600, TAIL_SOURCE)
}
#[rustfmt::skip]
const TAIL_SOURCE: &[(usize, &str, &str)] = &[
    (5346, "        let changed = current.replacen(\"macro_rules!\", \"macro_rulex!\", 1);\n        assert_ne!(changed, current);\n", "        let changed = CURRENT.replacen(\"macro_rules!\", \"macro_rulex!\", 1);\n        assert_ne!(changed, CURRENT);\n"),
    (5226, "        let changed = format!(\"{current} \");\n", "        let changed = format!(\"{CURRENT} \");\n"),
    (4911, "        let mut changed = current.to_owned();\n", "        let mut changed = CURRENT.to_owned();\n"),
    (4641, "        let stage = prepared_links::previous_macro(CURRENT);\n        let current = stage.as_str();\n        let previous = previous_stage;\n", ""),
    (1151, "    restore(&prepared_links::previous_owner(current), 21817, OWNER)\n", "    restore(current, 21817, OWNER)\n"),
    (982, "pub(super) fn previous(current: &str) -> String {\n    previous_stage(&prepared_links::previous_macro(current))\n}\n", ""),
    (881, "mod prepared_links;\nfn previous_stage(current: &str) -> String {\n", "pub(super) fn previous(current: &str) -> String {\n"),
];

#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        current: &str,
        inverse: fn(&str) -> String,
        before: &str,
        edits: &[(usize, &str, &str)],
    ) {
        let &(at, after, _) = edits
            .iter()
            .find(|(_, after, _)| !after.is_empty())
            .unwrap();
        let mut changed = current.to_owned();
        let end = at + after.chars().next().unwrap().len_utf8();
        let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
        changed.replace_range(at..end, replacement);
        assert!(std::panic::catch_unwind(|| inverse(&changed)).is_err());
        assert!(std::panic::catch_unwind(|| inverse(&format!("{current} "))).is_err());
        let at = current
            .char_indices()
            .find(|(i, c)| {
                c.is_ascii_alphanumeric()
                    && !edits
                        .iter()
                        .any(|(at, after, _)| *i >= *at && *i < at + after.len())
            })
            .unwrap()
            .0;
        let mut outside = current.to_owned();
        let replacement = if &outside[at..at + 1] == "x" {
            "y"
        } else {
            "x"
        };
        outside.replace_range(at..at + 1, replacement);
        let restored = std::panic::catch_unwind(|| inverse(&outside));
        assert!(restored.is_err() || restored.unwrap() != before);
    }
    #[test]
    fn complete_macro_owner_and_prior_tail_controls_have_exact_before_bytes() {
        assert_eq!(
            previous_macro(include_str!("../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_owner(&super::super::prepared_owner_stage(include_str!(
                "../../../comparison_owner.rs"
            ))),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-owner.rs.txt"
            ))
        );
        assert_eq!(
            previous_tail_source(&super::super::prepared_tail_stage(include_str!(
                "../tail_source.rs"
            ))),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-tail-source.rs.txt"
            ))
        );
    }
    #[test]
    fn unknown_span_extent_and_outside_body_drift_never_match_whole_before_sources() {
        let macro_stage =
            reachable_primitives::previous_macro(include_str!("../../lexical_cursor.rs"));
        hostile(
            &macro_stage,
            previous_macro_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &super::super::prepared_owner_stage(include_str!("../../../comparison_owner.rs")),
            previous_owner,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-owner.rs.txt"
            )),
            OWNER,
        );
        hostile(
            &super::super::prepared_tail_stage(include_str!("../tail_source.rs")),
            previous_tail_source,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/prepared-links-ad676-20261018/before-tail-source.rs.txt"
            )),
            TAIL_SOURCE,
        );
    }
}
