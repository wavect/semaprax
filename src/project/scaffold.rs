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

use crate::diagnostic::{quote_json, Diagnostic};

use super::{validate_owned_project_test, ProjectExecutionOptions, PROJECT_SCHEMA};

pub const PROJECT_SCAFFOLD_SCHEMA: &str = "semaprax.project-scaffold.v2";
pub const PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR: &str = "calculator";
pub const PROJECT_SCAFFOLD_TEMPLATE_LIBRARY: &str = "library";
pub const PROJECT_SCAFFOLD_TEMPLATES: [&str; 2] = [
    PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR,
    PROJECT_SCAFFOLD_TEMPLATE_LIBRARY,
];
pub const PROJECT_SCAFFOLD_FILE_COUNT: usize = 5;
pub const PROJECT_SCAFFOLD_LIBRARY_FILE_COUNT: usize = 6;
pub const MAX_PROJECT_SCAFFOLD_NAME_BYTES: usize = 64;
pub const MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES: usize = 65_536;

pub const PROJECT_SCAFFOLD_INVENTORY: [&str; PROJECT_SCAFFOLD_FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
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

/// The exact inventory of one built-in template.
#[must_use]
pub fn project_scaffold_inventory(template: &str) -> &'static [&'static str] {
    if template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY {
        &PROJECT_SCAFFOLD_LIBRARY_INVENTORY
    } else {
        &PROJECT_SCAFFOLD_INVENTORY
    }
}

const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v2\0";
const README: &str = "# {{name}}\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\nsemaprax build . --target web -o web\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const AGENTS: &str = "# Agent guide for {{name}}\n\nThis is a SEMAPRAX project. `semaprax.toml` lists its modules; the compiler\nis the authority on what the language admits. Read `semaprax help language`\nbefore writing source.\n\n## Commands\n\n- `semaprax check .` parses, resolves, type-checks, and verifies every module.\n- `semaprax test .` runs `{{module}}.tests`; `semaprax run .` runs the entry and prints its `i64`.\n- `semaprax fmt <file>` rewrites one file in canonical form.\n- `semaprax build . --target web -o dist/web` emits a browser package.\n- `semaprax help <command>` prints one command's exact grammar.\n\n## Rules that differ from other languages\n\n- Every file starts with `module dotted.name;`, and every declaration carries\n  `@id(\"...\")`. The id is the stable identity: rename freely, never change an id.\n- A function body is statements followed by exactly one tail expression. There\n  is no `return`, `for`, `else if`, tuple, or unit value.\n- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.\n- Contracts are `requires` and `ensures` lines; effects are `permit` at module\n  level plus `uses` on every function that performs or calls into one.\n- Check the whole project, not one file: modules import each other, so a\n  single file reports `SPX-G172` or `SPX-T105`.\n- A new module must be listed in `sources` in `semaprax.toml`, and a test\n  module in `tests`.\n- Diagnostics carry stable `SPX-` codes and, where the compiler knows the fix,\n  a `help:` line. `semaprax check . --json` prints one diagnostic per line.\n";
const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.add\"]\ntests = [\"{{module}}.tests\"]\n";
const APP: &str = "module {{module}}.app;\n\n@id(\"{{name}}.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";
const TESTS: &str = "module {{module}}.tests;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n";
const LIBRARY_README: &str = "# {{name}}\n\nA library package created by SEMAPRAX. `src/lib.spx` holds the public functions with their contracts, `src/examples.spx` is the entry that shows how to call them, and `src/tests.spx` is the conformance suite; both return `0` on success.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax run .\n```\n\nRead `AGENTS.md` before editing the source, whether you are a person or a\ncoding agent: it lists the commands and the rules that differ from other\nlanguages.\n";
const LIBRARY_MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.examples\"\nsources = [\"src/examples.spx\", \"src/lib.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.twice\"]\ntests = [\"{{module}}.tests\"]\n";
const LIBRARY_EXAMPLES: &str = "module {{module}}.examples;\nuse function @id(\"{{name}}.twice\") from {{module}}.lib as twice;\n\n@id(\"{{name}}.examples.main\")\nfn main() -> i64\n{\n    if twice(21) == 42 { 0 } else { 1 }\n}\n";
const LIBRARY_LIB: &str = "module {{module}}.lib;\n\n@id(\"{{name}}.twice\")\nfn twice(value: i64) -> i64\n    requires value >= -4611686018427387904 && value <= 4611686018427387903\n    ensures result == value * 2\n{\n    value * 2\n}\n";
const LIBRARY_TESTS: &str = "module {{module}}.tests;\nuse function @id(\"{{name}}.twice\") from {{module}}.lib as twice;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    let mut failed = 0;\n    failed = failed + if twice(0) == 0 { 0 } else { 1 };\n    failed = failed + if twice(-3) == -6 { 0 } else { 2 };\n    failed\n}\n";
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
    project_name: String,
    files: Vec<ProjectScaffoldFileV1>,
    digest: String,
}

