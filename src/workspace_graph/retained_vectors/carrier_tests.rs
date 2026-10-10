//! Retaining a fully selected allocation charges all physically spare slots.
use super::*;

#[test]
fn full_carrier_spare_slots_reuse_storage_at_exact_limit_and_refuse_one_short() {
    let make = || {
        let mut items = Vec::with_capacity(8);
        items.extend(0..5usize);
        items
    };
    let sample = make();
    let spare = (sample.capacity() - sample.len()) * std::mem::size_of::<usize>();
    assert!(spare < sample.len() * std::mem::size_of::<usize>());
    drop(sample);
    for accounted in [false, true] {
        let items = make();
        let original = items.as_ptr();
        let capacity = items.capacity();
        let (value, overflow, used) = crate::bounded_output::with_limit_usage(spare, || {
            if accounted {
                filter_owned_vec_accounted(
                    items,
                    std::mem::size_of::<usize>(),
                    |_| Ok(0),
                    |_| true,
                    true,
                )
            } else {
                filter_owned_vec(items, |_| true, true)
            }
        });
        let retained = value.unwrap();
        assert_eq!(retained, vec![0, 1, 2, 3, 4]);
        assert_eq!(
            retained.as_ptr(),
            original,
            "move the actual original allocation"
        );
        assert_eq!(retained.capacity(), capacity);
        assert_eq!(
            used, spare,
            "account every spare slot, without a replacement"
        );
        assert!(!overflow);

        let items = make();
        let (value, overflow, _) = crate::bounded_output::with_limit_usage(spare - 1, || {
            if accounted {
                filter_owned_vec_accounted(
                    items,
                    std::mem::size_of::<usize>(),
                    |_| Ok(0),
                    |_| true,
                    true,
                )
            } else {
                filter_owned_vec(items, |_| true, true)
            }
        });
        assert_eq!(value.unwrap_err()[0].code, "SPX-G171");
        assert!(overflow);
    }
    // A sidecar-discounted replacement cannot discount unused physical slots.
    let (reuse, overflow, used) =
        crate::bounded_output::with_limit_usage(0, || reuse_full_carrier::<usize>(5, 8, 5, 1));
    assert!(!reuse.unwrap());
    assert_eq!(used, 0);
    assert!(!overflow);
}
