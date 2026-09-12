//! Every test below starts from [`admitted_baseline`], a fixture
//! [`baseline_is_fully_admitted`] proves [`classify`] admits outright, and
//! changes exactly one field. Because the baseline already satisfies every
//! rule [`classify`] checks — including every rule that runs *before* the
//! one a given test targets, per the module documentation's precedence list
//! — a negative test's asserted [`Refusal`] cannot be attributed to any
//! other rule: every other rule is already known to pass on this exact
//! fixture, and the mutation touches nothing else. Where two rules examine
//! the same kind of field (for example a buffer parameter's type vs. its
//! capacity), the mutation is a single field write, visible directly in the
//! test body, so the isolation is not merely asserted but readable.

use super::*;

const NINE_PARAM_NAMES: [&str; 9] = [
    "p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8",
];

fn admitted_baseline() -> KernelCandidate {
    KernelCandidate {
        name: "vector_add",
        effects: vec![DeviceEffect::DeviceDispatch],
        declared_other_effects: Vec::new(),
        explicit_device_capability: true,
        params: vec![
            KernelParam {
                name: "input",
                ty: KernelType::Buffer {
                    elem: ScalarType::I32,
                    space: AddressSpace::Global,
                    len: 1024,
                },
                mode: ParamMode::ReadOnlyView,
            },
            KernelParam {
                name: "output",
                ty: KernelType::Buffer {
                    elem: ScalarType::I32,
                    space: AddressSpace::Global,
                    len: 1024,
                },
                mode: ParamMode::ReadWriteView,
            },
        ],
        grid: GridShape {
            workgroup_size: [64, 1, 1],
            grid_size: [16, 1, 1],
        },
        aliasing: AliasingClaim::Disjoint,
        body: vec![
            KernelOp::IntegerArithmetic,
            KernelOp::IndexedLoad {
                space: AddressSpace::Global,
                statically_in_bounds: false,
                dynamically_checked: true,
            },
            KernelOp::IndexedStore {
                space: AddressSpace::Global,
                statically_in_bounds: false,
                dynamically_checked: true,
            },
            KernelOp::Reduction {
                elem: ScalarType::I32,
                order: ReductionOrder::SequentialLeftToRight,
            },
            KernelOp::Atomic {
                is_integer: true,
                order: AtomicOrder::SequentiallyConsistentFixedOrder,
            },
            KernelOp::Barrier,
        ],
    }
}

#[test]
fn baseline_is_fully_admitted() {
    let baseline = admitted_baseline();
    let admitted = classify(&baseline).expect("the baseline fixture is admitted");
    assert_eq!(admitted.schema, COMPUTE_KERNEL_PROFILE_SCHEMA);
    assert_eq!(admitted.name, "vector_add");
    assert_eq!(admitted.param_count, 2);
    assert_eq!(admitted.grid, baseline.grid);
}

// --- SPX-GC001: effect outside the closed device-effect vocabulary -----

#[test]
fn declared_effect_outside_closed_vocabulary_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.declared_other_effects = vec!["fs.read"];
    assert_eq!(
        classify(&candidate),
        Err(Refusal::EffectOutsideClosedVocabulary { token: "fs.read" })
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        EFFECT_OUTSIDE_CLOSED_VOCABULARY
    );
}

// --- SPX-GC002: implicit device selection ------------------------------

#[test]
fn dispatch_without_explicit_device_capability_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.explicit_device_capability = false;
    assert_eq!(classify(&candidate), Err(Refusal::ImplicitDeviceSelection));
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        IMPLICIT_DEVICE_SELECTION
    );
}

// --- SPX-GC003: parameter count out of bounds --------------------------

#[test]
fn zero_parameters_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params.clear();
    assert_eq!(
        classify(&candidate),
        Err(Refusal::ParameterCountOutOfBounds { found: 0 })
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        PARAMETER_COUNT_OUT_OF_BOUNDS
    );
}

