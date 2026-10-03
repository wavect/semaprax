use super::*;

#[test]
fn opencode_repair_configuration_requires_explicit_host_provider_operands() {
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.opencode-denial.v1");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["schema"] = serde_json::json!("semaprax.source-live-cli.repair-config.v2");
    value.as_object_mut().unwrap().remove("turns");
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let error = run_repair("run", &config, &checkpoint)
        .expect_err("the production repair configuration must not select an implicit provider");
    assert!(error.reason.contains("requires --opencode"));
    assert!(
        !checkpoint.exists(),
        "provider authority refusal must happen before a checkpoint exists"
    );
}
