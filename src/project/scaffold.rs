//! Authority-free preparation and replay of the built-in project templates.
//!
//! Version 2 adds `AGENTS.md`, the in-project guide for coding agents and
//! people, to every template; [Public Project Scaffold Capsule
//! v2](../../docs/PROJECT-SCAFFOLD-V2.md) owns the contract.
//!
//! The returned artifact is only checked bytes. It owns no filesystem,
//! process, environment, current-directory, target-emission, or publication
//! authority.

use serde_json::Value;

#[path = "scaffold/service_config.rs"]
mod service_config;
use sha2::{Digest, Sha256};

use crate::agent_skill_bundle::generate_agent_skill_bundle;
use crate::diagnostic::{quote_json, Diagnostic};

use super::service_host_adapter_request::ServiceHostAdapterRequestV1;
use super::{validate_owned_project_test, ProjectExecutionOptions, PROJECT_SCHEMA};

/// Decode a canonical service configuration and independently replay its
/// bounded host-adapter handoff. This returns requirements only: it never
/// resolves a secret, creates an outbound policy, grants a capability, or
/// opens an adapter.
pub fn derive_service_host_adapter_request_v1(
    configuration: &[u8],
) -> Result<ServiceHostAdapterRequestV1, String> {
    let configuration = service_config::decode(configuration)?;
    super::service_host_adapter_request::decode(configuration.adapter_request_bytes())
}

pub const PROJECT_SCAFFOLD_SCHEMA: &str = "semaprax.project-scaffold.v2";
/// Additive capsule schema for the extensible `semaprax.manifest.v1` table
/// manifest layout. The v2 schema is frozen to the v1 manifest bytes; a table
/// manifest is a distinct capsule so no v2 byte or digest changes.
pub const PROJECT_SCAFFOLD_SCHEMA_V3: &str = "semaprax.project-scaffold.v3";
/// Capsule schema for the Project v25 native stream-text template. Its
/// `project_schema` identifies v25 while v3 remains frozen to Project v1.
pub const PROJECT_SCAFFOLD_SCHEMA_V4: &str = "semaprax.project-scaffold.v4";
/// Capsule schema for the Project v26 native file-text source-command
/// template. Its digest domain is independent of every earlier capsule.
pub const PROJECT_SCAFFOLD_SCHEMA_V5: &str = "semaprax.project-scaffold.v5";
/// Capsule schema for the Project v27 native stream-data command template.
/// Its digest domain is independent of every earlier capsule.
pub const PROJECT_SCAFFOLD_SCHEMA_V6: &str = "semaprax.project-scaffold.v6";
pub const PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR: &str = "calculator";
pub const PROJECT_SCAFFOLD_TEMPLATE_LIBRARY: &str = "library";
/// A multi-user service composed from bounded bundled standard-library decision
/// layers, including `std.log`, `std.log.redact`, `std.tracing`, and `std.webhook`; see
/// [Project Scaffold Service Template v1](../../docs/PROJECT-SCAFFOLD-SERVICE-V1.md).
/// It only derives under [`ScaffoldLayout::Tables`]: the frozen
/// `semaprax.project.v1` layout has no `[dependencies]` table to declare them
/// in, so [`derive_project_scaffold_v1_with_layout`] refuses the
/// `Frozen`/`service` pairing before rendering anything.
pub const PROJECT_SCAFFOLD_TEMPLATE_SERVICE: &str = "service";
/// A native Project v25 command that consumes arbitrarily long standard input
/// through the bounded reusable stream reader and may pass owned Strings to
/// private module helpers.
pub const PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT: &str = "stdin-stream-text";
/// A native Project v27 command that keeps stream input bounded and shows a
/// private immutable borrowed `Vec<Copy scalar>` helper boundary.
pub const PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA: &str = "stdin-stream-data";
/// A native Project v26 command that reads one UTF-8 file and imports the
/// bundled `std.int.decimal` source package through an exact dependency.
pub const PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT: &str = "source-command-file-text";
pub const PROJECT_SCAFFOLD_TEMPLATES: [&str; 6] = [
    PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR,
    PROJECT_SCAFFOLD_TEMPLATE_LIBRARY,
    PROJECT_SCAFFOLD_TEMPLATE_SERVICE,
    PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT,
    PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA,
    PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT,
];
pub const PROJECT_SCAFFOLD_FILE_COUNT: usize = 5;
pub const PROJECT_SCAFFOLD_TABLES_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_LIBRARY_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_SERVICE_FILE_COUNT: usize = 9;
pub const PROJECT_SCAFFOLD_STDIN_STREAM_TEXT_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_STDIN_STREAM_DATA_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_SOURCE_COMMAND_FILE_TEXT_FILE_COUNT: usize = 6;
pub const MAX_PROJECT_SCAFFOLD_NAME_BYTES: usize = 64;
pub const MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES: usize = 65_536;

