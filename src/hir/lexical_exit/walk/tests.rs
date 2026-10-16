//! Child-order witnesses compare borrowed pointers, not duplicated identities.
use super::*;
use crate::hir::{ResolvedMatchArm, ResolvedMatchPattern, ResolvedStatement};

fn function(source: &str) -> ResolvedFunction {
    crate::hir::resolve(&crate::check(source, "child-cursor.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn scalar_function() -> ResolvedFunction {
    function("module cursor.scalar; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}")
}

#[test]
fn unused_contours_acquire_no_backing_and_forged_bottom_still_replays() {
    let mut program = crate::hir::resolve(
        &crate::check(
            "module cursor.scalar; @id(\"main\") fn main()->i64 {0}",
            "contour-selection.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let ((result, metrics), refused, used) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            crate::hir::lexical_exit::validate_function(&program, function, &owner)
        })
    });
    result.unwrap();
    assert!(!refused && used == 0 && !metrics.invalid);
    assert_eq!(
        (metrics.allocations, metrics.allocated, metrics.live),
        (0, 0, 0)
    );

    // A type marker cannot authenticate itself as an absent lexical profile.
    program.functions[0].body.ty = crate::hir::ResolvedType::Never;
    let function = &program.functions[0];
    assert!(!requires(function).unwrap());
    assert!(requires_validation(function).unwrap());
    let error =
        crate::hir::lexical_exit::validate_function(&program, function, &owner).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "lexical completion type differs from independently replayed reachability"
    );
}
fn assert_order(expression: &ResolvedExpr) {
    let mut cursor = ChildCursor::new(expression);
    assert!(std::ptr::eq(cursor.expression(), expression));
    assert!(!cursor.is_started());
    let mut index = 0;
    while let Some(expected) = child(expression, index) {
        let actual = cursor.next().expect("every original child retained");
        assert!(std::ptr::eq(actual, expected), "child {index} order drift");
        assert!(cursor.is_started());
        index += 1;
    }
    assert!(cursor.next().is_none());
    assert!(cursor.next().is_none(), "cursor remains exhausted");
}

#[test]
fn checked_mixed_blocks_and_exit_operands_preserve_original_child_order() {
    let function = function(
        r#"module cursor.mixed;
@id("cursor.probe") fn probe(n:i64)->i64 {
 let mut i=0;
 while i<n {i=i+1;if i==2 {continue;}else{0} if i==3 {break;}else{0} 0}
 if i>0 {return i+1;}else{0}
 return 0;
}
@id("main") fn main()->i64 {probe(4)}
"#,
    );
    assert!(requires(&function).unwrap());
    let mut pending = vec![&function.body];
    let mut exits = 0;
    while let Some(expression) = pending.pop() {
        assert_order(expression);
        if matches!(&expression.kind, K::LexicalExit { .. }) {
            exits += 1;
        }
        pending.extend(ChildCursor::new(expression));
    }
    assert_eq!(exits, 4);
}

#[test]
fn wide_block_cursor_advances_each_statement_and_retains_all_statement_children() {
    let function = scalar_function();
    let K::Block { statements, tail } = &function.body.kind else {
        panic!("block");
    };
    let mut block = function.body.clone();
    let condition = tail.clone();
    let kinds = [
        statements[0].clone(),
        statements[1].clone(),
        ResolvedStatement::Unsafe {
            audit: "cursor shape".into(),
            body: tail.clone(),
            span: tail.span,
        },
        ResolvedStatement::While {
            condition: condition.clone(),
            body: tail.clone(),
            span: tail.span,
        },
        ResolvedStatement::LexicalWhile {
            condition,
            body: tail.clone(),
            latch: crate::hir::lexical_exit::LoopLatchKind::Condition,
            span: tail.span,
        },
    ];
    block.kind = K::Block {
        statements: (0..256).flat_map(|_| kinds.iter().cloned()).collect(),
        tail: tail.clone(),
    };
    assert_order(&block);
    let K::Block { statements, .. } = &block.kind else {
        unreachable!()
    };
    let mut cursor = ChildCursor::new(&block);
    assert!(cursor.last_statement_child().is_none());
    for (statement_index, statement) in statements.iter().enumerate() {
        for index in 0..statement.child_count() {
            let actual = cursor.next().unwrap();
            assert!(std::ptr::eq(actual, statement.child(index).unwrap()));
            let (owner, edge) = cursor.last_statement_child().unwrap();
            assert!(std::ptr::eq(owner, statement));
            assert_eq!(edge, index);
            let Position::Block { statement, child } = cursor.position() else {
                panic!("block position");
            };
            assert_eq!(statement, statement_index);
            assert_eq!(child, index + 1);
        }
    }
    assert!(
        cursor.next().is_some(),
        "block tail follows every statement"
    );
    assert!(cursor.last_statement_child().is_none());
    assert!(cursor.next().is_none());
    assert!(cursor.last_statement_child().is_none());
}

