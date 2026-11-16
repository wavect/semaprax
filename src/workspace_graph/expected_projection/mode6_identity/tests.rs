use super::{discount, Declaration};
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

#[test]
fn whole_mode6_identity_pays_before_growing_and_matches_every_original_slot() {
    let provider = crate::parse(
        "module provider; @id(\"provider.remote\") fn remote()->i64{1}",
        "provider.spx",
    )
    .unwrap();
    for (body, allocates) in [
        ("1", false),
        ("let a=1;a", false),
        ("let a=1;let b=a;b", true),
        ("remote()+remote()", true),
        ("let remote=1;remote", false),
        ("let a=1; if true {let b=a;b} else {a}", true),
    ] {
        let app = crate::parse(
                &format!("module app; use function @id(\"provider.remote\") from provider as remote; @id(\"app.main\") fn main()->i64{{{body}}}"),
                "app.spx",
            )
            .unwrap();
        let programs = [provider.clone(), app];
        let app = &programs[1];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        for imports in [false, true] {
            let expected =
                super::super::call_identity::discount_with_imports(app, &authored, imports);
            let run = |limit| {
                with_limit_usage_refusal(limit, || {
                    let (result, allocation) = measure(|| {
                        let result = discount(app, &authored, imports);
                        result.map_err(|errors| {
                            let code = errors[0].code;
                            drop(errors);
                            code
                        })
                    });
                    if result.is_err() {
                        assert!(!reserve_active_required(0));
                    }
                    (result, allocation)
                })
            };
            let ((result, allocation), refused, paid, first) = run(usize::MAX);
            assert_eq!(result, Ok(expected));
            assert_eq!(allocation.allocated, paid);
            assert_eq!(paid != 0, allocates);
            assert!(!refused && first.is_none());
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert_eq!(run(paid).0 .0, Ok(expected));
            if paid > 0 {
                for limit in [paid - 1, 0] {
                    let ((result, allocation), refused, _, first) = run(limit);
                    assert_eq!(result, Err("SPX-G171"));
                    assert!(refused && first.is_some());
                    assert!(!allocation.invalid && allocation.live == 0);
                    assert_eq!(allocation.allocated, allocation.freed);
                }
            }
        }
    }
}
#[test]
fn whole_mode6_identity_preflight_preserves_bounds_and_the_live_floor() {
    use crate::bounded_output::{active_remaining, set_active_floor};
    use crate::workspace_graph::ACTIVE_BUILDER_LIMIT;

    struct RestoreLimit(usize);
    impl Drop for RestoreLimit {
        fn drop(&mut self) {
            ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }

    let programs = [crate::parse(
            "module app; @id(\"app.leaf\") fn leaf()->i64{1} @id(\"app.main\") fn main()->i64{leaf()+leaf()}",
            "app.spx",
        )
        .unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let run = || discount(&programs[0], &authored, true);
    let expected = 2 * std::mem::size_of::<Declaration<'_>>();
    let previous = ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(expected));
    let _restore = RestoreLimit(previous);
    assert_eq!(active_remaining(), None);
    let (result, allocation) = measure(|| run().unwrap());
    assert_eq!(result, 4);
    assert_eq!(allocation.allocated, expected);
    assert_eq!(allocation.allocated, allocation.freed);
    assert_eq!(allocation.live, 0);
    for limit in [expected - 1, 0] {
        ACTIVE_BUILDER_LIMIT.with(|value| value.set(limit));
        assert_eq!(run().unwrap_err()[0].code, "SPX-G171");
        assert_eq!(active_remaining(), None);
    }
    ACTIVE_BUILDER_LIMIT.with(|value| value.set(expected));
    for (limit, succeeds) in [(expected + 1, true), (expected, false)] {
        let (result, refused, paid, first) = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(1));
            let result = run();
            if result.is_err() {
                assert!(!reserve_active_required(0));
            }
            result
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(refused, !succeeds);
        if succeeds {
            assert_eq!(paid, expected);
            assert!(first.is_none());
        } else {
            let first = first.unwrap();
            assert_eq!(
                (first.requested, first.remaining, first.floor),
                (expected, expected, 1)
            );
            assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
        }
    }

    let (result, refused, _, first) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(7));
        run()
    });
    assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
    assert!(refused);
    assert_eq!(first.unwrap().requested, 7);
}
