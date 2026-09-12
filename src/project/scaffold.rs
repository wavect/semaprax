//! Authority-free preparation and replay of the built-in project templates:
//! the calculator application and the library package.
//!
//! Version 2 adds `AGENTS.md`, the in-project guide for coding agents and
//! people, to every template; [Public Project Scaffold Capsule
//! v2](../../docs/PROJECT-SCAFFOLD-V2.md) owns the contract.
//!
//! The returned artifact is only checked bytes. It owns no filesystem,
//! process, environment, current-directory, target-emission, or publication
//! authority.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agent_skill_bundle::generate_agent_skill_bundle;
use crate::diagnostic::{quote_json, Diagnostic};

use super::{validate_owned_project_test, ProjectExecutionOptions, PROJECT_SCHEMA};

pub const PROJECT_SCAFFOLD_SCHEMA: &str = "semaprax.project-scaffold.v2";
/// Additive capsule schema for the extensible `semaprax.manifest.v1` table
/// manifest layout. The v2 schema is frozen to the v1 manifest bytes; a table
/// manifest is a distinct capsule so no v2 byte or digest changes.
pub const PROJECT_SCAFFOLD_SCHEMA_V3: &str = "semaprax.project-scaffold.v3";
pub const PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR: &str = "calculator";
pub const PROJECT_SCAFFOLD_TEMPLATE_LIBRARY: &str = "library";
/// A multi-user service composed from two bundled standard-library decision
/// layers (`std.auth`, `std.jobs`); see
/// [Project Scaffold Service Template v1](../../docs/PROJECT-SCAFFOLD-SERVICE-V1.md).
/// It only derives under [`ScaffoldLayout::Tables`]: the frozen
/// `semaprax.project.v1` layout has no `[dependencies]` table to declare them
/// in, so [`derive_project_scaffold_v1_with_layout`] refuses the
/// `Frozen`/`service` pairing before rendering anything.
pub const PROJECT_SCAFFOLD_TEMPLATE_SERVICE: &str = "service";
pub const PROJECT_SCAFFOLD_TEMPLATES: [&str; 3] = [
    PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR,
    PROJECT_SCAFFOLD_TEMPLATE_LIBRARY,
    PROJECT_SCAFFOLD_TEMPLATE_SERVICE,
];
pub const PROJECT_SCAFFOLD_FILE_COUNT: usize = 5;
pub const PROJECT_SCAFFOLD_TABLES_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_LIBRARY_FILE_COUNT: usize = 6;
pub const PROJECT_SCAFFOLD_SERVICE_FILE_COUNT: usize = 6;
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
/// file contents, not its file names, differ from the calculator's own
/// `Tables` inventory.
pub const PROJECT_SCAFFOLD_SERVICE_INVENTORY: [&str; PROJECT_SCAFFOLD_SERVICE_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

/// The exact inventory of one built-in template.
#[must_use]
pub fn project_scaffold_inventory(template: &str) -> &'static [&'static str] {
    if template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY {
        &PROJECT_SCAFFOLD_LIBRARY_INVENTORY
    } else if template == PROJECT_SCAFFOLD_TEMPLATE_SERVICE {
        &PROJECT_SCAFFOLD_SERVICE_INVENTORY
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
    } else if layout == ScaffoldLayout::Tables {
        &PROJECT_SCAFFOLD_TABLES_INVENTORY
    } else {
        &PROJECT_SCAFFOLD_INVENTORY
    }
}

const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v2\0";
const DIGEST_DOMAIN_V3: &[u8] = b"semaprax.project-scaffold.digest.v3\0";

/// Which `semaprax.toml` layout a scaffold emits. `Frozen` is the frozen
/// `semaprax.project.v1` line layout under capsule schema v2 (byte-identical to
/// the shipped default); `Tables` is the extensible `semaprax.manifest.v1`
/// table layout under capsule schema v3. Both lower to the same Project v1
/// contract. The calculator table layout also demonstrates a stable-ID import
/// from a separate `core` module; the frozen v2 inventory remains unchanged.
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
            PROJECT_SCAFFOLD_SCHEMA_V3 => Some(Self::Tables),
            _ => None,
        }
    }
}

