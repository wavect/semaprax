//! Literal receiver storage/work controls, independent of collector estimates.
use super::*;
use std::cell::Cell;

#[test]
fn scratch_exact_replacement_overlap_one_short_and_spare_slot_failure() {
    let header = 3 * std::mem::size_of::<usize>();
    let exact = 3 * header + 3 * std::mem::size_of::<u64>();
    for (bytes, admitted, allocated) in [(exact, true, 24), (exact - 1, false, 8)] {
        let work = Cell::new(3usize);
        let backing = Cell::new(bytes);
        let failed = Cell::new(false);
        let mut charge = |units| match work.get().checked_sub(units) {
            Some(left) if !failed.get() => {
                work.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        let mut reserve = |units| match backing.get().checked_sub(units) {
            Some(left) if !failed.get() => {
                backing.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        let (success, observed) = crate::test_allocation_observer::measure(|| {
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let mut scratch = Scratch::new(&mut admission).unwrap();
            scratch.push(11u64, &mut admission).unwrap();
            let success = scratch.push(13u64, &mut admission).is_ok();
            if success {
                assert_eq!(scratch.as_slice(), &[11, 13]);
            } else {
                assert_eq!(scratch.as_slice(), &[11]);
                assert!(admission.reserve(0).is_err());
            }
            drop(scratch);
            success
        });
        assert_eq!(success, admitted);
        assert!(!observed.invalid);
        assert_eq!(observed.allocated, allocated);
        assert_eq!(observed.highwater, allocated);
        assert_eq!(observed.live, 0);
        assert!(observed.allocated <= bytes - backing.get());
    }
    let mut charge = |_| true;
    let refused = Cell::new(false);
    let mut reserve = |_| !refused.get();
    let mut admission = Admission::new(&mut charge, &mut reserve);
    let mut scratch = Scratch::new(&mut admission).unwrap();
    for value in 0..3 {
        scratch.push(value, &mut admission).unwrap();
    }
    // Capacity four already exists: refusal elsewhere must still prevent push.
    refused.set(true);
    assert_eq!(scratch.push(99, &mut admission), Err(Refusal::Backing));
    assert_eq!(scratch.as_slice(), &[0, 1, 2]);
}

struct Text<'a>(&'a str);
impl std::fmt::Debug for Text<'_> {
    fn fmt(&self, writer: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for character in self.0.chars() {
            writer.write_str(character.encode_utf8(&mut [0; 4]))?;
        }
        Ok(())
    }
}
#[test]
fn exact_debug_stream_matches_saved_contains_and_literal_work_backing() {
    for (needle, text) in [
        ("aba", "ababa"),
        ("aaab", "aaaaab"),
        ("🙂\0", "before🙂\0after"),
        ("xyz", "none"),
        ("", "any"),
    ] {
        let mut charge = |_| true;
        let mut reserve = |_| true;
        let mut admission = Admission::new(&mut charge, &mut reserve);
        let matcher = debug_contains::Needle::new(&Text(needle), &mut admission).unwrap();
        assert_eq!(
            matcher.contains(&Text(text), &mut admission).unwrap(),
            format!("{:?}", Text(text)).contains(&format!("{:?}", Text(needle)))
        );
    }
    // "aba": two 3-byte formatting passes, 3 prefix initializations and four
    // prefix-build units. "ababa": eight streamed/comparison units = 21 total.
    let exact_work = 21usize;
    let exact_backing = 6 * std::mem::size_of::<usize>() + 3 + 3 * std::mem::size_of::<usize>();
    for units in [exact_work, exact_work - 1] {
        let work = Cell::new(units);
        let bytes = Cell::new(exact_backing);
        let mut charge = |n| match work.get().checked_sub(n) {
            Some(left) => {
                work.set(left);
                true
            }
            None => false,
        };
        let mut reserve = |n| match bytes.get().checked_sub(n) {
            Some(left) => {
                bytes.set(left);
                true
            }
            None => false,
        };
        let (result, observed) = crate::test_allocation_observer::measure(|| {
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let matcher = debug_contains::Needle::new(&Text("aba"), &mut admission).unwrap();
            let result = matcher.contains(&Text("ababa"), &mut admission);
            if result.is_err() {
                assert!(admission.units(0).is_err());
            }
            drop(matcher);
            result
        });
        assert_eq!(
            result,
            if units == exact_work {
                Ok(true)
            } else {
                Err(Refusal::Work)
            }
        );
        assert_eq!(observed.allocated, 3 + 3 * std::mem::size_of::<usize>());
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
        assert_eq!(bytes.get(), 0);
    }
}

#[test]
fn borrowed_directory_preserves_family_bits_priority_opposing_keys_and_literal_backing() {
    let source = crate::check(
        r#"module directory; @id("main") fn main()->i64 {let x=0;x}"#,
        "directory.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("fixture block");
    };
    let crate::hir::ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("fixture binding");
    };
    // Synthetic one-byte keys keep this insertion/sort ledger independent of
    // scoped source identity spelling; current subject checks remain real.
    let first = ExpressionId::from_owned("a".to_owned());
    let last = ExpressionId::from_owned("z".to_owned());
    // Two entries: replacement requests 1+2 entries and their two transient
    // Vec headers, one retained header and the exact Directory carrier.
    let header = 3 * std::mem::size_of::<usize>();
    let entry = std::mem::size_of::<Entry<'_>>();
    // Two borrowed pointers + one ordinal + four one-byte tags/alignment.
    assert_eq!(entry, 4 * std::mem::size_of::<usize>());
    assert_eq!(
        std::mem::size_of::<Directory<'_>>(),
        5 * std::mem::size_of::<usize>()
    );
    let exact_backing = std::mem::size_of::<Directory<'_>>() + 3 * header + 3 * entry;
    // One membership, three push/move units, heap-sort ten fixed units plus
    // one two-byte key bound, two compact visits + one two-byte key bound.
    let exact_work = 19usize;
    for (bytes, work, admitted) in [
        (exact_backing, exact_work, true),
        (exact_backing - 1, exact_work, false),
        (exact_backing, exact_work - 1, false),
    ] {
        let remaining = Cell::new(bytes);
        let units = Cell::new(work);
        let failed = Cell::new(false);
        let mut reserve = |bytes| match remaining.get().checked_sub(bytes) {
            Some(left) if !failed.get() => {
                remaining.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        let mut charge = |work| match units.get().checked_sub(work) {
            Some(left) if !failed.get() => {
                units.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        let (accepted, observation) = crate::test_allocation_observer::measure(|| {
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let result =
                Directory::build(&program, function, &mut admission, |entries, admission| {
                    Directory::emit(entries, Family::Vec, &first, binding, admission)?;
                    Directory::emit(entries, Family::Iterator, &last, binding, admission)
                });
            let accepted = result.is_ok();
            drop(result);
            if !accepted {
                assert!(admission.reserve(0).is_err());
                assert!(admission.units(0).is_err());
            }
            accepted
        });
        assert_eq!(accepted, admitted);
        assert!(!observation.invalid);
        assert_eq!(observation.live, 0);
        assert!(observation.allocated <= bytes - remaining.get());
        if admitted {
            assert_eq!(observation.allocated, 3 * entry);
            assert_eq!(observation.highwater, 3 * entry);
            assert_eq!(units.get(), 0);
        }
    }
    let mut charge = |_| true;
    let mut reserve = |_| true;
    let mut admission = Admission::new(&mut charge, &mut reserve);
    let directory = Directory::build(&program, function, &mut admission, |entries, admission| {
        for family in [
            Family::Helper,
            Family::Iterator,
            Family::String,
            Family::ByteSet,
            Family::Vec,
        ] {
            Directory::emit(entries, family, &first, binding, admission)?;
        }
        Ok(())
    })
    .unwrap();
    let (found, families) = directory
        .receive(&program, function, &first, &mut admission)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(found, binding));
    for family in [
        Family::Helper,
        Family::Iterator,
        Family::String,
        Family::ByteSet,
        Family::Vec,
    ] {
        assert!(Directory::contains(families, family));
    }
    assert_eq!(directory.entries[0].family, Family::Helper);
    let detached = function.clone();
    assert!(directory
        .receive(&program, &detached, &first, &mut admission)
        .is_err());
    assert!(directory
        .receive(&program, function, &first, &mut admission)
        .is_err());
}

#[test]
fn paid_whole_family_collectors_match_saved_per_query_oracle_and_current_subject() {
    hostile_collector_order_and_closure_universe();
    let sources = [
        r#"module directory_vec;
@id("row") record Row { @id("row.s") text:string, @id("row.n") n:i64, }
@id("main") fn main()->i64 {
 let mut values=vec_with_capacity<i64>(1usize);
 let mut rows=vec_with_capacity<Row>(1usize);
 let mut bytes=bytes_zeroed(1usize);let mut i=0;
 while i<1 {values=vec_push<i64>(values,i);rows=vec_push<Row>(rows,Row{text:"row",n:i});bytes=bytes_set(bytes,0usize,65u8);i=i+1;0}
 0
}"#,
        r#"module directory_iterator;
@id("main") fn main()->i64 {
 let values=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let mut target=vec_with_capacity<i64>(1usize);
 for own item in vec_into_iter<i64>(values) {if item==1 {target=vec_push<i64>(target,item);0}else{0};0}
 0
}"#,
        r#"module directory_string;
@id("main") fn main()->i64 { let mut text="a"; let mut i=0;
 while i<1 {text=string_concat("prefix",text);i=i+1;0} 0 }
"#,
        r#"module directory_consuming;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});
 let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0 }
"#,
        include_str!("../../../../tests/owned_data/byte_helper_renewal.spx"),
    ];
    for source in sources {
        let source = crate::check(source, "directory-family.spx").unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        for function in &program.functions {
            let iterator = crate::hir::iterator_loop::renewal_bindings(&program, function);
            let vectors = crate::hir::vec_loop_renewal::bindings_in(&program, function);
            let bytes = crate::byte_ops::same_owner_set_bindings(function);
            let strings = crate::string_ops::replacement::bindings(function);
            let mut charge = |_| true;
            let mut reserve = |_| true;
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let directory =
                Directory::collect(&program, function, &helpers, &mut admission).unwrap();
            let mut old = iterator.clone();
            old.extend(vectors.clone());
            old.extend(bytes.clone());
            old.extend(strings.clone());
            if let Some(sites) = helpers.function_sites(function) {
                for (at, binding) in sites.borrowed_entries() {
                    old.insert(at.clone(), binding);
                }
            }
            assert_eq!(directory.entries.len(), old.len());
            for (at, expected) in old {
                let (binding, families) = directory
                    .receive(&program, function, &at, &mut admission)
                    .unwrap()
                    .unwrap();
                assert!(std::ptr::eq(binding, expected));
                for (family, present) in [
                    (Family::Iterator, iterator.contains_key(&at)),
                    (Family::Vec, vectors.contains_key(&at)),
                    (Family::ByteSet, bytes.contains_key(&at)),
                    (Family::String, strings.contains_key(&at)),
                ] {
                    assert_eq!(Directory::contains(families, family), present);
                }
            }
            let detached = function.clone();
            assert!(Directory::collect(&program, &detached, &helpers, &mut admission).is_err());
            assert!(admission.reserve(0).is_err());
        }
    }
}

// Malformed HIR here is classifier input, never a source admission claim.
fn hostile_collector_order_and_closure_universe() {
    use crate::hir::{ResolvedExprKind as K, ResolvedStatement as S};
    let source=crate::check(r#"module hostile_directory;
@id("main") fn main()->i64 {let mut left=bytes_zeroed(1usize);let mut right=bytes_zeroed(1usize);let mut i=0;
 while i<1 {if true {left=bytes_set(left,0usize,1u8);0}else{0};if true {right=bytes_set(right,0usize,2u8);0}else{0};i=i+1;0} 0}
"#,"hostile-directory.spx").unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    let function = &mut program.functions[0];
    let K::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let S::While { body, .. } = &mut statements[3] else {
        panic!("while");
    };
    let K::Block { statements, .. } = &mut body.kind else {
        panic!("loop block")
    };
    fn assignment(statement: &mut S) -> &mut crate::hir::ResolvedExpr {
        let S::Let {
            value: expression, ..
        } = statement
        else {
            panic!("if statement")
        };
        let K::If { then_branch, .. } = &mut expression.kind else {
            panic!("if")
        };
        let K::Block { statements, .. } = &mut then_branch.kind else {
            panic!("then block")
        };
        let S::Assign { value, .. } = &mut statements[0] else {
            panic!("assignment")
        };
        value
    }
    let duplicate = assignment(&mut statements[0]).id.clone();
    assignment(&mut statements[1]).id = duplicate.clone();
    let function = &program.functions[0];
    let expected = crate::byte_ops::same_owner_set_bindings(function);
    assert_eq!(expected.len(), 1);
    let mut work = |_| true;
    let mut backing = |_| true;
    let mut admission = Admission::new(&mut work, &mut backing);
    let directory = Directory::build(&program, function, &mut admission, |entries, admission| {
        leaf::collect(function, Family::ByteSet, entries, admission)
    })
    .unwrap();
    let found = directory
        .receive(&program, function, &duplicate, &mut admission)
        .unwrap()
        .unwrap()
        .0;
    assert!(std::ptr::eq(found, expected[&duplicate]));

    let source=crate::check(r#"module closure_directory; @id("main") fn main()->i64 {let mut values=vec_with_capacity<i64>(1usize);let mut i=0;while i<1 {values=vec_push<i64>(values,i);i=i+1;0}0}"#,"closure-directory.spx").unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    let original = program.functions[0].body.clone();
    let K::Block { statements, .. } = &mut program.functions[0].body.kind else {
        panic!("block")
    };
    let S::Let { mutable, .. } = &mut statements[0] else {
        panic!("let")
    };
    *mutable = false;
    let mut closure = original.clone();
    closure.kind = K::Closure {
        parameters: Vec::new(),
        captures: Vec::new(),
        body: Box::new(original),
    };
    let mut closure_statement = statements[0].clone();
    let S::Let { value, mutable, .. } = &mut closure_statement else {
        panic!("let")
    };
    *value = closure;
    *mutable = false;
    statements.push(closure_statement);
    let function = &program.functions[0];
    assert!(crate::hir::vec_loop_renewal::bindings_in(&program, function).is_empty());
    let mut work = |_| true;
    let mut backing = |_| true;
    let mut admission = Admission::new(&mut work, &mut backing);
    let directory = Directory::build(&program, function, &mut admission, |entries, admission| {
        vector::collect(&program, function, entries, admission)
    })
    .unwrap();
    assert!(
        directory.entries.is_empty(),
        "closure body cannot confer outer mutable declaration"
    );
}

#[test]
fn suffix_search_matches_complete_original_with_exact_and_short_work() {
    let source = crate::check(
        r#"module directory; @id("main") fn main()->i64 {let x=0;x}"#,
        "directory.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("block")
    };
    let crate::hir::ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("binding")
    };
    let keys: Vec<_> = (0..65)
        .map(|i| ExpressionId::from_owned(format!("shared.🙂.{}.{i:03}", "prefix".repeat(16))))
        .collect();
    let mut work = |_| true;
    let mut reserve = |_| true;
    let mut admission = Admission::new(&mut work, &mut reserve);
    let directory = Directory::build(&program, function, &mut admission, |entries, admission| {
        for key in &keys {
            Directory::emit(entries, Family::Vec, key, binding, admission)?;
        }
        Ok(())
    })
    .unwrap();
    let mut queries = keys.clone();
    for suffix in ["", "\0", "a", "shared.", "shared.🙂", "shared.🙂.z", "🙂"] {
        queries.push(ExpressionId::from_owned(suffix.to_owned()));
    }
    for key in &keys {
        queries.push(ExpressionId::from_owned(format!("{}\0", key.as_str())));
        queries.push(ExpressionId::from_owned(format!("{}🙂", key.as_str())));
    }
    for query in &queries {
        let mut original_work = |_| true;
        let mut original_reserve = |_| true;
        let mut original_admission = Admission::new(&mut original_work, &mut original_reserve);
        let original = directory
            .receive_original(&program, function, query, &mut original_admission)
            .unwrap();
        let spent = Cell::new(0usize);
        let mut charge = |units| {
            spent.set(spent.get() + units);
            true
        };
        let mut receiving = Admission::new(&mut charge, &mut reserve);
        let actual = directory
            .receive(&program, function, query, &mut receiving)
            .unwrap();
        assert_eq!(
            actual.map(|(b, f)| (b as *const _, f)),
            original.map(|(b, f)| (b as *const _, f))
        );
        let exact = spent.get();
        for allowance in [exact, exact - 1] {
            let remaining = Cell::new(allowance);
            let mut charge = |units| match remaining.get().checked_sub(units) {
                Some(left) => {
                    remaining.set(left);
                    true
                }
                None => false,
            };
            let mut receiving = Admission::new(&mut charge, &mut reserve);
            let result = directory.receive(&program, function, query, &mut receiving);
            assert_eq!(result.is_ok(), allowance == exact);
            if result.is_err() {
                assert!(receiving.units(0).is_err());
            }
        }
    }
}