impl ProjectScaffoldV1 {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        PROJECT_SCAFFOLD_SCHEMA
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

/// Derive the exact built-in Project-v1 subject of one template in memory.
pub fn derive_project_scaffold_v1(
    project_name: &str,
    template: &str,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    let template = validate_template(template)?;
    validate_project_name(project_name)?;
    let module = project_name.replace('-', "_");
    let sources: &[&str] = if template == PROJECT_SCAFFOLD_TEMPLATE_LIBRARY {
        &[
            LIBRARY_README,
            AGENTS,
            LIBRARY_MANIFEST,
            LIBRARY_EXAMPLES,
            LIBRARY_LIB,
            LIBRARY_TESTS,
        ]
    } else {
        &[README, AGENTS, MANIFEST, APP, TESTS]
    };
    let inventory = project_scaffold_inventory(template);
    debug_assert_eq!(sources.len(), inventory.len());
    let files = sources
        .iter()
        .zip(inventory)
        .map(|(source, path)| {
            let bytes = source
                .replace("{{name}}", project_name)
                .replace("{{module}}", &module)
                .into_bytes();
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
        project_name: project_name.to_owned(),
        files,
        digest: String::new(),
    };
    artifact.digest = artifact_digest(&render_descriptor_without_digest(&artifact));
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
    let root = value
        .as_object()
        .filter(|root| {
            root.len() == 8
                && root.get("schema").and_then(Value::as_str) == Some(PROJECT_SCAFFOLD_SCHEMA)
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
    let rebuilt = derive_project_scaffold_v1(project_name, template)?;
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
            scaffold_error("unknown project scaffold template; expected calculator or library")
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
            .map_err(|_| {
                scaffold_error(format!(
                    "built-in {template} project failed exact check or test"
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
        quote_json(PROJECT_SCAFFOLD_SCHEMA),
        quote_json(&artifact.digest),
        &body[1..]
    )
}

fn render_descriptor_without_digest(artifact: &ProjectScaffoldV1) -> String {
    let body = render_descriptor_tail(artifact);
    format!(
        "{{\"schema\":{},{}",
        quote_json(PROJECT_SCAFFOLD_SCHEMA),
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
    format!(
        "{{\"template\":{},\"project_schema\":{},\"project_name\":{},\"files\":[{}],\"limits\":{{\"descriptor_bytes\":{},\"files\":{},\"project_name_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(artifact.template),
        quote_json(PROJECT_SCHEMA),
        quote_json(&artifact.project_name),
        files,
        MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES,
        PROJECT_SCAFFOLD_FILE_COUNT,
        MAX_PROJECT_SCAFFOLD_NAME_BYTES,
        nonclaims,
    )
}

fn artifact_digest(canonical_without_digest: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(DIGEST_DOMAIN);
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