pub const PROJECT_SCAFFOLD_INVENTORY: [&str; PROJECT_SCAFFOLD_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/tests.spx",
];
pub const PROJECT_SCAFFOLD_TABLES_INVENTORY: [&str; PROJECT_SCAFFOLD_TABLES_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
/// The library template mirrors a standard-library package: one library
/// module, an examples module as the entry, and a conformance test module.
pub const PROJECT_SCAFFOLD_LIBRARY_INVENTORY: [&str; PROJECT_SCAFFOLD_LIBRARY_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/examples.spx",
    "src/lib.spx",
    "src/tests.spx",
];
/// The service template mirrors the calculator's table layout shape (a
/// separate `core` module) so it can carry a `[dependencies]` table; only its
/// semantic source shape. It additionally carries the closed host
/// configuration schema, a credential-free deterministic fixture instance, and
/// its explicit capability-free host-adapter request.
pub const PROJECT_SCAFFOLD_SERVICE_INVENTORY: [&str; PROJECT_SCAFFOLD_SERVICE_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
    "service-config.schema.json",
    "service.config.json",
    "service-host-adapter-request.json",
];
pub const PROJECT_SCAFFOLD_STDIN_STREAM_TEXT_INVENTORY: [&str;
    PROJECT_SCAFFOLD_STDIN_STREAM_TEXT_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/input.spx",
    "src/tests.spx",
];
pub const PROJECT_SCAFFOLD_STDIN_STREAM_DATA_INVENTORY: [&str;
    PROJECT_SCAFFOLD_STDIN_STREAM_DATA_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/input.spx",
    "src/tests.spx",
];
pub const PROJECT_SCAFFOLD_SOURCE_COMMAND_FILE_TEXT_INVENTORY: [&str;
    PROJECT_SCAFFOLD_SOURCE_COMMAND_FILE_TEXT_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/tests.spx",
    "digits",
];

/// The exact inventory of one built-in template.
#[must_use]
pub fn project_scaffold_inventory(template: &str) -> &'static [&'static str] {
    if template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY {
        &PROJECT_SCAFFOLD_LIBRARY_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SERVICE {
        &PROJECT_SCAFFOLD_SERVICE_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT {
        &PROJECT_SCAFFOLD_STDIN_STREAM_TEXT_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA {
        &PROJECT_SCAFFOLD_STDIN_STREAM_DATA_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT {
        &PROJECT_SCAFFOLD_SOURCE_COMMAND_FILE_TEXT_INVENTORY
    } else {
        &PROJECT_SCAFFOLD_INVENTORY
    }
}

/// The exact inventory for one template and manifest layout.
#[must_use]
pub fn project_scaffold_inventory_with_layout(
    template: &str,
    layout: ScaffoldLayout,
) -> &'static [&'static str] {
    if template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY {
        &PROJECT_SCAFFOLD_LIBRARY_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SERVICE {
        &PROJECT_SCAFFOLD_SERVICE_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT {
        &PROJECT_SCAFFOLD_STDIN_STREAM_TEXT_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA {
        &PROJECT_SCAFFOLD_STDIN_STREAM_DATA_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT {
        &PROJECT_SCAFFOLD_SOURCE_COMMAND_FILE_TEXT_INVENTORY
    } else if layout == ScaffoldLayout::Tables {
        &PROJECT_SCAFFOLD_TABLES_INVENTORY
    } else {
        &PROJECT_SCAFFOLD_INVENTORY
    }
}

const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v2\0";
const DIGEST_DOMAIN_V3: &[u8] = b"semaprax.project-scaffold.digest.v3\0";
const DIGEST_DOMAIN_V4: &[u8] = b"semaprax.project-scaffold.digest.v4\0";
const DIGEST_DOMAIN_V5: &[u8] = b"semaprax.project-scaffold.digest.v5\0";
const DIGEST_DOMAIN_V6: &[u8] = b"semaprax.project-scaffold.digest.v6\0";

/// Which `semaprax.toml` layout a scaffold emits. `Frozen` is the frozen
/// `semaprax.project.v1` line layout under capsule schema v2 (byte-identical to
/// the shipped default); `Tables` is the extensible `semaprax.manifest.v1`
/// table layout under capsule schema v3 for Project-v1 templates. The
/// `stdin-stream-text` template uses additive capsule v4 to identify its
/// Project-v25 manifest, and `source-command-file-text` uses v5 for Project
/// v26. The calculator table layout also demonstrates a stable-ID import from
/// a separate `core` module; frozen v2 inventory stays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScaffoldLayout {
    Frozen,
    Tables,
}

impl ScaffoldLayout {
    const fn schema(self) -> &'static str {
        match self {
            Self::Frozen => PROJECT_SCAFFOLD_SCHEMA,
            Self::Tables => PROJECT_SCAFFOLD_SCHEMA_V3,
        }
    }

    const fn digest_domain(self) -> &'static [u8] {
        match self {
            Self::Frozen => DIGEST_DOMAIN,
            Self::Tables => DIGEST_DOMAIN_V3,
        }
    }

    fn from_schema(schema: &str) -> Option<Self> {
        match schema {
            PROJECT_SCAFFOLD_SCHEMA => Some(Self::Frozen),
            PROJECT_SCAFFOLD_SCHEMA_V3
            | PROJECT_SCAFFOLD_SCHEMA_V4
            | PROJECT_SCAFFOLD_SCHEMA_V5
            | PROJECT_SCAFFOLD_SCHEMA_V6 => Some(Self::Tables),
            _ => None,
        }
    }
}

const README: &str = "# {{name}}\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build . --target web -o web\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const AGENTS: &str = "# Agent guide for {{name}}\n\nThis is a SEMAPRAX project. `semaprax.toml` lists its modules; the compiler\nis the authority on what the language admits. Before writing source, list\nthe language topics with `semaprax help language topics` and read only the one\nyou need with `semaprax help language <topic>`.\n\n## Commands\n\n- `semaprax check .` parses, resolves, type-checks, and verifies every module.\n- `semaprax test .` runs `{{module}}.tests`; `semaprax run .` runs the entry and prints its `i64`.\n- `semaprax fmt <file>` rewrites one file in canonical form.\n- `semaprax build . --target web -o dist/web` emits a browser package.\n- `semaprax help <command>` prints one command's exact grammar.\n\n## Rules that differ from other languages\n\n- Every file starts with `module dotted.name;`, and every declaration carries\n  `@id(\"...\")`. The id is the stable identity: rename freely, never change an id.\n- A function body is statements followed by exactly one tail expression. There\n  is no `return`, `break`, range `for`, `else if`, cast, tuple, or unit value.\n- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.\n- Contracts are `requires` and `ensures` lines; effects are `permit` at module\n  level plus `uses` on every function that performs or calls into one.\n- Check the whole project, not one file: modules import each other, so a\n  single file reports `SPX-G172` or `SPX-T105`.\n- A new module must be listed in `sources` in `semaprax.toml`, and a test\n  module in `tests`.\n- Tests live in the `tests` module: `fn main() -> i64` returns 0 on success, and\n  every `fn test_<name>() -> i64` with an `@id` runs as a named case that\n  `semaprax test .` reports on failure.\n- Diagnostics carry stable `SPX-` codes and, where the compiler knows the fix,\n  a `help:` line. Plain output is smaller than `--json`, which is for tools.\n";
const PROJECT_BOUNDARY_GUIDE: &str = "\n## Project v1 function boundaries\n\nFunction parameters and results are Copy scalars. Records, classes, variants,\n`Option`, and `Result` may stay inside scalar-signature functions but cannot\ncross their boundaries; `SPX-G174` points at a declaration that must change.\n";
const AGENT_SKILL_WORKFLOW_HEADER: &str = "\n## Installed Agent Skill workflow\n\nGenerated from `semaprax agent skill`; each verb names its authority class:\n\n";

/// Render the `## Installed Agent Skill workflow` section of `AGENTS.md` from
/// the real, installed `semaprax.agent-skill.v1` bundle
/// ([`generate_agent_skill_bundle`]) rather than restating its verbs by hand:
/// a verb added to, renamed in, or removed from
/// [`crate::agent_skill_bundle::PUBLIC_WORKFLOW`] changes this section on the
/// next scaffold derivation, with no second place to keep in sync.
///
/// Pure and deterministic: the bundle itself takes no path argument and
/// performs no I/O, and this function iterates its `public_workflow` array in
/// the bundle's own (sorted-by-verb) order, never a hash-keyed collection.
fn agent_skill_workflow_guide() -> Result<String, Vec<Diagnostic>> {
    let bundle = generate_agent_skill_bundle().map_err(|diagnostics| {
        scaffold_error(format!(
            "installed Agent Skill bundle failed to generate: {}",
            diagnostics
                .first()
                .map_or("unknown diagnostic", |diagnostic| diagnostic
                    .message
                    .as_str())
        ))
    })?;
    let value: Value = serde_json::from_str(&bundle)
        .map_err(|_| scaffold_error("installed Agent Skill bundle is not valid JSON"))?;
    let workflow = value
        .get("payload")
        .and_then(|payload| payload.get("public_workflow"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            scaffold_error("installed Agent Skill bundle is missing its public workflow")
        })?;
    let mut guide = AGENT_SKILL_WORKFLOW_HEADER.to_owned();
    for verb in workflow {
        let name = verb.get("verb").and_then(Value::as_str).ok_or_else(|| {
            scaffold_error("installed Agent Skill bundle workflow entry is missing its verb")
        })?;
        let authority_class = verb
            .get("authority_class")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                scaffold_error(
                    "installed Agent Skill bundle workflow entry is missing its authority class",
                )
            })?;
        let usage = verb
            .get("cli_usage")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                scaffold_error("installed Agent Skill bundle workflow entry is missing its usage")
            })?;
        guide.push_str(&format!("- `{name}` ({authority_class}): `{usage}`\n"));
    }
    Ok(guide)
}
const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.add\"]\ntests = [\"{{module}}.tests\"]\n";
const MANIFEST_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{name}}.add\"]\n";
const APP: &str = "module {{module}}.app;\n\n@id(\"{{name}}.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";
const APP_TABLES: &str = "module {{module}}.app;\nuse function @id(\"{{name}}.add\") from {{module}}.core as add;\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";
const CORE: &str = "module {{module}}.core;\n\n@id(\"{{name}}.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n";
const TESTS: &str = "module {{module}}.tests;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n";
const LIBRARY_README: &str = "# {{name}}\n\nA library package created by SEMAPRAX. `src/lib.spx` holds the public functions with their contracts, `src/examples.spx` is the entry that shows how to call them, and `src/tests.spx` is the conformance suite; both return `0` on success.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const LIBRARY_MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.examples\"\nsources = [\"src/examples.spx\", \"src/lib.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.twice\"]\ntests = [\"{{module}}.tests\"]\n";
const LIBRARY_MANIFEST_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"{{module}}.examples\"\nsources = [\"src/examples.spx\", \"src/lib.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{name}}.twice\"]\n";
const LIBRARY_EXAMPLES: &str = "module {{module}}.examples;\nuse function @id(\"{{name}}.twice\") from {{module}}.lib as twice;\n\n@id(\"{{name}}.examples.main\")\nfn main() -> i64\n{\n    if twice(21) == 42 { 0 } else { 1 }\n}\n";
const LIBRARY_LIB: &str = "module {{module}}.lib;\n\n@id(\"{{name}}.twice\")\nfn twice(value: i64) -> i64\n    requires value >= -4611686018427387904 && value <= 4611686018427387903\n    ensures result == value * 2\n{\n    value * 2\n}\n";
const LIBRARY_TESTS: &str = "module {{module}}.tests;\nuse function @id(\"{{name}}.twice\") from {{module}}.lib as twice;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    let mut failed = 0;\n    failed = failed + if twice(0) == 0 { 0 } else { 1 };\n    failed = failed + if twice(-3) == -6 { 0 } else { 2 };\n    failed\n}\n";
const SERVICE_README: &str = "# {{name}}\n\nA small multi-user task-tracking service created by SEMAPRAX, composed from ten bundled standard-library decision layers: register, log in, validate a request and migration, create a domain record, enqueue and complete a background job, admit a bounded redacted structured log, admit a bounded metric and exporter batch, admit an idempotent bounded webhook delivery, query its status, log out, and reject an unauthorized or invalid request. Every step is deterministic fixture mode -- no socket, no file, and no real clock are touched. `std.db`, `std.http`, `std.log`, `std.log.redact`, `std.metrics`, `std.export.policy`, `std.tracing`, and `std.webhook` contribute pure admission or shape checks only; they do not open storage, transport, or telemetry.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands, the rules that differ from other\nlanguages, and this template's bundled dependencies.\n";
const SERVICE_DEPENDENCY_GUIDE: &str = "\n## This template's dependencies\n\n`{{module}}.core` imports `std.auth` (session lifecycle, password-hash\npolicy bounds), `std.db` (identifier, migration, and transaction decisions),\n`std.http` (request-line admission), `std.jobs` (claim/lease/retry/idempotency\nstate machines), `std.log` (level admission), `std.log.redact` (bounded\nfield-count and protected-field admission), `std.metrics` (guarded\nmetric-series admission), `std.export.policy` (bounded batch and sink\nadmission), `std.tracing` (trace-context shape plus caller-classified secret\nsafety), and `std.webhook` (signature-envelope, replay-window, retry-bound,\nand caller-classified secret policy) by stable `@id`, declared in\n`semaprax.toml`'s `[dependencies]` table. All ten are pure decision layers:\nthey neither hash a password, open a socket or database, sign a payload,\nschedule a retry, write a log, nor emit a span. A real deployment performs\nclassification, hashing, signing, storage, transport, and any telemetry\nemission outside this decision.\n";
const SERVICE_MANIFEST_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{module}}.core.identifier_is_valid\", \"{{module}}.core.method_is_rejected\"]\n\n[dependencies]\nstd.auth = \"=0.1.0\"\nstd.db = \"=0.1.0\"\nstd.export.policy = \"=0.1.0\"\nstd.http = \"=0.1.0\"\nstd.jobs = \"=0.1.0\"\nstd.log = \"=0.1.0\"\nstd.log.redact = \"=0.1.0\"\nstd.metrics = \"=0.1.0\"\nstd.tracing = \"=0.1.0\"\nstd.webhook = \"=0.1.0\"\n";
const SERVICE_APP: &str = "module {{module}}.app;\nuse function @id(\"{{module}}.core.run_scenario\") from {{module}}.core as run_scenario;\n\n// The acceptance scenario this reference application exists to demonstrate:\n// register, log in, create/update a domain record (a task), enqueue a\n// background job, query its status, log out, then confirm that an\n// unauthorized session and an invalid request are both rejected. Every step\n// runs in deterministic fixture mode: no socket, no file, and no real clock\n// are touched, matching `std.auth` and `std.jobs`'s own non-claims (both are\n// pure decision layers with no host authority of their own). See\n// `{{module}}.core.run_scenario` for the full walk.\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    run_scenario()\n}\n";
const SERVICE_CORE: &str = include_str!("../../examples/task-service-project/src/core.spx");
const SERVICE_TESTS: &str = include_str!("../../examples/task-service-project/src/tests.spx");
const SERVICE_CONFIG_SCHEMA: &str =
    include_str!("../../examples/task-service-project/service-config.schema.json");
