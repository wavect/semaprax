use super::*;

const SOURCE: &str = r#"module creation.preview;
variant Role { Admin, Agent, Viewer, }
variant State { Draft, Approved, }
record User { login: string, active: bool, role: Role, divisor: i64, }
record Owned { owner: i64, }
record Reverse { owner: i64, }
record Disjoin { owner: i64, }
record RowOnly { score: i64, }
record Conditional { owner: i64, }
record MatchOwn { owner: i64, }
record Guarded { state: State, }
record Local { owner: i64, }
record Textual { label: string, }
record Negated { owner: i64, }
record Trap { score: i64, }
record NotTrap { score: i64, }
record UnknownTrap { score: i64, }
record LiteralMatch { score: i64, }
record BoundMatch { score: i64, }
record Requires { owner: i64, score: i64, }
record RequiresRole { owner: i64, }
record EnsuresRole { owner: i64, }
record EnsuresUnknown { score: i64, }
record DefaultItem { owner: i64, }
record AccountOnly { text: string, }
fn user_account(login: string, active: bool) -> bool { active }
fn user_can_write(my_role: Role) -> bool { my_role == Role::Admin {} }
fn role_allowed(role: Role) -> bool {
    match role { Role::Admin {} => true, Role::Agent {} => true, _ => false, }
}
fn nested(role: Role, owner: i64, me: i64) -> bool { role_allowed(role) && owner == me }
fn can_write(owner: i64, me: i64, my_role: Role) -> bool { nested(my_role, owner, me) }
fn owned_can_write(owner: i64, me: i64, my_role: Role) -> bool { nested(my_role, owner, me) }
fn reverse_can_write(owner: i64, me: i64, my_role: Role) -> bool { owner == me && role_allowed(my_role) }
fn disjoin_can_write(owner: i64, me: i64, my_role: Role) -> bool { owner == me || my_role == Role::Admin {} }
fn row_only_can_write(score: i64) -> bool { score > 0 }
fn conditional_can_write(owner: i64, me: i64, my_role: Role) -> bool {
    if owner == me { my_role == Role::Agent {} } else { my_role == Role::Admin {} }
}
fn match_own_can_write(owner: i64, me: i64, my_role: Role) -> bool {
    match my_role { Role::Admin {} => true, Role::Agent {} => owner == me, _ => false, }
}
fn guarded_can_write(state: State, my_role: Role) -> bool {
    match state { State::Draft {} if role_allowed(my_role) => true, _ => false, }
}
fn local_can_write(owner: i64, me: i64, my_role: Role) -> bool {
    let allowed = role_allowed(my_role);
    let own_row = owner == me;
    allowed && own_row
}
fn textual_can_write(label: string, my_role: Role) -> bool { string_len(label) > 0 && role_allowed(my_role) }
fn negated_can_write(owner: i64, me: i64, my_role: Role) -> bool { !(owner != me || my_role == Role::Viewer {}) }
fn trap_can_write(score: i64, my_divisor: i64) -> bool { score > 0 && 10 / my_divisor > 0 }
fn not_trap_can_write(score: i64, my_divisor: i64) -> bool { !(10 / my_divisor > 0) || score > 0 }
fn unknown_trap_can_write(score: i64) -> bool { 10 / score > 0 }
fn literal_match_can_write(score: i64, my_role: Role) -> bool {
    match score { 0 => false, _ => role_allowed(my_role), }
}
fn bound_match_can_write(score: i64, my_role: Role) -> bool {
    match score { value => value > 0 && role_allowed(my_role), }
}
fn requires_can_write(owner: i64, score: i64, me: i64, my_role: Role) -> bool
    requires score > 0
{ nested(my_role, owner, me) }
fn requires_role_can_write(owner: i64, me: i64, my_role: Role) -> bool
    requires role_allowed(my_role)
{ owner == me }
fn ensures_role_can_write(owner: i64, me: i64, my_role: Role) -> bool
    ensures role_allowed(my_role)
{ owner == me }
fn ensures_unknown_can_write(score: i64, my_role: Role) -> bool
    ensures result || score == 0
{ role_allowed(my_role) && score > 0 }
fn account_only_can_write(my_role: Role) -> bool { role_allowed(my_role) }
"#;

#[test]
fn creation_policy_projection_preserves_definite_refusals_and_unknown_row_inputs() {
    let path = write_temp("creation-policy", SOURCE);
    let projection = generate(&path).unwrap();
    let text = schema(&projection);
    assert!(text.contains("create: (u) =>"));
    // Emission is deterministic; abstract projection doesn't mutate ordinary
    // helper naming, source tests, or account-only policy metadata.
    assert_eq!(text, schema(&generate(&path).unwrap()));
    let out = path.with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let result = std::process::Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webapp/tests/creation_policy.mjs"))
        .arg(&out)
        .output()
        .expect("Node is required for the generated creation-policy gate");
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn creation_preview_never_admits_an_unsupported_authoritative_policy() {
    let path = write_temp("creation-unsupported", "module m; record User { login: string, active: bool, } record Item { score: i64, } fn user_account(login: string, active: bool) -> bool { active } fn item_can_write(score: i64) -> bool { let mut n = score; n = n + 1; n > 0 }");
    let errors = generate(&path).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-WA103"),
        "{errors:?}"
    );
}

#[test]
fn creation_policy_helper_expansion_is_bounded_and_conservative() {
    let mut source = String::from("module bounded; record User { login: string, active: bool, } record Item { score: i64, } fn user_account(login: string, active: bool) -> bool { active } fn base(value: i64) -> bool { value > 0 } ");
    let mut previous = "base".to_owned();
    for index in 0..38 {
        let name = format!("chain_{index}");
        source.push_str(&format!(
            "fn {name}(value: i64) -> bool {{ {previous}(value) || {previous}(value) }} "
        ));
        previous = name;
    }
    source.push_str(&format!(
        "fn item_can_write(score: i64) -> bool {{ {previous}(score) }}"
    ));
    let path = write_temp("creation-bounded", &source);
    let projection = generate(&path).unwrap();
    assert!(
        schema(&projection).len() < 1_000_000,
        "abstract helper projection must stay bounded"
    );
    let out = path.with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let script = "import{pathToFileURL}from'node:url';import assert from'node:assert/strict';const{entities}=await import(pathToFileURL(process.argv[1]));assert.equal(entities.find(e=>e.path==='item').canWrite.create({}),null);";
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "-e", script])
        .arg(out.join("schema.js"))
        .output()
        .expect("Node is required for creation policy gate");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_dir_all(out).unwrap();
}
