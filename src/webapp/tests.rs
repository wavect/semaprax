use super::*;

fn write_temp(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("semaprax-webapp-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.spx");
    std::fs::write(&path, source).unwrap();
    path
}

const DESK: &str = "module team.desk;

variant Priority { Low, High, }

record Project {
    name: string,
    budget: f64,
}

record TimeEntry {
    project_id: i64,
    hours: i64,
    priority: Priority,
    billable: bool,
    initial: char,
}

fn project_valid(name: string, budget: f64) -> bool
    requires string_len(name) >= 2
    requires budget >= 0.0
{
    true
}

fn time_entry_weight(hours: i64, priority: Priority) -> i64
{
    let base = hours * 2;
    match priority { Priority::High {} => double(base), _ => base, }
}

fn time_entry_flag(billable: bool, hours: i64) -> string
{
    if billable && hours > 8 { \"long\" } else { \"ok\" }
}

fn double(value: i64) -> i64
{
    value + value
}
";

fn schema(projection: &Projection) -> &str {
    &projection.files[0].1
}

#[test]
fn projects_entities_rules_computed_and_helpers() {
    let path = write_temp("desk", DESK);
    let projection = generate(&path).unwrap();
    assert_eq!(
        (
            projection.counts.entities,
            projection.counts.enums,
            projection.counts.rules,
            projection.counts.computed
        ),
        (2, 1, 2, 2)
    );
    let schema = schema(&projection);
    assert!(schema.starts_with(SCHEMA_HEADER));
    assert!(schema.contains("export const app = { module: \"team.desk\", title: \"Desk\" };"));
    assert!(schema.contains("Priority: [\"Low\", \"High\"]"));
    assert!(schema.contains("name: \"TimeEntry\", path: \"time_entry\", label: \"id\""));
    assert!(schema.contains("{ name: \"project_id\", type: \"ref\", ref: \"project\" },"));
    assert!(schema.contains("{ name: \"initial\", type: \"char\" },"));
    assert!(schema.contains(
        "{ text: \"string_len(name) >= 2\", fields: [\"name\"], test: (r) => (rt.len(r.name) >= 2n) }"
    ));
    assert!(schema.contains("test: (r) => (r.budget >= 0.0)"));
    assert!(schema.contains("function f_double(p_value) { return rt.add(p_value, p_value); }"));
    assert!(schema.contains("name: \"weight\", type: \"int\""));
    assert!(schema.contains("rt.mul(r.hours, 2n)"));
    assert!(schema.contains("=== \"High\" ? f_double(v"));
    assert!(schema.contains("((r.billable && (r.hours > 8n)) ? \"long\" : \"ok\")"));
    let names: Vec<&str> = projection
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "schema.js",
            "runtime.js",
            "server.mjs",
            "security.mjs",
            "state.mjs",
            "index.html",
            "app.js",
            "style.css"
        ]
    );
    // Deterministic: the same source projects to the same bytes.
    assert_eq!(generate(&path).unwrap().files, projection.files);
}

#[test]
fn explicit_title_metadata_is_bounded_and_deterministic() {
    let path = write_temp("title", DESK);
    let options = ProjectionOptions::default()
        .with_title("TeamDesk Enterprise")
        .unwrap();
    let first = generate_with_options(&path, &options).unwrap();
    let second = generate_with_options(&path, &options).unwrap();
    assert_eq!(first.files, second.files);
    assert!(schema(&first)
        .contains("export const app = { module: \"team.desk\", title: \"TeamDesk Enterprise\" };"));

    for invalid in [
        "".to_owned(),
        "x".repeat(MAX_APP_TITLE_BYTES + 1),
        "bad\ntitle".to_owned(),
    ] {
        assert_eq!(
            ProjectionOptions::default()
                .with_title(invalid)
                .unwrap_err()
                .code,
            "SPX-WA106"
        );
    }
}