const SERVICE_CONFIG_FIXTURE: &str =
    include_str!("../../examples/task-service-project/service.config.json");
const SERVICE_ADAPTER_REQUEST_FIXTURE: &str =
    include_str!("../../examples/task-service-project/service-host-adapter-request.json");
const STDIN_STREAM_TEXT_GUIDE: &str = "\n## Streaming command\n\nThis project uses Project v25 profile `language-command-io.stream-text.v1` and\ninput `argv-utf8+stdin-stream.v1`. The manifest selects exactly one stable\n`command` export. Build it with `semaprax build --manifest-path semaprax.toml\n--target native --output app`, then run `./app`. `semaprax doctor --profile` reports compiler support; it does not select a Project command. `semaprax run .`\nruns the separate `main` entry. Web, Wasm, and npm targets are not admitted.\n\n`input.spx` opens one reusable 4096-byte reader. Process each borrowed chunk\ninside its block before calling `stdin_stream_next`; a short positive read is a\nchunk, and only a zero-length read is EOF. Its `normalize` helper demonstrates\na private owned-String call across modules.\n";
const STDIN_STREAM_TEXT_README: &str = "# {{name}}\n\nA native command project that reads standard input incrementally.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build --manifest-path semaprax.toml --target native --output app\n```\n\nThe generated `command` is selected by the Project manifest. `semaprax run .`\nexecutes the ordinary `main`; run `./app` to execute the streaming command.\nWeb, Wasm, and npm targets are refused for this profile. Read `AGENTS.md` before\nediting the source.\n";
const STDIN_STREAM_TEXT_MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"language-command-io.stream-text.v1\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/input.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{command}}\"]\n\n[command]\nfunction = \"{{command}}\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n";
const STDIN_STREAM_TEXT_APP: &str = "module {{module}}.app;\nuse function @id(\"{{name}}.read_stream\") from {{module}}.input as read_stream;\nuse function @id(\"{{name}}.normalize\") from {{module}}.input as normalize;\n\npermit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }\n\n@id(\"{{command}}\")\nfn command() -> i64\n    uses { process.stdin.read }\n{\n    let saw_chunk = read_stream();\n    let marker = normalize(\" ready \");\n    if string_len(marker) == 5 { if saw_chunk { 0 } else { 1 } } else { 1 }\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    let marker = normalize(\" ready \");\n    if string_len(marker) == 5 { 0 } else { 1 }\n}\n";
const STDIN_STREAM_TEXT_INPUT: &str = "module {{module}}.input;\n\npermit { process.stdin.read }\n\n@id(\"{{name}}.read_stream\")\nfn read_stream() -> bool\n    uses { process.stdin.read }\n{\n    let mut reader = stdin_stream_open();\n    let mut saw_chunk = false;\n    while !stdin_stream_eof(reader) {\n        let chunk_size = { let chunk = stdin_stream_chunk(reader); byte_len(chunk) };\n        saw_chunk = saw_chunk || chunk_size > 0usize;\n        reader = stdin_stream_next(reader);\n        0\n    }\n    saw_chunk\n}\n\n@id(\"{{name}}.normalize\")\nfn normalize(text: string) -> string\n{\n    string_trim(text)\n}\n";
const STDIN_STREAM_TEXT_TESTS: &str = "module {{module}}.tests;\nuse function @id(\"{{name}}.normalize\") from {{module}}.input as normalize;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    let marker = normalize(\" ready \");\n    if string_len(marker) == 5 { 0 } else { 1 }\n}\n";
const STDIN_STREAM_DATA_GUIDE: &str = "\n## Native stream-data command\n\nThis project selects Project v27 profile `language-command-io.stream-data.v1`\nand input `argv-utf8+stdin-stream.v1`. Build the selected command with\n`semaprax build --manifest-path semaprax.toml --target native --output app`,\nthen run `./app`. Web, Wasm, npm, and interpreter execution are not admitted.\n\n`input.spx` demonstrates one private helper that immutably borrows a\n`Vec<i64>`. It is a fixed-capacity sample, not an input buffer: process each\nborrowed stdin chunk in its block and keep any aggregate local to the command.\n";
const STDIN_STREAM_DATA_README: &str = "# {{name}}\n\nA native stream-data command project with a private borrowed scalar-vector\nhelper.\n\n```sh\nsemaprax check .\nsemaprax build --manifest-path semaprax.toml --target native --output app\n./app\n```\n\nThe generated `command` is selected by the Project manifest. Its vector is a\nsmall helper example, not a request to buffer standard input. Web, Wasm, npm,\nand interpreter execution are refused for this profile. Read `AGENTS.md` before\nediting the source.\n";
const STDIN_STREAM_DATA_MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"language-command-io.stream-data.v1\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/input.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{command}}\"]\n\n[command]\nfunction = \"{{command}}\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n";
const STDIN_STREAM_DATA_APP: &str = "module {{module}}.app;\nuse function @id(\"{{name}}.count\") from {{module}}.input as count;\n\npermit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }\n\n@id(\"{{command}}\")\nfn command() -> i64\n    uses { process.stdin.read }\n{\n    let mut reader = stdin_stream_open();\n    let mut saw_chunk = false;\n    while !stdin_stream_eof(reader) {\n        let chunk_size = { let chunk = stdin_stream_chunk(reader); byte_len(chunk) };\n        saw_chunk = saw_chunk || chunk_size > 0usize;\n        reader = stdin_stream_next(reader);\n        0\n    }\n    let values = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);\n    let observed = count(values);\n    if observed == 1usize { 0 } else { 1 }\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    let values = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);\n    if count(values) == 1usize { 0 } else { 1 }\n}\n";
const STDIN_STREAM_DATA_INPUT: &str = "module {{module}}.input;\n\n@id(\"{{name}}.count\")\nfn count(values: borrow Vec<i64>) -> usize\n{\n    let mut index = 0usize;\n    let mut total = 0;\n    while index < vec_len<i64>(values) {\n        total = total + vec_get<i64>(values, index);\n        index = index + 1usize;\n        0\n    }\n    if total == 7 { index } else { 0usize }\n}\n";
const STDIN_STREAM_DATA_TESTS: &str = "module {{module}}.tests;\nuse function @id(\"{{name}}.count\") from {{module}}.input as count;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    let values = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);\n    if count(values) == 1usize { 0 } else { 1 }\n}\n";
const SOURCE_COMMAND_FILE_TEXT_GUIDE: &str = "\n## Native file-text source command\n\nThis project selects Project v26 profile `source-command.v1`, input\n`argv-utf8+file-text.v1`, and native64 only. The command expects one path below\nthe current directory. Build and run the generated executable with:\n\n```sh\nsemaprax check .\nsemaprax build --manifest-path semaprax.toml --target native --output app\n./app digits\n```\n\n`std.int.decimal = \"=0.1.0\"` is an exact bundled dependency. Inspect its\nsmall API card with `semaprax help library std.int.decimal`; import functions by\nthe stable IDs shown there. This starter demonstrates `canonicalize`, `add`,\nand `divide` without copying decimal arithmetic into the application.\n\nThe interpreter run/test routes retain `SPX-F102`; use\n`semaprax test . --target native` for this profile's authenticated test roots.\nWeb, Wasm, and npm command targets refuse it. `SPX-G174`\nmeans a function signature is outside the selected profile: keep the aggregate\nlocal, use a carrier admitted by Project v26, or select the profile that owns\nthe intended boundary. For the exact `SPX-H006` message `function exceeds\n4,096 loan program points`, extract named helpers whose signatures remain\nadmitted by this profile; do not raise the verifier limit. Other `SPX-H006`\nmessages are closed trust-boundary refusals and require their stated fix.\n";
const SOURCE_COMMAND_FILE_TEXT_README: &str = "# {{name}}\n\nA native command project that reads a UTF-8 file and uses the bundled decimal\nsource package.\n\n```sh\nsemaprax check .\nsemaprax test . --target native\nsemaprax build --manifest-path semaprax.toml --target native --output app\n./app digits\n```\n\nThe command prints `(canonical decimal input + 1) / 3`. Plain `semaprax test .`\nselects the interpreter and retains `SPX-F102`; `--target native` runs the\nauthenticated test module with bounded native execution. Web, Wasm, and npm\ncommand targets are refused for this profile. Read `AGENTS.md` before editing\nthe source.\n";
const SOURCE_COMMAND_FILE_TEXT_MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"source-command.v1\"\n\n[modules]\nentry = \"{{module}}.command\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = []\n\n[command]\nfunction = \"{{command}}\"\ninput = \"argv-utf8+file-text.v1\"\n\n[capabilities]\nrequired = [\"fs.read\", \"process.args.read\", \"process.stderr.write\", \"process.stdout.write\"]\n\n[dependencies]\nstd.int.decimal = \"=0.1.0\"\n\n[targets]\nmatrix = [\"native64\"]\n";
const SOURCE_COMMAND_FILE_TEXT_APP: &str = "module {{module}}.command;\nuse function @id(\"std.int.decimal.canonicalize\") from std.int.decimal as canonicalize;\nuse function @id(\"std.int.decimal.add\") from std.int.decimal as add;\nuse function @id(\"std.int.decimal.divide\") from std.int.decimal as divide;\n\npermit { fs.read, process.args.read, process.stderr.write, process.stdout.write }\n\n@id(\"{{command}}\")\nfn main() -> i64\n    uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }\n{\n    if args_len() != 1usize { let usage = \"usage: app <relative-file>\\n\"; let view = string_as_str(usage); let ignored = stderr_write(str_as_bytes(view)); 2 } else { let path = arg_utf8(0usize); let input = file_read_text(path); let value = canonicalize(input); let sum = add(value, \"1\"); let divisor = \"3\"; let output = divide(sum, string_as_str(divisor)); let view = string_as_str(output); let ignored = stdout_write(str_as_bytes(view)); 0 }\n}\n";
const SOURCE_COMMAND_FILE_TEXT_TESTS: &str =
    "module {{module}}.tests;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";
