//! Owned device-buffer lifecycle: transfers, bounds, misuse, cancellation,
//! device loss, stale artifacts, sticky failure, and cleanup order.

use crate::cleanup_plan::StatusCase;
use crate::compute_profile::classifier::{
    DeviceEffect, ALIASING_BEYOND_CHECKED_RULE, CAPACITY_BOUND_EXCEEDED, GRID_SHAPE_OUT_OF_BOUNDS,
};

use super::*;

const MAP4: KernelShape = KernelShape::ElementwiseMap { workgroup_size: 4 };

/// A session with two uploaded eight-element `i64` inputs, an output, and
/// the loaded `k.affine` map.
struct Fixture {
    program: ResolvedProgram,
    session: CpuReferenceSession,
    artifact: KernelArtifact,
    x: BufferHandle,
    y: BufferHandle,
    out: BufferHandle,
}

fn fixture() -> Fixture {
    let program = resolve(KERNELS);
    let mut session = session();
    let artifact = session.load_kernel(&program, "k.affine", MAP4).unwrap();
    let x = session.alloc(ScalarKind::I64, 8).unwrap();
    let y = session.alloc(ScalarKind::I64, 8).unwrap();
    let out = session.alloc(ScalarKind::I64, 8).unwrap();
    session
        .upload(x, 0, &i64s(&[1, 2, 3, 4, 5, 6, 7, 8]))
        .unwrap();
    session.upload(y, 0, &i64s(&[0; 8])).unwrap();
    Fixture {
        program,
        session,
        artifact,
        x,
        y,
        out,
    }
}

impl Fixture {
    fn dispatch(&mut self, control: DispatchControl) -> Result<DispatchOutcome, ComputeRefusal> {
        self.session.dispatch_map(
            &self.program,
            &self.artifact,
            &[self.x, self.y],
            self.out,
            control,
        )
    }
}

fn release(buffer: u32, cause: ReleaseCause, device_lost: bool) -> ReleaseEvent {
    ReleaseEvent {
        buffer,
        cause,
        device_lost,
    }
}

fn effect(effect: DeviceEffect, buffer: u32) -> EffectEvent {
    EffectEvent { effect, buffer }
}

#[test]
fn successful_lifecycle_journals_effects_and_releases_each_buffer_once_in_canonical_order() {
    let mut f = fixture();
    let outcome = f.dispatch(DispatchControl::default()).unwrap();
    assert_eq!(outcome, DispatchOutcome::Completed { invocations: 8 });
    assert_eq!(f.session.download(f.out, 2, 3).unwrap(), i64s(&[9, 12, 15]));
    f.session.release(f.y).unwrap();
    let settlement = f.session.settle();
    assert_eq!(settlement.selected, None);
    assert_eq!(
        settlement.releases,
        vec![
            release(1, ReleaseCause::Explicit, false),
            release(2, ReleaseCause::Settlement, false),
            release(0, ReleaseCause::Settlement, false),
        ]
    );
    assert_eq!(
        settlement.effects,
        vec![
            effect(DeviceEffect::DeviceAlloc, 0),
            effect(DeviceEffect::DeviceAlloc, 1),
            effect(DeviceEffect::DeviceAlloc, 2),
            effect(DeviceEffect::DeviceCopyIn, 0),
            effect(DeviceEffect::DeviceCopyIn, 1),
            effect(DeviceEffect::DeviceDispatch, 2),
            effect(DeviceEffect::DeviceCopyOut, 2),
            effect(DeviceEffect::DeviceRelease, 1),
            effect(DeviceEffect::DeviceRelease, 2),
            effect(DeviceEffect::DeviceRelease, 0),
        ]
    );
}

#[test]
fn identical_sessions_produce_identical_outputs_settlements_and_fingerprints() {
    let run = || {
        let mut f = fixture();
        f.dispatch(DispatchControl::default()).unwrap();
        let output = f.session.download(f.out, 0, 8).unwrap();
        let fingerprint = f.artifact.fingerprint().to_owned();
        (output, fingerprint, f.session.settle())
    };
    let first = run();
    assert_eq!(first, run());
    assert_eq!(first.1.len(), 64);
}

