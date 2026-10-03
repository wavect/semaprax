use super::*;
use semaprax::native_rust_binding::foreign_law::{
    self, DeclaredForeignSummary, ForeignBoundary, ForeignLawRequest,
};

const LOCK: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const ADAPTER: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const PROPOSITION: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const TARGET: &str = "x86_64-unknown-linux-gnu";

fn fixture() -> (
    hir::ResolvedProgram,
    semaprax::native_rust_binding::ScalarBindingPlan,
) {
    let program = hir::resolve(&parse(SOURCE, Path::new("law-foreign.spx")).unwrap()).unwrap();
    let binding = prepare_scalar_binding(
        &program.interfaces[0].imports[0],
        selected_item(
            "first",
            "first::combine",
            "fn combine(left: i64, selected: bool) -> i64",
        ),
    )
    .unwrap();
    (program, binding)
}

fn declaration() -> DeclaredForeignSummary {
    DeclaredForeignSummary {
        assumption_id: "foreign.combine.behavior".into(),
        proposition_digest: PROPOSITION.into(),
        assumes_no_effects: true,
        assumes_no_callbacks: true,
        assumes_no_panics: true,
        assumes_no_shared_state: true,
        return_i64_range: Some((0, 42)),
    }
}

fn law() -> ForeignLawRequest {
    ForeignLawRequest {
        law_id: "law.combine.bound".into(),
        permit_assumptions: true,
        require_theorem: false,
        require_no_effects: true,
        require_no_callbacks: true,
        require_no_panics: true,
        require_no_shared_state: true,
        require_return_guard: true,
    }
}

fn boundary<'a>(lock: &'a str, target: &'a str, adapter: &'a str) -> ForeignBoundary<'a> {
    ForeignBoundary {
        project_lock_digest: lock,
        target,
        adapter_digest: adapter,
    }
}