const SOURCE_COMMAND_FILE_TEXT_DIGITS: &str = "000999999999999999999999";
const SERVICE_CONFIGURATION_GUIDE: &str = "\n## Host configuration\n\n`service-config.schema.json` is the closed host-configuration contract and\n`service.config.json` is its credential-free fixture instance. Database, HTTP,\nand telemetry adapters are explicitly `fixture`; endpoints and secret\nreferences are absent. `service-host-adapter-request.json` is the compiler\nrendered, bounded handoff for that fixture and declares an empty capability\nset. A host-mode configuration renders the exact snapshot-store, TLS-serve,\nsecret-resolve, and either signed JSON-event or OTLP/HTTP JSON telemetry\nselection it needs. Only TLS serve, secret resolve, and telemetry emit require\nhost capabilities; the declaration gains none of them, and a separately\nvalidated host must provide and execute every adapter outside Semaprax source.\n";
const NONCLAIMS: [&str; 4] = [
    "no_filesystem_or_publication_authority",
    "no_process_environment_or_current_directory_authority",
    "no_target_emission_or_runtime_claim",
    "no_release_or_host_support_claim",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectScaffoldFileV1 {
    path: &'static str,
    bytes: Vec<u8>,
    sha256: String,
}

impl ProjectScaffoldFileV1 {
    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn utf8(&self) -> &str {
        // Construction and replay admit only compiler-owned UTF-8 templates.
        std::str::from_utf8(&self.bytes).expect("Project scaffold invariant")
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectScaffoldV1 {
    template: &'static str,
    layout: ScaffoldLayout,
    schema: &'static str,
    project_schema: &'static str,
    project_name: String,
    files: Vec<ProjectScaffoldFileV1>,
    digest: String,
}

impl ProjectScaffoldV1 {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    #[must_use]
    pub const fn template(&self) -> &'static str {
        self.template
    }

    #[must_use]
    pub const fn project_schema(&self) -> &'static str {
        self.project_schema
    }

    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    #[must_use]
    pub fn files(&self) -> &[ProjectScaffoldFileV1] {
        &self.files
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        render_descriptor(self).into_bytes()
    }
}

/// Derive the exact built-in Project-v1 subject of one template in memory, in
/// the frozen `semaprax.project.v1` manifest layout under capsule schema v2.
pub fn derive_project_scaffold_v1(
    project_name: &str,
    template: &str,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    derive_project_scaffold_v1_with_layout(project_name, template, ScaffoldLayout::Frozen)
}

/// Derive a scaffold in the chosen manifest layout. `Frozen` is byte-identical
/// to [`derive_project_scaffold_v1`]; `Tables` uses the extensible
/// `semaprax.manifest.v1` layout and renders under capsule schema v3, except
/// the Project-v25 stream-text template, which uses capsule schema v4, and the
/// Project-v26 file-text source-command template, which uses capsule schema
/// v5, and the Project-v27 stream-data template uses capsule schema v6. The
/// calculator also demonstrates a cross-module stable-ID import.
pub fn derive_project_scaffold_v1_with_layout(
    project_name: &str,
    template: &str,
    layout: ScaffoldLayout,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    let template = validate_template(template)?;
    validate_project_name(project_name)?;
    let is_service = template == PROJECT_SCAFFOLD_TEMPLATE_SERVICE;
    let is_stdin_stream_text = template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT;
    let is_stdin_stream_data = template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA;
    let is_source_command_file_text =
        template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT;
    if is_service && layout == ScaffoldLayout::Frozen {
        return Err(scaffold_error(
            "the service template declares a [dependencies] table, so it needs the tables manifest layout; the frozen semaprax.project.v1 layout has no such table",
        ));
    }
    if is_stdin_stream_text && layout == ScaffoldLayout::Frozen {
        return Err(scaffold_error(
            "the stdin-stream-text template uses the [package], [command], and [capabilities] tables, so it needs the tables manifest layout",
        ));
    }
    if is_stdin_stream_data && layout == ScaffoldLayout::Frozen {
        return Err(scaffold_error(
            "the stdin-stream-data template uses the [package], [command], and [capabilities] tables, so it needs the tables manifest layout",
        ));
    }
    if is_source_command_file_text && layout == ScaffoldLayout::Frozen {
        return Err(scaffold_error(
            "the source-command-file-text template uses the [package], [command], [capabilities], [dependencies], and [targets] tables, so it needs the tables manifest layout",
        ));
    }
    let module = project_name.replace('-', "_");
    let command_id = if project_name.len() <= 24 {
        format!("{project_name}.command")
    } else {
        let digest = ordinary_sha256(project_name.as_bytes());
        format!("p{}.command", &digest[7..27])
    };
    if is_service {
        let decoded = service_config::decode(SERVICE_CONFIG_FIXTURE.as_bytes())
            .map_err(|message| scaffold_error(message))?;
        debug_assert_eq!(decoded.canonical_bytes(), SERVICE_CONFIG_FIXTURE.as_bytes());
        if decoded.adapter_request_bytes() != SERVICE_ADAPTER_REQUEST_FIXTURE.as_bytes() {
            return Err(scaffold_error(
                "service adapter request fixture does not match the canonical configuration handoff",
            ));
        }
    }
    let manifest = if is_source_command_file_text {
        SOURCE_COMMAND_FILE_TEXT_MANIFEST
    } else if is_stdin_stream_data {
        STDIN_STREAM_DATA_MANIFEST
    } else if is_stdin_stream_text {
        STDIN_STREAM_TEXT_MANIFEST
    } else if is_service {
        SERVICE_MANIFEST_TABLES
    } else {
        match (template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY, layout) {
            (true, ScaffoldLayout::Frozen) => LIBRARY_MANIFEST,
            (true, ScaffoldLayout::Tables) => LIBRARY_MANIFEST_TABLES,
            (false, ScaffoldLayout::Frozen) => MANIFEST,
            (false, ScaffoldLayout::Tables) => MANIFEST_TABLES,
        }
    };
    let sources: Vec<&str> = if is_source_command_file_text {
        vec![
            SOURCE_COMMAND_FILE_TEXT_README,
            AGENTS,
            manifest,
            SOURCE_COMMAND_FILE_TEXT_APP,
            SOURCE_COMMAND_FILE_TEXT_TESTS,
            SOURCE_COMMAND_FILE_TEXT_DIGITS,
        ]
    } else if is_stdin_stream_data {
        vec![
            STDIN_STREAM_DATA_README,
            AGENTS,
            manifest,
            STDIN_STREAM_DATA_APP,
            STDIN_STREAM_DATA_INPUT,
            STDIN_STREAM_DATA_TESTS,
        ]
    } else if is_stdin_stream_text {
        vec![
            STDIN_STREAM_TEXT_README,
            AGENTS,
            manifest,
            STDIN_STREAM_TEXT_APP,
            STDIN_STREAM_TEXT_INPUT,
            STDIN_STREAM_TEXT_TESTS,
        ]
    } else if is_service {
        vec![
            SERVICE_README,
            AGENTS,
            manifest,
            SERVICE_APP,
            SERVICE_CORE,
            SERVICE_TESTS,
            SERVICE_CONFIG_SCHEMA,
            SERVICE_CONFIG_FIXTURE,
            SERVICE_ADAPTER_REQUEST_FIXTURE,
        ]
    } else {
        match (template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY, layout) {
            (true, _) => vec![
                LIBRARY_README,
                AGENTS,
                manifest,
                LIBRARY_EXAMPLES,
                LIBRARY_LIB,
                LIBRARY_TESTS,
            ],
            (false, ScaffoldLayout::Frozen) => vec![README, AGENTS, manifest, APP, TESTS],
            (false, ScaffoldLayout::Tables) => {
                vec![README, AGENTS, manifest, APP_TABLES, CORE, TESTS]
            }
        }
    };
    let inventory = project_scaffold_inventory_with_layout(template, layout);
    debug_assert_eq!(sources.len(), inventory.len());
    let agent_skill_workflow_guide = agent_skill_workflow_guide()?;
    let files = sources
        .iter()
        .zip(inventory)
        .map(|(source, path)| {
            let mut combined = (*source).to_owned();
            if is_service && matches!(*path, "src/core.spx" | "src/tests.spx") {
                // The checked-in reference is the source of truth for the
                // service's two evolving semantic modules. Turn its concrete
                // package/module spellings back into template placeholders
                // before the ordinary rendering pass below, so generated and
                // reference projects cannot drift by a copied implementation.
                combined = combined
                    .replace("task-service", "{{name}}")
                    .replace("task_service", "{{module}}");
            }
            if *path == "AGENTS.md" && layout == ScaffoldLayout::Tables {
                combined.push_str(PROJECT_BOUNDARY_GUIDE);
            }
            if *path == "AGENTS.md" && is_service {
                combined.push_str(SERVICE_DEPENDENCY_GUIDE);
            }
            if *path == "AGENTS.md" && is_stdin_stream_text {
                combined = combined
                    .replace(
                        "- `semaprax build . --target web -o dist/web` emits a browser package.",
                        "- `semaprax build --manifest-path semaprax.toml --target native --output app` builds the command.",
                    )
                    .replace(
                        "is no `return`, `break`, range `for`, `else if`, cast, tuple, or unit value.",
                        "is no `return`, `break`, range `for`, cast, tuple, or unit value.",
                    )
                    .replace(
                        "- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.",
                        "- Value `if` requires `else`; statement `if` permits an omitted `else` and `else if`.\n  A `while` condition decides whether to loop; its body ends with a tail expression.",
                    );
                combined.push_str(STDIN_STREAM_TEXT_GUIDE);
            }
            if *path == "AGENTS.md" && is_stdin_stream_data {
                combined = combined.replace(
                    "- `semaprax build . --target web -o dist/web` emits a browser package.",
                    "- `semaprax build --manifest-path semaprax.toml --target native --output app` builds the command.",
                );
                combined.push_str(STDIN_STREAM_DATA_GUIDE);
            }
            if *path == "AGENTS.md" && is_source_command_file_text {
                combined = combined.replace(
                    "- `semaprax build . --target web -o dist/web` emits a browser package.",
                    "- `semaprax build --manifest-path semaprax.toml --target native --output app` builds the command.",
                );
                combined.push_str(SOURCE_COMMAND_FILE_TEXT_GUIDE);
            }
            if *path == "README.md" && is_service {
                combined.push_str(SERVICE_CONFIGURATION_GUIDE);
            }
            if *path == "AGENTS.md" {
                combined.push_str(&agent_skill_workflow_guide);
            }
            let rendered = combined
                .replace("{{name}}", project_name)
                .replace("{{module}}", &module)
                .replace("{{command}}", &command_id);
            let bytes = rendered.into_bytes();
            ProjectScaffoldFileV1 {
                path,
                sha256: ordinary_sha256(&bytes),
                bytes,
            }
        })
        .collect::<Vec<_>>();
    validate_rendered_project(template, &files)?;
    let mut artifact = ProjectScaffoldV1 {
        template,
        layout,
        schema: capsule_schema(template, layout),
        project_schema: project_schema(template),
        project_name: project_name.to_owned(),
        files,
        digest: String::new(),
    };
    artifact.digest = artifact_digest(
        template,
        layout,
        &render_descriptor_without_digest(&artifact),
    );
    if artifact.canonical_bytes().len() > MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES {
        return Err(capacity(
            "project scaffold descriptor exceeds its exact byte limit",
        ));
    }
    Ok(artifact)
}

/// Replay submitted bytes against the exact selected name and built-in template.
pub fn replay_project_scaffold_v1(
    project_name: &str,
    template: &str,
    descriptor_bytes: &[u8],
    digest: &str,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    let template = validate_template(template)?;
    validate_project_name(project_name)?;
    if descriptor_bytes.len() > MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES {
        return Err(capacity(
            "project scaffold descriptor exceeds its exact byte limit",
        ));
    }
    let value: Value = serde_json::from_slice(descriptor_bytes)
        .map_err(|_| scaffold_error("project scaffold descriptor JSON is invalid"))?;
    let layout = value
        .as_object()
        .and_then(|root| root.get("schema"))
        .and_then(Value::as_str)
        .and_then(ScaffoldLayout::from_schema)
        .ok_or_else(|| scaffold_error("project scaffold descriptor schema is unknown"))?;
    let root = value
        .as_object()
        .filter(|root| {
            root.len() == 8
                && root.get("schema").and_then(Value::as_str)
                    == Some(capsule_schema(template, layout))
                && root.get("digest").and_then(Value::as_str).is_some()
                && root.get("template").and_then(Value::as_str) == Some(template)
                && root.get("project_schema").and_then(Value::as_str)
                    == Some(project_schema(template))
                && root.get("project_name").and_then(Value::as_str).is_some()
                && root.get("files").and_then(Value::as_array).is_some()
                && root.get("limits").and_then(Value::as_object).is_some()
                && root.get("nonclaims").and_then(Value::as_array).is_some()
        })
        .ok_or_else(|| scaffold_error("project scaffold descriptor root is not closed"))?;
    if root.keys().any(|key| {
        !matches!(
            key.as_str(),
            "schema"
                | "digest"
                | "template"
                | "project_schema"
                | "project_name"
                | "files"
                | "limits"
                | "nonclaims"
        )
    }) {
        return Err(scaffold_error(
            "project scaffold descriptor contains an unknown field",
        ));
    }
    if root.get("project_name").and_then(Value::as_str) != Some(project_name) {
        return Err(scaffold_error(
            "project scaffold descriptor does not bind the selected project name",
        ));
    }
    if root.get("digest").and_then(Value::as_str) != Some(digest) {
        return Err(scaffold_error(
            "project scaffold descriptor digest does not match the submitted digest",
        ));
    }
    let rebuilt = derive_project_scaffold_v1_with_layout(project_name, template, layout)?;
    if digest != rebuilt.digest() || descriptor_bytes != rebuilt.canonical_bytes().as_slice() {
        return Err(scaffold_error(
            "project scaffold descriptor does not replay against the built-in template",
        ));
    }
    Ok(rebuilt)
}

fn validate_template(template: &str) -> Result<&'static str, Vec<Diagnostic>> {
    PROJECT_SCAFFOLD_TEMPLATES
        .into_iter()
        .find(|known| *known == template)
        .ok_or_else(|| {
            scaffold_error(format!(
                "unknown project scaffold template; expected {}",
                PROJECT_SCAFFOLD_TEMPLATES.join(" or ")
            ))
        })
}

