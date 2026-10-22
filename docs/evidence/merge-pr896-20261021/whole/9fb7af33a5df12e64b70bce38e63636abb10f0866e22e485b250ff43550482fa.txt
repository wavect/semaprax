//! Exact pair-local link and sealed unit-driver successor.
fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(current.len(), length, "unknown shared-step subject extent");
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("shared step inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(&result[at..end], after, "unknown shared step source span");
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
pub(super) fn previous_owner(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
        "23469f3a0ec012a0975f50e627811aa4874fdf4215b6a1c734b1c93a7a043955",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
        )),
    );
    let current = stage.as_str();
    previous_owner_tail_stage(&paired_tails::previous_owner(current))
}
fn previous_owner_tail_stage(current: &str) -> String {
    restore(current, 23894, OWNER)
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (16417, "                            right_links,\n", ""),
    (15869, "                            left_links,\n", ""),
    (15042, "                            left_links,\n                            right_links,\n", ""),
    (14776, "                    let mut left_links = owner.left_cursor.links.as_slice();\n                    let mut right_links = owner.right_cursor.links.as_slice();\n", ""),
    (14296, "                    PAIR_SCOPE + 2 * lexical_cursor::TAIL_VIEW + 2 * lexical_cursor::PAIR_LINK_VIEW,\n", "                    PAIR_SCOPE + 2 * lexical_cursor::TAIL_VIEW,\n"),
];
pub(super) fn previous_macro(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
        "b66e1c1f47583c8c687e352be56bcb01e49da9456b81c3e36862a359416e5583",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
        )),
    );
    let current = stage.as_str();
    previous_macro_tail_stage(&paired_tails::previous_macro(current))
}
fn previous_macro_tail_stage(current: &str) -> String {
    restore(current, 21368, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (20827, "        #[cfg(test)]\n        $census.shared_turns(); // sole terminal None\n", ""),
    (20817, "", "            shared_remaining -= 1;\n"),
    (20015, "", "            if shared_remaining == 0 {\n                break;\n            }\n"),
    (19793, "        let shared_work_driver: lexical_cursor::SharedUnitDriver =\n            std::iter::repeat(()).take(shared_length);\n        for () in shared_work_driver {\n", "        let mut shared_remaining = shared_length;\n        loop {\n            // Full condition/decrement envelope, including the zero sentinel.\n            paid!(\n                $budget,\n                $function,\n                4 * size_of::<usize>() + size_of::<bool>(),\n                $phase\n            )?;\n"),
    (19617, "        paid!($budget, $function, lexical_cursor::RUN_SETUP, $phase)?;\n", "        paid!(\n            $budget,\n            $function,\n            2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>(),\n            $phase\n        )?;\n"),
    (18298, "                            $right_links = right_links_rest;\n                            $owner.right_link = right_next;\n", "                            let Some(right_link) = $owner.right_cursor.links.pop() else {\n                                unreachable!(\"validated ordinary plan next segment\");\n                            };\n                            $owner.right_link = right_link;\n"),
    (16878, "                            $left_links = left_links_rest;\n                            $owner.left_link = left_next;\n", "                            let Some(left_link) = $owner.left_cursor.links.pop() else {\n                                unreachable!(\"validated ordinary plan next segment\");\n                            };\n                            $owner.left_link = left_link;\n"),
    (15366, "                                    + (4 * size_of::<usize>() + 4 * size_of::<usize>())\n                                    + 4 * size_of::<bool>()\n", ""),
    (15226, "                                2 * size_of::<&[usize]>()\n", "                                4 * size_of::<&mut Vec<usize>>()\n                                    + 2 * size_of::<Option<usize>>()\n                                    + 10 * size_of::<usize>()\n                                    + 6 * size_of::<bool>()\n"),
    (14120, "\n            if let &[ref left_links_rest @ .., left_next] = $left_links {\n                if let &[ref right_links_rest @ .., right_next] = $right_links {\n", "            let left_links = $owner.left_cursor.links.as_slice();\n            let right_links = $owner.right_cursor.links.as_slice();\n            if let &[.., left_next] = left_links {\n                if let &[.., right_next] = right_links {\n"),
    (13928, "                2 * size_of::<&[usize]>() + 4 * size_of::<usize>() + 3 * size_of::<bool>(),\n", "                4 * size_of::<&Vec<usize>>()\n                    + 4 * size_of::<&[usize]>()\n                    + 4 * size_of::<usize>()\n                    + 3 * size_of::<bool>(),\n"),
    (13567, "    ($owner:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n", "    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n"),
    (12924, "                shared_plan_boundary!(\n                    $owner,\n                    $left_links,\n                    $right_links,\n                    $census,\n                    $function,\n                    $budget,\n                    $phase\n                );\n", "                shared_plan_boundary!($owner, $census, $function, $budget, $phase);\n"),
    (11892, "    ($owner:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n", "    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n"),
    (11543, "// Five payment and five work sites have distinct complete primitive frames in\n", "// Six payment and five work sites have distinct complete primitive frames in\n"),
    (8679, "macro_rules! lexical_pair_plan_next {\n    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,\n     $tail:ident, $links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {{\n        $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n        'plan_yield: {\n            loop {\n                paid!(\n                    $budget,\n                    $function,\n                    lexical_cursor::STEP + lexical_cursor::EMPTY,\n                    $phase\n                )?;\n                #[cfg(test)]\n                $census.step();\n                if $owner.$cursor.segment >= $owner.$limit {\n                    break;\n                }\n                $budget.charge(\n                    $function,\n                    1,\n                    \"ordinary plan history forward segment traversal\",\n                )?;\n                if !$owner.$loaded {\n                    paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;\n                    #[cfg(test)]\n                    $census.load();\n                    let &[ref links_rest @ .., link] = $links else {\n                        unreachable!(\"prepared ordinary pair-local links\");\n                    };\n                    $links = links_rest;\n                    remember_plan_segment!($owner, $cursor, link);\n                    $owner.$items = $owner.arena[link].items.as_slice();\n                    $owner.$loaded = true;\n                }\n                match $owner.$items {\n                    [head, rest @ ..] => {\n                        $owner.$items = rest;\n                        $owner.$cursor.item += 1;\n                        break 'plan_yield Some::<&SkeletonObservation>(head.as_ref());\n                    }\n                    [] => {\n                        $owner.$cursor.segment += 1;\n                        $owner.$cursor.item = 0;\n                        $owner.$loaded = false;\n                    }\n                }\n            }\n            paid!($budget, $function, lexical_cursor::TAIL, $phase)?;\n            #[cfg(test)]\n            $census.tail();\n            match $tail {\n                [head, rest @ ..] => {\n                    $tail = rest;\n                    $owner.$cursor.item += 1;\n                    Some::<&SkeletonObservation>(head.as_ref())\n                }\n                [] => None,\n            }\n        }\n    }};\n}\n\n", ""),
    (6025, "    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,\n     $tail:ident, $links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        lexical_pair_plan_next!(\n            $owner, $cursor, $items, $loaded, $limit, $tail, $links, $census, $function, $budget,\n            $phase\n        )\n    };\n", ""),
    (4829, "// A fixed run-local Some(())/terminal-None caller frame. Both variants carry\n// no owned or borrowed data. This is separate from the accepted Result rule.\npub(super) type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;\npub(super) const RUN_SETUP: usize = 2 * size_of::<&[Rc<SkeletonObservation>]>() + 2 * size_of::<usize>() // len actual/formal/result/receiver\n    + 2 * size_of::<usize>() + 6 * size_of::<SharedUnitDriver>() // take constructor and IntoIterator transfers\n    + 2 * size_of::<&mut SharedUnitDriver>() + 2 * size_of::<Option<()>>(); // one retained next frame\nconst _: () = assert!(size_of::<()>() == 0 && !std::mem::needs_drop::<SharedUnitDriver>());\npub(super) const PAIR_LINK_VIEW: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>();\npub(super) const PAIR_LOAD: usize = 2 * size_of::<&[usize]>() + 2 * size_of::<usize>() + size_of::<bool>() // suffix pattern, assignment, copied/remembered link\n    + 2 * size_of::<&[Segment]>() + 2 * size_of::<usize>() + size_of::<&Segment>()\n    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n    + size_of::<bool>(); // loaded=true\n\n", ""),
    (489, "pub(super) const PRIMITIVE_FRAMES: usize = 41 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 42 * PAYMENT_HEADER;\n"),
    (339, "// The shared immutable-step branch adds five payment and five work sites.\n", "// The shared immutable-step branch adds six payment and five work sites.\n"),
];
#[path = "paired_links/constructor.rs"]
mod constructor;
pub(super) fn previous_pair_tests(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
        "b46e7bfec30cd08b716e6070975651a38d235b0a8ad3ffd851d7534fc0b94d52",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"
        )),
    );
    let current = stage.as_str();
    previous_pair_tests_stage(&constructor::previous_pair_tests(current))
}
fn previous_pair_tests_stage(current: &str) -> String {
    restore(current, 50482, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (49755, "\n#[test]\nfn sealed_unit_driver_is_exactly_unit_only_and_never_allocates_or_owns_cleanup() {\n    use lexical_cursor::SharedUnitDriver;\n    fn unit_iter<I: Iterator<Item = ()>>(_: &I) {}\n    assert_eq!(size_of::<()>(), 0);\n    assert!(!std::mem::needs_drop::<SharedUnitDriver>());\n    for length in [0, 1, 2, 4, 257] {\n        let ((), heap) = measure(|| {\n            let mut driver: SharedUnitDriver = std::iter::repeat(()).take(length);\n            unit_iter(&driver);\n            for _ in 0..length {\n                assert_eq!(driver.next(), Some(()));\n            }\n            assert_eq!(driver.next(), None); // this sole terminal call ends the primitive\n        });\n        assert_eq!(heap, Metrics::default());\n    }\n}\n", ""),
    (6523, "        + (4 * size_of::<&Vec<usize>>() + 4 * size_of::<&[usize]>()) * row.pairs\n        + (2 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 4 * size_of::<usize>()\n            + 6 * size_of::<std::iter::Take<std::iter::Repeat<()>>>()\n            + 2 * size_of::<&mut std::iter::Take<std::iter::Repeat<()>>>()\n            + 2 * size_of::<Option<()>>())\n            * row.shared_runs\n", "        + (2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>()) * row.shared_runs\n        + (4 * size_of::<usize>() + size_of::<bool>()) * row.shared_turns\n"),
    (6248, "        + if paths >= 2 { 10 * header } else { 0 }\n", "        + if paths >= 2 { 11 * header } else { 0 }\n"),
    (4886, "        + (4 * size_of::<usize>() + 4 * size_of::<usize>())\n        + 4 * size_of::<bool>()\n", ""),
    (4788, "    let advance = 2 * size_of::<&[usize]>()\n", "    let advance = 4 * size_of::<&mut Vec<usize>>()\n        + 2 * size_of::<Option<usize>>()\n        + 10 * size_of::<usize>()\n        + 6 * size_of::<bool>()\n"),
    (4447, "    let links = 2 * size_of::<&[usize]>() + 4 * size_of::<usize>() + 3 * size_of::<bool>();\n", "    let links = 4 * size_of::<&Vec<usize>>()\n        + 4 * size_of::<&[usize]>()\n        + 4 * size_of::<usize>()\n        + 3 * size_of::<bool>();\n"),
];
#[path = "paired_links/json_ledger.rs"]
mod json_ledger;
#[path = "paired_links/paired_tails.rs"]
mod paired_tails;
pub(super) fn previous_tail_census(current: &str) -> String {
    paired_tails::previous_census(current)
}
pub(super) fn previous_route(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/bounded_output/stage_debits/route_debits.rs",
        "250e351274cf1e0f745a0b8326f24fa6addbab59a4923313229bb9f8394c8c2a",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bounded_output/stage_debits/route_debits.rs"
        )),
    );
    let current = stage.as_str();
    previous_route_stage(&json_ledger::previous_route(current))
}
fn previous_route_stage(current: &str) -> String {
    restore(current, 35701, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (35487, "    for (phase, _) in direct.into_iter().chain(forwarded).chain(shared) {\n", "    for phase in direct.into_iter().chain(forwarded).chain(shared) {\n"),
    (34732, "            .matches(\"shared_plan_boundary!($owner,$left_links,$right_links,$census,$function,$budget,$phase);\")\n", "            .matches(\"shared_plan_boundary!($owner,$census,$function,$budget,$phase);\")\n"),
    (34545, "    assert_eq!(compact.matches(\"lexical_pair_plan_next!($owner,$cursor,$items,$loaded,$limit,$tail,$links,$census,$function,$budget,$phase)\").count(), 1);\n", ""),
    (34269, "    assert_eq!(sites.len(), 15, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 13, \"audit every generic payment site\");\n"),
    (33884, "    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 10).count(), 2);\n    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 11).count(), 2);\n", ""),
    (33551, "    let direct = phases(owner, \"paid!(\", &[4]);\n    let forwarded = phases(owner, \"lexical_plan_next!(\", &[10, 11]);\n    let shared = phases(owner, \"shared_plan_pair_step!(\", &[7]);\n", "    let direct = phases(owner, \"paid!(\", 4);\n    let forwarded = phases(owner, \"lexical_plan_next!(\", 10);\n    let shared = phases(owner, \"shared_plan_pair_step!(\", 5);\n"),
    (33303, "            phases.push((&body[first + 1..last], arity));\n", "            phases.push(&body[first + 1..last]);\n"),
    (33085, "            let arity = body[..first].matches(',').count() + 1;\n            assert!(arguments.contains(&arity), \"unknown macro arity\");\n", "            assert_eq!(body[..first].matches(',').count(), arguments - 1);\n"),
    (31549, "    fn phases<'a>(source: &'a str, token: &str, arguments: &[usize]) -> Vec<(&'a str, usize)> {\n", "    fn phases<'a>(source: &'a str, token: &str, arguments: usize) -> Vec<&'a str> {\n"),
];
pub(super) fn previous_reachable(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs",
        "15ffb8da630f6bfdd8bcd92399dd4bb3fef1716c1cc0e7b635a89bfd5a02393d",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 11150, REACHABLE)
}
#[rustfmt::skip]
const REACHABLE: &[(usize, &str, &str)] = &[
    (899, "mod paired_links;\n", ""),
];
pub(super) fn previous_stage1(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs",
        "0c0437fd827d8461cdda760700ce7d8186c79b49a6242cd30eb909c2d9044002",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 18935, STAGE1)
}
#[rustfmt::skip]
const STAGE1: &[(usize, &str, &str)] = &[
    (18550, "            &super::super::paired_links::previous_reachable(include_str!(\n                \"../reachable_primitives.rs\"\n            )),\n", "            include_str!(\"../reachable_primitives.rs\"),\n"),
    (17409, "            &super::super::paired_links::previous_owner(include_str!(\n                \"../../../../../comparison_owner.rs\"\n            )),\n", "            include_str!(\"../../../../../comparison_owner.rs\"),\n"),
    (12631, "    let current = super::paired_links::previous_reachable(current);\n    let current = current.as_str();\n", ""),
    (5993, "    let current = super::paired_links::previous_owner(current);\n    let current = current.as_str();\n", ""),
];
pub(super) fn previous_stage3(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs",
        "084e3c34169f471e5cc7a2f9b2b1593da9ce0b447a17178799877762f4c8e92d",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs"
        )),
    );
    let current = stage.as_str();
    previous_stage3_tail_stage(&paired_tails::previous_runs(current))
}
fn previous_stage3_tail_stage(current: &str) -> String {
    restore(current, 19581, STAGE3)
}
#[rustfmt::skip]
const STAGE3: &[(usize, &str, &str)] = &[
    (19327, "            )),\n", "            ),\n"),
    (19151, "            &super::super::super::super::paired_links::previous_route(include_str!(\n", "            include_str!(\n"),
    (18714, "            &super::super::super::super::paired_links::previous_boundary_controls(include_str!(\n                \"../shared_boundaries.rs\"\n            )),\n", "            include_str!(\"../shared_boundaries.rs\"),\n"),
    (17973, "            &super::super::super::super::paired_links::previous_pair_tests(include_str!(\n                \"../../../../../../pair_scope_tests.rs\"\n            )),\n", "            include_str!(\"../../../../../../pair_scope_tests.rs\"),\n"),
    (17572, "            &super::super::super::super::paired_links::previous_macro(include_str!(\n                \"../../../../../../lexical_cursor.rs\"\n            )),\n", "            include_str!(\"../../../../../../lexical_cursor.rs\"),\n"),
    (13800, "    let current = super::super::super::paired_links::previous_route(current);\n    let current = current.as_str();\n", ""),
    (11764, "    let current = super::super::super::paired_links::previous_boundary_controls(current);\n    let current = current.as_str();\n", ""),
    (6605, "    let current = super::super::super::paired_links::previous_pair_tests(current);\n    let current = current.as_str();\n", ""),
    (785, "    let current = super::super::super::paired_links::previous_macro(current);\n    let current = current.as_str();\n", ""),
];
pub(super) fn previous_boundary_controls(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs",
        "db28863e00f2732ab65d3322bbebe3bf2f5f9928f8cad813260d6e5ab6890b85",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 23201, STAGE2)
}
#[rustfmt::skip]
const STAGE2: &[(usize, &str, &str)] = &[
    (22410, "            &super::super::super::paired_links::previous_stage1(include_str!(\"../shared_steps.rs\")),\n            previous_shared_steps_stage,\n", "            include_str!(\"../shared_steps.rs\"),\n            previous_shared_steps,\n"),
    (15926, "    previous_shared_steps_stage(&super::super::paired_links::previous_stage1(current))\n}\nfn previous_shared_steps_stage(current: &str) -> String {\n", ""),
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
        let stage = super::incoming_current_stage(current);
        let current = stage.as_str();
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
    fn complete_pair_local_sources_restore_every_predecessor() {
        assert_eq!(
            previous_owner(include_str!("../../../../../comparison_owner.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-owner.rs.txt"
            ))
        );
        assert_eq!(
            previous_macro(include_str!("../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-route.rs.txt"
            ))
        );
        assert_eq!(
            previous_reachable(include_str!("../reachable_primitives.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-reachable.rs.txt"
            ))
        );
        assert_eq!(
            previous_stage1(include_str!("shared_steps.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage1.rs.txt"
            ))
        );
        assert_eq!(
            previous_stage3(include_str!(
                "shared_steps/shared_boundaries/shared_runs.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage3.rs.txt"
            ))
        );
        assert_eq!(
            previous_boundary_controls(include_str!("shared_steps/shared_boundaries.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage2.rs.txt"
            ))
        );
    }
    #[test]
    fn pair_local_sources_refuse_span_extent_and_outside_drift() {
        hostile(
            &super::paired_tails::previous_owner(include_str!(
                "../../../../../comparison_owner.rs"
            )),
            previous_owner_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-owner.rs.txt"
            )),
            OWNER,
        );
        hostile(
            &super::paired_tails::previous_macro(include_str!("../../../../lexical_cursor.rs")),
            previous_macro_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &super::constructor::previous_pair_tests(include_str!(
                "../../../../pair_scope_tests.rs"
            )),
            previous_pair_tests_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            &super::json_ledger::previous_route(include_str!(
                "../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            previous_route_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-route.rs.txt"
            )),
            ROUTE,
        );
        hostile(
            include_str!("../reachable_primitives.rs"),
            previous_reachable,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-reachable.rs.txt"
            )),
            REACHABLE,
        );
        hostile(
            include_str!("shared_steps.rs"),
            previous_stage1,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage1.rs.txt"
            )),
            STAGE1,
        );
        hostile(
            &super::paired_tails::previous_runs(include_str!(
                "shared_steps/shared_boundaries/shared_runs.rs"
            )),
            previous_stage3_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage3.rs.txt"
            )),
            STAGE3,
        );
        hostile(
            include_str!("shared_steps/shared_boundaries.rs"),
            previous_boundary_controls,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/pair-links-7e-20261018/before-stage2.rs.txt"
            )),
            STAGE2,
        );
    }
}

