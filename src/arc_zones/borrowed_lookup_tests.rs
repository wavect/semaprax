//! Actual borrowed queries and complete independently saved model receivers.
use super::*;
use crate::test_allocation_observer::measure;
#[allow(
    dead_code,
    reason = "complete historical receiver retained for independent test comparisons"
)]
mod old;

fn current_model() -> ArcZonesModel {
    ArcZonesModel::try_new(
        vec![ZoneSpec::root("root", "thread")],
        vec![
            ObjectSpec::new("α", "root", ShareableMark::Shareable),
            ObjectSpec::new("β", "root", ShareableMark::Shareable),
        ],
    )
    .unwrap()
}
fn original_model() -> old::ArcZonesModel {
    old::ArcZonesModel::try_new(
        vec![old::ZoneSpec::root("root", "thread")],
        vec![
            old::ObjectSpec::new("α", "root", old::ShareableMark::Shareable),
            old::ObjectSpec::new("β", "root", old::ShareableMark::Shareable),
        ],
    )
    .unwrap()
}
#[test]
fn borrowed_strong_count_observes_no_backing_and_keeps_original_values() {
    let model = current_model();
    let original = original_model();
    let mut run = model.prepare_run(&[Op::enter_zone("root"), Op::construct("α")]);
    let mut before = original.prepare_run(&[old::Op::enter_zone("root"), old::Op::construct("α")]);
    run.step().unwrap();
    before.step().unwrap();
    run.step().unwrap();
    before.step().unwrap();
    assert_eq!(run.strong_count("α"), Some(1));
    let keys = ["α", "β", "missing", "", "α\0tail"];
    for key in keys {
        assert_eq!(run.strong_count(key), before.strong_count(key));
    }
    let (_, actual) = measure(|| {
        for _ in 0..128 {
            for key in keys {
                std::hint::black_box(run.strong_count(key));
            }
        }
    });
    assert!(!actual.invalid);
    assert_eq!(actual.allocated, 0);
    assert_eq!(actual.live, 0);
    let (_, saved) = measure(|| {
        for key in keys {
            std::hint::black_box(before.strong_count(key));
        }
    });
    assert!(!saved.invalid && saved.allocated > 0);
    assert_eq!(saved.live, 0);
    assert_eq!(saved.allocated, saved.freed);
}
#[test]
fn complete_original_arc_model_trace_cycle_selection_and_receiving_drop_match() {
    for cycle in [false, true] {
        let model = current_model();
        let original = original_model();
        assert_eq!(model.canonical_json(), original.canonical_json());
        assert_eq!(model.fingerprint(), original.fingerprint());
        let mut script = vec![
            Op::enter_zone("root"),
            Op::construct("α"),
            Op::construct("β"),
            Op::link("α", "β"),
        ];
        let mut saved = vec![
            old::Op::enter_zone("root"),
            old::Op::construct("α"),
            old::Op::construct("β"),
            old::Op::link("α", "β"),
        ];
        if cycle {
            script.push(Op::link("β", "α"));
            saved.push(old::Op::link("β", "α"));
        }
        script.push(Op::exit_zone("root"));
        saved.push(old::Op::exit_zone("root"));
        let mut run = model.prepare_run(&script);
        let mut before = original.prepare_run(&saved);
        for _ in 0..=script.len() {
            if run.is_complete() || run.is_rejected() {
                break;
            }
            assert_eq!(format!("{:?}", run.step()), format!("{:?}", before.step()));
            assert_eq!(run.trace_canonical_json(), before.trace_canonical_json());
            assert_eq!(run.trace_digest(), before.trace_digest());
            assert_eq!(run.is_complete(), before.is_complete());
            assert_eq!(run.is_rejected(), before.is_rejected());
            for key in ["α", "β", "missing"] {
                assert_eq!(run.strong_count(key), before.strong_count(key));
            }
        }
        assert!(run.is_complete() || run.is_rejected());
        assert!(before.is_complete() || before.is_rejected());
        assert_eq!(
            format!("{:?}", run.finish()),
            format!("{:?}", before.finish())
        );
    }
    for saved in [false, true] {
        let (_, storage) = measure(|| {
            if saved {
                let model = original_model();
                let mut run =
                    model.prepare_run(&[old::Op::enter_zone("root"), old::Op::construct("α")]);
                run.step().unwrap();
                run.step().unwrap();
                drop(run);
                drop(model);
            } else {
                let model = current_model();
                let mut run = model.prepare_run(&[Op::enter_zone("root"), Op::construct("α")]);
                run.step().unwrap();
                run.step().unwrap();
                drop(run);
                drop(model);
            }
        });
        assert!(!storage.invalid && storage.allocated > 0);
        assert_eq!(storage.live, 0);
        assert_eq!(storage.allocated, storage.freed);
    }
}
