//! The displayed program and corrections stay bound to ordinary checked source.
use super::super::{MAX_BYTES, lookup};
use super::CARD;

fn source() -> &'static str {
    CARD.split_once("```spx\n")
        .unwrap()
        .1
        .split_once("```\n")
        .unwrap()
        .0
}
fn refusal(source: &str, code: &str) {
    let diagnostics = semaprax::check(source, "help-contract-refused.spx").unwrap_err();
    assert!(
        diagnostics.iter().any(|error| error.code == code),
        "{code}: {diagnostics:?}"
    );
}

#[test]
fn contract_owner_card_is_bounded_deterministic_and_discoverable() {
    assert_eq!(lookup("author:contract-owners").unwrap(), CARD);
    assert!(CARD.len() <= MAX_BYTES);
    let routes = lookup("author:routes").unwrap();
    assert!(routes.len() <= MAX_BYTES);
    assert_eq!(routes.matches("author:contract-owners").count(), 1);
    let found = lookup("author:find:provisional:0").unwrap();
    assert!(found.len() <= MAX_BYTES);
    assert!(found.contains("author:contract-owners"));
    assert_eq!(found, lookup("author:find:provisional:0").unwrap());
    for boundary in [
        "qualification pending",
        "only ensures",
        "SPX-O105",
        "SPX-O101",
        "SPX-T202",
        "SPX-T201",
        "SPX-T263",
        "SPX-T267",
        "32 allocation-event",
        "2097152-byte",
        "without refunds",
        "physical OOM has no new recovery",
        "publishes no result",
    ] {
        assert!(
            CARD.to_ascii_lowercase()
                .contains(&boundary.to_ascii_lowercase()),
            "{boundary}"
        );
    }
    assert!(lookup("author:contract-owners:0").is_err());
}

#[test]
fn displayed_contract_locals_and_both_result_views_check_roundtrip_and_replay() {
    let checked = semaprax::check(source(), "help-contract-owners.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let reparsed = semaprax::check(&canonical, "help-contract-owners.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let resolved = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    for name in ["help.consume", "help.make", "help.text", "main"] {
        assert!(
            resolved
                .functions
                .iter()
                .any(|function| function.id.as_str() == name)
        );
    }
}

#[test]
fn displayed_corrections_do_not_grant_transfer_freshness_or_early_result_authority() {
    refusal(
        &source().replace("consume(moved)", "consume(moved) && consume(moved)"),
        "SPX-O101",
    );
    refusal(&source().replace("ensures {let text=\"ok\";string_len(text)==2}",
        "ensures {let text=\"ok\";string_len(text)==2} ensures byte_len(bytes_as_slice(local))==2usize"), "SPX-T202");
    refusal(
        &source().replace(
            "requires {let local=bytes_zeroed(2usize);let moved=local;consume(moved)}",
            "requires byte_len(bytes_as_slice(result))==2usize",
        ),
        "SPX-T201",
    );
    refusal(
        &source().replace(
            "ensures byte_len(bytes_as_slice(result))==2usize",
            "ensures consume(result)",
        ),
        "SPX-O105",
    );
    refusal(
        &source().replace(
            "ensures byte_len(bytes_as_slice(result))==2usize",
            "ensures byte_len(str_as_bytes(string_as_str(result)))==2usize",
        ),
        "SPX-T263",
    );
    refusal(
        r#"module help.contract_outer;
@id("take") fn take(input:own Bytes)->i64 requires {let stolen=input;true}{0}
@id("main") fn main()->i64{0}
"#,
        "SPX-O105",
    );
    refusal(&source().replace("let local=bytes_zeroed(2usize)", "let local=factory(2usize)")
        .replace("@id(\"help.consume\")", "@id(\"help.factory\") fn factory(size:usize)->Bytes {bytes_zeroed(2usize)}\n@id(\"help.consume\")"), "SPX-O105");
}
