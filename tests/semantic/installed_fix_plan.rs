use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;
use semaprax::installed_fix_plan::{
    current_source_fix_plan, installed_fix_plan_catalog, FixPlan, FixPlanRequest,
    CURRENT_SOURCE_FIX_PLAN_SCHEMA, INSTALLED_FIX_PLAN_CATALOG_SCHEMA,
    MAX_CURRENT_SOURCE_FIX_PLAN_BYTES, MAX_INSTALLED_FIX_PLAN_CATALOG_BYTES,
};
use semaprax::repair::{self, DiagnosticRepairQuery, PersistentDeclarationId};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = "module fix.plan;\nfn helper(value:i64)->i64{value+1}\n@id(\"fix.caller\") fn caller(value:i64)->i64{helper(value)}\n@id(\"app.main\") fn main()->i64{caller(1)}\n";
const TARGET: &str = "auto:fix.plan.helper";

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "spx-installed-fix-plan-v1-{}-{label}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("module.spx");
        std::fs::write(&source, SOURCE).unwrap();
        Self { directory, source }
    }

    fn invoke(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.directory)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn inventory(&self) -> BTreeSet<String> {
        std::fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            let mut result = Map::new();
            for key in keys {
                result.insert(key.clone(), sorted(&object[key]));
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

fn canonical(value: &Value) -> String {
    let mut output = serde_json::to_string(&sorted(value)).unwrap();
    output.push('\n');
    output
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn envelope(bytes: &str, schema: &str, domain: &[u8], limit: usize) -> Value {
    assert!(bytes.ends_with('\n'));
    assert!(bytes.len() <= limit);
    let value: Value = serde_json::from_str(bytes).unwrap();
    assert_eq!(value["schema"], schema);
    assert_eq!(canonical(&value), bytes);
    let payload = canonical(&value["payload"]);
    assert_eq!(value["digest"], digest(domain, payload.as_bytes()));
    assert_eq!(value["payload"]["authority"], false);
    assert_eq!(value["payload"]["compiler"]["package"], "semaprax");
    assert_eq!(
        value["payload"]["compiler"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        value["payload"]["compiler"]["binary_identity_claimed"],
        false
    );
    value
}

fn code(errors: Vec<Diagnostic>) -> &'static str {
    assert_eq!(errors.len(), 1);
    errors[0].code
}

fn request() -> FixPlanRequest {
    FixPlanRequest::assign_function_id(TARGET).unwrap()
}

#[test]
fn installed_catalog_is_one_exact_deterministic_authority_free_operation() {
    let catalog = installed_fix_plan_catalog().unwrap();
    assert_eq!(catalog, installed_fix_plan_catalog().unwrap());
    let value = envelope(
        catalog.to_json(),
        INSTALLED_FIX_PLAN_CATALOG_SCHEMA,
        b"semaprax.installed-fix-plan-catalog.payload.digest.v1\0",
        MAX_INSTALLED_FIX_PLAN_CATALOG_BYTES,
    );
    assert_eq!(value["digest"], catalog.digest());
    assert_eq!(
        value["payload"]["operations"],
        json!([{
            "classification":"breaking_identity_rebase",
            "diagnostic":"SPX-S103",
            "kind":"assign_function_id",
            "plan_availability":"requires_exact_current_source_and_automatic_function_id",
            "required_instantiation_input":{
                "name":"persistent_id","required":true,"type":"persistent_declaration_id"
            },
            "source_report_schema":"semaprax.diagnostic-repair.v1"
        }])
    );
    assert!(value["payload"]["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|claim| claim == "planning_does_not_instantiate_or_apply_a_patch"));
}

#[test]
fn current_source_plan_exactly_embeds_existing_repair_discovery_and_replays() {
    let fixture = Fixture::new("current");
    let inventory = fixture.inventory();
    let plan = current_source_fix_plan(&fixture.source, &request()).unwrap();
    assert_eq!(
        plan,
        current_source_fix_plan(&fixture.source, &request()).unwrap()
    );
    let value = envelope(
        plan.to_json(),
        CURRENT_SOURCE_FIX_PLAN_SCHEMA,
        b"semaprax.current-source-fix-plan.payload.digest.v1\0",
        MAX_CURRENT_SOURCE_FIX_PLAN_BYTES,
    );
    let legacy = repair::query(
        &fixture.source,
        &DiagnosticRepairQuery::assign_function_id(TARGET).unwrap(),
    )
    .unwrap();
    assert_eq!(plan.repair_report(), legacy);
    assert_eq!(
        value["payload"]["repair_discovery"]["value"],
        serde_json::from_str::<Value>(&legacy).unwrap()
    );
    assert_eq!(
        plan.repair_report_digest(),
        digest(
            b"semaprax.current-source-fix-plan.repair-report.digest.v1\0",
            legacy.as_bytes()
        )
    );
    assert_eq!(
        value["payload"]["repair_discovery"]["digest"],
        plan.repair_report_digest()
    );
    assert_eq!(
        value["payload"]["source_binding"]["base_revision"],
        value["payload"]["repair_discovery"]["value"]["base_revision"]
    );
    assert_eq!(
        value["payload"]["source_binding"]["source_digest"],
        value["payload"]["repair_discovery"]["value"]["source"]["digest"]
    );
    assert_eq!(
        FixPlan::replay_current_source(
            &fixture.source,
            &request(),
            plan.digest(),
            plan.to_json().as_bytes(),
        )
        .unwrap(),
        plan
    );
    assert_eq!(fixture.inventory(), inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
}

#[test]
fn fix_cli_prints_exact_catalog_and_current_source_core_artifacts() {
    let fixture = Fixture::new("cli");
    let inventory = fixture.inventory();
    let catalog = installed_fix_plan_catalog().unwrap();
    let output = fixture.invoke(&["fix", "--plan"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, catalog.to_json().as_bytes());

    let plan = current_source_fix_plan(&fixture.source, &request()).unwrap();
    let output = fixture.invoke(&[
        "fix",
        fixture.source.to_str().unwrap(),
        "assign-function-id",
        TARGET,
        "--plan",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, plan.to_json().as_bytes());
    assert_eq!(fixture.inventory(), inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
}

#[test]
fn malformed_unavailable_stale_tampered_and_oversized_plans_fail_closed() {
    assert_eq!(
        code(FixPlanRequest::assign_function_id("").unwrap_err()),
        "SPX-R101"
    );
    let fixture = Fixture::new("hostile");
    let unavailable = FixPlanRequest::assign_function_id("fix.caller").unwrap();
    assert_eq!(
        code(current_source_fix_plan(&fixture.source, &unavailable).unwrap_err()),
        "SPX-R101"
    );
    let plan = current_source_fix_plan(&fixture.source, &request()).unwrap();
    assert_eq!(
        code(
            FixPlan::replay_current_source(
                &fixture.source,
                &request(),
                plan.digest(),
                plan.to_json().trim_end().as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G544"
    );
    let mut tampered: Value = serde_json::from_str(plan.to_json()).unwrap();
    tampered["payload"]["plan"]["status"] = json!("tampered");
    let tampered = canonical(&tampered);
    assert_eq!(
        code(
            FixPlan::replay_current_source(
                &fixture.source,
                &request(),
                plan.digest(),
                tampered.as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G547"
    );
    assert_eq!(
        code(
            FixPlan::replay_current_source(
                &fixture.source,
                &request(),
                "invalid",
                plan.to_json().as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G544"
    );
    let oversized = vec![b' '; MAX_CURRENT_SOURCE_FIX_PLAN_BYTES + 1];
    assert_eq!(
        code(
            FixPlan::replay_current_source(&fixture.source, &request(), plan.digest(), &oversized,)
                .unwrap_err()
        ),
        "SPX-G545"
    );
    std::fs::write(&fixture.source, SOURCE.replace("value+1", "value+2")).unwrap();
    assert_eq!(
        code(
            FixPlan::replay_current_source(
                &fixture.source,
                &request(),
                plan.digest(),
                plan.to_json().as_bytes(),
            )
            .unwrap_err()
        ),
        "SPX-G547"
    );

    for arguments in [
        &["fix"][..],
        &["fix", "--plan", "extra"][..],
        &["fix", "--unknown"][..],
        &[
            "fix",
            fixture.source.to_str().unwrap(),
            "assign-function-id",
            TARGET,
        ][..],
        &[
            "fix",
            fixture.source.to_str().unwrap(),
            "other",
            TARGET,
            "--plan",
        ][..],
    ] {
        let output = fixture.invoke(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn legacy_repair_core_and_cli_bytes_remain_exact() {
    let fixture = Fixture::new("legacy");
    let query = DiagnosticRepairQuery::assign_function_id(TARGET).unwrap();
    let report = repair::query(&fixture.source, &query).unwrap();
    let output = fixture.invoke(&[
        "repairs",
        fixture.source.to_str().unwrap(),
        "assign-function-id",
        TARGET,
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, format!("{report}\n").as_bytes());

    let report_value: Value = serde_json::from_str(&report).unwrap();
    let repair_id = report_value["repair"]["id"].as_str().unwrap();
    let persistent = PersistentDeclarationId::new("fix.plan.helper").unwrap();
    let preview = repair::instantiate(&fixture.source, repair_id, &persistent).unwrap();
    let output = fixture.invoke(&[
        "repair",
        fixture.source.to_str().unwrap(),
        repair_id,
        "--persistent-id",
        persistent.as_str(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, format!("{preview}\n").as_bytes());
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
    assert_eq!(
        fixture.inventory(),
        BTreeSet::from(["module.spx".to_owned()])
    );

    let checked = PathBuf::from("tests/fixtures/diagnostic_repair_v1.spx");
    let checked_query =
        DiagnosticRepairQuery::assign_function_id("auto:repair.phase_a.helper").unwrap();
    let checked_report = repair::query(&checked, &checked_query).unwrap();
    let checked_value: Value = serde_json::from_str(&checked_report).unwrap();
    let checked_preview = repair::instantiate(
        &checked,
        checked_value["repair"]["id"].as_str().unwrap(),
        &PersistentDeclarationId::new("repair.phase_a.helper").unwrap(),
    )
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(checked_report.as_bytes()))
        ),
        "ef689fed2c742dea6cedb0b8ec3d449e5facd8748dd00cb8a8f2e6115be82075"
    );
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(checked_preview.as_bytes()))
        ),
        "ae779749b252e5d9661172dfebcd3317211b97310eed57a0a6b7a692be1053e4"
    );
}