#[test]
fn parameter_count_over_the_bound_is_refused() {
    let mut candidate = admitted_baseline();
    let template = candidate.params[1].clone();
    candidate.params = NINE_PARAM_NAMES
        .iter()
        .map(|&name| KernelParam {
            name,
            ..template.clone()
        })
        .collect();
    assert_eq!(candidate.params.len(), 9);
    assert!(candidate.params.len() > MAX_KERNEL_PARAMS);
    assert_eq!(
        classify(&candidate),
        Err(Refusal::ParameterCountOutOfBounds { found: 9 })
    );
}

#[test]
fn parameter_count_at_the_bound_is_admitted() {
    let mut candidate = admitted_baseline();
    let template = candidate.params[1].clone();
    candidate.params = NINE_PARAM_NAMES[..MAX_KERNEL_PARAMS]
        .iter()
        .map(|&name| KernelParam {
            name,
            ..template.clone()
        })
        .collect();
    assert_eq!(candidate.params.len(), MAX_KERNEL_PARAMS);
    assert!(classify(&candidate).is_ok());
}

// --- SPX-GC004: type outside the kernel-safe vocabulary ----------------

#[test]
fn floating_point_parameter_type_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params[0].ty = KernelType::Scalar(ScalarType::F32);
    assert_eq!(
        classify(&candidate),
        Err(Refusal::TypeOutsideKernelVocabulary { param: "input" })
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        TYPE_OUTSIDE_KERNEL_VOCABULARY
    );
}

#[test]
fn vector_with_unadmitted_lane_count_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params[0].ty = KernelType::Vector {
        elem: ScalarType::I32,
        lanes: 5,
    };
    assert_eq!(
        classify(&candidate),
        Err(Refusal::TypeOutsideKernelVocabulary { param: "input" })
    );
}

// --- SPX-GC005: ownership mode not admitted -----------------------------

#[test]
fn owned_transfer_parameter_mode_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params[0].mode = ParamMode::OwnedTransfer;
    // The parameter's type is untouched, still an admitted, in-bounds
    // buffer: SPX-GC004/SPX-GC013 already pass on this exact fixture.
    assert!(matches!(candidate.params[0].ty, KernelType::Buffer { .. }));
    assert_eq!(
        classify(&candidate),
        Err(Refusal::OwnershipModeNotAdmitted { param: "input" })
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        OWNERSHIP_MODE_NOT_ADMITTED
    );
}

#[test]
fn shared_alias_parameter_mode_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params[0].mode = ParamMode::SharedAlias;
    assert_eq!(
        classify(&candidate),
        Err(Refusal::OwnershipModeNotAdmitted { param: "input" })
    );
}

// --- SPX-GC006: grid/workgroup shape out of bounds ----------------------

#[test]
fn zero_workgroup_dimension_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.grid.workgroup_size = [0, 1, 1];
    assert_eq!(classify(&candidate), Err(Refusal::GridShapeOutOfBounds));
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        GRID_SHAPE_OUT_OF_BOUNDS
    );
}

#[test]
fn workgroup_dimension_over_the_bound_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.grid.workgroup_size = [MAX_WORKGROUP_DIM + 1, 1, 1];
    assert_eq!(classify(&candidate), Err(Refusal::GridShapeOutOfBounds));
}

#[test]
fn workgroup_invocation_product_over_the_bound_is_refused() {
    let mut candidate = admitted_baseline();
    // Each dimension individually stays within MAX_WORKGROUP_DIM; only the
    // product exceeds MAX_WORKGROUP_INVOCATIONS.
    candidate.grid.workgroup_size = [MAX_WORKGROUP_DIM, 2, 1];
    assert!(candidate
        .grid
        .workgroup_size
        .iter()
        .all(|&d| d <= MAX_WORKGROUP_DIM));
    assert_eq!(classify(&candidate), Err(Refusal::GridShapeOutOfBounds));
}

