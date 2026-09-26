//! Issue #297: declared `.spx` session protocols -- parser/formatter
//! round-trip, `SPX-K1xx` diagnostics, erasure from both backends, and the
//! field-for-field drift gate between the two canonical declarations and the
//! kernel specs the two real runtime subsystems execute.

use crate::session_protocol::protocols::{
    database_transaction_protocol, project_agent_session_protocol,
};
use crate::session_protocol::source::drift_from_spec;

pub(crate) const DECLARED: &str = include_str!("fixtures/declared.spx");
const PROJECT_AGENT_SESSION: &str = include_str!("fixtures/project_agent_session.spx");
const DATABASE_TRANSACTION: &str = include_str!("fixtures/database_transaction.spx");

fn codes(source: &str) -> Vec<&'static str> {
    match crate::parse(source, "session.spx") {
        Err(diagnostic) => vec![diagnostic.code],
        Ok(program) => crate::verify::verify(&program)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

fn messages(source: &str) -> String {
    let program = crate::parse(source, "session.spx").unwrap();
    crate::verify::verify(&program)
        .into_iter()
        .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn all_three_fixtures_parse_verify_and_carry_one_declaration() {
    for source in [DECLARED, PROJECT_AGENT_SESSION, DATABASE_TRANSACTION] {
        let program = crate::check(source, "session.spx").unwrap();
        assert_eq!(program.session_protocols.len(), 1);
    }
}

#[test]
fn canonical_projection_is_a_fixed_point_and_preserves_the_declaration() {
    for source in [DECLARED, PROJECT_AGENT_SESSION, DATABASE_TRANSACTION] {
        let program = crate::parse(source, "session.spx").unwrap();
        let canonical = crate::format::canonical(&program);
        let reparsed = crate::parse(&canonical, "session.spx").unwrap();
        assert_eq!(crate::format::canonical(&reparsed), canonical);
        let strip = |program: &crate::ast::Program| {
            crate::session_protocol::source::declarations_json(&program.session_protocols)
                .split("\"span\":")
                .map(|part| {
                    part.split_once('}')
                        .map_or(part, |(_, rest)| rest)
                        .to_owned()
                })
                .collect::<String>()
        };
        assert_eq!(strip(&program), strip(&reparsed));
    }
}

#[test]
fn canonical_text_of_the_declared_fixture_is_exact() {
    let program = crate::parse(DECLARED, "session.spx").unwrap();
    let canonical = crate::format::canonical(&program);
    let expected = "@id(\"fixture.session.transaction\")
session protocol \"fixture-transaction-v1\" {
    states { Idle, Open, Committed, Failed }
    initial Idle;
    terminal Committed cleanup { release_snapshot }
    terminal Failed cleanup { discard_snapshot }
    on Idle begin: send BeginRequest via \"fixture.session.begin\" -> Open;
    on Idle misuse: fail Unit -> Failed;
    on Open commit: send CommitRequest requires capability db.write via \"fixture.session.commit\" -> choice { committed: Committed, refused: Failed };
    on Open lost: fail Unit -> Failed;
}
";
    assert!(canonical.contains(expected), "{canonical}");
    // A non-canonical spelling (extra whitespace, trailing commas) projects
    // to the same bytes.
    let loose = DECLARED
        .replace(
            "states { Idle, Open, Committed, Failed }",
            "states {Idle,Open,Committed,Failed,}",
        )
        .replace(
            "cleanup { release_snapshot }",
            "cleanup {  release_snapshot , }",
        );
    let loose = crate::parse(&loose, "session.spx").unwrap();
    assert_eq!(crate::format::canonical(&loose), canonical);
}

#[test]
fn comments_around_a_declaration_survive_the_commented_projection() {
    let source = DECLARED.replace(
        "@id(\"fixture.session.transaction\")",
        "// leading protocol comment\n@id(\"fixture.session.transaction\")",
    );
    let (_, canonical) = crate::parse_canonical(&source, "session.spx").unwrap();
    assert!(
        canonical.contains("// leading protocol comment\n@id(\"fixture.session.transaction\")"),
        "{canonical}"
    );
}

#[test]
fn canonical_declarations_match_the_kernel_specs_field_for_field() {
    for (source, spec) in [
        (PROJECT_AGENT_SESSION, project_agent_session_protocol()),
        (DATABASE_TRANSACTION, database_transaction_protocol()),
    ] {
        let program = crate::check(source, "session.spx").unwrap();
        let drift = drift_from_spec(&program.session_protocols[0], &spec);
        assert!(drift.is_empty(), "{}: {drift:?}", spec.name);
    }
}

#[test]
fn drift_gate_refuses_a_mutated_declaration_and_a_mutated_kernel_spec() {
    // Mutating the declaration: one continuation target.
    let mutated = PROJECT_AGENT_SESSION.replace(
        "consumes resource -> Applying;",
        "consumes resource -> Open;",
    );
    assert_ne!(mutated, PROJECT_AGENT_SESSION);
    let program = crate::parse(&mutated, "session.spx").unwrap();
    let drift = drift_from_spec(
        &program.session_protocols[0],
        &project_agent_session_protocol(),
    );
    assert!(
        drift.iter().any(|entry| entry.contains("`Prepared.apply`")),
        "{drift:?}"
    );
    // Mutating the declaration: dropping resource consumption.
    let mutated = PROJECT_AGENT_SESSION.replace(" consumes resource", "");
    let program = crate::parse(&mutated, "session.spx").unwrap();
    assert!(!drift_from_spec(
        &program.session_protocols[0],
        &project_agent_session_protocol()
    )
    .is_empty());
    // Mutating the kernel spec: cleanup order is canonical, so a reorder is drift.
    let program = crate::check(DATABASE_TRANSACTION, "session.spx").unwrap();
    let mut spec = database_transaction_protocol();
    spec.cleanup.reverse();
    let drift = drift_from_spec(&program.session_protocols[0], &spec);
    assert!(
        drift.iter().any(|entry| entry.starts_with("cleanup")),
        "{drift:?}"
    );
    let mut spec = database_transaction_protocol();
    spec.transitions[2].required_capability = Some("db.commit");
    assert!(!drift_from_spec(&program.session_protocols[0], &spec).is_empty());
}

#[test]
fn k101_repeated_state_and_duplicate_identity_are_refused() {
    let repeated = DECLARED.replace(
        "states { Idle, Open, Committed, Failed }",
        "states { Idle, Open, Committed, Failed, Open }",
    );
    assert_eq!(codes(&repeated), vec!["SPX-K101"]);
    let colliding = DECLARED.replace(
        "@id(\"fixture.session.transaction\")",
        "@id(\"fixture.session.main\")",
    );
    assert_eq!(codes(&colliding), vec!["SPX-K101"]);
}

#[test]
fn k102_kernel_validation_refusals_name_the_source_state() {
    let no_escape = DECLARED.replace("    on Open lost: fail Unit -> Failed;\n", "");
    assert_eq!(codes(&no_escape), vec!["SPX-K102"]);
    assert!(
        messages(&no_escape)
            .contains("non-terminal state `Open` has no `cancel`, `timeout`, or `fail` escape"),
        "{}",
        messages(&no_escape)
    );
    let unknown = DECLARED.replace(
        "-> choice { committed: Committed, refused: Failed }",
        "-> Nowhere",
    );
    assert!(messages(&unknown).contains("undeclared state `Nowhere`"));
    let missing_cleanup =
        DECLARED.replace("    terminal Failed cleanup { discard_snapshot }\n", "");
    // `Failed` is then neither terminal nor escaping: the kernel reports it.
    assert!(codes(&missing_cleanup)
        .iter()
        .all(|code| *code == "SPX-K102"));
    assert!(!codes(&missing_cleanup).is_empty());
}

#[test]
fn k103_bounded_reachability_catches_an_orphan_the_static_check_admits() {
    let orphan = DECLARED
        .replace(
            "states { Idle, Open, Committed, Failed }",
            "states { Idle, Open, Committed, Failed, Orphan }",
        )
        .replace(
            "    on Open lost: fail Unit -> Failed;\n",
            "    on Open lost: fail Unit -> Failed;\n    on Orphan drop: fail Unit -> Failed;\n",
        );
    assert_eq!(codes(&orphan), vec!["SPX-K103"]);
    assert!(messages(&orphan).contains("state `Orphan` is unreachable from `Idle`"));
}

#[test]
fn k104_via_must_name_an_ordinary_function_of_this_module() {
    let missing = DECLARED.replace(
        "via \"fixture.session.begin\"",
        "via \"fixture.session.absent\"",
    );
    assert_eq!(codes(&missing), vec!["SPX-K104"]);
}

#[test]
fn k105_ordering_metadata_cannot_mint_a_capability_its_via_does_not_declare() {
    // `begin` declares no effects; naming `db.write` on its transition is
    // refused instead of granting it.
    let minted = DECLARED.replace(
        "on Idle begin: send BeginRequest via",
        "on Idle begin: send BeginRequest requires capability db.write via",
    );
    assert_eq!(codes(&minted), vec!["SPX-K105"]);
    // The positive control: `commit` declares `db.write`, so its transition
    // is admitted (the fixture itself verifies cleanly).
    assert!(codes(DECLARED).is_empty());
}

#[test]
fn a_protocol_capability_never_satisfies_the_ordinary_effect_check() {
    // `main` calls `commit`, whose transition names `db.write`. Removing
    // `main`'s own `uses` must still be the ordinary SPX-E102 refusal: the
    // protocol's ordering metadata grants `main` nothing.
    let unauthorized = DECLARED.replacen(
        "fn main() -> i64\n    uses { db.write }\n",
        "fn main() -> i64\n",
        1,
    );
    assert_ne!(unauthorized, DECLARED);
    assert!(
        codes(&unauthorized).contains(&"SPX-E102"),
        "{:?}",
        codes(&unauthorized)
    );
}

#[test]
fn k106_declaration_capacity_is_enforced_at_parse_time() {
    let states = (0..65)
        .map(|index| format!("S{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let oversized = DECLARED.replace(
        "states { Idle, Open, Committed, Failed }",
        &format!("states {{ {states} }}"),
    );
    assert_eq!(codes(&oversized), vec!["SPX-K106"]);
}

#[test]
fn malformed_declaration_syntax_is_a_parser_error() {
    let bad_kind = DECLARED.replace("on Idle misuse: fail Unit", "on Idle misuse: explode Unit");
    assert_eq!(codes(&bad_kind), vec!["SPX-P105"]);
    let unnamed = DECLARED.replace(
        "session protocol \"fixture-transaction-v1\"",
        "session protocol Fixture",
    );
    assert_eq!(codes(&unnamed), vec!["SPX-P105"]);
}

fn without_declaration(source: &str) -> String {
    let start = source.find("@id(\"fixture.session.transaction\")").unwrap();
    source[..start].to_owned()
}

#[test]
fn the_declaration_is_erased_from_native_and_wasm_output() {
    let with = crate::check(DECLARED, "session.spx").unwrap();
    let without_source = without_declaration(DECLARED);
    let without = crate::check(&without_source, "session.spx").unwrap();
    assert!(without.session_protocols.is_empty());
    assert_eq!(
        crate::codegen::emit_c(&with).unwrap(),
        crate::codegen::emit_c(&without).unwrap()
    );
    assert_eq!(
        crate::wasm::emit_module(&with).unwrap(),
        crate::wasm::emit_module(&without).unwrap()
    );
}

#[test]
fn hir_binding_refuses_a_via_the_checked_hir_does_not_retain() {
    let program = crate::check(DECLARED, "session.spx").unwrap();
    let mut resolved = crate::hir::resolve(&program).unwrap();
    crate::session_protocol::source::bind_to_hir(&program, &resolved).unwrap();
    resolved
        .functions
        .retain(|function| function.id.as_str() != "fixture.session.begin");
    let error = crate::session_protocol::source::bind_to_hir(&program, &resolved).unwrap_err();
    assert_eq!(error.code, "SPX-K104");
}

#[test]
fn source_program_carrying_a_declaration_roundtrips_through_the_cache_codec() {
    let program = crate::parse(DECLARED, "session.spx").unwrap();
    let bytes = crate::cache_codec::encode(&program).unwrap();
    let restored: crate::ast::Program = crate::cache_codec::decode(&bytes).unwrap();
    assert_eq!(restored.session_protocols, program.session_protocols);
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), bytes);
}

