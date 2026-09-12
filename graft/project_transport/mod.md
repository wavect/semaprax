# project_transport/mod.rs

- codec · module · L7-L7 — pub(crate) mod codec;
- config · module · L8-L8 — mod config;
- framing · module · L9-L9 — pub(crate) mod framing;
- sdk · module · L10-L10 — mod sdk;
- session · module · L11-L11 — mod session;
- TRANSPORT_SCHEMA · constant · L21-L21 — pub const TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v2";
- PROJECT_RENAME_TRANSPORT_SCHEMA · constant · L23-L23 — pub const PROJECT_RENAME_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v3";
- PROJECT_WORKFLOW_TRANSPORT_SCHEMA · constant · L25-L25 — pub const PROJECT_WORKFLOW_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v4";
- PROJECT_OWNED_DATA_TRANSPORT_SCHEMA · constant · L27-L27 — pub const PROJECT_OWNED_DATA_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v5";
- PROJECT_PUBLIC_API_TRANSPORT_SCHEMA · constant · L29-L29 — pub const PROJECT_PUBLIC_API_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v6";
- run_from_args · function · L34-L39 — pub fn run_from_args(arguments: impl IntoIterator<Item = OsString>) -> Result<(), String>
