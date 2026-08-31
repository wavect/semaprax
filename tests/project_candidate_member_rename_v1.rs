//! Stable-ID member rename evidence, authored and intentionally unrun.
use semaprax::ast::{Program, TypeDeclarationKind};
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateTestPolicy, ProjectCandidate, SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const CORE: &str = r#"module members.core;
@id("members.pair") record Pair { @id("members.pair.value") value:i64, @id("members.pair.count") count:i64, }
@id("members.outer") record Outer { @id("members.outer.inner") inner:Pair, }
@id("members.other") record Other { @id("members.other.value") value:i64, }
@id("members.choice") variant Choice { @id("members.choice.some") Some { @id("members.choice.some.value") value:i64, @id("members.choice.some.flag") flag:bool, }, @id("members.choice.none") None, }
@id("members.other-choice") variant OtherChoice { @id("members.other-choice.some") Some { @id("members.other-choice.value") value:i64, }, @id("members.other-choice.none") None, }
@id("members.make") fn make(value:i64)->Pair { Pair {value:value, count:1} }
@id("members.update") fn update(input:Pair)->Pair requires input.value >= 0 ensures result.value >= 0 { input with {value:input.value + 1} }
@id("members.nested") fn nested(input:Outer)->i64 { input.inner.value }
@id("members.unpack") fn unpack(input:Outer)->i64 {match input {Outer {inner:Pair {value: extracted, count: _}} => extracted,}}
@id("members.shorthand") fn shorthand(input:Pair)->i64 {match input {Pair {value, count: _} => value,}}
@id("members.choose") fn choose(input:Choice)->i64 {match input {Choice::Some {value, flag} => if flag {value} else {0}, Choice::None {} => 0,}}
@id("members.construct") fn construct(value:i64)->Choice {Choice::Some {value:value, flag:true}}
@id("members.wrong-owner") fn wrong_owner(input:OtherChoice)->i64 {let other = Other {value:2}; match input {OtherChoice::Some {value: extracted} => extracted + other.value, OtherChoice::None {} => other.value,}}
@id("members.public") fn public_value(value:i64)->i64 {value}
@id("members.spare") fn spare(value:i64)->i64 {value}
"#;
const APP: &str = r#"module members.app;
use type @id("members.pair") from members.core as Metric;
use type @id("members.choice") from members.core as Signal;
use function @id("members.public") from members.core as public_value;
@id("members.app-read") fn read(input:Metric)->i64 {input.value}
@id("members.app-update") fn update(input:Metric)->Metric {input with {value:input.value + 1}}
@id("members.app-choose") fn choose(input:Signal)->i64 {match input {Signal::Some {value: extracted, flag: truth} => if truth {extracted} else {0}, Signal::None {} => 0,}}
@id("members.main") fn main()->i64 {let value = 40; let pair = Metric {value:value, count:1}; public_value(read(update(pair))) + choose(Signal::Some {value:1, flag:true})}
"#;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-member-rename-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.manifest(),
            r#"schema = "semaprax.project.v8"
name = "member-rename"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "members.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["members.public"]
tests = ["members.tests"]
"#,
        )
        .unwrap();
        fixture.write("core", CORE);
        fixture.write("app", APP);
        fixture.write(
            "tests",
            r#"module members.tests;
use function @id("members.public") from members.core as public_value;
@id("members.test") fn main()->i64 {if public_value(42) == 42 {0}else{1}}
"#,
        );
        fixture
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn write(&self, module: &str, text: &str) {
        std::fs::write(
            self.0.join(format!("src/{module}.spx")),
            canonical(text, module),
        )
        .unwrap();
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn canonical(text: &str, module: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(text, format!("src/{module}.spx")).unwrap())
}
fn source<'a>(candidate: &'a ProjectCandidate, module: &str) -> &'a str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == format!("src/{module}.spx"))
        .unwrap()
        .source()
}
fn program(candidate: &ProjectCandidate, module: &str) -> Program {
    semaprax::parse(source(candidate, module), format!("src/{module}.spx")).unwrap()
}
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &intent)?,
    )
}
fn error<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|e| e.code == code), "{errors:?}"),
    }
}
fn preserved(base: &ProjectCandidate, candidate: &ProjectCandidate) {
    // Full graph identity/origin and stable dependency inventories must remain
    // equal; only canonical source display spellings change.
    let before: Value = serde_json::from_str(base.revision().semantic_graph()).unwrap();
    let after: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    assert!(!before["declarations"].as_array().unwrap().is_empty());
    assert_eq!(before["declarations"], after["declarations"]);
    assert_eq!(before["edges"], after["edges"]);
    assert_eq!(source(base, "tests"), source(candidate, "tests"));
    for alias in ["as Metric;", "as Signal;", "as public_value;"] {
        assert!(source(candidate, "app").contains(alias));
    }
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
}

