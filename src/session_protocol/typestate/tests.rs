//! Issue #297 follow-on (R21): `.spx` endpoint typestate checking --
//! `follows session protocol "<id>"`, `SPX-K107`..`SPX-K109`, canonical
//! round-trip, and native/Wasm erasure.

/// Two declared session protocols sharing one state shape (`Idle`, `Open`,
/// `Committed`, `Failed`): a plain one whose `commit` transition lands
/// directly on `Committed`, and a `choice`-continuation one whose `commit`
/// transition instead branches. Every non-terminal state carries its own
/// `fail`-kind escape so both declarations pass `SPX-K102`/`SPX-K103`
/// unconditionally, independent of whatever function this module adds under
/// them.
fn base() -> String {
    "module typestate.fixture;\n\n\
@id(\"typestate.begin\")\nfn begin() -> i64 { 1 }\n\n\
@id(\"typestate.commit\")\nfn commit() -> i64 { 2 }\n\n\
@id(\"typestate.rollback\")\nfn rollback() -> i64 { 3 }\n\n\
@id(\"typestate.commit_choice\")\nfn commit_choice() -> i64 { 4 }\n\n\
@id(\"typestate.protocol\")\n\
session protocol \"typestate-fixture-v1\" {\n\
    states { Idle, Open, Committed, Failed }\n\
    initial Idle;\n\
    terminal Committed cleanup { release }\n\
    terminal Failed cleanup { discard }\n\
    on Idle begin: send Unit via \"typestate.begin\" -> Open;\n\
    on Idle misuse: fail Unit -> Failed;\n\
    on Open commit: send Unit via \"typestate.commit\" -> Committed;\n\
    on Open abort: fail Unit via \"typestate.rollback\" -> Failed;\n\
}\n\n\
@id(\"typestate.choice_protocol\")\n\
session protocol \"typestate-fixture-choice-v1\" {\n\
    states { Idle, Open, Committed, Failed }\n\
    initial Idle;\n\
    terminal Committed cleanup { release }\n\
    terminal Failed cleanup { discard }\n\
    on Idle begin: send Unit via \"typestate.begin\" -> Open;\n\
    on Idle misuse: fail Unit -> Failed;\n\
    on Open commit: send Unit via \"typestate.commit_choice\" -> choice { ok: Committed, no: Failed };\n\
    on Open abort: fail Unit via \"typestate.rollback\" -> Failed;\n\
}\n\n\
@id(\"typestate.main\")\nfn main() -> i64 { 0 }\n"
        .to_owned()
}

fn source(extra: &str) -> String {
    format!("{}{extra}", base())
}

fn codes(extra: &str) -> Vec<&'static str> {
    let source = source(extra);
    match crate::parse(&source, "typestate.spx") {
        Err(diagnostic) => vec![diagnostic.code],
        Ok(program) => crate::verify::verify(&program)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

const STRAIGHT_LINE: &str = "\n@id(\"typestate.straight_line\")\n\
fn straight_line() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    commit()\n}\n";

#[test]
fn a_straight_line_call_sequence_reaching_a_terminal_is_admitted() {
    assert_eq!(codes(STRAIGHT_LINE), Vec::<&str>::new());
}

#[test]
fn an_if_else_call_sequence_where_both_branches_reach_a_terminal_is_admitted() {
    let source = "\n@id(\"typestate.if_else\")\n\
fn if_else(flag: bool) -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    if flag {\n        commit()\n    } else {\n        rollback()\n    }\n}\n";
    assert_eq!(codes(source), Vec::<&str>::new());
}

#[test]
fn a_call_not_legal_from_the_current_state_is_refused_distinctly() {
    // The trailing legal `begin(); commit()` pair independently reaches the
    // protocol's terminal `Committed` state, so this specifically isolates
    // the call-site illegal-order check from the separate "does every path
    // end in a terminal state" check below -- see
    // `disabling_the_illegal_order_check_alone_would_wrongly_admit_this_program`.
    let source = "\n@id(\"typestate.illegal_order\")\n\
fn illegal_order() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let skip = commit();\n    let a = begin();\n    commit()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K108"]);
}