#[test]
fn wide_guarded_match_retains_scrutinee_guard_body_and_exhaustion_order() {
    let function = scalar_function();
    let K::Block { tail, .. } = &function.body.kind else {
        panic!("tail");
    };
    let mut expression = function.body.clone();
    let arms = (0..1024)
        .map(|index| ResolvedMatchArm {
            pattern: ResolvedMatchPattern::Wildcard,
            guard: (index % 3 != 0).then(|| tail.clone()),
            value: tail.as_ref().clone(),
            span: tail.span,
        })
        .collect();
    // This is a structural enumeration witness, not a forged HIR admission.
    expression.kind = K::Match {
        scrutinee: tail.clone(),
        arms,
        mode: crate::hir::ResolvedMatchMode::Value,
    };
    assert_order(&expression);
}

#[test]
fn cursor_scans_preserve_depth_refusal_and_fail_closed_contains() {
    let mut function = scalar_function();
    let K::Block { tail, .. } = &function.body.kind else {
        panic!("tail");
    };
    let seed = tail.as_ref().clone();
    let mut nested = seed.clone();
    for _ in 0..crate::cache_codec::MAX_DEPTH + 2 {
        let mut parent = seed.clone();
        parent.kind = K::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(nested),
        };
        nested = parent;
    }
    function.body = nested;
    assert!(!requires(&function).unwrap());
    assert!(!contains(&function.body));
    // Keep the original forged 258-node subject: allowing a presence scan to
    // finish grants no HIR authority to its duplicated expression identities.
    let mut forged = crate::hir::resolve(
        &crate::check(
            "module cursor.scalar; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}",
            "child-cursor.spx",
        )
        .unwrap(),
    )
    .unwrap();
    forged.functions[0].body = std::mem::replace(&mut function.body, seed.clone());
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let mut nested = seed.clone();
    for _ in 0..PRESENCE_DEPTH + 2 {
        let mut parent = seed.clone();
        parent.kind = K::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(nested),
        };
        nested = parent;
    }
    function.body = nested;
    assert!(
        requires(&function).is_err(),
        "forged excessive depth remains refused"
    );
    assert!(
        contains(&function.body),
        "unchecked depth remains fail closed"
    );
    let plain = scalar_function();
    assert!(!requires(&plain).unwrap());
    assert!(!contains(&plain.body));
}