#[test]
fn allocation_is_zero_filled_and_transfers_are_bounds_checked() {
    let mut f = fixture();
    assert_eq!(f.session.download(f.out, 0, 8).unwrap(), i64s(&[0; 8]));
    for (offset, len) in [(0, 9), (8, 1), (usize::MAX, 2)] {
        let refusal = f.session.download(f.x, offset, len).unwrap_err();
        assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS, "{offset}+{len}");
    }
    let refusal = f.session.upload(f.x, 6, &i64s(&[9, 9, 9])).unwrap_err();
    assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS);
    // The refused upload wrote nothing.
    assert_eq!(
        f.session.download(f.x, 0, 8).unwrap(),
        i64s(&[1, 2, 3, 4, 5, 6, 7, 8])
    );
    f.session.upload(f.x, 6, &i64s(&[70, 80])).unwrap();
    assert_eq!(f.session.download(f.x, 5, 3).unwrap(), i64s(&[6, 70, 80]));

    let refusal = f.session.alloc(ScalarKind::U8, 0).unwrap_err();
    assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS);
    let refusal = f
        .session
        .alloc(
            ScalarKind::U8,
            crate::compute_profile::boundary_profile::MAX_BUFFER_ELEMENTS + 1,
        )
        .unwrap_err();
    assert_eq!(refusal.code(), CAPACITY_BOUND_EXCEEDED);
}

#[test]
fn element_type_mismatches_refuse_before_any_effect() {
    let mut f = fixture();
    let refusal = f.session.upload(f.x, 0, &[Scalar::I32(1)]).unwrap_err();
    assert_eq!(refusal.code(), ELEMENT_TYPE_MISMATCH);
    let wrong = f.session.alloc(ScalarKind::I32, 8).unwrap();
    let refusal = f
        .session
        .dispatch_map(
            &f.program,
            &f.artifact,
            &[f.x, wrong],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), ELEMENT_TYPE_MISMATCH);
    let short = f.session.alloc(ScalarKind::I64, 7).unwrap();
    let refusal = f
        .session
        .dispatch_map(
            &f.program,
            &f.artifact,
            &[f.x, short],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS);
    // Nothing was dispatched.
    let settlement = f.session.settle();
    assert!(!settlement
        .effects
        .iter()
        .any(|event| event.effect == DeviceEffect::DeviceDispatch));
}

#[test]
fn use_after_release_and_double_release_refuse_and_release_stays_exactly_once() {
    let mut f = fixture();
    f.session.release(f.x).unwrap();
    assert_eq!(
        f.session.release(f.x).unwrap_err(),
        ComputeRefusal::BufferReleased { buffer: 0 }
    );
    assert_eq!(
        f.session.upload(f.x, 0, &i64s(&[1])).unwrap_err().code(),
        BUFFER_RELEASED
    );
    assert_eq!(
        f.session.download(f.x, 0, 1).unwrap_err().code(),
        BUFFER_RELEASED
    );
    assert_eq!(
        f.dispatch(DispatchControl::default()).unwrap_err().code(),
        BUFFER_RELEASED
    );
    let settlement = f.session.settle();
    assert_eq!(settlement.selected, None);
    let released: Vec<u32> = settlement
        .releases
        .iter()
        .map(|event| event.buffer)
        .collect();
    assert_eq!(released, vec![0, 2, 1]);
}

#[test]
fn aliasing_output_and_unbounded_workgroups_refuse_with_admission_codes() {
    let mut f = fixture();
    let refusal = f
        .session
        .dispatch_map(
            &f.program,
            &f.artifact,
            &[f.x, f.out],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), ALIASING_BEYOND_CHECKED_RULE);
    // Two read-only views of one input are not a mutable alias.
    assert_eq!(
        f.session
            .dispatch_map(
                &f.program,
                &f.artifact,
                &[f.x, f.x],
                f.out,
                DispatchControl::default(),
            )
            .unwrap(),
        DispatchOutcome::Completed { invocations: 8 }
    );
    for workgroup_size in [0, 1025] {
        let refusal = f
            .session
            .load_kernel(
                &f.program,
                "k.affine",
                KernelShape::ElementwiseMap { workgroup_size },
            )
            .unwrap_err();
        assert_eq!(refusal.code(), GRID_SHAPE_OUT_OF_BOUNDS, "{workgroup_size}");
    }
    f.session
        .load_kernel(
            &f.program,
            "k.affine",
            KernelShape::ElementwiseMap {
                workgroup_size: 1024,
            },
        )
        .unwrap();
}

