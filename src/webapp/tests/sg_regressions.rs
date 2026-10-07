use super::*;

#[test]
fn sg_projection_names_fail_before_output() {
    for (name, source) in [
        ("route", "record WorkItem { name: string, } record Work_item { count: i64, } fn main() -> i64 { 0 }"),
        ("computed-id", "record Thing { value: i64, } fn thing_id(value: i64) -> i64 { value + 100 }"),
        ("computed-password", "record Thing { value: i64, } fn thing_password(value: i64) -> i64 { value }"),
        ("computed-alias", "record WorkItem { value: i64, } fn work_item_total(value: i64) -> i64 { value } fn workitem_total(value: i64) -> i64 { value }"),
    ] {
        let path = write_temp(name, &format!("module m; {source}"));
        let errors = generate(&path).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-WA102"), "{errors:?}");
    }
}

#[test]
fn sg_generated_http_runtime_regressions() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node unavailable; SG webapp HTTP regressions NOT RUN");
        return;
    }
    let path = write_temp(
        "sg-runtime",
        r#"module m;
variant State { Draft, Approved, Cancelled, }
record User { login: string, active: bool, admin: bool, }
record Item { user_id: i64, state: State, text: string, }
record Number { value: i64, }
record Keyed { numerator: i64, denominator: i64, }
record Proto { __proto__: string, constructor: string, toString: string, }
fn user_account(login: string, active: bool) -> bool { active }
fn user_can_write() -> bool { true }
fn item_can_write(user_id: i64, me: i64, my_admin: bool) -> bool { my_admin || user_id == me }
fn item_state_step(from: State, to: State) -> bool { from == State::Draft {} }
fn number_checked(value: i64) -> i64
    requires value >= 0
    ensures positive(result)
    ensures result == value
{ checked_helper(value) }
fn number_fault(value: i64) -> i64
    requires value >= 0
    ensures result > 0
{ value / 0 }
fn positive(value: i64) -> bool { value > 0 }
fn checked_helper(value: i64) -> i64
    ensures result == value
{ value }
fn keyed_key(numerator: i64, denominator: i64) -> i64 { numerator / denominator }
"#,
    );
    let projection = generate(&path).unwrap();
    let out = path.with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let run = std::process::Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webapp/tests/runtime_regressions.mjs"))
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(out).unwrap();
}
