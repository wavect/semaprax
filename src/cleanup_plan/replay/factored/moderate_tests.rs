//! The earlier representation retains full enumeration and weighted languages.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;

mod before;

fn fixture(decisions: usize, owned: bool) -> ResolvedProgram {
    let branches = (0..decisions)
        .map(|index| format!("let branch_{index}=if 0=={index} {{1}}else{{2}};"))
        .collect::<String>();
    let parameters = if owned { "input: own Bytes" } else { "" };
    let source = format!(
        "module moderate.paths; @id(\"moderate.paths\") fn paths({parameters})->i64 {{{branches}42}}"
    );
    crate::hir::resolve(&crate::check(&source, "moderate-paths.spx").unwrap()).unwrap()
}

fn enumerated(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut hir = hir_skeleton_paths(program, function, &mut budget)?;
    let mut plan = plan_skeleton_paths(function, &mut budget)?;
    hir.sort();
    plan.sort();
    Ok(hir == plan)
}

#[test]
fn moderate_scalar_route_keeps_owned_fallbacks_and_complete_enumerated_multisets() {
    for (decisions, owned) in [
        (8, false),
        (9, false),
        (10, false),
        (8, true),
        (9, true),
        (10, true),
    ] {
        let program = fixture(decisions, owned);
        let function = &program.functions[0];
        assert_eq!(!function.cleanup_plan.slots.is_empty(), owned);
        assert!(function.cleanup_plan.lexical_control.is_none());
        let cfg = branch_sensitive_cfg_bounds(function)
            .unwrap()
            .terminal_paths;
        let hir = hir_terminal_path_bound(function).unwrap();
        assert_eq!(cfg, 1 << decisions);
        assert_eq!(hir, cfg);
        assert!(cfg <= MAX_REPLAY_PATHS);
        assert!(enumerated(&program, function).unwrap());
        assert!(!before::selected_paid(function, cfg, hir, &mut ReplayBudget::new()).unwrap());
        assert_eq!(selected(function, cfg, hir), !owned && decisions > 8);
        assert_eq!(
            selected_paid(function, cfg, hir, &mut ReplayBudget::new()).unwrap(),
            !owned && decisions > 8
        );
        assert_eq!(
            compare(
                &program,
                function,
                &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
            )
            .unwrap(),
            Some(true)
        );
        assert_eq!(
            validate(
                &program,
                function,
                &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
            )
            .unwrap(),
            !owned && decisions > 8
        );
    }
}

#[test]
fn weighted_representation_cannot_hide_a_changed_boolean_route_or_its_multiplicity() {
    let program = fixture(9, false);
    let original = &program.functions[0];
    let mut changed = original.clone();
    let condition = changed
        .cleanup_plan
        .edges
        .iter_mut()
        .find_map(|edge| match &mut edge.condition {
            EdgeCondition::BooleanResult(_, value) => Some(value),
            _ => None,
        })
        .expect("fixture contains retained Boolean observations");
    *condition = !*condition;
    assert!(!enumerated(&program, &changed).unwrap());
    assert_eq!(
        compare(
            &program,
            &changed,
            &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
        )
        .unwrap(),
        Some(false)
    );
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn moderate_comparison_actual_builder_keeps_exact_short_zero_and_floor_refusals() {
    use crate::bounded_output::set_active_floor;
    let program = fixture(9, false);
    let function = &program.functions[0];
    let observe = |available, floor| {
        with_limit_usage_refusal(available + floor, || {
            assert!(set_active_floor(floor));
            let mut budget = ReplayBudget::new();
            let result = compare(&program, function, &mut budget);
            if crate::bounded_output::active_refused() {
                assert!(result.is_err());
                assert!(!crate::bounded_output::reserve_active_required(0));
            }
            result
        })
    };
    let (result, refused, used, receipt) = observe(usize::MAX, 0);
    assert_eq!(result.unwrap(), Some(true));
    assert!(!refused && receipt.is_none() && used > 0);
    for floor in [0, 37] {
        let (result, refused, exact, receipt) = observe(used, floor);
        assert_eq!(result.unwrap(), Some(true));
        assert!(!refused && receipt.is_none());
        assert_eq!(exact, used);
        for available in [0, used - 1] {
            let (result, refused, actual, receipt) = observe(available, floor);
            assert!(result.is_err());
            assert!(refused);
            let receipt = receipt.unwrap();
            assert_eq!(receipt.floor, floor);
            assert!(actual <= available);
        }
    }
}

#[test]
fn original_selector_and_whole_source_receivers_remain_bound() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/moderate-factored-v1");
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    for row in rows.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(row["path"].as_str().unwrap())).unwrap();
        let restore = |text: &str| -> Option<String> {
            let mut text = text.to_owned();
            for edit in row["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if text.matches(new).count() != 1 {
                    return None;
                }
                text = text.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(text)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
    let original = std::fs::read_to_string(evidence.join("0.before.txt")).unwrap();
    let selector = original
        .split_once("pub(super) fn selected_paid(")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]")
        .unwrap()
        .0
        .trim_end();
    let current = include_str!("moderate_tests/before.rs");
    let prefix = "use super::*;\nconst FACTORED_PATH_THRESHOLD: usize = 4_096;\n\npub(super) fn selected_paid(";
    assert_eq!(current.strip_prefix(prefix).unwrap().trim_end(), selector);
}
