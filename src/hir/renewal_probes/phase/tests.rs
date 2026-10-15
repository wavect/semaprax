use super::*;
#[test]
fn paid_borrowed_search_matches_all_old_tables_and_exact_storage_drop() {
    let source = crate::check(
        r#"module probes; @id("r") record Row {@id("r.n") n:i64,} @id("main") fn main()->i64 {0}"#,
        "probes.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let index = &program.declarations;
    let mut charged = 0usize;
    let mut reserve = |bytes| {
        charged += bytes;
        true
    };
    let mut work = |_| true;
    let mut admission = Admission::new(&mut work, &mut reserve);
    let probes = Probes::new(index, &mut admission).unwrap();
    for (id, value) in &index.declarations {
        assert!(std::ptr::eq(
            probes.declaration(id, &mut |_| true).unwrap().unwrap(),
            value
        ));
    }
    for (id, value) in &index.type_parameters {
        assert!(std::ptr::eq(
            probes.parameters(id, &mut |_| true).unwrap().unwrap(),
            &**value
        ));
    }
    for (id, value) in &index.record_fields {
        assert!(std::ptr::eq(
            probes.fields(id, &mut |_| true).unwrap().unwrap(),
            &**value
        ));
    }
    let query = DeclarationId::new("absent");
    assert!(probes.declaration(&query, &mut |_| true).unwrap().is_none());
    assert!(probes.declaration(&query, &mut |_| false).is_err());
    drop(probes);
    #[allow(
        clippy::drop_non_drop,
        reason = "End this borrowed proof at the measured lifetime boundary."
    )]
    drop(admission);
    // Each map's independent source cardinality requires capacities 1,2,4,...;
    // two borrowed words per entry, and an overlapping Vec header per growth.
    let mut requested = 0usize;
    let mut expected = 9 * std::mem::size_of::<usize>(); // Probes' three retained Vecs
    for count in [
        index.declarations.len(),
        index.type_parameters.len(),
        index.record_fields.len(),
    ] {
        expected += 3 * std::mem::size_of::<usize>(); // collector header
        let mut capacity = 1usize;
        while capacity / 2 < count && count != 0 {
            let bytes = capacity * 2 * std::mem::size_of::<usize>();
            requested += bytes;
            expected += bytes + 3 * std::mem::size_of::<usize>();
            if capacity >= count {
                break;
            }
            capacity *= 2;
        }
    }
    assert_eq!(charged, expected);
    for limit in [expected, expected - 1] {
        let left = std::cell::Cell::new(limit);
        let failed = std::cell::Cell::new(false);
        let (success, observed) = crate::test_allocation_observer::measure(|| {
            let mut reserve = |bytes| {
                if failed.get() {
                    return false;
                }
                match left.get().checked_sub(bytes) {
                    Some(n) => {
                        left.set(n);
                        true
                    }
                    None => {
                        failed.set(true);
                        false
                    }
                }
            };
            let mut work = |_| true;
            let mut admission = Admission::new(&mut work, &mut reserve);
            let result = Probes::new(index, &mut admission);
            let success = result.is_ok();
            if !success {
                assert!(admission.reserve(0).is_err());
            }
            drop(result);
            success
        });
        assert_eq!(success, limit == expected);
        assert!(!observed.invalid);
        if success {
            assert_eq!(observed.allocated, requested);
        }
        assert!(observed.allocated <= limit - left.get());
        assert_eq!(observed.live, 0);
    }
}