fn incoming_stage(current: &str, path: &str, sha: &str, live: &str) -> String {
    if current == live {
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, sha, current)
    } else {
        current.to_owned()
    }
}

// Closed current-first acquisition precedes mutations at genuine historical offsets.
fn incoming_current_stage(current: &str) -> String {
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bounded_output/stage_debits/route_debits.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/bounded_output/stage_debits/route_debits.rs",
                "250e351274cf1e0f745a0b8326f24fa6addbab59a4923313229bb9f8394c8c2a",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
                "23469f3a0ec012a0975f50e627811aa4874fdf4215b6a1c734b1c93a7a043955",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
                "b66e1c1f47583c8c687e352be56bcb01e49da9456b81c3e36862a359416e5583",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
                "e0ff7f5bd4f8a4fef1cc91114440e0819fde58098f5c64e1ba3bf28546537bbc",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
                "b46e7bfec30cd08b716e6070975651a38d235b0a8ad3ffd851d7534fc0b94d52",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs",
                "15ffb8da630f6bfdd8bcd92399dd4bb3fef1716c1cc0e7b635a89bfd5a02393d",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs",
                "0c85f1f992d5b831e601ea927e832e4f1df0cf5b0bcc27688eea9aa944a3b6f2",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs",
                "9dc41df52f983bad1552fd556c0af3db2622a8e7eb1328356db230db906520ae",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs",
                "4e01eab3ec8c582b781958702d3a82e044e959c93a49ba94e02c4be65ce8fa63",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs",
                "9bcdf29f73430c509a567889238b552be190de2d5b11bfb7ee6c22c21fae62e5",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs",
                "0c0437fd827d8461cdda760700ce7d8186c79b49a6242cd30eb909c2d9044002",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs",
                "db28863e00f2732ab65d3322bbebe3bf2f5f9928f8cad813260d6e5ab6890b85",
                live,
            );
        }
    }
    {
        let live = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs"
        ));
        if current == live {
            return incoming_stage(
                current,
                "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs",
                "084e3c34169f471e5cc7a2f9b2b1593da9ce0b447a17178799877762f4c8e92d",
                live,
            );
        }
    }
    current.to_owned()
}