#[test]
fn unsupported_shapes_fail_closed_with_stable_codes() {
    let field = write_temp(
        "field",
        "module m;\n\nrecord Item {\n    count: usize,\n}\n\nfn one() -> i64\n{\n    1\n}\n",
    );
    let errors = generate(&field).unwrap_err();
    assert_eq!(errors[0].code, "SPX-WA102");

    let reserved = write_temp(
        "reserved",
        "module m;\n\nrecord Item {\n    id: i64,\n}\n\nfn one() -> i64\n{\n    1\n}\n",
    );
    assert_eq!(generate(&reserved).unwrap_err()[0].code, "SPX-WA102");

    let param = write_temp(
        "param",
        "module m;\n\nrecord Item {\n    count: i64,\n}\n\nfn item_twice(total: i64) -> i64\n{\n    total * 2\n}\n",
    );
    assert_eq!(generate(&param).unwrap_err()[0].code, "SPX-WA102");

    let subset = write_temp(
        "subset",
        "module m;\n\nrecord Item {\n    count: i64,\n}\n\nfn item_loop(count: i64) -> i64\n{\n    let mut total = 0;\n    total = count;\n    total\n}\n",
    );
    let errors = generate(&subset).unwrap_err();
    assert_eq!(errors[0].code, "SPX-WA103");
    assert!(errors[0]
        .help
        .as_deref()
        .unwrap_or("")
        .contains("string_len"));

    let empty = write_temp("empty", "module m;\n\nfn helper() -> i64\n{\n    1\n}\n");
    assert_eq!(generate(&empty).unwrap_err()[0].code, "SPX-WA102");
}

#[test]
fn verifier_errors_still_reject_the_module() {
    let broken = write_temp(
        "broken",
        "module m;\n\nrecord Item {\n    count: i64,\n}\n\nfn item_bad(count: i64) -> bool\n{\n    count\n}\n",
    );
    let errors = generate(&broken).unwrap_err();
    assert!(errors.iter().all(|error| !error.code.starts_with("SPX-WA")));
}

