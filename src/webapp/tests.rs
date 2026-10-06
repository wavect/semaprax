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
            projection.entities,
            projection.enums,
            projection.rules,
            projection.computed
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
            "index.html",
            "app.js",
            "style.css"
        ]
    );
    // Deterministic: the same source projects to the same bytes.
    assert_eq!(generate(&path).unwrap().files, projection.files);
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
    assert_eq!(generate(&joined).unwrap().computed, 1);
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
            projection.entities,
            projection.enums,
            projection.rules,
            projection.computed
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
}