#[test]
fn record_field_rename_migrates_cross_file_places_updates_contracts_and_nested_patterns() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("members.pair.value", "amount")).unwrap();
    let text = source(&candidate, "core");
    for expected in [
        "@id(\"members.pair.value\")",
        "amount: i64",
        "Pair { amount: value, count: 1 }",
        "input.amount >= 0",
        "result.amount >= 0",
        "input with { amount: input.amount + 1 }",
        "input.inner.amount",
        "Pair { amount: extracted, count: _ }",
        "Pair { amount: value, count: _ } => value",
        "Other { value: 2 }",
        "other.value",
        "Choice::Some { value, flag }",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    let app = source(&candidate, "app");
    for expected in [
        "input.amount",
        "input with { amount: input.amount + 1 }",
        "Metric { amount: value, count: 1 }",
        "let value = 40",
        "Signal::Some { value: 1, flag: true }",
    ] {
        assert!(app.contains(expected), "missing {expected}: {app}");
    }
    let parsed = program(&candidate, "core");
    let pair = parsed
        .types
        .iter()
        .find(|ty| ty.stable_id == "members.pair")
        .unwrap();
    let TypeDeclarationKind::Record { fields } = &pair.kind else {
        panic!("record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|f| (f.stable_id.as_str(), f.name.as_str(), f.explicit_id))
            .collect::<Vec<_>>(),
        [
            ("members.pair.value", "amount", true),
            ("members.pair.count", "count", true)
        ]
    );
    preserved(&base, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn variant_case_rename_changes_only_the_selected_owner_in_provider_and_alias_consumers() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("members.choice.some", "Present")).unwrap();
    // Exact complete source expectations prove unrelated names and every
    // imported alias remain unchanged, not only the selected declaration.
    let core = CORE
        .replace(
            "@id(\"members.choice.some\") Some",
            "@id(\"members.choice.some\") Present",
        )
        .replace("{Choice::Some", "{Choice::Present");
    let app = APP.replace("Signal::Some", "Signal::Present");
    assert_eq!(source(&candidate, "core"), canonical(&core, "core"));
    assert_eq!(source(&candidate, "app"), canonical(&app, "app"));
    assert!(source(&candidate, "core").contains("OtherChoice::Some"));
    preserved(&base, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn payload_field_rename_expands_shorthand_without_renaming_lexical_bindings_or_other_owners() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("members.choice.some.value", "amount")).unwrap();
    let core = CORE
        .replace(
            "@id(\"members.choice.some.value\") value",
            "@id(\"members.choice.some.value\") amount",
        )
        .replace(
            "Choice::Some {value, flag}",
            "Choice::Some {amount:value, flag}",
        )
        .replace("{Choice::Some {value:value", "{Choice::Some {amount:value");
    let app = APP.replace("Signal::Some {value:", "Signal::Some {amount:");
    assert_eq!(source(&candidate, "core"), canonical(&core, "core"));
    assert_eq!(source(&candidate, "app"), canonical(&app, "app"));
    let text = source(&candidate, "core");
    assert!(text.contains("if flag {"));
    assert!(text.contains("OtherChoice::Some { value: extracted }"));
    assert!(text.contains("Pair { value: value, count: 1 }"));
    preserved(&base, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn member_namespace_collisions_invalid_targets_and_stale_bases_leave_inputs_unchanged() {
    let fixture = Fixture::new();
    fixture.write("core", &(CORE.to_owned() + r#"
@id("members.implicit-fields") record ImplicitFields { value:i64, }
record ImplicitOwner { @id("members.implicit-owner.field") value:i64, }
@id("members.implicit-cases") variant ImplicitCases { Some { @id("members.implicit-case.payload") value:i64, }, None, }
"#));
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let unchanged = base.to_json().to_owned();
    let parsed = program(&base, "core");
    let owner = parsed
        .types
        .iter()
        .find(|ty| ty.stable_id == "members.implicit-fields")
        .unwrap();
    let TypeDeclarationKind::Record { fields } = &owner.kind else {
        panic!("record")
    };
    assert!(!fields[0].explicit_id);
    for request in [
        rename("members.pair.value", "count"),
        rename("members.choice.some", "None"),
        rename("members.choice.some.value", "flag"),
        rename("members.pair.value", "value"),
        rename("members.pair.value", "bad {}"),
        rename(&fields[0].stable_id, "other"),
        rename("members.implicit-owner.field", "other"),
        rename("members.implicit-case.payload", "other"),
        rename("core.option", "other"),
        rename("missing.member", "other"),
    ] {
        assert!(apply(&base, request).is_err());
        assert_eq!(base.to_json(), unchanged);
    }
    error(
        apply(&base, rename("members.pair.value", "bad {}")),
        "SPX-G225",
    );
    error(
        apply(&base, rename("members.implicit-case.payload", "other")),
        "SPX-G225",
    );
    let candidate = apply(&base, rename("members.pair.value", "amount")).unwrap();
    let stale = SemanticChange::new(
        base.revision().project_revision(),
        &rename("members.choice.some", "Present"),
    )
    .unwrap();
    error(
        candidate.apply(candidate.candidate_digest(), &stale),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn member_history_merges_unrelated_functions_but_conflicts_on_competing_and_net_zero_renames() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for (target, name, competing_name, original_name) in [
        ("members.pair.value", "amount", "total", "value"),
        ("members.choice.some", "Present", "Found", "Some"),
        ("members.choice.some.value", "amount", "total", "value"),
    ] {
        let candidate = apply(&base, rename(target, name)).unwrap();
        let unrelated = apply(&base, rename("members.spare", "other_spare")).unwrap();
        let merged = candidate
            .merge(
                candidate.candidate_digest(),
                &unrelated,
                unrelated.candidate_digest(),
            )
            .unwrap()
            .into_candidate();
        assert!(source(&merged, "core").contains("fn other_spare("));
        preserved(&base, &candidate);
        let competing = apply(&base, rename(target, competing_name)).unwrap();
        let net_zero = apply(&competing, rename(target, original_name)).unwrap();
        for other in [&competing, &net_zero] {
            error(
                candidate.merge(
                    candidate.candidate_digest(),
                    other,
                    other.candidate_digest(),
                ),
                "SPX-G235",
            );
        }
        error(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(competing.revision()),
                competing.revision().project_revision(),
            ),
            "SPX-G235",
        );
    }
    // Rebinding must find this new member in the original intermediate
    // candidate, since its identity is absent from the common base.
    let added = apply(&base, json!({"kind":"add_declaration","target":"members.spare","declaration":{"kind":"record","id":"members.new","name":"NewRecord","fields":[{"id":"members.new.value","name":"value","type":"i64"}]}})).unwrap();
    let renamed = apply(&added, rename("members.new.value", "amount")).unwrap();
    let unrelated = apply(&base, rename("members.spare", "other_spare")).unwrap();
    let merged = renamed
        .merge(
            renamed.candidate_digest(),
            &unrelated,
            unrelated.candidate_digest(),
        )
        .unwrap()
        .into_candidate();
    assert!(source(&merged, "core").contains("amount: i64"));
    let restored = ProjectCandidate::restore(
        Arc::clone(merged.base_revision()),
        merged.base_revision().project_revision(),
        merged.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), merged.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_member_renames_preserve_template_parameters_and_direct_concrete_instances() {
    let fixture = Fixture::new();
    fixture.write("core", &(CORE.to_owned() + r#"
@id("members.box") record Box<T> { @id("members.box.value") value:T, }
@id("members.generic-choice") variant GenericChoice<T> { @id("members.generic-choice.some") Some { @id("members.generic-choice.value") value:T, }, @id("members.generic-choice.none") None, }
@id("members.box-make") fn boxed(value:i64)->Box<i64> {Box<i64> {value:value}}
@id("members.box-update") fn box_update(input:Box<i64>)->Box<i64> {input with {value:input.value + 1}}
@id("members.generic-choice-read") fn generic_read(input:GenericChoice<i64>)->i64 {match input {GenericChoice::Some {value} => value, GenericChoice::None {} => 0,}}
@id("members.generic-choice-make") fn generic_make(value:i64)->i64 {generic_read(GenericChoice<i64>::Some {value:value})}
"#));
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let record = apply(&base, rename("members.box.value", "item")).unwrap();
    let candidate = apply(&record, rename("members.generic-choice.value", "payload")).unwrap();
    let text = source(&candidate, "core");
    for expected in [
        "record Box<T>",
        "item: T",
        "Box<i64> { item: value }",
        "input with { item: input.item + 1 }",
        "variant GenericChoice<T>",
        "payload: T",
        "GenericChoice<i64>::Some { payload: value }",
        "GenericChoice::Some { payload: value } => value",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    // The ordinary same-spelling Pair and Choice members still select their
    // own declarations, including their original lexical pattern bindings.
    assert!(text.contains("Pair { value: value, count: 1 }"));
    assert!(text.contains("Choice::Some { value, flag }"));
    assert_eq!(source(&base, "app"), source(&candidate, "app"));
    let before = program(&base, "core");
    let after = program(&candidate, "core");
    for id in ["members.box", "members.generic-choice"] {
        let old = before.types.iter().find(|ty| ty.stable_id == id).unwrap();
        let new = after.types.iter().find(|ty| ty.stable_id == id).unwrap();
        assert_eq!(old.type_parameters, new.type_parameters);
        assert_eq!(old.name, new.name);
    }
    preserved(&base, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_bytes_field_rename_preserves_projected_borrow_and_sibling_move_meaning() {
    let fixture = Fixture::new();
    // This is the admitted flat projected-field loan shape from
    // shared_loan_runtime_v1, isolated from unrelated aggregate profiles.
    fixture.write("core", r#"module members.core;
@id("members.packet") record Packet { @id("members.packet.left") left:Bytes, @id("members.packet.right") right:Bytes, @id("members.packet.marker") marker:i64, }
@id("members.consume") fn consume(value:own Bytes)->i64 {7}
@id("members.projected") fn projected_field()->i64 {
    let left_source = [8u8, 9u8];
    let right_source = [7u8];
    let packet = Packet {left:bytes_copy(array_as_slice(left_source)), right:bytes_copy(array_as_slice(right_source)), marker:35};
    let view = bytes_as_slice(packet.left);
    let alias = view;
    let range = byte_range(alias, 0usize, byte_len(alias));
    let consumed_sibling = consume(packet.right);
    let observed = if byte_len(range) == 2usize {packet.marker} else {0};
    consumed_sibling + observed
}
@id("members.public") fn public_value(value:i64)->i64 {value}
"#);
    fixture.write(
        "app",
        r#"module members.app;
use function @id("members.public") from members.core as public_value;
@id("members.main") fn main()->i64 {public_value(42)}
"#,
    );
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, rename("members.packet.left", "payload")).unwrap();
    let expected = source(&base, "core")
        .replace("left: Bytes", "payload: Bytes")
        .replace("left: bytes_copy", "payload: bytes_copy")
        .replace("packet.left", "packet.payload");
    assert_eq!(source(&candidate, "core"), expected);
    assert!(expected.contains("bytes_as_slice(packet.payload)"));
    assert!(expected.contains("consume(packet.right)"));
    assert!(expected.contains("let left_source ="));
    for module in ["app", "tests"] {
        assert_eq!(source(&base, module), source(&candidate, module));
    }
    let before: Value = serde_json::from_str(base.revision().semantic_graph()).unwrap();
    let after: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    assert_eq!(before["declarations"], after["declarations"]);
    assert_eq!(before["edges"], after["edges"]);
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
    assert_eq!(fixture.bytes(), disk);
}

fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    let request =
        json!({"jsonrpc":"2.0","id":"member","method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}

#[test]
fn existing_v5_catalog_apply_and_static_test_plan_support_members_without_widening_authority() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            test_policy: Some(CandidateTestPolicy::new(10_000, 4096, 16_384).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    let root = payload(call(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let schema = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.project-change-catalog.v1")
        .unwrap();
    let rename_schema = schema["properties"]["operations"]["items"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|shape| shape["properties"]["kind"]["const"] == "rename_declaration")
        .unwrap();
    assert_eq!(rename_schema["additionalProperties"], false);
    assert_eq!(
        rename_schema["properties"]["member_kind"]["enum"],
        json!(["record_field", "variant_case", "variant_field"])
    );
    assert!(!rename_schema["required"]
        .as_array()
        .unwrap()
        .contains(&json!("member_kind")));
    for target in ["members.public", "members.pair"] {
        let catalog = payload(call(
            &mut session,
            "change/catalog",
            json!({"candidate_revision":root,"target":target}),
        ));
        let legacy = catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["kind"] == "rename_declaration")
            .unwrap();
        assert!(legacy.get("member_kind").is_none());
    }
    for (target, name, member_kind) in [
        ("members.pair.value", "amount", "record_field"),
        ("members.choice.some", "Present", "variant_case"),
        ("members.choice.some.value", "amount", "variant_field"),
    ] {
        let catalog = payload(call(
            &mut session,
            "change/catalog",
            json!({"candidate_revision":root,"target":target}),
        ));
        let operation = catalog["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["kind"] == "rename_declaration")
            .unwrap();
        assert_eq!(
            operation["required_fields"],
            json!(["kind", "target", "name"])
        );
        assert_eq!(operation["member_kind"], member_kind);
        let expected = apply(&base, rename(target, name)).unwrap();
        let changed = payload(call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":root,"intent":rename(target,name)}),
        ));
        assert_eq!(changed["candidate_revision"], expected.candidate_digest());
        payload(call(
            &mut session,
            "candidate/validate",
            json!({"candidate_revision":expected.candidate_digest()}),
        ));
        let plan = payload(call(
            &mut session,
            "candidate/test-plan",
            json!({"candidate_revision":expected.candidate_digest()}),
        ));
        assert_eq!(plan["execution"], "not_run");
        assert_eq!(plan["selected"], true);
        assert!(plan["conservative_reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("non_callable_member_display_change")));
    }
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(
        call(
            &mut readonly,
            "candidate/apply-intent",
            json!({"candidate_revision":root,"intent":rename("members.pair.value","unauthorized")})
        )["error"]["code"],
        -32601
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