#[test]
fn write_refuses_foreign_directories_and_replaces_generated_ones() {
    let path = write_temp("write", DESK);
    let projection = generate(&path).unwrap();
    let out = path.with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    write(&out, &projection).unwrap();
    assert!(out.join("server.mjs").is_file());
    let foreign = path.with_file_name("foreign");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("notes.txt"), "keep").unwrap();
    assert_eq!(write(&foreign, &projection).unwrap_err().code, "SPX-WA104");
    assert_eq!(
        std::fs::read_to_string(foreign.join("notes.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn run_together_prefixes_count_and_orphan_functions_fail_closed() {
    let joined = write_temp(
        "joined",
        "module m;\n\nrecord TimeEntry {\n    hours: i64,\n}\n\nfn timeentry_double(hours: i64) -> i64\n{\n    hours * 2\n}\n",
    );
    assert_eq!(generate(&joined).unwrap().counts.computed, 1);
    let orphan = write_temp(
        "orphan",
        "module m;\n\nrecord TimeEntry {\n    hours: i64,\n}\n\nfn entry_double(hours: i64) -> i64\n{\n    hours * 2\n}\n",
    );
    let errors = generate(&orphan).unwrap_err();
    assert_eq!(errors[0].code, "SPX-WA105");
    assert!(errors[0]
        .help
        .as_deref()
        .unwrap_or("")
        .contains("time_entry_"));
}

#[test]
fn snake_cases_record_names() {
    assert_eq!(snake("TimeEntry"), "time_entry");
    assert_eq!(snake("Team"), "team");
}

#[test]
fn the_language_card_web_example_projects() {
    let card = include_str!("../../docs/AGENT-QUICK-REFERENCE.md");
    let start = card.find("```spx webapp\n").unwrap() + "```spx webapp\n".len();
    let end = start + card[start..].find("```\n").unwrap();
    let source = &card[start..end];
    let path = write_temp("card", source);
    let projection = generate(&path).unwrap();
    assert_eq!(
        (
            projection.counts.entities,
            projection.counts.enums,
            projection.counts.rules,
            projection.counts.computed
        ),
        (2, 1, 2, 2)
    );
    let program = crate::parse(source, &path).unwrap();
    assert_eq!(
        crate::format::canonical(&program),
        source,
        "card example must be canonical"
    );
}

#[test]
fn the_generated_benchmark_app_passes_its_own_self_test() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node is not installed; the generated self-test is not exercised");
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/webapp-tokens-v1/semaprax/teamdesk.spx");
    let projection = generate(&source).unwrap();
    let out = write_temp("selftest", "").with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let run = std::process::Command::new("node")
        .arg(out.join("server.mjs"))
        .arg("--self-test")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.starts_with("self-test ok: 10 entities, 25 rules, 10 computed"),
        "{stdout}"
    );

    let offline_cwd = out.with_file_name("selftest-offline-cwd");
    let offline_guard = out.with_file_name("offline-self-test-guard.mjs");
    std::fs::write(
        &offline_guard,
        r#"import http from 'node:http';
import net from 'node:net';
import childProcess from 'node:child_process';
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
const deny = (name) => () => { console.error(`OFFLINE_GUARD_TRIP ${name}`); throw new Error(`offline self-test attempted ${name}`); };
http.Server.prototype.listen = deny('http listener');
net.Server.prototype.listen = deny('network listener');
for (const name of ['spawn','spawnSync','exec','execSync','execFile','execFileSync','fork']) childProcess[name] = deny(`child_process.${name}`);
for (const name of ['mkdirSync','writeFileSync','appendFileSync','writeSync','writevSync','truncateSync','ftruncateSync','renameSync','unlinkSync','rmdirSync','rmSync','mkdtempSync','createWriteStream','mkdir','writeFile','appendFile','write','writev','truncate','ftruncate','rename','unlink','rmdir','rm','mkdtemp','copyFile','cp','link','symlink']) fs[name] = deny(`fs.${name}`);
for (const name of ['mkdir','writeFile','appendFile','write','truncate','rename','unlink','rmdir','rm','mkdtemp','copyFile','cp','link','symlink']) fs.promises[name] = deny(`fs.promises.${name}`);
const writeFlags = fs.constants.O_WRONLY | fs.constants.O_RDWR | fs.constants.O_APPEND | fs.constants.O_CREAT | fs.constants.O_TRUNC;
for (const [object, name] of [[fs, 'openSync'], [fs, 'open'], [fs.promises, 'open']]) {
  const original = object[name];
  object[name] = (...args) => {
    const flags = args[1] ?? 'r';
    if (typeof flags === 'number' ? (flags & writeFlags) !== 0 : /[wa+]/.test(flags)) deny(`fs.${name} write`)();
    return original.apply(object, args);
  };
}
globalThis.fetch = deny('fetch');
syncBuiltinESMExports();
"#,
    )
    .unwrap();
    let run_offline = |server: &Path, cwd: &Path| {
        let _ = std::fs::remove_dir_all(cwd);
        std::fs::create_dir_all(cwd).unwrap();
        let mut child = std::process::Command::new("node")
            .arg("--import")
            .arg(&offline_guard)
            .arg(server)
            .arg("--self-test-offline")
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                completed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !completed {
            let _ = child.kill();
        }
        let output = child.wait_with_output().unwrap();
        let empty = std::fs::read_dir(cwd).unwrap().next().is_none();
        let _ = std::fs::remove_dir_all(cwd);
        (output, completed, empty)
    };
    let (offline, offline_completed, offline_empty) =
        run_offline(&out.join("server.mjs"), &offline_cwd);
    let offline_stdout = String::from_utf8_lossy(&offline.stdout);
    assert!(
        offline.status.success(),
        "{offline_stdout}{}",
        String::from_utf8_lossy(&offline.stderr)
    );
    assert!(
        offline_completed,
        "offline self-test did not exit within the bound"
    );
    assert!(
        offline_stdout.starts_with("offline self-test ok: 10 entities,"),
        "{offline_stdout}"
    );
    assert!(
        !offline_stdout.contains("listening on") && offline_empty,
        "offline self-test started a listener or wrote into its working directory: {offline_stdout}"
    );
    assert!(
        !String::from_utf8_lossy(&offline.stderr).contains("OFFLINE_GUARD_TRIP"),
        "offline self-test attempted a forbidden operation: {}",
        String::from_utf8_lossy(&offline.stderr)
    );
    let empty_source = write_temp(
        "offline-empty-entity",
        "module offline.empty;\nrecord Empty {}\nfn main() -> i64 { 0 }\n",
    );
    let empty_projection = generate(&empty_source).unwrap();
    let empty_out = empty_source.with_file_name("offline-empty-out");
    let _ = std::fs::remove_dir_all(&empty_out);
    write(&empty_out, &empty_projection).unwrap();
    let (empty_run, empty_completed, empty_cwd) = run_offline(
        &empty_out.join("server.mjs"),
        &empty_out.with_file_name("offline-empty-cwd"),
    );
    let empty_stdout = String::from_utf8_lossy(&empty_run.stdout);
    assert!(
        empty_run.status.success(),
        "{empty_stdout}{}",
        String::from_utf8_lossy(&empty_run.stderr)
    );
    assert!(
        empty_completed && empty_cwd,
        "empty-entity offline self-test did not remain offline: {empty_stdout}"
    );
    assert!(
        empty_stdout.starts_with("offline self-test ok: 1 entities, 0 types,"),
        "{empty_stdout}"
    );
    assert!(
        !String::from_utf8_lossy(&empty_run.stderr).contains("OFFLINE_GUARD_TRIP"),
        "empty-entity offline self-test attempted a forbidden operation: {}",
        String::from_utf8_lossy(&empty_run.stderr)
    );
    let bad_source = write_temp("offline-bad-computed", DESK);
    let bad_projection = generate(&bad_source).unwrap();
    let bad_out = bad_source.with_file_name("offline-bad-computed-out");
    let _ = std::fs::remove_dir_all(&bad_out);
    write(&bad_out, &bad_projection).unwrap();
    let schema_path = bad_out.join("schema.js");
    let schema = std::fs::read_to_string(&schema_path).unwrap();
    let correct_flag = "((r.billable && (r.hours > 8n)) ? \"long\" : \"ok\")";
    assert_eq!(
        schema.matches(correct_flag).count(),
        1,
        "computed fixture expression drifted"
    );
    std::fs::write(&schema_path, schema.replacen(correct_flag, "1n", 1)).unwrap();
    let (bad_run, bad_completed, bad_cwd) = run_offline(
        &bad_out.join("server.mjs"),
        &bad_out.with_file_name("offline-bad-computed-cwd"),
    );
    let bad_stdout = String::from_utf8_lossy(&bad_run.stdout);
    assert!(
        !bad_run.status.success(),
        "wrong-type computed fixture unexpectedly passed: {bad_stdout}"
    );
    assert!(
        bad_completed && bad_cwd,
        "wrong-type computed fixture did not remain bounded and offline: {bad_stdout}"
    );
    assert!(
        bad_stdout.contains("derive flag with declared type"),
        "wrong-type computed fixture was not rejected as a type error: {bad_stdout}"
    );
    assert!(
        !String::from_utf8_lossy(&bad_run.stderr).contains("OFFLINE_GUARD_TRIP"),
        "wrong-type computed fixture attempted a forbidden operation: {}",
        String::from_utf8_lossy(&bad_run.stderr)
    );
    let _ = std::fs::remove_file(offline_guard);
}