#[test]
fn zero_grid_dimension_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.grid.grid_size = [0, 1, 1];
    assert_eq!(classify(&candidate), Err(Refusal::GridShapeOutOfBounds));
}

#[test]
fn grid_dimension_over_the_bound_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.grid.grid_size = [MAX_GRID_DIM + 1, 1, 1];
    assert_eq!(classify(&candidate), Err(Refusal::GridShapeOutOfBounds));
}

#[test]
fn workgroup_and_grid_at_their_bounds_are_admitted() {
    let mut candidate = admitted_baseline();
    candidate.grid.workgroup_size = [MAX_WORKGROUP_DIM, 1, 1];
    candidate.grid.grid_size = [MAX_GRID_DIM, 1, 1];
    assert!(classify(&candidate).is_ok());
}

// --- SPX-GC007: unchecked indexed access --------------------------------

#[test]
fn unchecked_indexed_load_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[1] = KernelOp::IndexedLoad {
        space: AddressSpace::Global,
        statically_in_bounds: false,
        dynamically_checked: false,
    };
    assert_eq!(classify(&candidate), Err(Refusal::UncheckedIndexedAccess));
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        UNCHECKED_INDEXED_ACCESS
    );
}

#[test]
fn unchecked_indexed_store_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[2] = KernelOp::IndexedStore {
        space: AddressSpace::Global,
        statically_in_bounds: false,
        dynamically_checked: false,
    };
    assert_eq!(classify(&candidate), Err(Refusal::UncheckedIndexedAccess));
}

// --- SPX-GC008: non-deterministic reduction order -----------------------

#[test]
fn tree_associative_reduction_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[3] = KernelOp::Reduction {
        elem: ScalarType::I32,
        order: ReductionOrder::TreeAssociative,
    };
    assert_eq!(
        classify(&candidate),
        Err(Refusal::NondeterministicReductionOrder)
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        NONDETERMINISTIC_REDUCTION_ORDER
    );
}

#[test]
fn unspecified_reduction_order_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[3] = KernelOp::Reduction {
        elem: ScalarType::I32,
        order: ReductionOrder::Unspecified,
    };
    assert_eq!(
        classify(&candidate),
        Err(Refusal::NondeterministicReductionOrder)
    );
}

// --- SPX-GC009: non-deterministic atomic --------------------------------

#[test]
fn atomic_without_fixed_order_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[4] = KernelOp::Atomic {
        is_integer: true,
        order: AtomicOrder::RelaxedDriverDefined,
    };
    assert_eq!(classify(&candidate), Err(Refusal::NondeterministicAtomic));
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        NONDETERMINISTIC_ATOMIC
    );
}

#[test]
fn non_integer_atomic_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.body[4] = KernelOp::Atomic {
        is_integer: false,
        order: AtomicOrder::SequentiallyConsistentFixedOrder,
    };
    assert_eq!(classify(&candidate), Err(Refusal::NondeterministicAtomic));
}

// --- SPX-GC010: floating-point body operation ---------------------------

#[test]
fn floating_point_body_operation_is_refused() {
    let mut candidate = admitted_baseline();
    // Position 0: nothing earlier in the body list, and every non-body
    // rule (effects, params, grid, aliasing) is untouched from the
    // baseline.
    candidate.body[0] = KernelOp::FloatArithmetic;
    assert_eq!(
        classify(&candidate),
        Err(Refusal::FloatingPointNotAdmittedInV1)
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        FLOATING_POINT_NOT_ADMITTED_IN_V1
    );
}

// --- SPX-GC011: aliasing beyond the checked rule ------------------------

#[test]
fn overlapping_buffer_aliasing_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.aliasing = AliasingClaim::MayOverlap;
    assert_eq!(
        classify(&candidate),
        Err(Refusal::AliasingBeyondCheckedRule)
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        ALIASING_BEYOND_CHECKED_RULE
    );
}