mod before_cursor {
    use super::*;
    /// Borrowed authored-order child enumeration. The cursor has no heap backing.
    /// Block statements and match arms are visited once rather than rescanned for
    /// each flattened child index. Consumers retain their own depth/work bounds.
    #[derive(Clone, Copy)]
    pub(crate) struct BeforeCursor<'a> {
        expression: &'a ResolvedExpr,
        position: Position,
        started: bool,
    }
    #[derive(Clone, Copy)]
    enum Position {
        Indexed(usize),
        Block { statement: usize, child: usize },
        Match { arm: usize, guard: bool },
    }
    impl<'a> BeforeCursor<'a> {
        pub(crate) fn new(expression: &'a ResolvedExpr) -> Self {
            let position = match &expression.kind {
                K::Block { .. } | K::VecConstructor { .. } => Position::Block {
                    statement: 0,
                    child: 0,
                },
                K::Match { .. } => Position::Match {
                    arm: 0,
                    guard: true,
                },
                _ => Position::Indexed(0),
            };
            Self {
                expression,
                position,
                started: false,
            }
        }
        pub(crate) fn expression(&self) -> &'a ResolvedExpr {
            self.expression
        }
        /// Statement edge most recently returned by `next`; tails have no statement.
        /// This uses the saved position and never rescans a block prefix.
        pub(crate) fn last_statement_child(
            &self,
        ) -> Option<(&'a crate::hir::ResolvedStatement, usize)> {
            match (&self.expression.kind, self.position) {
                (
                    K::Block { statements, .. } | K::VecConstructor { statements, .. },
                    Position::Block { statement, child },
                ) if self.started && child != 0 => {
                    statements.get(statement).map(|item| (item, child - 1))
                }
                _ => None,
            }
        }
        pub(crate) fn is_started(&self) -> bool {
            self.started
        }
    }
    impl<'a> Iterator for BeforeCursor<'a> {
        type Item = &'a ResolvedExpr;
        fn next(&mut self) -> Option<Self::Item> {
            let first = !self.started;
            self.started = true;
            match (&self.expression.kind, &mut self.position) {
                (
                    K::Block { statements, tail }
                    | K::VecConstructor {
                        statements, tail, ..
                    },
                    Position::Block {
                        statement,
                        child: index,
                    },
                ) => {
                    while let Some(item) = statements.get(*statement) {
                        if *index < item.child_count() {
                            let result = item.child(*index);
                            *index += 1;
                            return result;
                        }
                        *statement += 1;
                        *index = 0;
                    }
                    if *index == 0 {
                        *index = 1;
                        Some(tail)
                    } else {
                        None
                    }
                }
                (
                    K::Match {
                        scrutinee, arms, ..
                    },
                    Position::Match { arm, guard },
                ) => {
                    if first {
                        return Some(scrutinee);
                    }
                    if let Some(item) = arms.get(*arm) {
                        if *guard {
                            *guard = false;
                            if let Some(value) = &item.guard {
                                return Some(value);
                            }
                        }
                        *arm += 1;
                        *guard = true;
                        return Some(&item.value);
                    }
                    None
                }
                (_, Position::Indexed(index)) => {
                    let result = child(self.expression, *index);
                    if result.is_some() {
                        *index += 1;
                    }
                    result
                }
                _ => unreachable!("child cursor position matches its immutable expression"),
            }
        }
    }
    impl std::iter::FusedIterator for BeforeCursor<'_> {}

    impl<'a> BeforeCursor<'a> {
        #[cfg(test)]
        pub(crate) fn next_with_work<E>(
            &mut self,
            work: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> Result<Option<&'a ResolvedExpr>, E> {
            work(1)?;
            let first = !self.started;
            self.started = true;
            Ok(match (&self.expression.kind, &mut self.position) {
                (
                    K::Block { statements, tail }
                    | K::VecConstructor {
                        statements, tail, ..
                    },
                    Position::Block {
                        statement,
                        child: index,
                    },
                ) => {
                    loop {
                        work(1)?;
                        let Some(item) = statements.get(*statement) else {
                            break;
                        };
                        if *index < item.child_count() {
                            let result = item.child(*index);
                            *index += 1;
                            return Ok(result);
                        }
                        *statement += 1;
                        *index = 0;
                    }
                    if *index == 0 {
                        *index = 1;
                        Some(tail)
                    } else {
                        None
                    }
                }
                (
                    K::Match {
                        scrutinee, arms, ..
                    },
                    Position::Match { arm, guard },
                ) => {
                    if first {
                        return Ok(Some(scrutinee));
                    }
                    if let Some(item) = arms.get(*arm) {
                        if *guard {
                            *guard = false;
                            if let Some(value) = &item.guard {
                                return Ok(Some(value));
                            }
                        }
                        *arm += 1;
                        *guard = true;
                        return Ok(Some(&item.value));
                    }
                    None
                }
                (_, Position::Indexed(index)) => {
                    let result = child(self.expression, *index);
                    if result.is_some() {
                        *index += 1;
                    }
                    result
                }
                _ => unreachable!("child cursor position matches its immutable expression"),
            })
        }
    }

    impl BeforeCursor<'_> {
        pub(super) fn position(&self) -> super::Position {
            match self.position {
                Position::Indexed(i) => super::Position::Indexed(i),
                Position::Block { statement, child } => super::Position::Block { statement, child },
                Position::Match { arm, guard } => super::Position::Match { arm, guard },
            }
        }
    }
}