#[test]
fn generated_benchmark_app_is_valid_as_an_es_module() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node is not installed; generated module syntax is not checked");
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/webapp-tokens-v2/semaprax/teamdesk.spx");
    let projection = generate(&source).unwrap();
    let out = write_temp("module-syntax-v2", "").with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let app = std::fs::read(out.join("app.js")).unwrap();
    let mut check = std::process::Command::new("node")
        .args(["--input-type=module", "--check"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
    check.stdin.take().unwrap().write_all(&app).unwrap();
    let result = check.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn v2_self_test_reports_owned_and_foreign_row_permissions() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("node is not installed; the generated self-test is not exercised");
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/webapp-tokens-v2/semaprax/teamdesk.spx");
    let projection = generate(&source).unwrap();
    let out = write_temp("selftest-v2-own-rows", "").with_file_name("out");
    let _ = std::fs::remove_dir_all(&out);
    write(&out, &projection).unwrap();
    let run = std::process::Command::new("node")
        .arg(out.join("server.mjs"))
        .arg("--self-test")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.contains(
            "permissions: 4 roles x 20 entities plus 28 own-row fixtures agree with schema"
        ),
        "{stdout}"
    );
    let agent_own = stdout
        .lines()
        .find(|line| line.starts_with("  Agent on own-account rows:"))
        .unwrap_or_else(|| panic!("missing Agent own-row evidence: {stdout}"));
    assert!(
        agent_own.contains("writes: Task Comment TimeEntry Ticket TicketReply Expense Leave"),
        "{agent_own}"
    );
    let agent_foreign = stdout
        .lines()
        .find(|line| line.starts_with("  Agent on other-account rows:"))
        .unwrap_or_else(|| panic!("missing Agent foreign-row evidence: {stdout}"));
    assert!(agent_foreign.contains("hidden: Expense"), "{agent_foreign}");
    assert!(
        agent_foreign.contains("denied writes: Task Comment TimeEntry Ticket TicketReply Leave"),
        "{agent_foreign}"
    );
    assert!(agent_foreign.contains("writes: none"), "{agent_foreign}");
}