#[test]
fn cancellation_mid_dispatch_publishes_nothing_and_settles_every_buffer() {
    let mut f = fixture();
    let outcome = f
        .dispatch(DispatchControl {
            cancel_before_invocation: Some(3),
            ..DispatchControl::default()
        })
        .unwrap();
    let cancelled = SessionFailure::Cancelled {
        completed_invocations: 3,
    };
    assert_eq!(outcome, DispatchOutcome::Failed(cancelled.clone()));
    assert_eq!(
        f.session.download(f.out, 0, 8).unwrap_err(),
        ComputeRefusal::FailureAlreadySelected {
            failure: cancelled.clone()
        }
    );
    assert_eq!(
        f.dispatch(DispatchControl::default()).unwrap_err().code(),
        FAILURE_ALREADY_SELECTED
    );
    assert_eq!(
        f.session.alloc(ScalarKind::I64, 1).unwrap_err().code(),
        FAILURE_ALREADY_SELECTED
    );
    f.session.release(f.x).unwrap();
    let settlement = f.session.settle();
    assert_eq!(settlement.selected, Some(cancelled));
    assert_eq!(
        settlement.releases,
        vec![
            release(0, ReleaseCause::Explicit, false),
            release(2, ReleaseCause::Settlement, false),
            release(1, ReleaseCause::Settlement, false),
        ]
    );
}

#[test]
fn simulated_device_loss_is_sticky_and_settles_host_side_exactly_once() {
    let mut f = fixture();
    let outcome = f
        .dispatch(DispatchControl {
            device_loss_before_invocation: Some(2),
            // Loss wins when both name the same ordinal.
            cancel_before_invocation: Some(2),
        })
        .unwrap();
    let lost = SessionFailure::DeviceLost {
        completed_invocations: 2,
    };
    assert_eq!(outcome, DispatchOutcome::Failed(lost.clone()));
    for refusal in [
        f.session.upload(f.x, 0, &i64s(&[1])).unwrap_err(),
        f.session.download(f.x, 0, 1).unwrap_err(),
        f.session.alloc(ScalarKind::I64, 1).map(|_| ()).unwrap_err(),
        f.session
            .load_kernel(&f.program, "k.affine", MAP4)
            .map(|_| ())
            .unwrap_err(),
    ] {
        assert_eq!(
            refusal,
            ComputeRefusal::FailureAlreadySelected {
                failure: lost.clone()
            }
        );
    }
    f.session.release(f.out).unwrap();
    assert_eq!(
        f.session.release(f.out).unwrap_err().code(),
        BUFFER_RELEASED
    );
    let settlement = f.session.settle();
    assert_eq!(settlement.selected, Some(lost));
    assert_eq!(
        settlement.releases,
        vec![
            release(2, ReleaseCause::Explicit, true),
            release(1, ReleaseCause::Settlement, true),
            release(0, ReleaseCause::Settlement, true),
        ]
    );
    // No device release effect is journaled once the device is gone.
    assert!(!settlement
        .effects
        .iter()
        .any(|event| event.effect == DeviceEffect::DeviceRelease));
}

#[test]
fn kernel_status_failure_is_sticky_and_cleanup_cannot_replace_it() {
    let mut f = fixture();
    f.session
        .upload(f.x, 5, &i64s(&[i64::MAX, i64::MAX]))
        .unwrap();
    let outcome = f.dispatch(DispatchControl::default()).unwrap();
    let failure = SessionFailure::KernelStatus {
        declaration: "k.affine".to_owned(),
        invocation: 5,
        status: StatusCase::MulOverflow,
    };
    assert_eq!(outcome, DispatchOutcome::Failed(failure.clone()));
    assert_eq!(
        f.dispatch(DispatchControl {
            device_loss_before_invocation: Some(0),
            ..DispatchControl::default()
        })
        .unwrap_err()
        .code(),
        FAILURE_ALREADY_SELECTED
    );
    let settlement = f.session.settle();
    assert_eq!(settlement.selected, Some(failure));
    assert_eq!(settlement.releases.len(), 3);
}

