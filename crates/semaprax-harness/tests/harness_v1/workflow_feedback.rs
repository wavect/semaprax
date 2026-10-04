//! TC-06 end to end: identical failing-candidate fixtures, the full serialized
//! next request before (raw four-entry history, the old shape) and after the
//! feedback projection. Fixture prefix `hp-tc06`.

use super::*;

fn old_shape(msgs: &[String], upto: usize) -> Vec<Value> {
    (0..upto)
        .map(|i| {
            json!({"attempt": i + 1, "stage": "preview", "code": "SPX-HPD040", "message": msgs[i]})
        })
        .collect()
}

#[test]
fn hp_tc06_next_request_is_smaller_and_carries_the_current_diagnostic_verbatim() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    // Four large, distinct compiler refusals, then an honest proposal.
    let tails: Vec<String> = (1..=4)
        .map(|i| format!("failure-{i} {}", "unexpected token near `x`; ".repeat(400)))
        .collect();
    let msgs: Vec<String> = tails
        .iter()
        .map(|t| format!("compiler refused `project-candidate-preview`: {t}"))
        .collect();
    let mut items: Vec<Value> = tails
        .iter()
        .map(|t| intent("replace_function_body", json!({"fake_refuse": t})))
        .collect();
    items.push(ok_body());
    let seq = Seq::new(items);
    let cfg = session(&e, |t| t.session.as_mut().unwrap().max_attempts = 5);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let prompts = seq.prompts.borrow();
    assert_eq!(prompts.len(), 5);
    let after = &prompts[4];

    // After: the whole serialized request actually sent.
    let after_len = after.to_string().len();
    // Before: the identical request with the old verbatim four-entry history.
    let mut before = after.clone();
    before["feedback"] = json!(old_shape(&msgs, 4));
    let before_len = before.to_string().len();
    assert!(
        after_len * 2 < before_len,
        "after {after_len} vs before {before_len}"
    );

    // The current diagnostic is verbatim in the actual prompt.
    let fb = after["feedback"].as_array().unwrap();
    assert_eq!(fb.last().unwrap()["message"], msgs[3].as_str());
    assert!(after.to_string().contains(&tails[3]));
    assert_eq!(fb.last().unwrap()["code"], "SPX-HPD040");
    // Older failures are compact, not repeated whole.
    assert!(fb[..fb.len() - 1]
        .iter()
        .all(|x| x["message"].as_str().map_or(true, |m| m.len() <= 130)));
    // Accounting-only fields are in the report, not the prompt.
    assert!(r.session["feedback_projection"].as_array().unwrap().len() >= 4);
    assert!(!after.to_string().contains("raw_bytes"));
}