#[test]
fn comments_inside_the_declaration_body_hoist_above_it_deterministically() {
    // A declaration is one comment-placement leaf, like a static `protocol`:
    // a comment anywhere inside its body is printed, in source order, above
    // its `@id`. The result is a fixed point.
    let source = DECLARED
        .replace(
            "    initial Idle;\n",
            "    initial Idle; // after initial\n",
        )
        .replace(
            "    on Idle misuse: fail Unit -> Failed;\n",
            "    // before misuse\n    on Idle misuse: fail Unit -> Failed;\n",
        );
    let (_, canonical) = crate::parse_canonical(&source, "session.spx").unwrap();
    assert!(
        canonical.contains(
            "// after initial\n// before misuse\n@id(\"fixture.session.transaction\")\nsession protocol"
        ),
        "{canonical}"
    );
    let (_, again) = crate::parse_canonical(&canonical, "session.spx").unwrap();
    assert_eq!(again, canonical);
}

#[test]
fn lowering_capacity_is_a_k106_diagnostic_not_a_panic() {
    // The parser's own bounds keep a parsed declaration inside the lowering
    // pool; a directly constructed one past it must still fail closed.
    let mut program = crate::parse(DECLARED, "session.spx").unwrap();
    let template = program.session_protocols[0].states[0].clone();
    let declaration = &mut program.session_protocols[0];
    for index in 0..40_000 {
        let mut state = template.clone();
        state.name = format!("Extra{index}");
        declaration.states.push(state);
    }
    let diagnostics = crate::session_protocol::source::check(&program);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        vec!["SPX-K106"]
    );
}
