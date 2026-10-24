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
    // The complete original library-shaped literal is not an executable.
    // Retain its exact six-field refusal before adding a separate legal main;
    // paths remains the first function and every original replay assertion uses it.
    let diagnostics = crate::check(&source, "moderate-paths.spx").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.code, "SPX-T105");
    assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
    assert_eq!(
        diagnostic.message,
        "executable module must define `fn main() -> i64`"
    );
    assert_eq!(diagnostic.path.as_deref(), Some("moderate-paths.spx"));
    let start = source.find("fn paths").unwrap();
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start, span.end, span.line, span.column)),
        Some((start, source.len(), 1, start + 1))
    );
    assert_eq!(diagnostic.help.as_deref(), Some("a module without `fn main() -> i64` is a library module: check it through the project that owns it with `semaprax check <project-dir>`, or add `main` to run this file alone"));
    let source = format!("{source}\n@id(\"moderate.main\") fn main()->i64 {{0}}");
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
        let current = joined_input(row["path"].as_str().unwrap(), &current);
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
    joined_hostiles();
}

// Only this incoming historical owner receives a finite view. Production and
// complete raw source inventories continue to inspect the actual joined bytes.
const JOINED_LENGTH: usize = 319918;
const JOINED_SHA256: &str = "3c8e81fe794761cd061189a17ac6c003311920c299599c9b1cbb57b771fda258";
const JOINED_PATH: &str =
    "docs/evidence/moderate-factored-joined-custody-20261017/joined-inputs.json";
fn joined_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[allow(
    clippy::assertions_on_constants,
    reason = "These assertions pin the exact control layout and resource limits."
)]
fn joined_data() -> String {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut file = std::fs::File::open(root.join(JOINED_PATH)).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert!(JOINED_LENGTH <= 2 * 1024 * 1024);
    assert_eq!(metadata.len(), JOINED_LENGTH as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(JOINED_LENGTH).unwrap();
    bytes.resize(JOINED_LENGTH, 0);
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0]).unwrap(), 0);
    assert_eq!(joined_digest(&bytes), JOINED_SHA256);
    String::from_utf8(bytes).unwrap()
}
fn joined_checked(data: &str, path: &str, current: &str) -> Option<String> {
    if data.len() != JOINED_LENGTH || joined_digest(data.as_bytes()) != JOINED_SHA256 {
        return None;
    }
    let rows: serde_json::Value = serde_json::from_str(data).ok()?;
    let rows = rows.as_array()?;
    if rows.len() != 3 || rows.iter().filter(|row| row["path"] == path).count() != 1 {
        return None;
    }
    let row = rows.iter().find(|row| row["path"] == path)?;
    if current.len() != row["current_length"].as_u64()? as usize
        || joined_digest(current.as_bytes()) != row["current_sha256"].as_str()?
    {
        return None;
    }
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array()?.iter().rev() {
        let start = usize::try_from(edit["start"].as_u64()?).ok()?;
        let end = usize::try_from(edit["end"].as_u64()?).ok()?;
        if restored.get(start..end)? != edit["current"].as_str()? {
            return None;
        }
        restored.replace_range(start..end, edit["incoming"].as_str()?);
    }
    if restored.len() != row["incoming_length"].as_u64()? as usize
        || joined_digest(restored.as_bytes()) != row["incoming_sha256"].as_str()?
    {
        return None;
    }
    Some(restored)
}
fn joined_input(path: &str, current: &str) -> String {
    let checkpoint = joined_checkpoint_input(path, current);
    crate::source_reflows_merge::main7f8_source_inputs::moderate_452_input(
        "moderate-original",
        path,
        &checkpoint,
    )
}
fn joined_checkpoint_input(path: &str, current: &str) -> String {
    joined_checked(&joined_data(), path, current)
        .expect("complete joined-to-incoming source domain")
}
fn joined_hostiles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = joined_data();
    let rows: serde_json::Value = serde_json::from_str(&data).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let raw = std::fs::read_to_string(root.join(path)).unwrap();
        let target =
            std::fs::read_to_string(root.join(row["incoming_archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            joined_checked(&data, path, &raw).as_deref(),
            Some(target.as_str())
        );
        for wrong in [
            String::new(),
            raw[..raw.len() / 2].to_owned(),
            format!("{raw} "),
            format!("!{}", &raw[1..]),
        ] {
            assert!(joined_checked(&data, path, &wrong).is_none());
        }
        if raw != target {
            assert!(joined_checked(&data, path, &target).is_none());
        }
    }
    assert!(joined_checked(&data, "src/foreign.rs", "foreign").is_none());
    assert!(joined_checked(&data[..data.len() - 1], "src/foreign.rs", "foreign").is_none());
    assert!(joined_checked(&format!("!{}", &data[1..]), "src/foreign.rs", "foreign").is_none());
}