const V2: &str = "module m;

variant Role { Admin, Agent, }

variant Stage { Open, Done, }

record Team {
    name: string,
}

record Member {
    team_id: i64,
    email: string,
    role: Role,
    active: bool,
}

record Job {
    team_id: i64,
    member_id: i64,
    code: string,
    stage: Stage,
    hours: i64,
    cost: f64,
}

fn member_account(email: string, active: bool) -> bool
{
    active
}

fn can_write(my_role: Role) -> bool
{
    match my_role { Role::Admin {} => true, _ => false, }
}

fn job_can_write(my_role: Role, member_id: i64, me: i64) -> bool
{
    can_write(my_role) || member_id == me
}

fn job_key(team_id: i64, code: string) -> string
{
    string_concat(string_from_i64(team_id), code)
}

fn job_stage_step(from: Stage, to: Stage) -> bool
{
    match from { Stage::Open {} => true, _ => false, }
}

fn job_open(stage: Stage) -> bool
{
    match stage { Stage::Open {} => true, _ => false, }
}

fn team_jobs(count_job: i64, count_job_open: i64) -> i64
{
    count_job * 100 + count_job_open
}

fn team_cost(sum_job_cost: f64) -> f64
{
    sum_job_cost
}
";

#[test]
fn v2_conventions_project_accounts_permissions_keys_steps_and_rollups() {
    let path = write_temp("v2", V2);
    let projection = generate(&path).unwrap();
    let counts = projection.counts;
    assert_eq!(
        (
            counts.entities,
            counts.keys,
            counts.workflows,
            counts.rollups,
            counts.permissions
        ),
        (3, 1, 1, 3, 3)
    );
    assert!(counts.accounts);
    assert!(counts
        .summary()
        .ends_with(", 1 keys, 1 workflows, 3 rollups, 3 permissions, accounts"));
    let schema = schema(&projection);
    assert!(schema.contains(
        "export const account = { entity: \"member\", login: \"email\", allowed: (r) => r.active };"
    ));
    // The unprefixed default applies where an entity has no policy of its own.
    assert_eq!(
        schema
            .matches("canWrite: { row: false, test: (r, u) => ((m4) => m4 === \"Admin\" ? true")
            .count(),
        2
    );
    assert!(schema.contains(
        "canWrite: { row: true, test: (r, u) => (f_can_write(u.role) || (r.member_id === u.id)), create: (u) =>"
    ));
    assert!(schema.contains("{ name: \"key\", fields: [\"team_id\", \"code\"], value: (r) =>"));
    assert!(schema.contains("{ field: \"stage\", test: (from, to) => ((m"));
    assert!(schema.contains(
        "{ name: \"count_job_open\", kind: \"count\", child: \"job\", via: \"team_id\", field: \"open\", type: \"int\" }"
    ));
    assert!(schema.contains(
        "{ name: \"sum_job_cost\", kind: \"sum\", child: \"job\", via: \"team_id\", field: \"cost\", type: \"float\" }"
    ));
    assert!(schema.contains("value: (r) => rt.add(rt.mul(r.count_job, 100n), r.count_job_open)"));
}

