//! Complete immediate fused-reader source inverses; no prior checks removed.
mod independent_segments;
fn restore(current: &str, before: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(current.len(), length, "unknown fused-reader subject extent");
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, previous) in edits {
        let end = at
            .checked_add(after.len())
            .expect("fused-reader inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(&result[at..end], after, "unknown fused-reader source span");
        result.replace_range(at..end, previous);
        last = at;
    }
    assert_eq!(
        result, before,
        "whole predecessor must match, including outside spans"
    );
    result
}
pub(super) fn previous_owner(current: &str) -> String {
    if !exact_main_subject(current, 22498, "c8f66d4154fd2ed0bb830b4bcecb2c9153949b9c636ea2d4fc8977edfbf130f8") {
        return previous_owner_before_independent(current);
    }
    previous_owner_before_independent(&independent_segments::previous_owner(current))
}
fn previous_owner_before_independent(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
        "23469f3a0ec012a0975f50e627811aa4874fdf4215b6a1c734b1c93a7a043955",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-owner.rs.txt"
        )),
        22396,
        OWNER,
    )
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (16013, "                        lexical_plan_next!(owner, left_path, right_path, left_tail, right_tail,\n                            left_links, right_links, census, function, budget, 'paired_observations,\n                            \"ordinary plan history paired cursor next\");\n", "                        let right = lexical_plan_next!(\n                            owner,\n                            right_cursor,\n                            right_items,\n                            right_loaded,\n                            right_segments,\n                            right_tail,\n                            right_links,\n                            census,\n                            function,\n                            budget,\n                            \"ordinary plan history paired cursor next\"\n                        );\n                        match (left, right) {\n                            (Some(left), Some(right)) => {\n                                owner.ordering = left.cmp(right);\n                                if owner.ordering == Ordering::Equal {\n                                    budget.charge(\n                                        function,\n                                        1,\n                                        \"ordinary plan history observation comparison\",\n                                    )?;\n                                    #[cfg(test)]\n                                    census.round();\n                                    continue;\n                                }\n                            }\n                            (None, None) => {\n                                owner.ordering = left_path.terminal.cmp(&right_path.terminal)\n                            }\n                            (None, Some(_)) => owner.ordering = Ordering::Less,\n                            (Some(_), None) => owner.ordering = Ordering::Greater,\n                        }\n                        break;\n"),
    (15915, "", "                            function,\n                            budget,\n"),
    (15879, "", "                            PAIR_MATCH,\n                            \"ordinary plan history paired cursor next\"\n                        )?;\n                        let left = lexical_plan_next!(\n                            owner,\n                            left_cursor,\n                            left_items,\n                            left_loaded,\n                            left_segments,\n                            left_tail,\n                            left_links,\n"),
    (15761, "                        lexical_plan_next!(\n", "                        paid!(\n"),
    (15182, "                    'paired_observations: loop {\n", "                    loop {\n"),
];
pub(super) fn previous_macro(current: &str) -> String {
    if !exact_main_subject(current, 33725, "768c23a10fcec91ef7dfdbd67dabd64885266291f236c66efd0e80ad21b29b6e") {
        return previous_macro_before_independent(current);
    }
    previous_macro_before_independent(&independent_segments::previous_macro(current))
}
fn previous_macro_before_independent(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
        "b66e1c1f47583c8c687e352be56bcb01e49da9456b81c3e36862a359416e5583",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-macro.rs.txt"
        )),
        30104,
        MACRO,
    )
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (24787, "\nmacro_rules! paired_plan_observations {\n ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,\n  $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident,\n  $pair_loop:lifetime, $phase:literal) => {\n  $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n  let left_head = 'left_loan: {\n   loop {\n    paid!($budget, $function, lexical_cursor::PAIR_LEFT_STEP, $phase)?;\n  #[cfg(test)]\n  $census.pair_left_steps();\n    #[cfg(test)]\n    $census.step();\n    if $owner.left_cursor.segment >= $owner.left_segments { break; }\n    $budget.charge($function, 1, \"ordinary plan history forward segment traversal\")?;\n    if !$owner.left_loaded {\n     paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;\n     #[cfg(test)]\n     $census.load();\n     let &[ref links_rest @ .., link] = $left_links else { unreachable!(\"prepared ordinary pair-local links\"); };\n     $left_links = links_rest;\n     remember_plan_segment!($owner, left_cursor, link);\n     $owner.left_items = $owner.arena[link].items.as_slice();\n     $owner.left_loaded = true;\n    }\n    match $owner.left_items {\n     [head, rest @ ..] => { $owner.left_items = rest; $owner.left_cursor.item += 1; break 'left_loan head; }\n     [] => { $owner.left_cursor.segment += 1; $owner.left_cursor.item = 0; $owner.left_loaded = false; }\n    }\n   }\n   paid!($budget, $function, lexical_cursor::PAIR_LEFT_TAIL, $phase)?;\n  #[cfg(test)]\n  $census.pair_left_tails();\n   #[cfg(test)]\n   { $census.tail_marks(); $census.tail(); }\n   remember_plan_tail!($owner, left_cursor);\n   match $left_tail {\n    [head, rest @ ..] => { $left_tail = rest; $owner.left_cursor.item += 1; break 'left_loan head; }\n    [] =>\n{\n $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n loop {\n  paid!($budget, $function, lexical_cursor::PAIR_RIGHT_STEP, $phase)?;\n  #[cfg(test)]\n  $census.pair_right_steps();\n  #[cfg(test)]\n  $census.step();\n  if $owner.right_cursor.segment >= $owner.right_segments { break; }\n  $budget.charge($function, 1, \"ordinary plan history forward segment traversal\")?;\n  if !$owner.right_loaded {\n   paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;\n   #[cfg(test)]\n   $census.load();\n   let &[ref links_rest @ .., link] = $right_links else { unreachable!(\"prepared ordinary pair-local links\"); };\n   $right_links = links_rest;\n   remember_plan_segment!($owner, right_cursor, link);\n   $owner.right_items = $owner.arena[link].items.as_slice();\n   $owner.right_loaded = true;\n  }\n  match $owner.right_items {\n   [_, rest @ ..] => {\n    $owner.right_items = rest;\n    $owner.right_cursor.item += 1;\n    $owner.ordering = Ordering::Less;\nbreak $pair_loop;\n   }\n   [] => { $owner.right_cursor.segment += 1; $owner.right_cursor.item = 0; $owner.right_loaded = false; }\n  }\n }\n paid!($budget, $function, lexical_cursor::PAIR_RIGHT_TAIL, $phase)?;\n  #[cfg(test)]\n  $census.pair_right_tails();\n #[cfg(test)]\n { $census.tail_marks(); $census.tail(); }\n remember_plan_tail!($owner, right_cursor);\n match $right_tail {\n  [_, rest @ ..] => { $right_tail = rest; $owner.right_cursor.item += 1; $owner.ordering = Ordering::Less;\nbreak $pair_loop; }\n  [] => { $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal);\nbreak $pair_loop; }\n }\n}\n\n   }\n  };\n\n{\n $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n loop {\n  paid!($budget, $function, lexical_cursor::PAIR_RIGHT_STEP, $phase)?;\n  #[cfg(test)]\n  $census.pair_right_steps();\n  #[cfg(test)]\n  $census.step();\n  if $owner.right_cursor.segment >= $owner.right_segments { break; }\n  $budget.charge($function, 1, \"ordinary plan history forward segment traversal\")?;\n  if !$owner.right_loaded {\n   paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;\n   #[cfg(test)]\n   $census.load();\n   let &[ref links_rest @ .., link] = $right_links else { unreachable!(\"prepared ordinary pair-local links\"); };\n   $right_links = links_rest;\n   remember_plan_segment!($owner, right_cursor, link);\n   $owner.right_items = $owner.arena[link].items.as_slice();\n   $owner.right_loaded = true;\n  }\n  match $owner.right_items {\n   [right_head, rest @ ..] => {\n    $owner.right_items = rest;\n    $owner.right_cursor.item += 1;\n    $owner.ordering = left_head.as_ref().cmp(right_head.as_ref());\nif $owner.ordering == Ordering::Equal {\n    $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n    #[cfg(test)]\n    $census.round();\n    continue $pair_loop;\n}\nbreak $pair_loop;\n   }\n   [] => { $owner.right_cursor.segment += 1; $owner.right_cursor.item = 0; $owner.right_loaded = false; }\n  }\n }\n paid!($budget, $function, lexical_cursor::PAIR_RIGHT_TAIL, $phase)?;\n  #[cfg(test)]\n  $census.pair_right_tails();\n #[cfg(test)]\n { $census.tail_marks(); $census.tail(); }\n remember_plan_tail!($owner, right_cursor);\n match $right_tail {\n  [right_head, rest @ ..] => { $right_tail = rest; $owner.right_cursor.item += 1; $owner.ordering = left_head.as_ref().cmp(right_head.as_ref());\nif $owner.ordering == Ordering::Equal {\n    $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n    #[cfg(test)]\n    $census.round();\n    continue $pair_loop;\n}\nbreak $pair_loop; }\n  [] => { $owner.ordering = Ordering::Greater;\nbreak $pair_loop; }\n }\n}\n\n };\n}\n", ""),
    (10425, "", "            #[cfg(test)]\n            $census.tail();\n            match $tail {\n                [head, rest @ ..] => {\n                    $tail = rest;\n                    $owner.$cursor.item += 1;\n                    Some::<&SkeletonObservation>(head.as_ref())\n                }\n                [] => None,\n            }\n        }\n    }};\n}\n\nmacro_rules! lexical_pair_plan_next {\n    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,\n     $tail:ident, $links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {{\n        $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n        'plan_yield: {\n            loop {\n                paid!(\n                    $budget,\n                    $function,\n                    lexical_cursor::STEP + lexical_cursor::EMPTY,\n                    $phase\n                )?;\n                #[cfg(test)]\n                $census.step();\n                if $owner.$cursor.segment >= $owner.$limit {\n                    break;\n                }\n                $budget.charge(\n                    $function,\n                    1,\n                    \"ordinary plan history forward segment traversal\",\n                )?;\n                if !$owner.$loaded {\n                    paid!($budget, $function, lexical_cursor::PAIR_LOAD, $phase)?;\n                    #[cfg(test)]\n                    $census.load();\n                    let &[ref links_rest @ .., link] = $links else {\n                        unreachable!(\"prepared ordinary pair-local links\");\n                    };\n                    $links = links_rest;\n                    remember_plan_segment!($owner, $cursor, link);\n                    $owner.$items = $owner.arena[link].items.as_slice();\n                    $owner.$loaded = true;\n                }\n                match $owner.$items {\n                    [head, rest @ ..] => {\n                        $owner.$items = rest;\n                        $owner.$cursor.item += 1;\n                        break 'plan_yield Some::<&SkeletonObservation>(head.as_ref());\n                    }\n                    [] => {\n                        $owner.$cursor.segment += 1;\n                        $owner.$cursor.item = 0;\n                        $owner.$loaded = false;\n                    }\n                }\n            }\n            paid!(\n                $budget,\n                $function,\n                lexical_cursor::TAIL + 3 * size_of::<u8>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.tail_marks();\n            remember_plan_tail!($owner, $cursor);\n"),
    (8440, "        );\n", "        )\n"),
    (7688, "    ($budget:ident, $function:ident, $census:ident, $phase:literal) => {\n        paid!($budget, $function, lexical_cursor::PAIR_CONTENT, $phase)?;\n        #[cfg(test)]\n        $census.pair_receivers();\n    };\n    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,\n     $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident,\n     $pair_loop:lifetime, $phase:literal) => {\n        paired_plan_observations!(\n            $owner,\n            $left_path,\n            $right_path,\n            $left_tail,\n            $right_tail,\n            $left_links,\n            $right_links,\n            $census,\n            $function,\n            $budget,\n            $pair_loop,\n", "    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident,\n     $tail:ident, $links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        lexical_pair_plan_next!(\n            $owner, $cursor, $items, $loaded, $limit, $tail, $links, $census, $function, $budget,\n"),
    (6393, "pub(super) const PAIR_LEFT_STEP: usize = 2 * size_of::<usize>()\n    + 3 * size_of::<bool>()\n    + size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n    + 2 * size_of::<&Rc<SkeletonObservation>>()\n    + size_of::<usize>()\n    + EMPTY;\npub(super) const PAIR_RIGHT_STEP: usize = 2 * size_of::<usize>()\n    + 3 * size_of::<bool>()\n    + size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n    + size_of::<usize>()\n    + EMPTY;\npub(super) const PAIR_LEFT_TAIL: usize = size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n    + 2 * size_of::<&Rc<SkeletonObservation>>()\n    + size_of::<usize>()\n    + 3 * size_of::<u8>();\npub(super) const PAIR_RIGHT_TAIL: usize = size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n    + size_of::<usize>()\n    + 3 * size_of::<u8>();\npub(super) const PAIR_CONTENT: usize = 4 * size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&SkeletonObservation>()\n    + 4 * size_of::<&SkeletonObservation>()\n    + 4 * size_of::<&SkeletonTerminal>()\n    + 2 * size_of::<Ordering>();\nconst _: () =\n    assert!(PAIR_LEFT_STEP + PAIR_RIGHT_STEP + PAIR_CONTENT < 2 * (STEP + EMPTY) + PAIR_MATCH);\n\n", ""),
    (723, "// The fused fallback replaces two readers with left/right-some/right-none readers:\n// five extra primitive sites, separately funded even for mutually exclusive arms.\npub(super) const PRIMITIVE_FRAMES: usize = 51 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 46 * PAYMENT_HEADER;\n"),
    (261, "// Two final-HIR readers and three paired branches each retain every work site.\n// The paired branches transport plain immutable Rc loans without Option outputs.\n", "// Four expansions each have forward+segment work and STEP+LOAD+TAIL payment.\n"),
];
pub(super) fn previous_census(current: &str) -> String {
    if !exact_main_subject(current, 22294, "38549d9a47ec78e4f1153bdc27f04944a05a7a4d40ab284550f4b210d9cb0570") {
        return previous_census_before_independent(current);
    }
    previous_census_before_independent(&independent_segments::previous_census(current))
}
fn previous_census_before_independent(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
        "e0ff7f5bd4f8a4fef1cc91114440e0819fde58098f5c64e1ba3bf28546537bbc",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-census.rs.txt"
        )),
        21432,
        CENSUS,
    )
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (20049, "    #[test]\n    fn fused_reader_counter_overflow_is_incomplete_and_does_not_spend_work() {\n        for counter in 0..5 {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                match counter {\n                    0 => trace.rows[0].pair_left_steps = usize::MAX,\n                    1 => trace.rows[0].pair_right_steps = usize::MAX,\n                    2 => trace.rows[0].pair_left_tails = usize::MAX,\n                    3 => trace.rows[0].pair_right_tails = usize::MAX,\n                    _ => trace.rows[0].pair_receivers = usize::MAX,\n                }\n            });\n            match counter {\n                0 => guard.pair_left_steps(),\n                1 => guard.pair_right_steps(),\n                2 => guard.pair_left_tails(),\n                3 => guard.pair_right_tails(),\n                _ => guard.pair_receivers(),\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (6438, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn pair_left_steps(&self) {\n        self.update(|r| match r.pair_left_steps.checked_add(1) {\n            Some(n) => r.pair_left_steps = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn pair_right_steps(&self) {\n        self.update(|r| match r.pair_right_steps.checked_add(1) {\n            Some(n) => r.pair_right_steps = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn pair_left_tails(&self) {\n        self.update(|r| match r.pair_left_tails.checked_add(1) {\n            Some(n) => r.pair_left_tails = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn pair_right_tails(&self) {\n        self.update(|r| match r.pair_right_tails.checked_add(1) {\n            Some(n) => r.pair_right_tails = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn pair_receivers(&self) {\n        self.update(|r| match r.pair_receivers.checked_add(1) {\n            Some(n) => r.pair_receivers = n,\n", ""),
    (1487, "    pair_left_steps: 0,\n    pair_right_steps: 0,\n    pair_left_tails: 0,\n    pair_right_tails: 0,\n    pair_receivers: 0,\n", ""),
    (592, "    pub pair_left_steps: usize,\n    pub pair_right_steps: usize,\n    pub pair_left_tails: usize,\n    pub pair_right_tails: usize,\n    pub pair_receivers: usize,\n", ""),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    if !exact_main_subject(current, 55874, "2ef71d1a1613cab3e894b35d5aa07f9207e81917557827e10bc84a2e988dda0a") {
        return previous_pair_tests_before_independent(current);
    }
    previous_pair_tests_before_independent(&independent_segments::previous_pair_tests(current))
}
fn previous_pair_tests_before_independent(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
        "b46e7bfec30cd08b716e6070975651a38d235b0a8ad3ffd851d7534fc0b94d52",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-pair_tests.rs.txt"
        )),
        55208,
        PAIR_TESTS,
    )
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (52177, "\n#[test]\nfn fused_fallback_real_values_keep_success_and_every_work_refusal() {\n    let program = super::super::super::tests::program();\n    let function = program\n        .functions\n        .iter()\n        .find(|f| f.id.as_str() == \"f\")\n        .unwrap();\n    let item = Rc::new(SkeletonObservation::Boolean {\n        expression: function.body.id.clone(),\n        value: true,\n    });\n    let other = Rc::new(SkeletonObservation::Boolean {\n        expression: function.body.id.clone(),\n        value: false,\n    });\n    let expected = vec![\n        SkeletonPath {\n            observations: vec![item.clone(); 64].into(),\n            terminal: SkeletonTerminal::Success,\n        };\n        2\n    ];\n    for mutation in 0..2 {\n        let actual = || {\n            let mut left = vec![item.clone(); 64];\n            if mutation == 1 {\n                left[63] = other.clone();\n            }\n            HistorySet {\n                arena: vec![\n                    Segment {\n                        previous: None,\n                        items: left,\n                        depth: 1,\n                    },\n                    Segment {\n                        previous: None,\n                        items: vec![item.clone(); 64],\n                        depth: 1,\n                    },\n                ],\n                max_depth: 1,\n                paths: (0..2)\n                    .map(|side| PlanPath {\n                        history: History {\n                            previous: Some(side),\n                            tail: Vec::new(),\n                        },\n                        terminal: SkeletonTerminal::Success,\n                    })\n                    .collect(),\n            }\n        };\n        pair_scope_census::reset();\n        let (old, old_heap, _) =\n            observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);\n        pair_scope_census::reset();\n        let (new, heap, _) =\n            observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);\n        let (rows, lost) = pair_scope_census::finish();\n        assert_eq!(lost, 0);\n        assert_eq!(rows.len(), 1);\n        let row = rows[0];\n        assert!(!row.old && row.ended && !row.unwound && !row.truncated && !row.overflow);\n        assert_eq!(row.complete, mutation == 0);\n        assert_eq!(new.result.is_ok(), mutation == 0);\n        assert!(row.pair_receivers > 0 && row.pair_left_steps > 0 && row.pair_right_steps > 0);\n        assert_eq!(new, old);\n        assert_eq!(heap, old_heap);\n        for cut in 0..=MAX_REPLAY_WORK_UNITS - old.remaining {\n            pair_scope_census::reset();\n            let (old, _, _) = observe_shaped_pair(function, &expected, actual(), true, cut);\n            pair_scope_census::reset();\n            let (new, _, _) = observe_shaped_pair(function, &expected, actual(), false, cut);\n            assert_eq!(\n                new, old,\n                \"all diagnostic fields/work, mutation{mutation} cut{cut}\"\n            );\n        }\n    }\n}\n", ""),
    (7548, "\n// Independent of the producer constants and reservation delta. All old\n// physical/work/diagnostic fixtures use this invoice, including full HIR.\nfn fused_reader_prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {\n    let header = 2 * size_of::<&ResolvedFunction>()\n        + 2 * size_of::<&mut ReplayBudget>()\n        + 2 * size_of::<&str>()\n        + 2 * size_of::<usize>()\n        + 2 * size_of::<Result<(), Diagnostic>>();\n    let options = 2 * size_of::<Option<&SkeletonObservation>>();\n    let as_ref = 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>();\n    let left_loan = 2 * size_of::<&Rc<SkeletonObservation>>();\n    let old_match = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()\n        + 2 * size_of::<&SkeletonObservation>();\n    let comparison_loans =\n        4 * size_of::<&Rc<SkeletonObservation>>() + 2 * size_of::<&SkeletonObservation>();\n    (\n        (options + as_ref)\n            * (row.pair_left_steps\n                + row.pair_right_steps\n                + row.pair_left_tails\n                + row.pair_right_tails)\n            + old_match * row.pair_receivers,\n        if paths >= 2 { 5 * header } else { 0 }\n            + left_loan * (row.pair_left_steps + row.pair_left_tails)\n            + comparison_loans * row.pair_receivers,\n    )\n}\n\n", ""),
    (7371, "        (pair + 2 * step) * row.shared_steps + boundary_removed + tail_removed + reader_removed,\n        retained + boundary_retained + tail_retained + reader_retained,\n", "        (pair + 2 * step) * row.shared_steps + boundary_removed + tail_removed,\n        retained + boundary_retained + tail_retained,\n"),
    (7288, "    let (reader_removed, reader_retained) = fused_reader_prices(paths, row);\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    if !exact_main_subject(current, 36241, "9e0b85cb25e817c7e7c87ef86455819f54c7b9657df6064165d41c99f5f0592d") {
        return previous_route_before_independent(current);
    }
    previous_route_before_independent(&independent_segments::previous_route(current))
}
fn previous_route_before_independent(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/bounded_output/stage_debits/route_debits.rs",
        "250e351274cf1e0f745a0b8326f24fa6addbab59a4923313229bb9f8394c8c2a",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bounded_output/stage_debits/route_debits.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-route.rs.txt"
        )),
        36033,
        ROUTE,
    )
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (34776, "    assert!(!compact.contains(\"lexical_pair_plan_next!\"));\n    assert_eq!(compact.matches(\"paired_plan_observations!($owner,$left_path,$right_path,$left_tail,$right_tail,$left_links,$right_links,$census,$function,$budget,$pair_loop,$phase);\").count(), 1);\n", "    assert_eq!(compact.matches(\"lexical_pair_plan_next!($owner,$cursor,$items,$loaded,$limit,$tail,$links,$census,$function,$budget,$phase)\").count(), 1);\n"),
    (34644, "    assert_eq!(lexical.matches(\"remember_plan_tail!(\").count(), 3);\n", "    assert_eq!(lexical.matches(\"remember_plan_tail!(\").count(), 1);\n"),
    (34365, "    assert_eq!(sites.len(), 24, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 17, \"audit every generic payment site\");\n"),
    (33981, "    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 4).count(), 1);\n    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 12).count(), 1);\n", "    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 11).count(), 2);\n"),
    (33830, "        (12, 4, 1),\n", "        (13, 4, 1),\n"),
    (33621, "    let forwarded = phases(owner, \"lexical_plan_next!(\", &[4, 10, 12]);\n", "    let forwarded = phases(owner, \"lexical_plan_next!(\", &[10, 11]);\n"),
];
pub(super) fn previous_controls(current: &str) -> String {
    let stage = super::super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs",
        "9bcdf29f73430c509a567889238b552be190de2d5b11bfb7ee6c22c21fae62e5",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/paired_tails.rs"
        )),
    );
    let current = stage.as_str();
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/fused-readers-eb819-20261018/before-controls.rs.txt"
        )),
        22664,
        CONTROLS,
    )
}
#[rustfmt::skip]
const CONTROLS: &[(usize, &str, &str)] = &[
    (21023, "            &fused_readers::previous_route(include_str!(\n                \"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"\n            )),\n            previous_route_before_fused,\n", "            include_str!(\"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"),\n            previous_route,\n"),
    (20646, "            &fused_readers::previous_pair_tests(include_str!(\"../../../../../pair_scope_tests.rs\")),\n            previous_pair_tests_before_fused,\n", "            include_str!(\"../../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (20284, "            &fused_readers::previous_census(include_str!(\"../../../../../pair_scope_census.rs\")),\n            previous_census_before_fused,\n", "            include_str!(\"../../../../../pair_scope_census.rs\"),\n            previous_census,\n"),
    (19929, "            &fused_readers::previous_macro(include_str!(\"../../../../../lexical_cursor.rs\")),\n            previous_macro_before_fused,\n", "            include_str!(\"../../../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (19569, "            &fused_readers::previous_owner(include_str!(\"../../../../../../comparison_owner.rs\")),\n            previous_owner_before_fused,\n", "            include_str!(\"../../../../../../comparison_owner.rs\"),\n            previous_owner,\n"),
    (10149, "    previous_route_before_fused(&fused_readers::previous_route(current))\n}\nfn previous_route_before_fused(current: &str) -> String {\n", ""),
    (9245, "    previous_pair_tests_before_fused(&fused_readers::previous_pair_tests(current))\n}\nfn previous_pair_tests_before_fused(current: &str) -> String {\n", ""),
    (6957, "    previous_census_before_fused(&fused_readers::previous_census(current))\n}\nfn previous_census_before_fused(current: &str) -> String {\n", ""),
    (1848, "    previous_macro_before_fused(&fused_readers::previous_macro(current))\n}\nfn previous_macro_before_fused(current: &str) -> String {\n", ""),
    (816, "    previous_owner_before_fused(&fused_readers::previous_owner(current))\n}\nfn previous_owner_before_fused(current: &str) -> String {\n", ""),
    (73, "#[path = \"fused_readers.rs\"]\nmod fused_readers;\n", ""),
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_fused_reader_subjects_restore_and_refuse_all_source_drift() {
        {
            let current = include_str!("../../../../../../comparison_owner.rs");
            assert!(std::panic::catch_unwind(|| previous_owner(&format!("{current} "))).is_err());
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_owner(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_owner(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-owner.rs.txt"
                ))
            );
            for (at, _, _) in OWNER {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_owner(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_owner(&format!("{current} "))).is_err());
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_owner(&outside)).is_err());
        }
        {
            let current = include_str!("../../../../../lexical_cursor.rs");
            assert!(std::panic::catch_unwind(|| previous_macro(&format!("{current} "))).is_err());
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_macro(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_macro(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-macro.rs.txt"
                ))
            );
            for (at, _, _) in MACRO {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_macro(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_macro(&format!("{current} "))).is_err());
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_macro(&outside)).is_err());
        }
        {
            let current = include_str!("../../../../../pair_scope_census.rs");
            assert!(std::panic::catch_unwind(|| previous_census(&format!("{current} "))).is_err());
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_census(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_census(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-census.rs.txt"
                ))
            );
            for (at, _, _) in CENSUS {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_census(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_census(&format!("{current} "))).is_err());
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_census(&outside)).is_err());
        }
        {
            let current = include_str!("../../../../../pair_scope_tests.rs");
            assert!(
                std::panic::catch_unwind(|| previous_pair_tests(&format!("{current} "))).is_err()
            );
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_pair_tests(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_pair_tests(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-pair_tests.rs.txt"
                ))
            );
            for (at, _, _) in PAIR_TESTS {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_pair_tests(&changed)).is_err());
                }
            }
            assert!(
                std::panic::catch_unwind(|| previous_pair_tests(&format!("{current} "))).is_err()
            );
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_pair_tests(&outside)).is_err());
        }
        {
            let current = include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            );
            assert!(std::panic::catch_unwind(|| previous_route(&format!("{current} "))).is_err());
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_route(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_route(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-route.rs.txt"
                ))
            );
            for (at, _, _) in ROUTE {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_route(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_route(&format!("{current} "))).is_err());
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_route(&outside)).is_err());
        }
        {
            let current = include_str!("paired_tails.rs");
            assert!(
                std::panic::catch_unwind(|| previous_controls(&format!("{current} "))).is_err()
            );
            let mut physical_drift = current.to_owned();
            let end = physical_drift.chars().next().unwrap().len_utf8();
            let replacement = if &physical_drift[..end] == "x" {
                "y"
            } else {
                "x"
            };
            physical_drift.replace_range(..end, replacement);
            assert!(std::panic::catch_unwind(|| previous_controls(&physical_drift)).is_err());
            let stage = super::super::super::incoming_current_stage(current);
            let current = stage.as_str();
            let restored = previous_controls(current);
            assert_eq!(
                restored,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/fused-readers-eb819-20261018/before-controls.rs.txt"
                ))
            );
            for (at, _, _) in CONTROLS {
                if *at < current.len() {
                    let mut changed = current.to_owned();
                    let end = *at + current[*at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[*at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(*at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_controls(&changed)).is_err());
                }
            }
            assert!(
                std::panic::catch_unwind(|| previous_controls(&format!("{current} "))).is_err()
            );
            let outside = current.replacen("//!", "//?", 1);
            assert_ne!(outside, current);
            assert!(std::panic::catch_unwind(|| previous_controls(&outside)).is_err());
        }
    }
}

// Preserve both independently fixed source stages; unknown bodies are never
// repaired by the main-only inverse before the ordinary full comparison.
fn exact_main_subject(current: &str, length: usize, expected: &str) -> bool {
    use sha2::{Digest, Sha256};
    current.len() == length
        && format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))) == expected
}
