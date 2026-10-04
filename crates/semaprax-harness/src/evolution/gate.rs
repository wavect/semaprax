//! Predeclared quality/cost gate over host-graded held-out scores.

use super::spec::GateCfg;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub passed: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scores {
    pub validation_base: Score,
    pub validation_cand: Score,
    pub test_base: Score,
    pub test_cand: Score,
}

/// Exact-match grading (trimmed); the expected value never leaves the host.
pub fn grade(answer: &str, expected: &str) -> bool {
    answer.trim() == expected.trim()
}

/// `Ok(())` admits; `Err(reasons)` rejects. Validation must be strictly
/// better; the final test split may not regress beyond the declared tolerance.
pub fn decide(s: &Scores, body_bytes: usize, cfg: &GateCfg) -> Result<(), Vec<String>> {
    let mut why = Vec::new();
    if s.validation_cand.passed <= s.validation_base.passed {
        why.push(format!(
            "validation not strictly better ({} <= {})",
            s.validation_cand.passed, s.validation_base.passed
        ));
    }
    if s.test_cand.passed + cfg.max_test_regression < s.test_base.passed {
        why.push(format!(
            "test regressed ({} < {})",
            s.test_cand.passed, s.test_base.passed
        ));
    }
    if body_bytes > cfg.max_skill_bytes {
        why.push(format!(
            "skill is {body_bytes} bytes, over the {} byte cost gate",
            cfg.max_skill_bytes
        ));
    }
    if why.is_empty() {
        Ok(())
    } else {
        Err(why)
    }
}

pub fn to_json(s: &Scores) -> Value {
    let f = |a: Score| json!({"passed": a.passed, "total": a.total});
    json!({
        "validation": {"baseline": f(s.validation_base), "candidate": f(s.validation_cand)},
        "test": {"baseline": f(s.test_base), "candidate": f(s.test_cand)},
    })
}