#[test]
fn v2_conventions_fail_closed() {
    let cases = [
        // `me` needs an account entity.
        ("me", "module m;\n\nrecord Job {\n    owner: i64,\n}\n\nfn job_can_write(me: i64, owner: i64) -> bool\n{\n    me == owner\n}\n"),
        // A rollup over an entity that does not reference this one.
        ("rollup", "module m;\n\nrecord Team {\n    name: string,\n}\n\nrecord Job {\n    hours: i64,\n}\n\nfn team_jobs(count_job: i64) -> i64\n{\n    count_job\n}\n"),
        // A step over the wrong type.
        ("step", "module m;\n\nvariant Stage { Open, Done, }\n\nrecord Job {\n    stage: Stage,\n}\n\nfn job_stage_step(from: i64, to: i64) -> bool\n{\n    from < to\n}\n"),
        // `password` is kept by the server, not declared.
        ("password", "module m;\n\nrecord Member {\n    password: string,\n}\n\nfn one() -> i64\n{\n    1\n}\n"),
    ];
    for (name, source) in cases {
        let errors = generate(&write_temp(name, source)).unwrap_err();
        assert_eq!(
            errors[0].code, "SPX-WA102",
            "{name}: {:?}",
            errors[0].message
        );
    }
}

#[test]
fn record_invariants_become_entity_rules_beside_valid_functions() {
    let source = "module team.crew;

variant Role { Lead, Member, Guest, }

record Crew {
    name: string,
    role: Role,
    seats: i64,
}
    requires string_len(name) >= 2
    requires role != Role::Guest {} || seats <= 1

fn crew_valid(seats: i64) -> bool
    requires seats >= 1
{
    true
}

fn crew_staffed(role: Role, seats: i64) -> bool
{
    match role { Role::Lead {} | Role::Member {} => seats > 0, Role::Guest {} => false, }
}
";
    let path = write_temp("crew", source);
    let projection = generate(&path).unwrap();
    let schema = schema(&projection);
    assert!(
        schema.contains("text: \"string_len(name) >= 2\""),
        "{schema}"
    );
    assert!(
        schema.contains("text: \"role != Role::Guest {} || seats <= 1\""),
        "{schema}"
    );
    assert!(schema.contains("text: \"seats >= 1\""), "{schema}");
    assert!(schema.contains("(r.role !== \"Guest\")"), "{schema}");
    assert!(
        schema.contains("(m1 === \"Lead\" || m1 === \"Member\")"),
        "{schema}"
    );
}

#[test]
fn api_listing_and_parameter_help_name_exact_options() {
    let projection = generate(&write_temp("api", V2)).unwrap();
    assert!(projection.api.starts_with(
        "auth: POST /api/session {\"login\": <member.email>, \"password\"} (also accepts \"email\" as the login key) sets the session cookie;"
    ));
    assert!(projection.api.contains(
        "\njob team_id->team member_id->member code:string stage:Stage hours:int cost:float | unique(team_id,code) workflow(stage) open:bool | read any, write row rule\n"
    ));
    assert!(projection
        .api
        .contains("\nteam name:string | jobs:int cost:float | read any, write role rule\n"));

    let wrong = V2.replace("sum_job_cost", "sum_job_price");
    let errors = generate(&write_temp("api-wrong", &wrong)).unwrap_err();
    assert_eq!(errors[0].code, "SPX-WA102");
    let help = errors[0].help.as_deref().unwrap_or("");
    assert!(help.contains("name: string"), "{help}");
    assert!(help.contains("count_job: i64"), "{help}");
    assert!(help.contains("count_job_open: i64"), "{help}");
    assert!(help.contains("sum_job_cost: f64"), "{help}");
    assert!(help.contains("sum_job_hours: i64"), "{help}");
}