const README: &str = "# {{name}}\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build . --target web -o web\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const AGENTS: &str = "# Agent guide for {{name}}\n\nThis is a SEMAPRAX project. `semaprax.toml` lists its modules; the compiler\nis the authority on what the language admits. Read `semaprax help language`\nbefore writing source.\n\n## Commands\n\n- `semaprax check .` parses, resolves, type-checks, and verifies every module.\n- `semaprax test .` runs `{{module}}.tests`; `semaprax run .` runs the entry and prints its `i64`.\n- `semaprax fmt <file>` rewrites one file in canonical form.\n- `semaprax build . --target web -o dist/web` emits a browser package.\n- `semaprax help <command>` prints one command's exact grammar.\n\n## Rules that differ from other languages\n\n- Every file starts with `module dotted.name;`, and every declaration carries\n  `@id(\"...\")`. The id is the stable identity: rename freely, never change an id.\n- A function body is statements followed by exactly one tail expression. There\n  is no `return`, `for`, `else if`, tuple, or unit value.\n- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.\n- Contracts are `requires` and `ensures` lines; effects are `permit` at module\n  level plus `uses` on every function that performs or calls into one.\n- Check the whole project, not one file: modules import each other, so a\n  single file reports `SPX-G172` or `SPX-T105`.\n- A new module must be listed in `sources` in `semaprax.toml`, and a test\n  module in `tests`.\n- Tests live in the `tests` module: `fn main() -> i64` returns 0 on success, and\n  every `fn test_<name>() -> i64` with an `@id` runs as a named case that\n  `semaprax test .` reports on failure.\n- Diagnostics carry stable `SPX-` codes and, where the compiler knows the fix,\n  a `help:` line. `semaprax check . --json` prints one diagnostic per line.\n";
const PROJECT_BOUNDARY_GUIDE: &str = "\n## Project v1 function boundaries\n\nFunction parameters and results are Copy scalars. Records, classes, variants,\n`Option`, and `Result` may stay inside scalar-signature functions but cannot\ncross their boundaries; `SPX-G174` points at a declaration that must change.\n";
const AGENT_SKILL_WORKFLOW_HEADER: &str = "\n## Installed Agent Skill workflow\n\nThe installed compiler also publishes `semaprax.agent-skill.v1` (`semaprax\nagent skill`), a small, authority-labeled public workflow over the commands\nabove. The list below is generated from that installed bundle, so it always\nmatches this compiler; each entry names its authority class and the exact\nCLI command it wraps:\n\n";

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
const SERVICE_README: &str = "# {{name}}\n\nA small multi-user task-tracking service created by SEMAPRAX, composed from two bundled standard-library decision layers: register, log in, create a domain record, enqueue and complete a background job, query its status, log out, and reject an unauthorized or invalid request. Every step is deterministic fixture mode -- no socket, no file, and no real clock are touched.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands, the rules that differ from other\nlanguages, and this template's two bundled dependencies.\n";
const SERVICE_DEPENDENCY_GUIDE: &str = "\n## This template's dependencies\n\n`{{module}}.core` imports `std.auth` (session lifecycle, password-hash\npolicy bounds) and `std.jobs` (claim/lease/retry/idempotency state\nmachines) by stable `@id`, declared in `semaprax.toml`'s `[dependencies]`\ntable. Both are pure decision layers: neither hashes a password nor opens a\nsocket, database, or job queue -- a real deployment performs hashing,\nsigning, storage, and transport outside this decision.\n";
const SERVICE_MANIFEST_TABLES: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{name}}.identifier_is_valid\", \"{{name}}.method_is_rejected\"]\n\n[dependencies]\nstd.auth = \"=0.1.0\"\nstd.jobs = \"=0.1.0\"\n";
const SERVICE_APP: &str = "module {{module}}.app;\nuse function @id(\"{{name}}.run_scenario\") from {{module}}.core as run_scenario;\n\n// The acceptance scenario this reference application exists to demonstrate:\n// register, log in, create/update a domain record (a task), enqueue a\n// background job, query its status, log out, then confirm that an\n// unauthorized session and an invalid request are both rejected. Every step\n// runs in deterministic fixture mode: no socket, no file, and no real clock\n// are touched, matching `std.auth` and `std.jobs`'s own non-claims (both are\n// pure decision layers with no host authority of their own). See\n// `{{name}}.run_scenario` for the full walk.\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    run_scenario()\n}\n";
const SERVICE_CORE: &str = "module {{module}}.core;\nuse function @id(\"std.auth.password.policy_within_bounds\") from std.auth as password_policy_within_bounds;\nuse function @id(\"std.auth.session.is_usable\") from std.auth as session_is_usable;\nuse function @id(\"std.auth.session.next_state_on_access\") from std.auth as session_next_state_on_access;\nuse function @id(\"std.auth.session.next_state_on_logout\") from std.auth as session_next_state_on_logout;\nuse function @id(\"std.jobs.claim.is_legal\") from std.jobs as claim_is_legal;\nuse function @id(\"std.jobs.idempotency.enqueue_outcome\") from std.jobs as idempotency_enqueue_outcome;\nuse function @id(\"std.jobs.retry.next_state_after_outcome\") from std.jobs as retry_next_state_after_outcome;\nuse function @id(\"std.jobs.schedule.is_due\") from std.jobs as schedule_is_due;\nuse function @id(\"std.jobs.state.is_terminal\") from std.jobs as state_is_terminal;\n\n// This module models the domain record (a task) and its background job as\n// plain scalar facts -- an id, an owner account id, a status -- rather than\n// an authored `record`, deliberately: `Public Useful Data Export v1` (the\n// `[exports].web` gate every `semaprax.manifest.v1` project must satisfy)\n// admits no authored aggregate anywhere in a project that also declares a\n// web export, and every dependency-consuming project needs the manifest.v1\n// schema.\n// A closed safe-identifier grammar for table names and usernames: a letter\n// or underscore, then letters, digits, or underscores, 1..63 bytes. This\n// mirrors `std.db.identifier.is_valid`'s grammar (kept local here rather\n// than as a dependency -- see `examples/task-service-project/README.md` for\n// the `SPX-G171` workspace-graph budget a third dependency would reach).\n@id(\"{{name}}.identifier_byte_ok\")\nfn identifier_byte_ok(byte: u8) -> bool\n{\n    byte >= 65u8 && byte <= 90u8 || byte >= 97u8 && byte <= 122u8 || byte >= 48u8 && byte <= 57u8 || byte == 95u8\n}\n\n@id(\"{{name}}.identifier_is_valid\")\nfn identifier_is_valid(name: borrow Slice<u8>) -> bool\n{\n    let length = byte_len(name);\n    let first_ok = match byte_get(name, 0usize) { Option::Some { value } => identifier_byte_ok(value) && !(value >= 48u8 && value <= 57u8), Option::None {} => false, };\n    let mut index = 1usize;\n    let mut valid = length >= 1usize && length <= 63usize && first_ok;\n    while index < length && valid {\n        valid = match byte_get(name, index) { Option::Some { value } => identifier_byte_ok(value), Option::None {} => false, };\n        index = index + 1usize;\n        index < length && valid\n    }\n    valid\n}\n\n// An HTTP method is one to sixteen uppercase ASCII letters, mirroring\n// `std.http.method_is_valid`'s grammar.\n@id(\"{{name}}.method_is_valid\")\nfn method_is_valid(method: borrow Slice<u8>) -> bool\n{\n    let length = byte_len(method);\n    let mut index = 0usize;\n    let mut valid = length >= 1usize && length <= 16usize;\n    while index < length && valid {\n        valid = match byte_get(method, index) { Option::Some { value } => value >= 65u8 && value <= 90u8, Option::None {} => false, };\n        index = index + 1usize;\n        index < length && valid\n    }\n    valid\n}\n\n@id(\"{{name}}.method_is_rejected\")\nfn method_is_rejected(method: borrow Slice<u8>) -> bool\n{\n    !method_is_valid(method)\n}\n\n// Registration is a pure admission decision over caller-supplied facts: no\n// password is hashed here (see `docs/AUTHENTICATION-SESSIONS-V1.md`'s\n// non-claims -- this bounded interpreter has no hashing host capability),\n// and no row is written. A deployment's own host code performs both after\n// this decision admits the request.\n@id(\"{{name}}.registration_admitted\")\nfn registration_admitted(username: borrow Slice<u8>, active_count: usize, max_accounts: usize, password_memory_cost_kib: usize, password_time_cost: usize, password_parallelism: usize) -> bool\n{\n    identifier_is_valid(username) && active_count < max_accounts && password_policy_within_bounds(password_memory_cost_kib, password_time_cost, password_parallelism)\n}\n\n// Row-level authorization: a usable session is necessary but not\n// sufficient -- the session's own account must also own the row.\n@id(\"{{name}}.task_owner_authorized\")\nfn task_owner_authorized(task_owner_account_id: i64, session_account_id: i64, session_usable: bool) -> bool\n{\n    session_usable && task_owner_account_id == session_account_id\n}\n\n@id(\"{{name}}.enqueue_is_legal\")\nfn enqueue_is_legal(pending_state: usize, now_tick: usize, next_run_tick: usize) -> bool\n{\n    claim_is_legal(pending_state, schedule_is_due(now_tick, next_run_tick))\n}\n\n// A caller-supplied idempotency key decides whether a retried enqueue\n// request is fresh (0), a harmless duplicate (1), or a conflicting reuse of\n// the same key for different work (2) -- see `std.jobs.idempotency`.\n@id(\"{{name}}.enqueue_outcome\")\nfn enqueue_outcome(key_exists: bool, existing_descriptor: borrow Slice<u8>, candidate_descriptor: borrow Slice<u8>) -> usize\n{\n    idempotency_enqueue_outcome(key_exists, existing_descriptor, candidate_descriptor)\n}\n\n@id(\"{{name}}.mark_job_succeeded\")\nfn mark_job_succeeded(attempt: u8, max_attempts: u8) -> usize\n{\n    retry_next_state_after_outcome(0usize, attempt, max_attempts)\n}\n\n@id(\"{{name}}.job_status_is_complete\")\nfn job_status_is_complete(state: usize) -> bool\n{\n    state_is_terminal(state)\n}\n\n// The full acceptance scenario: register, log in, create/update the domain\n// record, enqueue and complete a background job, query its status, log out,\n// then confirm both an unauthorized access and an invalid request are\n// rejected. Returns 0 on success, 1 otherwise.\n@id(\"{{name}}.run_scenario\")\nfn run_scenario() -> i64\n{\n    let idle_deadline = 1900usize;\n    let absolute_deadline = 5000usize;\n    let login_tick = 1000usize;\n    let update_tick = 1200usize;\n    let logout_tick = 1201usize;\n    let username = [97u8, 108u8, 105u8, 99u8, 101u8];\n    let registered = registration_admitted(array_as_slice(username), 0usize, 10usize, 65536usize, 3usize, 4usize);\n    let active = 0usize;\n    let owner_account_id = 1;\n    let usable_at_login = session_is_usable(active, login_tick, idle_deadline, absolute_deadline);\n    let table_name = [116u8, 97u8, 115u8, 107u8, 115u8];\n    let table_ok = identifier_is_valid(array_as_slice(table_name));\n    let task_id = 1;\n    let state_at_update = session_next_state_on_access(active, update_tick, idle_deadline, absolute_deadline);\n    let session_usable_at_update = session_is_usable(state_at_update, update_tick, idle_deadline, absolute_deadline);\n    let update_authorized = task_owner_authorized(owner_account_id, owner_account_id, session_usable_at_update);\n    let in_progress_status = 1usize;\n    let job_key = [1u8, 4u8, 3u8];\n    let enqueue_legal = enqueue_is_legal(0usize, update_tick, update_tick);\n    let enqueue_outcome_code = enqueue_outcome(false, array_as_slice(job_key), array_as_slice(job_key));\n    let finished_job_state = mark_job_succeeded(0u8, 3u8);\n    let status_done = job_status_is_complete(finished_job_state);\n    let logged_out = session_next_state_on_logout(state_at_update);\n    let usable_after_logout = session_is_usable(logged_out, logout_tick, idle_deadline, absolute_deadline);\n    let intruder_account_id = 2;\n    let intruder_rejected = !task_owner_authorized(owner_account_id, intruder_account_id, true);\n    let retired_session_rejected = !usable_after_logout;\n    let bad_method = [103u8, 101u8, 116u8];\n    let method_rejected = method_is_rejected(array_as_slice(bad_method));\n    let bad_identifier = [49u8, 116u8, 97u8, 115u8, 107u8];\n    let identifier_rejected = !identifier_is_valid(array_as_slice(bad_identifier));\n    if registered && usable_at_login && table_ok && task_id == 1 && update_authorized && in_progress_status == 1usize && enqueue_legal && enqueue_outcome_code == 0usize && status_done && intruder_rejected && retired_session_rejected && method_rejected && identifier_rejected { 0 } else { 1 }\n}\n";
const SERVICE_TESTS: &str = "module {{module}}.tests;\nuse function @id(\"{{name}}.enqueue_outcome\") from {{module}}.core as enqueue_outcome;\nuse function @id(\"{{name}}.identifier_is_valid\") from {{module}}.core as identifier_is_valid;\nuse function @id(\"{{name}}.job_status_is_complete\") from {{module}}.core as job_status_is_complete;\nuse function @id(\"{{name}}.mark_job_succeeded\") from {{module}}.core as mark_job_succeeded;\nuse function @id(\"{{name}}.method_is_rejected\") from {{module}}.core as method_is_rejected;\nuse function @id(\"{{name}}.registration_admitted\") from {{module}}.core as registration_admitted;\nuse function @id(\"{{name}}.run_scenario\") from {{module}}.core as run_scenario;\nuse function @id(\"{{name}}.task_owner_authorized\") from {{module}}.core as task_owner_authorized;\n\n// The whole acceptance scenario succeeds end to end.\n@id(\"{{name}}.tests.scenario_succeeds\")\nfn scenario_succeeds() -> bool\n{\n    run_scenario() == 0\n}\n\n// Success: registration, ownership, and job completion all admit on\n// well-formed input.\n@id(\"{{name}}.tests.success_path\")\nfn success_path() -> bool\n{\n    let username = [98u8, 111u8, 98u8];\n    let job_state = mark_job_succeeded(0u8, 3u8);\n    let table_name = [116u8, 97u8, 115u8, 107u8, 115u8];\n    registration_admitted(array_as_slice(username), 2usize, 10usize, 65536usize, 3usize, 4usize) && task_owner_authorized(3, 3, true) && job_status_is_complete(job_state) && identifier_is_valid(array_as_slice(table_name))\n}\n\n// Reject unauthorized: a session bound to a different account is refused\n// row access even though the row itself exists.\n@id(\"{{name}}.tests.unauthorized_is_rejected\")\nfn unauthorized_is_rejected() -> bool\n{\n    !task_owner_authorized(3, 4, true)\n}\n\n// Reject invalid: a malformed HTTP method and an unsafe table identifier\n// (leading digit) are both refused before touching storage or routing.\n@id(\"{{name}}.tests.invalid_is_rejected\")\nfn invalid_is_rejected() -> bool\n{\n    let bad_method = [103u8, 101u8, 116u8];\n    let bad_identifier = [57u8, 120u8];\n    method_is_rejected(array_as_slice(bad_method)) && !identifier_is_valid(array_as_slice(bad_identifier))\n}\n\n// A retried enqueue with the same idempotency key is a harmless duplicate\n// (outcome 1), never a second job.\n@id(\"{{name}}.tests.duplicate_enqueue_is_idempotent\")\nfn duplicate_enqueue_is_idempotent() -> bool\n{\n    let key = [1u8, 4u8, 3u8];\n    enqueue_outcome(true, array_as_slice(key), array_as_slice(key)) == 1usize\n}\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    if scenario_succeeds() && success_path() && unauthorized_is_rejected() && invalid_is_rejected() && duplicate_enqueue_is_idempotent() { 0 } else { 1 }\n}\n";
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
    project_name: String,
    files: Vec<ProjectScaffoldFileV1>,
    digest: String,
}