fn assert_same_progress(current: &ChildCursor<'_>, before: &before_cursor::BeforeCursor<'_>) {
    assert!(std::ptr::eq(current.expression(), before.expression()));
    assert_eq!(current.is_started(), before.is_started());
    assert_eq!(current.position(), before.position());
    match (
        current.last_statement_child(),
        before.last_statement_child(),
    ) {
        (Some((a, ai)), Some((b, bi))) => {
            assert!(std::ptr::eq(a, b));
            assert_eq!(ai, bi);
        }
        (None, None) => {}
        _ => panic!("statement-child ownership drift"),
    }
}
fn pointer(value: Option<&ResolvedExpr>) -> Option<usize> {
    value.map(|expression| expression as *const ResolvedExpr as usize)
}
fn compare_complete_cursor(expression: &ResolvedExpr) {
    let mut current = ChildCursor::new(expression);
    let mut before = before_cursor::BeforeCursor::new(expression);
    loop {
        assert_same_progress(&current, &before);
        let old = before.next();
        assert_eq!(pointer(current.next()), pointer(old));
        if old.is_none() {
            break;
        }
    }
    for _ in 0..3 {
        assert_same_progress(&current, &before);
        assert_eq!(pointer(current.next()), pointer(before.next()));
    }
}
#[test]
fn compact_cursor_keeps_complete_before_order_and_state_for_every_checked_node() {
    for source in [
        "module cursor.scalar; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}",
        r#"module cursor.exits;
@id("main") fn main()->i64 {
 let mut n=0;
 while n<4 {n=n+1;if n==1 {continue;}else{0} if n==3 {break;}else{0} 0}
 return n;
}"#,
        include_str!("../../../../tests/fixtures/nested-byte-slice-traversal.spx"),
    ] {
        let function = function(source);
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            compare_complete_cursor(expression);
            // The complete before cursor selects every subject of comparison.
            pending.extend(before_cursor::BeforeCursor::new(expression));
        }
    }
    assert_eq!(
        std::mem::size_of::<ChildCursor<'_>>(),
        3 * std::mem::size_of::<usize>()
    );
    assert!(
        std::mem::size_of::<[Option<ChildCursor<'_>>; PRESENCE_DEPTH]>()
            < std::mem::size_of::<[Option<before_cursor::BeforeCursor<'_>>; PRESENCE_DEPTH]>()
    );
}

fn compare_work_frontiers(expression: &ResolvedExpr) {
    let mut before = before_cursor::BeforeCursor::new(expression);
    let mut total = 0usize;
    while before
        .next_with_work(&mut |units| {
            total += units;
            Ok::<_, ()>(())
        })
        .unwrap()
        .is_some()
    {}
    for limit in 0..=total + 2 {
        let (mut current, mut before) = (
            ChildCursor::new(expression),
            before_cursor::BeforeCursor::new(expression),
        );
        let (mut actual, mut old) = (0usize, 0usize);
        loop {
            let a = current.next_with_work(&mut |units| {
                if actual + units > limit {
                    Err(())
                } else {
                    actual += units;
                    Ok(())
                }
            });
            let b = before.next_with_work(&mut |units| {
                if old + units > limit {
                    Err(())
                } else {
                    old += units;
                    Ok(())
                }
            });
            assert_eq!(a.map(pointer), b.map(pointer));
            assert_eq!(actual, old);
            assert_same_progress(&current, &before);
            if !matches!(a, Ok(Some(_))) {
                break;
            }
        }
    }
}
#[test]
fn compact_cursor_preserves_every_original_work_refusal_and_partial_progress() {
    let function = scalar_function();
    compare_work_frontiers(&function.body);
    let K::Block { statements, tail } = &function.body.kind else {
        panic!("block")
    };
    let mut wide = function.body.clone();
    wide.kind = K::Block {
        statements: (0..64)
            .map(|i| statements[i % statements.len()].clone())
            .collect(),
        tail: tail.clone(),
    };
    compare_work_frontiers(&wide);
    wide.kind = K::Match {
        scrutinee: tail.clone(),
        arms: (0..64)
            .map(|i| ResolvedMatchArm {
                pattern: ResolvedMatchPattern::Wildcard,
                guard: (i % 3 != 0).then(|| tail.clone()),
                value: tail.as_ref().clone(),
                span: tail.span,
            })
            .collect(),
        mode: crate::hir::ResolvedMatchMode::Value,
    };
    compare_work_frontiers(&wide);
}

#[test]
fn compact_cursor_complete_before_sources_and_compiled_control_stay_bound() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/compact-child-cursor-v1");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = crate::source_reflows_merge::main7f8_source_inputs::source("eb2-compact", record["path"].as_str().unwrap(), &current);
        let current = crate::source_reflows_merge::main_eb2_inputs::input(record["path"].as_str().unwrap(), &current);
        let current = crate::source_reflows_merge::main7f8_presence::compact_after_eb2(record["path"].as_str().unwrap(), &current);
        let restore = |source: &str| -> Option<String> {
            let mut result = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if result.matches(new).count() != 1 {
                    return None;
                }
                result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(result)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen("ChildCursor", "ChangedCursor", 1)).as_deref(),
            Some(original.as_str())
        );
    }
    let original = std::fs::read_to_string(evidence.join("walk.rs.before.txt")).unwrap();
    let prefix = original
        .split("pub(crate) fn child(")
        .next()
        .unwrap()
        .split_once("/// Borrowed")
        .unwrap()
        .1;
    let suffix = original
        .split("#[cfg(test)]\nmod tests;\n")
        .nth(1)
        .unwrap()
        .trim_start();
    let expected = format!("/// Borrowed{prefix}{suffix}").replace("ChildCursor", "BeforeCursor");
    let tests = std::fs::read_to_string(root.join("src/hir/lexical_exit/walk/tests.rs")).unwrap();
    let compiled = tests
        .split("mod before_cursor {\n    use super::*;\n")
        .nth(1)
        .unwrap()
        .split("\n    impl BeforeCursor")
        .next()
        .unwrap();
    let compiled = compiled
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        compiled.trim(),
        expected.trim(),
        "complete original cursor is the compiled control"
    );
}