#[test]
fn row_aware_default_policies_cover_matching_entities_most_specific_first() {
    let source = V2
        .replace(
            "fn job_can_write(my_role: Role, member_id: i64, me: i64) -> bool\n{\n    can_write(my_role) || member_id == me\n}\n",
            "fn can_write_own(my_role: Role, member_id: i64, me: i64) -> bool\n{\n    can_write(my_role) || member_id == me\n}\n",
        );
    let projection = generate(&write_temp("defaults", &source)).unwrap();
    let schema = schema(&projection);
    // `job` has `member_id`, so the row-aware default wins over `can_write`.
    assert!(schema.contains(
        "canWrite: { row: true, test: (r, u) => (f_can_write(u.role) || (r.member_id === u.id)) }"
    ));
    assert_eq!(schema.matches("canWrite: { row: false, test:").count(), 2);
    let typo = source.replace(
        "fn can_write_own(my_role: Role, member_id: i64, me: i64) -> bool\n{\n    can_write(my_role) || member_id == me",
        "fn can_write_own(my_role: Role, owner_id: i64, me: i64) -> bool\n{\n    can_write(my_role) || owner_id == me",
    );
    assert_eq!(
        generate(&write_temp("defaults-typo", &typo)).unwrap_err()[0].code,
        "SPX-WA102"
    );
}

#[test]
fn v3_projects_pairwise_constraints_and_explicit_field_migrations() {
    let source = "module scheduling;
record Booking { room: i64, start: i64, end: i64, label: string, }
fn booking_constraint_overlap(room: i64, start: i64, end: i64, other_booking_room: i64, other_booking_start: i64, other_booking_end: i64) -> bool { room != other_booking_room || end <= other_booking_start || start >= other_booking_end }
fn booking_migrate_label() -> string { \"untitled\" }
fn booking_migrate_start(old_begin: i64) -> i64 { old_begin }
";
    let projection = generate(&write_temp("v3-constraints", source)).unwrap();
    let text = schema(&projection);
    assert!(text.contains("other: \"booking\""), "{text}");
    assert!(text.contains("test: (r, o) =>"), "{text}");
    assert!(text.contains("o.start"), "{text}");
    assert!(text.contains("field: \"label\", inputs: []"), "{text}");
    assert!(text.contains("name: \"begin\", type: \"int\""), "{text}");
    assert!(text.contains("value: (o) => o.begin"), "{text}");
}

#[test]
fn v3_bad_constraint_and_migration_shapes_fail_closed() {
    for (name, function) in [
        (
            "missing-other",
            "fn item_constraint(value: i64) -> bool { value > 0 }",
        ),
        (
            "wrong-other-type",
            "fn item_constraint(other_item_value: bool) -> bool { other_item_value }",
        ),
        (
            "wrong-destination",
            "fn item_migrate_missing() -> i64 { 1 }",
        ),
        ("wrong-return", "fn item_migrate_value() -> bool { true }"),
        (
            "wrong-input",
            "fn item_migrate_value(value: i64) -> i64 { value }",
        ),
    ] {
        let source = format!("module v3; record Item {{ value: i64, }} {function}");
        let errors = generate(&write_temp(name, &source)).err().unwrap();
        assert!(errors.iter().any(|e| e.code == "SPX-WA102"), "{errors:?}");
    }
}

#[test]
fn v3_runtime_security_migration_and_cross_row_contracts() {
    let dir =
        std::env::temp_dir().join(format!("semaprax-webapp-v3-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in RUNTIME_FILES {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(dir.join("package.json"), "{\"type\":\"module\"}").unwrap();
    std::fs::write(
        dir.join("contracts.mjs"),
        include_str!("runtime-tests/v3.mjs"),
    )
    .unwrap();
    std::fs::write(
        dir.join("http-contracts.mjs"),
        include_str!("runtime-tests/http-v3.mjs"),
    )
    .unwrap();
    let output = std::process::Command::new("node")
        .arg(dir.join("contracts.mjs"))
        .output()
        .expect("Node is required for the webapp v3 runtime contract");
    let http = std::process::Command::new("node")
        .arg(dir.join("http-contracts.mjs"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    for output in [output, http] {
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
mod creation_policy;
mod sg_regressions;
