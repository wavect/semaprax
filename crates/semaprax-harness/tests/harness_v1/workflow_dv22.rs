//! DV-22: host acceptance is judged inside the compiler-bound declaration of
//! the checked candidate, never in unlisted files, comments, strings,
//! imports or neighboring declarations. Fixture prefix `hp-hp04`.

use super::*;

fn lib(name: &str, extra: &str) -> String {
    format!("module t.lib;\n@id(\"t.f\")\nfn {name}(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses {{ clock.read }}\n{{\n    x\n}}\n{extra}")
}

fn task(session: bool) -> Task {
    let mut t = Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f to f_cost".into(),
        seed: Some("t.f".into()),
        acceptance: vec![json!({"stable_id": "t.f", "contains": "f_cost"})],
        ..Task::default()
    };
    if session {
        t.session = Some(SessionBounds {
            max_attempts: 1,
            ..Default::default()
        });
    }
    t
}

fn prop(src: &str) -> Vec<u8> {
    json!({"schema": "semaprax.harness-proposal.v1",
        "intent": {"kind": "replace_function_body", "target": "t.f", "fake_source": src}})
    .to_string()
    .into_bytes()
}

fn run_one(extra_files: &[(&str, &str)], candidate: &str, session: bool) -> Report {
    let e = setup(&lib("f", ""));
    for (p, c) in extra_files {
        write(&e.project, p, c);
    }
    let fake = Fake::new(candidate);
    go(
        &config(&e, task(session), None),
        &fake,
        None,
        prop(candidate),
    )
}

const DECOY: &str = "// Draft documentation outside manifest sources\n// @id(\"t.f\")\n// fn f_cost(x): draft\nnot valid\n";

#[test]
fn dv22_wrong_rename_is_rejected_even_with_an_unlisted_decoy() {
    for decoys in [
        vec![],
        vec![("docs/example.spx", DECOY)],
        vec![("zzz/example.spx", DECOY), ("aaa/example.spx", DECOY)],
    ] {
        for session in [false, true] {
            let r = run_one(&decoys, &lib("f_subtotal", ""), session);
            assert_ne!(r.status, "candidate-ready", "{session} {decoys:?}");
        }
    }
}

#[test]
fn dv22_real_rename_is_accepted_in_both_paths() {
    for session in [false, true] {
        let r = run_one(&[("docs/example.spx", DECOY)], &lib("f_cost", ""), session);
        assert_eq!(r.status, "candidate-ready", "{session} {:?}", r.refusals);
        if !session {
            assert_eq!(r.checks["acceptance_verified"], 1);
        }
    }
}

#[test]
fn dv22_comment_string_import_and_neighbor_cannot_stand_in() {
    let wrong = lib("f_subtotal", "");
    let cases = [
        format!("// f_cost\n{wrong}"),
        wrong.replace("{\n    x\n}", "{\n    // f_cost\n    x\n}"),
        format!("{wrong}\n@id(\"t.g\")\nfn f_cost() -> i64 {{ 1 }}\n"),
        format!("{wrong}\n@id(\"t.g\")\nfn g() -> text {{ \"@id(\\\"t.f\\\") f_cost\" }}\n"),
        format!("{wrong}\n// @id(\"t.f\")\nuse function @id(\"t.f\") from other as f_cost;\n"),
    ];
    for (i, src) in cases.iter().enumerate() {
        let r = run_one(&[], src, false);
        assert_ne!(r.status, "candidate-ready", "case {i}: {src}");
    }
}

#[test]
fn dv22_a_duplicate_declaration_site_fails_closed() {
    let dup = format!(
        "{}\n@id(\"t.f\")\nfn f_cost() -> i64 {{ 1 }}\n",
        lib("f_subtotal", "")
    );
    let r = run_one(&[], &dup, false);
    assert_ne!(r.status, "candidate-ready");
}
