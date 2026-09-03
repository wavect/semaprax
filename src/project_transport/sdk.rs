//! Authority-free, out-of-band discovery and generated codecs for Transport v6.

pub const PROJECT_PUBLIC_API_DISCOVERY_SCHEMA: &str =
    "semaprax.project-agent-transport-v6-discovery.v1";
const MAX_CLIENT_BYTES: usize = 256 * 1024;

const DISCOVERY: &str = concat!(
    "{\"schema\":\"semaprax.project-agent-transport-v6-discovery.v1\",",
    "\"protocol\":\"semaprax.agent-transport.v6\",",
    "\"methods\":[\"project/api-describe\",\"project/npm-build-inline\"],",
    "\"profiles\":[",
    "{\"project_schema\":\"semaprax.project.v8\",\"descriptor_schema\":\"semaprax.public-owned-data-api.v1\",\"carrier_schema\":\"semaprax.project-npm-build.v7\"},",
    "{\"project_schema\":\"semaprax.project.v9\",\"descriptor_schema\":\"semaprax.public-flat-owned-record-api.v1\",\"carrier_schema\":\"semaprax.project-npm-build.v8\"},",
    "{\"project_schema\":\"semaprax.project.v10\",\"descriptor_schema\":\"semaprax.public-owned-utf8-api.v1\",\"carrier_schema\":\"semaprax.project-npm-build.v9\"},",
    "{\"project_schema\":\"semaprax.project.v11\",\"descriptor_schema\":\"semaprax.public-nested-owned-record-api.v1\",\"carrier_schema\":\"semaprax.project-npm-build.v10\"}],",
    "\"limits\":{\"default_request_bytes\":65536,\"maximum_request_bytes\":1048576,\"default_response_bytes\":1048576,\"maximum_response_bytes\":16777216,\"maximum_inline_carrier_bytes\":41943040},",
    "\"clients\":[\"typescript\",\"python\",\"rust\"],",
    "\"nonclaims\":[\"caller_supplies_stdio_transport\",\"no_path_tool_environment_or_network_discovery\",\"no_process_launch_or_filesystem_authority\",\"no_source_workspace_or_publication_mutation\",\"outer_transport_validation_not_descriptor_semantic_replay\"]}\n"
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTransportClientLanguage {
    TypeScript,
    Python,
    Rust,
}

pub fn project_public_api_transport_discovery() -> &'static str {
    DISCOVERY
}

pub fn generate_project_public_api_transport_client(
    language: ProjectTransportClientLanguage,
) -> Result<String, &'static str> {
    let body = match language {
        ProjectTransportClientLanguage::TypeScript => include_str!("sdk/typescript.txt"),
        ProjectTransportClientLanguage::Python => include_str!("sdk/python.txt"),
        ProjectTransportClientLanguage::Rust => include_str!("sdk/rust.txt"),
    };
    let source = body.replace("__DISCOVERY__", &format!("{DISCOVERY:?}"));
    if source.len() > MAX_CLIENT_BYTES {
        Err("generated Transport v6 client exceeds its fixed byte bound")
    } else {
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_is_closed_canonical_and_matches_frozen_transport() {
        let value: serde_json::Value = serde_json::from_str(DISCOVERY).unwrap();
        assert!(DISCOVERY.starts_with(
            "{\"schema\":\"semaprax.project-agent-transport-v6-discovery.v1\",\"protocol\":"
        ));
        assert!(DISCOVERY.ends_with("}\n"));
        assert!(!DISCOVERY[..DISCOVERY.len() - 1].contains(['\n', '\r']));
        assert_eq!(value["schema"], PROJECT_PUBLIC_API_DISCOVERY_SCHEMA);
        assert_eq!(
            value["protocol"],
            super::super::PROJECT_PUBLIC_API_TRANSPORT_SCHEMA
        );
        assert_eq!(
            value["methods"],
            serde_json::json!(["project/api-describe", "project/npm-build-inline"])
        );
        assert_eq!(value["profiles"].as_array().unwrap().len(), 4);
        assert_eq!(value["nonclaims"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn all_clients_are_deterministic_bounded_and_authority_free() {
        for language in [
            ProjectTransportClientLanguage::TypeScript,
            ProjectTransportClientLanguage::Python,
            ProjectTransportClientLanguage::Rust,
        ] {
            let first = generate_project_public_api_transport_client(language).unwrap();
            assert_eq!(
                first,
                generate_project_public_api_transport_client(language).unwrap()
            );
            assert!(first.len() <= MAX_CLIENT_BYTES);
            assert!(first.contains(PROJECT_PUBLIC_API_DISCOVERY_SCHEMA));
            for forbidden in [
                "Command::new",
                "subprocess",
                "child_process",
                "fetch(",
                "std::fs",
                "open(",
                "process.env",
                "std::env",
            ] {
                assert!(
                    !first.contains(forbidden),
                    "generated client contains {forbidden}"
                );
            }
        }
    }
}