// --- SPX-GC012: kernel body not pure ------------------------------------

#[test]
fn kernel_body_host_effect_is_refused() {
    let mut candidate = admitted_baseline();
    // Position 0, same reasoning as the floating-point body-operation case
    // above: nothing earlier in the body list to mask.
    candidate.body[0] = KernelOp::HostEffect;
    assert_eq!(classify(&candidate), Err(Refusal::KernelBodyNotPure));
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        KERNEL_BODY_NOT_PURE
    );
}

// --- SPX-GC013: capacity bound exceeded ---------------------------------

#[test]
fn buffer_over_the_capacity_bound_is_refused() {
    let mut candidate = admitted_baseline();
    candidate.params[0].ty = KernelType::Buffer {
        elem: ScalarType::I32,
        space: AddressSpace::Global,
        len: MAX_BUFFER_ELEMENTS + 1,
    };
    // The element type and mode are untouched: SPX-GC004/SPX-GC005 already
    // pass on this exact fixture.
    assert_eq!(candidate.params[0].mode, ParamMode::ReadOnlyView);
    assert_eq!(
        classify(&candidate),
        Err(Refusal::CapacityBoundExceeded { param: "input" })
    );
    assert_eq!(
        classify(&candidate).unwrap_err().code(),
        CAPACITY_BOUND_EXCEEDED
    );
}

#[test]
fn buffer_at_the_capacity_bound_is_admitted() {
    let mut candidate = admitted_baseline();
    candidate.params[0].ty = KernelType::Buffer {
        elem: ScalarType::I32,
        space: AddressSpace::Global,
        len: MAX_BUFFER_ELEMENTS,
    };
    assert!(classify(&candidate).is_ok());
}

// --- Diagnostic rendering ------------------------------------------------

#[test]
fn every_refusal_code_matches_installed_diagnostics_v1_shape() {
    // A 4-to-16-byte SPX-<namespace><3 digits> token, per
    // docs/INSTALLED-DIAGNOSTICS-V1.md's static source scan rule.
    let codes = [
        EFFECT_OUTSIDE_CLOSED_VOCABULARY,
        IMPLICIT_DEVICE_SELECTION,
        PARAMETER_COUNT_OUT_OF_BOUNDS,
        TYPE_OUTSIDE_KERNEL_VOCABULARY,
        OWNERSHIP_MODE_NOT_ADMITTED,
        GRID_SHAPE_OUT_OF_BOUNDS,
        UNCHECKED_INDEXED_ACCESS,
        NONDETERMINISTIC_REDUCTION_ORDER,
        NONDETERMINISTIC_ATOMIC,
        FLOATING_POINT_NOT_ADMITTED_IN_V1,
        ALIASING_BEYOND_CHECKED_RULE,
        KERNEL_BODY_NOT_PURE,
        CAPACITY_BOUND_EXCEEDED,
    ];
    let mut seen = std::collections::BTreeSet::new();
    for code in codes {
        assert!(code.starts_with("SPX-GC"));
        assert_eq!(code.len(), 9, "{code} must be SPX-GC + 3 digits");
        assert!(
            code["SPX-GC".len()..].chars().all(|c| c.is_ascii_digit()),
            "{code} must end in three ASCII digits"
        );
        assert!(seen.insert(code), "{code} must be unique in this table");
    }
    assert_eq!(seen.len(), 13);
}

#[test]
fn refusal_diagnostic_carries_its_own_code_and_a_nonempty_message() {
    let candidate = {
        let mut candidate = admitted_baseline();
        candidate.aliasing = AliasingClaim::MayOverlap;
        candidate
    };
    let refusal = classify(&candidate).unwrap_err();
    let diagnostic = refusal.diagnostic();
    assert_eq!(diagnostic.code, ALIASING_BEYOND_CHECKED_RULE);
    assert!(!diagnostic.message.is_empty());
}