#[test]
fn foreign_law_reports_conditions_without_promoting_signature_to_theorem() {
    let (program, binding) = fixture();
    let import = &program.interfaces[0].imports[0];
    let summary = declaration();
    let request = law();
    let frontier = foreign_law::derive(
        import,
        &binding,
        boundary(LOCK, TARGET, ADAPTER),
        &summary,
        &request,
    )
    .unwrap();
    assert_eq!(frontier.conditions().len(), 4);
    assert_eq!(frontier.physical_symbol(), binding.physical_symbol);
    let view: serde_json::Value = serde_json::from_str(&frontier.public_view()).unwrap();
    assert_eq!(view["status"], "conditional_on_foreign_assumptions");
    assert_eq!(view["foreign_internals_proved"], false);
    for property in ["effects", "callbacks", "panics", "shared_state"] {
        assert_eq!(view["foreign_behavior"][property], "assumed_absent");
    }
    assert_eq!(view["feature_digest"], binding.feature_digest);
    assert_eq!(view["project_lock_digest"], LOCK);
    assert_eq!(view["adapter_digest"], ADAPTER);
    assert_eq!(view["physical_symbol"], binding.physical_symbol);
    assert!(view["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|condition| { condition == "foreign.combine.behavior:no_callbacks" }));
    assert_eq!(foreign_law::guard_i64_return(&frontier, 42).unwrap(), 42);
    // Seeded wrong foreign implementation: a valid i64 signature still cannot
    // publish a return outside the retained runtime boundary contract.
    assert_eq!(
        foreign_law::guard_i64_return(&frontier, 43)
            .unwrap_err()
            .code,
        "SPX-FL306"
    );
    let mut theorem = request.clone();
    theorem.require_theorem = true;
    assert_eq!(
        foreign_law::derive(
            import,
            &binding,
            boundary(LOCK, TARGET, ADAPTER),
            &summary,
            &theorem
        )
        .unwrap_err()
        .code,
        "SPX-FL301"
    );
    let mut strict = request;
    strict.permit_assumptions = false;
    assert_eq!(
        foreign_law::derive(
            import,
            &binding,
            boundary(LOCK, TARGET, ADAPTER),
            &summary,
            &strict
        )
        .unwrap_err()
        .code,
        "SPX-FL303"
    );
}

#[test]
fn exact_foreign_summary_replay_stales_all_identity_and_behavior_inputs() {
    let (program, binding) = fixture();
    let import = &program.interfaces[0].imports[0];
    let summary = declaration();
    let request = law();
    let original = foreign_law::derive(
        import,
        &binding,
        boundary(LOCK, TARGET, ADAPTER),
        &summary,
        &request,
    )
    .unwrap();
    foreign_law::replay(
        &original,
        import,
        &binding,
        boundary(LOCK, TARGET, ADAPTER),
        &summary,
        &request,
    )
    .unwrap();
    for changed in [
        boundary(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TARGET,
            ADAPTER,
        ),
        boundary(
            LOCK,
            TARGET,
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        ),
    ] {
        assert_eq!(
            foreign_law::replay(&original, import, &binding, changed, &summary, &request)
                .unwrap_err()
                .code,
            "SPX-FL304"
        );
    }
    assert_eq!(
        foreign_law::replay(
            &original,
            import,
            &binding,
            boundary(LOCK, "aarch64-unknown-linux-gnu", ADAPTER),
            &summary,
            &request,
        )
        .unwrap_err()
        .code,
        "SPX-FL300"
    );
    let mut changed_feature = binding.clone();
    changed_feature.feature_digest =
        "sha256:3333333333333333333333333333333333333333333333333333333333333333".into();
    assert_eq!(
        foreign_law::replay(
            &original,
            import,
            &changed_feature,
            boundary(LOCK, TARGET, ADAPTER),
            &summary,
            &request,
        )
        .unwrap_err()
        .code,
        "SPX-B142"
    );
    let mut changed_symbol = binding.clone();
    changed_symbol.physical_symbol = "spx_ri04_wrong".into();
    assert_eq!(
        foreign_law::replay(
            &original,
            import,
            &changed_symbol,
            boundary(LOCK, TARGET, ADAPTER),
            &summary,
            &request,
        )
        .unwrap_err()
        .code,
        "SPX-B142"
    );
    let mut changed_assumption = summary.clone();
    changed_assumption.assumes_no_callbacks = false;
    assert_eq!(
        foreign_law::replay(
            &original,
            import,
            &binding,
            boundary(LOCK, TARGET, ADAPTER),
            &changed_assumption,
            &request,
        )
        .unwrap_err()
        .code,
        "SPX-FL302"
    );
    let mut changed_proposition = summary.clone();
    changed_proposition.proposition_digest =
        "sha256:4444444444444444444444444444444444444444444444444444444444444444".into();
    assert_eq!(
        foreign_law::replay(
            &original,
            import,
            &binding,
            boundary(LOCK, TARGET, ADAPTER),
            &changed_proposition,
            &request,
        )
        .unwrap_err()
        .code,
        "SPX-FL304"
    );
}

#[test]
fn unknown_callback_effect_panic_and_shared_state_never_disappear() {
    let (program, binding) = fixture();
    let import = &program.interfaces[0].imports[0];
    let mut summary = declaration();
    let request = law();
    for field in 0..4 {
        let mut changed = summary.clone();
        match field {
            0 => changed.assumes_no_effects = false,
            1 => changed.assumes_no_callbacks = false,
            2 => changed.assumes_no_panics = false,
            _ => changed.assumes_no_shared_state = false,
        }
        assert_eq!(
            foreign_law::derive(
                import,
                &binding,
                boundary(LOCK, TARGET, ADAPTER),
                &changed,
                &request,
            )
            .unwrap_err()
            .code,
            "SPX-FL302"
        );
    }
    summary.return_i64_range = None;
    assert_eq!(
        foreign_law::derive(
            import,
            &binding,
            boundary(LOCK, TARGET, ADAPTER),
            &summary,
            &request
        )
        .unwrap_err()
        .code,
        "SPX-FL302"
    );
}
