use semaprax::project_transport::{
    generate_project_public_api_transport_client, project_public_api_transport_discovery,
    ProjectTransportClientLanguage, PROJECT_PUBLIC_API_DISCOVERY_SCHEMA,
    PROJECT_PUBLIC_API_TRANSPORT_SCHEMA,
};

#[test]
fn discovery_is_canonical_closed_and_matches_the_live_v6_inventory() {
    let source = project_public_api_transport_discovery();
    let value: serde_json::Value = serde_json::from_str(source).unwrap();
    assert!(source.starts_with(
        "{\"schema\":\"semaprax.project-agent-transport-v6-discovery.v1\",\"protocol\":"
    ));
    assert!(source.ends_with("}\n"));
    assert!(!source[..source.len() - 1].contains(['\n', '\r']));
    assert_eq!(value["schema"], PROJECT_PUBLIC_API_DISCOVERY_SCHEMA);
    assert_eq!(value["protocol"], PROJECT_PUBLIC_API_TRANSPORT_SCHEMA);
    assert_eq!(
        value["methods"],
        serde_json::json!(["project/api-describe", "project/npm-build-inline"])
    );
    assert_eq!(
        value["profiles"],
        serde_json::json!([
            {"project_schema":"semaprax.project.v8","descriptor_schema":"semaprax.public-owned-data-api.v1","carrier_schema":"semaprax.project-npm-build.v7"},
            {"project_schema":"semaprax.project.v9","descriptor_schema":"semaprax.public-flat-owned-record-api.v1","carrier_schema":"semaprax.project-npm-build.v8"},
            {"project_schema":"semaprax.project.v10","descriptor_schema":"semaprax.public-owned-utf8-api.v1","carrier_schema":"semaprax.project-npm-build.v9"},
            {"project_schema":"semaprax.project.v11","descriptor_schema":"semaprax.public-nested-owned-record-api.v1","carrier_schema":"semaprax.project-npm-build.v10"},
        ])
    );
    assert_eq!(value.as_object().unwrap().len(), 7);
}

#[test]
fn generated_clients_are_closed_codecs_not_transport_authority() {
    for language in [
        ProjectTransportClientLanguage::TypeScript,
        ProjectTransportClientLanguage::Python,
        ProjectTransportClientLanguage::Rust,
    ] {
        let source = generate_project_public_api_transport_client(language).unwrap();
        assert!(source.contains("project/api-describe"));
        assert!(source.contains("project/npm-build-inline"));
        assert!(source.contains("closed object mismatch"));
        assert!(source.contains("profile binding"));
        assert!(source.contains("response bound"));
        for forbidden in [
            "Command::new",
            "subprocess",
            "child_process",
            "process.env",
            "std::env",
            "std::fs",
            "TcpStream",
            "fetch(",
        ] {
            assert!(
                !source.contains(forbidden),
                "unexpected authority token {forbidden}"
            );
        }
    }
}
