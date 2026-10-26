//! Lexical join of the exact already-validated match-arm witness.
use super::*;
// Keep delegated require_type and hir_error complete. Only the physically
// absent join-helper actual/formal tuple and outer Result boundary are removed.
// Two additional reference slots conservatively fund the lexical value binding.
pub(in crate::hir::validation) const LEXICAL_JOIN: usize = PAYMENT
    + WITNESS_TYPE_CALL
    + WITNESS_ERROR_CALL
    + 3 * std::mem::size_of::<&ResolvedExpr>()
    + 2 * std::mem::size_of::<Option<&ResolvedExpr>>()
    + 4 * std::mem::size_of::<bool>();
macro_rules! lexical_body {
    ($validator:ident, $result:ident, $value:expr) => {{
        let value: &ResolvedExpr = &$value;
        if let Some(previous) = $result {
            if previous.ty != ResolvedType::Never && value.ty != ResolvedType::Never {
                $validator.require_type(&value.ty, &previous.ty, "match arm")?;
                if value.ownership != previous.ownership {
                    return Err(hir_error("resolved match arms have inconsistent ownership"));
                }
            }
            if previous.ty == ResolvedType::Never {
                $result = Some(value);
            }
        } else {
            $result = Some(value);
        }
    }};
}
pub(in crate::hir::validation) use lexical_body;
macro_rules! current_join {
    ($validator:ident, $result:ident, $value:expr) => {{
        #[cfg(test)]
        lexical_exit::observe_witness_frontier("join", lexical_exit::lexical_join::LEXICAL_JOIN);
        scope_checkpoint::charge::reserve(Some(lexical_exit::lexical_join::LEXICAL_JOIN))?;
        #[cfg(test)]
        lexical_exit::observe_witness_role("join");
        lexical_exit::lexical_join::lexical_body!($validator, $result, $value);
    }};
}
pub(in crate::hir::validation) use current_join;
macro_rules! join_validated_arm {
    ($validator:ident, $result:ident, $value:expr) => {{
        #[cfg(test)]
        if lexical_exit::lexical_join::OLD_JOIN.with(|flag| flag.get()) {
            lexical_exit::observe_witness_frontier("join", lexical_exit::WITNESS_JOIN);
            scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_JOIN))?;
            lexical_exit::observe_witness_role("join");
            $validator.join_lexical_match_witness(&mut $result, &$value)?;
        } else {
            lexical_exit::lexical_join::current_join!($validator, $result, $value);
        }
        #[cfg(not(test))]
        lexical_exit::lexical_join::current_join!($validator, $result, $value);
    }};
}
pub(in crate::hir::validation) use join_validated_arm;
#[cfg(test)]
thread_local! { pub(in crate::hir::validation) static OLD_JOIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
#[path = "../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
#[cfg(test)]
pub(in crate::hir::validation) fn before_source(source: &str, supplied: &str) -> Option<String> {
    if !matches!(
        source,
        "src/hir/validation.rs"
            | "src/hir/validation/lexical_exit.rs"
            | "src/hir/validation/frame_reuse/source_inverse.rs"
    ) {
        return None;
    }
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let metadata = include_str!("lexical_join_inverse.json");
    let metadata_path = root.join("src/hir/validation/lexical_exit/lexical_join_inverse.json");
    let metadata_sha = hash(metadata.as_bytes());
    assert_eq!(
        reader::read(&metadata_path, metadata.len(), &metadata_sha, 2 << 20).unwrap(),
        metadata
    );
    let data: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let child = include_str!("lexical_join.rs");
    let child_path = root.join("src/hir/validation/lexical_exit/lexical_join.rs");
    let child_sha = data["new_child"]["sha256"].as_str().unwrap();
    assert_eq!(
        child.len() as u64,
        data["new_child"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        reader::read(&child_path, child.len(), child_sha, 2 << 20).unwrap(),
        child
    );
    let row = data["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
        .unwrap();
    let compiled = match source {
        "src/hir/validation.rs" => include_str!("../../validation.rs"),
        "src/hir/validation/lexical_exit.rs" => include_str!("../lexical_exit.rs"),
        "src/hir/validation/frame_reuse/source_inverse.rs" => {
            include_str!("../frame_reuse/source_inverse.rs")
        }
        _ => unreachable!(),
    };
    let length = usize::try_from(row["current"]["length"].as_u64().unwrap()).unwrap();
    let sha = row["current"]["sha256"].as_str().unwrap();
    let actual = reader::read(&root.join(source), length, sha, 2 << 20).unwrap();
    assert_eq!(actual, compiled);
    let before = row["before_text"].as_str().unwrap();
    assert_eq!(
        before.len() as u64,
        row["before"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        hash(before.as_bytes()),
        row["before"]["sha256"].as_str().unwrap()
    );
    assert!(
        supplied == compiled || supplied == before,
        "exact immediate lexical-join source only"
    );
    assert_eq!(
        reader::read(&root.join(source), length, sha, 2 << 20).unwrap(),
        actual
    );
    assert_eq!(
        reader::read(&metadata_path, metadata.len(), &metadata_sha, 2 << 20).unwrap(),
        metadata
    );
    assert_eq!(
        reader::read(&child_path, child.len(), child_sha, 2 << 20).unwrap(),
        child
    );
    Some(before.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mode(bool);
    impl Mode {
        fn set(old: bool) -> Self {
            Self(OLD_JOIN.with(|flag| flag.replace(old)))
        }
    }
    impl Drop for Mode {
        fn drop(&mut self) {
            OLD_JOIN.with(|flag| flag.set(self.0));
        }
    }
    fn current<'v>(
        validator: &HirValidator<'_>,
        result: &mut Option<&'v ResolvedExpr>,
        value: &'v ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let mut selected = *result;
        lexical_body!(validator, selected, *value);
        *result = selected;
        Ok(())
    }
    #[test]
    fn lexical_join_keeps_every_old_selection_and_complete_diagnostic() {
        let source = "module join; @id(\"j.main\") fn main()->i64 {1}";
        let checked = crate::check(source, "join.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let validator = HirValidator::new(&program).unwrap();
        let base = &program.functions[0].body;
        for types in [
            vec![ResolvedType::Never],
            vec![ResolvedType::Never, ResolvedType::Never],
            vec![ResolvedType::Never, ResolvedType::I64, ResolvedType::Never],
            vec![ResolvedType::I64, ResolvedType::Never, ResolvedType::I64],
            vec![ResolvedType::I64, ResolvedType::Bool],
        ] {
            for mismatch in [false, true] {
                let values: Vec<_> = types
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, ty)| {
                        let mut value = base.clone();
                        value.ty = ty;
                        if mismatch && i > 0 {
                            value.ownership = OwnershipMode::Own;
                        }
                        value
                    })
                    .collect();
                let (mut old, mut new) = (None, None);
                for value in &values {
                    let a = validator.join_lexical_match_witness(&mut old, value);
                    let b = current(&validator, &mut new, value);
                    assert_eq!(a, b);
                    assert_eq!(old.map(std::ptr::from_ref), new.map(std::ptr::from_ref));
                    if a.is_err() {
                        break;
                    }
                }
            }
        }
        assert_eq!(
            LEXICAL_JOIN
                + 2 * std::mem::size_of::<WitnessJoinCall<'static, 'static>>()
                + 2 * std::mem::size_of::<Result<(), Diagnostic>>(),
            WITNESS_JOIN + 2 * std::mem::size_of::<&ResolvedExpr>()
        );
        assert!(LEXICAL_JOIN < WITNESS_JOIN);
    }
    #[test]
    fn complete_current_and_old_join_callers_keep_every_check_and_exact_bank_delta() {
        for source in super::super::witness_tests::sources() {
            let checked = crate::check(&source, "lexical-join-caller.spx").unwrap();
            let old = {
                let _mode = Mode::set(true);
                super::super::witness_tests::run_public(&checked, &source, 64 << 20, 0)
            };
            let new = {
                let _mode = Mode::set(false);
                super::super::witness_tests::run_public(&checked, &source, 64 << 20, 0)
            };
            assert!(old.0 && new.0 && !old.2 && !new.2 && old.4.is_none() && new.4.is_none());
            for metrics in [&old.1, &new.1] {
                assert!(
                    !metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed
                );
            }
            assert_eq!(old.5.joins, new.5.joins);
            assert_eq!(
                (
                    old.5.size,
                    old.5.alignment,
                    old.5.requested_capacity,
                    old.5.acquired_capacity
                ),
                (
                    new.5.size,
                    new.5.alignment,
                    new.5.requested_capacity,
                    new.5.acquired_capacity
                )
            );
            let removed = new
                .5
                .joins
                .checked_mul(WITNESS_JOIN - LEXICAL_JOIN)
                .unwrap();
            assert_eq!(old.3.checked_sub(new.3), Some(removed));
            if new.5.joins > 0 {
                assert!(new.3 < old.3);
            } else {
                assert_eq!(new.3, old.3);
            }
        }
    }
    #[test]
    fn first_lexical_join_keeps_exact_short_floor_zero_sticky_and_drop() {
        let source = "module cut; @id(\"c.main\") fn main()->i64 {match 0 {_=>1,}}";
        let checked = crate::check(source, "lexical-join-cuts.spx").unwrap();
        for old in [true, false] {
            let _mode = Mode::set(old);
            let full = super::super::witness_tests::run_public(&checked, source, 64 << 20, 0);
            assert!(full.0 && !full.2);
            let frontier = full.5.first_join.unwrap();
            let exact = (64usize << 20)
                .checked_sub(frontier.remaining)
                .unwrap()
                .checked_add(frontier.amount)
                .unwrap();
            for (limit, floor, acquired) in [
                (exact, 0, true),
                (exact - 1, 0, false),
                (exact, 1, false),
                (0, 0, false),
            ] {
                let run = super::super::witness_tests::run_public(&checked, source, limit, floor);
                assert!(!run.1.invalid && run.1.live == 0 && run.1.allocated == run.1.freed);
                assert_eq!(run.5.joins != 0, acquired);
                if !acquired {
                    assert!(!run.0 && run.2 && run.4.is_some());
                }
            }
        }
    }
    #[test]
    fn whole_immediate_sources_and_new_child_are_physically_authenticated() {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("lexical_join_inverse.json")).unwrap();
        for row in data["rows"].as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let current = reader::read(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source),
                usize::try_from(row["current"]["length"].as_u64().unwrap()).unwrap(),
                row["current"]["sha256"].as_str().unwrap(),
                2 << 20,
            )
            .unwrap();
            assert_eq!(
                before_source(source, &current).unwrap(),
                row["before_text"].as_str().unwrap()
            );
            let mut drift = current.into_bytes();
            drift[0] ^= 1;
            assert!(std::panic::catch_unwind(|| before_source(
                source,
                std::str::from_utf8(&drift).unwrap()
            ))
            .is_err());
        }
        let compiled = include_str!("lexical_join.rs");
        use sha2::{Digest, Sha256};
        let digest = format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(compiled.as_bytes()))
        );
        assert_eq!(digest, data["new_child"]["sha256"].as_str().unwrap());
        assert_eq!(
            reader::read(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/hir/validation/lexical_exit/lexical_join.rs"),
                compiled.len(),
                &digest,
                2 << 20
            )
            .unwrap(),
            compiled
        );
    }
}