impl ProjectScaffoldV1 {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.layout.schema()
    }

    #[must_use]
    pub const fn template(&self) -> &'static str {
        self.template
    }

    #[must_use]
    pub const fn project_schema(&self) -> &'static str {
        PROJECT_SCHEMA
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
/// `semaprax.manifest.v1` layout and renders under capsule schema v3. Its
/// calculator also demonstrates a cross-module stable-ID import. Both lower
/// to the same Project v1 contract.
pub fn derive_project_scaffold_v1_with_layout(
    project_name: &str,
    template: &str,
    layout: ScaffoldLayout,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    let template = validate_template(template)?;
    validate_project_name(project_name)?;
    let is_service = template == PROJECT_SCAFFOLD_TEMPLATE_SERVICE;
    if is_service && layout == ScaffoldLayout::Frozen {
        return Err(scaffold_error(
            "the service template declares a [dependencies] table, so it needs the tables manifest layout; the frozen semaprax.project.v1 layout has no such table",
        ));
    }
    let module = project_name.replace('-', "_");
    let manifest = if is_service {
        SERVICE_MANIFEST_TABLES
    } else {
        match (template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY, layout) {
            (true, ScaffoldLayout::Frozen) => LIBRARY_MANIFEST,
            (true, ScaffoldLayout::Tables) => LIBRARY_MANIFEST_TABLES,
            (false, ScaffoldLayout::Frozen) => MANIFEST,
            (false, ScaffoldLayout::Tables) => MANIFEST_TABLES,
        }
    };
    let sources: Vec<&str> = if is_service {
        vec![
            SERVICE_README,
            AGENTS,
            manifest,
            SERVICE_APP,
            SERVICE_CORE,
            SERVICE_TESTS,
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
            if *path == "AGENTS.md" && layout == ScaffoldLayout::Tables {
                combined.push_str(PROJECT_BOUNDARY_GUIDE);
            }
            if *path == "AGENTS.md" && is_service {
                combined.push_str(SERVICE_DEPENDENCY_GUIDE);
            }
            if *path == "AGENTS.md" {
                combined.push_str(&agent_skill_workflow_guide);
            }
            let rendered = combined
                .replace("{{name}}", project_name)
                .replace("{{module}}", &module);
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
        project_name: project_name.to_owned(),
        files,
        digest: String::new(),
    };
    artifact.digest = artifact_digest(layout, &render_descriptor_without_digest(&artifact));
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
                && root.get("schema").and_then(Value::as_str) == Some(layout.schema())
                && root.get("digest").and_then(Value::as_str).is_some()
                && root.get("template").and_then(Value::as_str) == Some(template)
                && root.get("project_schema").and_then(Value::as_str) == Some(PROJECT_SCHEMA)
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
    let sources = files[3..]
        .iter()
        .map(|file| (file.path, file.utf8()))
        .collect::<Vec<_>>();
    let execution =
        validate_owned_project_test(manifest, &sources, &ProjectExecutionOptions::default())
            .map_err(|diagnostics| {
                scaffold_error(format!(
                    "built-in {template} project failed exact check or test: {}",
                    diagnostics
                        .first()
                        .map_or("unknown diagnostic", |diagnostic| diagnostic
                            .message
                            .as_str())
                ))
            })?;
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
        quote_json(artifact.layout.schema()),
        quote_json(&artifact.digest),
        &body[1..]
    )
}

fn render_descriptor_without_digest(artifact: &ProjectScaffoldV1) -> String {
    let body = render_descriptor_tail(artifact);
    format!(
        "{{\"schema\":{},{}",
        quote_json(artifact.layout.schema()),
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
    let file_limit = if artifact.layout == ScaffoldLayout::Tables {
        PROJECT_SCAFFOLD_TABLES_FILE_COUNT
    } else {
        PROJECT_SCAFFOLD_FILE_COUNT
    };
    format!(
        "{{\"template\":{},\"project_schema\":{},\"project_name\":{},\"files\":[{}],\"limits\":{{\"descriptor_bytes\":{},\"files\":{},\"project_name_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(artifact.template),
        quote_json(PROJECT_SCHEMA),
        quote_json(&artifact.project_name),
        files,
        MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES,
        file_limit,
        MAX_PROJECT_SCAFFOLD_NAME_BYTES,
        nonclaims,
    )
}

fn artifact_digest(layout: ScaffoldLayout, canonical_without_digest: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(layout.digest_domain());
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