fn validate_project_name(project_name: &str) -> Result<(), Vec<Diagnostic>> {
    if project_name.len() > MAX_PROJECT_SCAFFOLD_NAME_BYTES {
        return Err(capacity(format!(
            "project scaffold name exceeds {MAX_PROJECT_SCAFFOLD_NAME_BYTES} bytes"
        )));
    }
    if !project_name.is_empty()
        && project_name.as_bytes()[0].is_ascii_lowercase()
        && project_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Ok(())
    } else {
        Err(scaffold_error(
            "project scaffold name must match lowercase [a-z][a-z0-9-]*",
        ))
    }
}

fn validate_rendered_project(
    template: &str,
    files: &[ProjectScaffoldFileV1],
) -> Result<(), Vec<Diagnostic>> {
    let manifest = files[2].utf8();
    let sources = files
        .iter()
        .filter(|file| file.path.ends_with(".spx"))
        .map(|file| (file.path, file.utf8()))
        .collect::<Vec<_>>();
    let execution = match validate_owned_project_test(
        manifest,
        &sources,
        &ProjectExecutionOptions::default(),
    ) {
        Ok(execution) => execution,
        Err(diagnostics)
            if matches!(
                template,
                PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT
                    | PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA
            ) && diagnostics.len() == 1
                && diagnostics[0].code == "SPX-F102" =>
        {
            // `validate_owned_project_test` checks the full revision before it
            // asks the interpreter to execute tests. Projects v26 and v27
            // deliberately refuse that interpreter route; their executable
            // gates are native.
            return Ok(());
        }
        Err(diagnostics) => {
            return Err(scaffold_error(format!(
                "built-in {template} project failed exact check or test: {}",
                diagnostics
                    .first()
                    .map_or("unknown diagnostic", |diagnostic| {
                        diagnostic.message.as_str()
                    })
            )));
        }
    };
    if execution.command_succeeded() {
        Ok(())
    } else {
        Err(scaffold_error(format!(
            "built-in {template} project tests did not pass"
        )))
    }
}

