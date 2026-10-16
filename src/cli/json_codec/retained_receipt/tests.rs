use super::*;
#[test]
fn receipt_selection_is_additive_exact_and_rejects_ambiguous_grammar_before_authority() {
    let args = [
        "missing",
        "--source",
        "src/schema.spx",
        "--type",
        "app.row",
        "--output",
        "new.json",
    ]
    .map(str::to_owned)
    .to_vec();
    assert!(parse(&args).is_ok());
    let report = |_: &[Diagnostic]| panic!("grammar failure must precede Project authority");
    for extra in [
        vec![],
        vec!["--retained-receipt", "--retained-receipt"],
        vec!["--retained-receipt", "--explain"],
        vec!["--retained-receipt", "--plan", "plan.json"],
    ] {
        let mut bad = args.clone();
        bad.extend(extra.into_iter().map(str::to_owned));
        assert_eq!(run(&bad, report), Err(2));
    }
}