#[test]
fn stale_artifacts_and_foreign_handles_refuse() {
    let mut f = fixture();
    // The same declaration with a different checked body.
    let edited = KERNELS.replace("let scaled = x * 3;", "let scaled = x * 5;");
    let edited = resolve(&edited);
    let refusal = f
        .session
        .dispatch_map(
            &edited,
            &f.artifact,
            &[f.x, f.y],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
    // The declaration removed from the program entirely.
    let removed = KERNELS.replace("@id(\"k.affine\")", "@id(\"k.affine_renamed\")");
    let removed = resolve(&removed);
    let refusal = f
        .session
        .dispatch_map(
            &removed,
            &f.artifact,
            &[f.x, f.y],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);

    // An artifact and buffers from another session.
    let mut other = session();
    let foreign_artifact = other.load_kernel(&f.program, "k.affine", MAP4).unwrap();
    let foreign_buffer = other.alloc(ScalarKind::I64, 8).unwrap();
    let refusal = f
        .session
        .dispatch_map(
            &f.program,
            &foreign_artifact,
            &[f.x, f.y],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), STALE_HANDLE);
    for refusal in [
        f.session.release(foreign_buffer).unwrap_err(),
        f.session
            .download(foreign_buffer, 0, 1)
            .map(|_| ())
            .unwrap_err(),
        f.session
            .dispatch_map(
                &f.program,
                &f.artifact,
                &[f.x, foreign_buffer],
                f.out,
                DispatchControl::default(),
            )
            .map(|_| ())
            .unwrap_err(),
    ] {
        assert_eq!(refusal.code(), STALE_HANDLE);
    }
    // The foreign session still owns and settles its own buffer.
    assert_eq!(other.settle().releases.len(), 1);
    // Nothing above changed this session: the current artifact still runs.
    assert_eq!(
        f.dispatch(DispatchControl::default()).unwrap(),
        DispatchOutcome::Completed { invocations: 8 }
    );
}

#[test]
fn capability_grants_are_enforced_and_release_is_always_granted_with_alloc() {
    assert_eq!(
        ComputeCapability::cpu_reference(&[DeviceEffect::DeviceAlloc]).unwrap_err(),
        ComputeRefusal::EffectNotGranted {
            effect: DeviceEffect::DeviceRelease
        }
    );
    let capability = ComputeCapability::cpu_reference(&[
        DeviceEffect::DeviceAlloc,
        DeviceEffect::DeviceRelease,
        DeviceEffect::DeviceCopyIn,
    ])
    .unwrap();
    let mut session = CpuReferenceSession::open(capability);
    let buffer = session.alloc(ScalarKind::Bool, 2).unwrap();
    session.upload(buffer, 0, &[Scalar::Bool(true)]).unwrap();
    assert_eq!(
        session.download(buffer, 0, 1).unwrap_err(),
        ComputeRefusal::EffectNotGranted {
            effect: DeviceEffect::DeviceCopyOut
        }
    );
    let program = resolve(KERNELS);
    let artifact = session
        .load_kernel(&program, "k.index_scale", MAP4)
        .unwrap();
    let refusal = session
        .dispatch_map(
            &program,
            &artifact,
            &[buffer],
            buffer,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), EFFECT_NOT_GRANTED);
    assert_eq!(session.settle().releases.len(), 1);
}

#[test]
fn shape_mismatched_dispatch_and_fold_output_extent_refuse() {
    let mut f = fixture();
    let fold = f
        .session
        .load_kernel(&f.program, "k.sum", KernelShape::SequentialFold)
        .unwrap();
    let refusal = f
        .session
        .dispatch_map(
            &f.program,
            &fold,
            &[f.x, f.y],
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), KERNEL_SELECTION_REFUSED);
    let refusal = f
        .session
        .dispatch_fold(
            &f.program,
            &fold,
            Scalar::I64(0),
            f.x,
            f.out,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), TRANSFER_OUT_OF_BOUNDS);
    let one = f.session.alloc(ScalarKind::I64, 1).unwrap();
    let refusal = f
        .session
        .dispatch_fold(
            &f.program,
            &fold,
            Scalar::U8(0),
            f.x,
            one,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), ELEMENT_TYPE_MISMATCH);
    let refusal = f
        .session
        .dispatch_fold(
            &f.program,
            &fold,
            Scalar::I64(0),
            one,
            one,
            DispatchControl::default(),
        )
        .unwrap_err();
    assert_eq!(refusal.code(), ALIASING_BEYOND_CHECKED_RULE);
    let outcome = f
        .session
        .dispatch_fold(
            &f.program,
            &fold,
            Scalar::I64(100),
            f.x,
            one,
            DispatchControl {
                cancel_before_invocation: Some(8),
                ..DispatchControl::default()
            },
        )
        .unwrap();
    // A cancellation point past the last invocation is never reached.
    assert_eq!(outcome, DispatchOutcome::Completed { invocations: 8 });
    assert_eq!(f.session.download(one, 0, 1).unwrap(), i64s(&[136]));
}
