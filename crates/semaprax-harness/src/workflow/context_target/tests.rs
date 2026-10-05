use super::*;
use crate::observe::Tokenizer;
use crate::workflow::budget::{BudgetConfig, ModelTokenizerMap, TokenizerSet};

struct Words;
impl Tokenizer for Words {
    fn name(&self) -> &str {
        "cl100k_base"
    }
    fn fingerprint(&self) -> &str {
        "words-fake"
    }
    fn count(&self, t: &str) -> usize {
        t.split_whitespace().count()
    }
}

fn item(label: &str, prov: &str, text: &str) -> ContextItem {
    ContextItem {
        label: label.into(),
        provenance: prov.into(),
        text: text.into(),
    }
}
fn native(label: &str, text: &str) -> ContextItem {
    item(label, COMPILER_VERIFIED, text)
}
fn ext(label: &str, text: &str) -> ContextItem {
    item(label, "external:inferred", text)
}
fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn lines(a: usize, b: usize) -> String {
    (a..=b)
        .map(|n| format!("line{n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Localized task, large repository: starts at the small target, below the
/// safety bound, and the selected set still carries every protected fact.
#[test]
fn localized_task_starts_below_the_ceiling() {
    let mut items = vec![native("src/a.spx:1-3", "fn rename_target body")];
    for i in 0..50 {
        items.push(ext(
            &format!("other/f{i}.py:1-2"),
            &format!("unrelated filler {i} {}", "x".repeat(200)),
        ));
    }
    items.push(ext("hit.py:1-1", "calls rename_target here"));
    let target = ContextTarget::new(None, 16384, 400, 400, 2);
    let sel = select(
        &items,
        &set(&["rename_target"]),
        &set(&[]),
        &target,
        &CostMeter::bytes(),
    )
    .unwrap();
    assert!(sel.used <= 400);
    assert!(sel.items.iter().any(|i| i.label == "src/a.spx:1-3"));
    assert!(
        sel.items.iter().any(|i| i.label == "hit.py:1-1"),
        "identifier hit ranks first"
    );
    assert!(sel.omitted.len() >= 40);
    // Deterministic.
    let again = select(
        &items,
        &set(&["rename_target"]),
        &set(&[]),
        &target,
        &CostMeter::bytes(),
    )
    .unwrap();
    assert_eq!(sel, again);
    // Report only: audit identities carry reasons.
    let j = sel.report_json();
    assert_eq!(j["exhaustive"], false);
    assert!(j["omitted"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("over target"));
}

/// Required references and constraints are protected even beyond the target;
/// when they cannot fit the safety bound the selection is refused.
#[test]
fn required_references_are_retained_or_refused() {
    let items = vec![
        native("a.spx:1-1", "contract law"),
        ext("b.py:1-1", &format!("old_name here {}", "y".repeat(300))),
        ext("c.py:1-1", "filler"),
    ];
    let t = ContextTarget::new(None, 16384, 20, 20, 1);
    let sel = select(
        &items,
        &set(&[]),
        &set(&["old_name"]),
        &t,
        &CostMeter::bytes(),
    )
    .unwrap();
    assert!(
        sel.items.iter().any(|i| i.label == "b.py:1-1"),
        "required ref kept over target"
    );
    assert!(sel
        .chosen
        .iter()
        .any(|c| c.reason.contains("required reference")));
    let tiny = ContextTarget::new(None, 100, 20, 20, 1);
    let e = select(
        &items,
        &set(&[]),
        &set(&["old_name"]),
        &tiny,
        &CostMeter::bytes(),
    )
    .unwrap_err();
    assert_eq!(e.code, "SPX-HPD020");
    let hard = ContextTarget::new(Some(10), 16384, 20, 20, 1);
    assert!(select(
        &items,
        &set(&[]),
        &set(&["old_name"]),
        &hard,
        &CostMeter::bytes()
    )
    .is_err());
}

#[test]
fn overlapping_same_revision_spans_are_represented_once() {
    let e = |l: &str, t: String, rev: &str| SpanEntry {
        item: native(l, &t),
        revision: rev.into(),
    };
    let (items, map) = dedup_spans(vec![
        e("f.rs:1-5", lines(1, 5), "r1"),
        e("f.rs:3-4", lines(3, 4), "r1"),
        e("f.rs:4-8", lines(4, 8), "r1"),
        e("f.rs:1-5", lines(1, 5), "r1"),
    ]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "f.rs:1-8");
    assert_eq!(items[0].text, lines(1, 8));
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].sources, vec!["f.rs:1-5", "f.rs:3-4", "f.rs:4-8"]);
}

#[test]
fn conflicting_revisions_and_external_text_stay_distinct() {
    let mk = |it: ContextItem, rev: &str| SpanEntry {
        item: it,
        revision: rev.into(),
    };
    let (items, map) = dedup_spans(vec![
        mk(native("f.rs:1-2", &lines(1, 2)), "r1"),
        mk(native("f.rs:1-2", &lines(1, 2)), "r2"),
        mk(ext("f.rs:1-2", &lines(1, 2)), "r1"),
        mk(native("g.rs:1-2", "line1\nDIFF"), "r1"),
        mk(native("g.rs:2-3", "other\nline3"), "r1"),
    ]);
    assert_eq!(items.len(), 5);
    assert_eq!(
        items
            .iter()
            .filter(|i| i.provenance != COMPILER_VERIFIED)
            .count(),
        1
    );
    // Same revision, disagreeing overlap: both kept, conflict recorded.
    let g = map.iter().find(|m| m.label == "g.rs:1-2").unwrap();
    assert_eq!(g.conflicts, vec!["g.rs:2-3"]);
}

#[test]
fn incomplete_retrieval_escalates_bounded_and_justified() {
    let mut t = ContextTarget::new(Some(1000), 16384, 200, 300, 2);
    assert!(t
        .escalate(&Trigger::MissingDependency(String::new()), None)
        .is_err());
    assert_eq!(
        t.escalate(&Trigger::MissingDependency("helper_fn".into()), Some(1000)),
        Ok(500)
    );
    assert_eq!(
        t.escalate(&Trigger::ValidationFailure("SPX-X".into()), None),
        Ok(800)
    );
    assert!(t
        .escalate(&Trigger::MissingDependency("more".into()), None)
        .unwrap_err()
        .contains("bound"));
    assert_eq!(t.escalation_log().len(), 2);
    // Never past the hard capacity; never past the remaining task budget.
    let mut c = ContextTarget::new(Some(250), 16384, 200, 300, 5);
    assert_eq!(
        c.escalate(&Trigger::MissingDependency("a".into()), None),
        Ok(250)
    );
    assert!(c
        .escalate(&Trigger::MissingDependency("b".into()), None)
        .is_err());
    let mut b = ContextTarget::new(None, 16384, 200, 300, 5);
    assert!(b
        .escalate(&Trigger::MissingDependency("a".into()), Some(10))
        .is_err());
    assert_eq!(b.current(), 200);
}

#[test]
fn extra_attempts_can_reject_the_smaller_target() {
    let bytes = CostUnit::Bytes;
    let base = [AttemptCost {
        input: 900,
        output: 100,
    }];
    let small_ok = [
        AttemptCost {
            input: 300,
            output: 100,
        },
        AttemptCost {
            input: 400,
            output: 100,
        },
    ];
    let small_bad = [
        AttemptCost {
            input: 300,
            output: 100,
        },
        AttemptCost {
            input: 450,
            output: 100,
        },
        AttemptCost {
            input: 500,
            output: 100,
        },
    ];
    assert_eq!(
        smaller_target_pays((&bytes, &small_ok), (&bytes, &base)),
        Ok(true)
    );
    assert_eq!(
        smaller_target_pays((&bytes, &small_bad), (&bytes, &base)),
        Ok(false)
    );
    let tok = CostUnit::Tokens {
        tokenizer: "cl100k_base".into(),
    };
    assert!(smaller_target_pays((&tok, &small_ok), (&bytes, &base)).is_err());
}

#[test]
fn named_tokens_and_byte_fallback_stay_distinct() {
    let mut ts = TokenizerSet::default();
    ts.add(Box::new(Words));
    let cfg = BudgetConfig {
        map: ModelTokenizerMap::empty().with("gpt-5", "cl100k_base"),
        tokenizers: ts,
        ..Default::default()
    };
    let b = RequestBudget {
        policy: cfg.policy.clone(),
        map: cfg.map.clone(),
        tokenizers: &cfg.tokenizers,
        cache: &cfg.cache,
    };
    let named = CostMeter::for_model(&b, "gpt-5-x");
    let fallback = CostMeter::for_model(&b, "unmapped-model");
    assert_eq!(
        named.unit(),
        &CostUnit::Tokens {
            tokenizer: "cl100k_base".into()
        }
    );
    assert_eq!(fallback.unit(), &CostUnit::Bytes);
    let it = ext("a.py:1-1", "one two three");
    assert_eq!(named.cost_text("one two three"), 3);
    assert_eq!(fallback.cost_text("one two three"), 13);
    assert_ne!(named.unit().label(), fallback.unit().label());
    assert!(fallback.unit().label().starts_with("bytes:"));
    // Selection reports the unit it used.
    let t = ContextTarget::new(None, 16384, 100, 100, 1);
    let sel = select(&[it], &set(&[]), &set(&[]), &t, &named).unwrap();
    assert_eq!(sel.report_json()["unit"], "tokens:cl100k_base");
}