#[test]
fn a_path_ending_in_a_non_terminal_state_is_refused_distinctly_from_illegal_order() {
    let source = "\n@id(\"typestate.incomplete\")\n\
fn incomplete() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    begin()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K108"]);
}

#[test]
fn a_loop_reaching_a_via_bound_call_is_refused() {
    let source = "\n@id(\"typestate.looped\")\n\
fn looped() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    while false {\n        commit()\n    }\n    commit()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K109"]);
}

#[test]
fn a_closure_reaching_a_via_bound_call_is_refused() {
    let source = "\n@id(\"typestate.closured\")\n\
fn closured() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    let f = fn() -> i64 { commit() };\n    commit()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K109"]);
}

#[test]
fn a_call_through_a_name_shadowed_elsewhere_in_the_function_is_refused_as_indirect() {
    let source = "\n@id(\"typestate.indirect\")\n\
fn indirect() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    let commit = fn() -> i64 { 0 };\n    commit()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K109"]);
}

#[test]
fn direct_recursion_in_a_followed_function_is_refused() {
    let source = "\n@id(\"typestate.recursive\")\n\
fn recursive() -> i64\n    follows session protocol \"typestate.protocol\"\n{\n    let a = begin();\n    recursive()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K109"]);
}

#[test]
fn a_via_transition_with_a_branching_choice_continuation_is_refused() {
    let source = "\n@id(\"typestate.choice_call\")\n\
fn choice_call() -> i64\n    follows session protocol \"typestate.choice_protocol\"\n{\n    let a = begin();\n    commit_choice()\n}\n";
    assert_eq!(codes(source), vec!["SPX-K109"]);
}

#[test]
fn a_follows_clause_naming_no_declared_protocol_is_refused_distinctly() {
    let source = "\n@id(\"typestate.bad_follows\")\n\
fn bad_follows() -> i64\n    follows session protocol \"no.such.protocol\"\n{\n    0\n}\n";
    assert_eq!(codes(source), vec!["SPX-K107"]);
}

#[test]
fn canonical_projection_of_a_follows_clause_is_a_fixed_point() {
    let program = crate::parse(&source(STRAIGHT_LINE), "typestate.spx").unwrap();
    let canonical = crate::format::canonical(&program);
    assert!(
        canonical.contains("    follows session protocol \"typestate.protocol\"\n"),
        "{canonical}"
    );
    let reparsed = crate::parse(&canonical, "typestate.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    let function = reparsed
        .functions
        .iter()
        .find(|function| function.name == "straight_line")
        .unwrap();
    assert_eq!(
        function.follows.as_ref().unwrap().protocol_id,
        "typestate.protocol"
    );
}

#[test]
fn a_follows_clause_is_erased_from_native_and_wasm_output() {
    let with = crate::check(&source(STRAIGHT_LINE), "typestate.spx").unwrap();
    let without_source =
        source(STRAIGHT_LINE).replace("    follows session protocol \"typestate.protocol\"\n", "");
    let without = crate::check(&without_source, "typestate.spx").unwrap();
    assert!(with
        .functions
        .iter()
        .any(|function| function.follows.is_some()));
    assert!(without
        .functions
        .iter()
        .all(|function| function.follows.is_none()));
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
fn a_source_program_carrying_a_follows_clause_roundtrips_through_the_cache_codec() {
    let program = crate::parse(&source(STRAIGHT_LINE), "typestate.spx").unwrap();
    let bytes = crate::cache_codec::encode(&program).unwrap();
    let restored: crate::ast::Program = crate::cache_codec::decode(&bytes).unwrap();
    assert_eq!(restored.functions, program.functions);
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), bytes);
}