fn render_descriptor(artifact: &ProjectScaffoldV1) -> String {
    let body = render_descriptor_tail(artifact);
    format!(
        "{{\"schema\":{},\"digest\":{},{}",
        quote_json(artifact.schema()),
        quote_json(&artifact.digest),
        &body[1..]
    )
}

fn render_descriptor_without_digest(artifact: &ProjectScaffoldV1) -> String {
    let body = render_descriptor_tail(artifact);
    format!(
        "{{\"schema\":{},{}",
        quote_json(artifact.schema()),
        &body[1..]
    )
}

fn render_descriptor_tail(artifact: &ProjectScaffoldV1) -> String {
    let files = artifact
        .files
        .iter()
        .map(|file| {
            format!(
                "{{\"path\":{},\"utf8\":{},\"sha256\":{}}}",
                quote_json(file.path),
                quote_json(file.utf8()),
                quote_json(&file.sha256)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let nonclaims = NONCLAIMS
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .join(",");
    let file_limit = artifact.files.len();
    format!(
        "{{\"template\":{},\"project_schema\":{},\"project_name\":{},\"files\":[{}],\"limits\":{{\"descriptor_bytes\":{},\"files\":{},\"project_name_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(artifact.template),
        quote_json(artifact.project_schema()),
        quote_json(&artifact.project_name),
        files,
        MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES,
        file_limit,
        MAX_PROJECT_SCAFFOLD_NAME_BYTES,
        nonclaims,
    )
}

fn capsule_schema(template: &str, layout: ScaffoldLayout) -> &'static str {
    if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA {
        PROJECT_SCAFFOLD_SCHEMA_V6
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT {
        PROJECT_SCAFFOLD_SCHEMA_V5
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT {
        PROJECT_SCAFFOLD_SCHEMA_V4
    } else {
        layout.schema()
    }
}

fn project_schema(template: &str) -> &'static str {
    if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA {
        super::PROJECT_SCHEMA_V27
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT {
        super::PROJECT_SCHEMA_V26
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT {
        super::PROJECT_SCHEMA_V25
    } else {
        PROJECT_SCHEMA
    }
}

fn artifact_digest(
    template: &str,
    layout: ScaffoldLayout,
    canonical_without_digest: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_DATA {
        DIGEST_DOMAIN_V6
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SOURCE_COMMAND_FILE_TEXT {
        DIGEST_DOMAIN_V5
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_STDIN_STREAM_TEXT {
        DIGEST_DOMAIN_V4
    } else {
        layout.digest_domain()
    });
    hash.update((canonical_without_digest.len() as u64).to_le_bytes());
    hash.update(canonical_without_digest.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn ordinary_sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn scaffold_error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J115", message)]
}

fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J116", message)]
}
