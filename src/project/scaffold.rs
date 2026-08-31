//! Authority-free preparation and replay of the built-in calculator project.
//!
//! The returned artifact is only checked bytes. It owns no filesystem,
//! process, environment, current-directory, target-emission, or publication
//! authority.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::{validate_owned_project_test, ProjectExecutionOptions, PROJECT_SCHEMA};

pub const PROJECT_SCAFFOLD_SCHEMA: &str = "semaprax.project-scaffold.v1";
pub const PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR: &str = "calculator";
pub const PROJECT_SCAFFOLD_FILE_COUNT: usize = 4;
pub const MAX_PROJECT_SCAFFOLD_NAME_BYTES: usize = 64;
pub const MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES: usize = 65_536;

pub const PROJECT_SCAFFOLD_INVENTORY: [&str; PROJECT_SCAFFOLD_FILE_COUNT] =
    ["README.md", "semaprax.toml", "src/app.spx", "src/tests.spx"];

const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v1\0";
const README: &str = "# {{name}}\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check semaprax.toml\nsemaprax test semaprax.toml\nsemaprax run semaprax.toml\nsemaprax build semaprax.toml --target web -o web\n```\n";
const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.add\"]\ntests = [\"{{module}}.tests\"]\n";
const APP: &str = "module {{module}}.app;\n\n@id(\"{{name}}.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";
const TESTS: &str = "module {{module}}.tests;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n";
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
    project_name: String,
    files: [ProjectScaffoldFileV1; PROJECT_SCAFFOLD_FILE_COUNT],
    digest: String,
}

impl ProjectScaffoldV1 {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        PROJECT_SCAFFOLD_SCHEMA
    }

    #[must_use]
    pub const fn template(&self) -> &'static str {
        PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR
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
    pub fn files(&self) -> &[ProjectScaffoldFileV1; PROJECT_SCAFFOLD_FILE_COUNT] {
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

/// Derive the exact built-in Project-v1 calculator subject in memory.
pub fn derive_project_scaffold_v1(
    project_name: &str,
    template: &str,
) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    validate_template(template)?;
    validate_project_name(project_name)?;
    let module = project_name.replace('-', "_");
    let rendered = [README, MANIFEST, APP, TESTS].map(|source| {
        source
            .replace("{{name}}", project_name)
            .replace("{{module}}", &module)
            .into_bytes()
    });
    let files = std::array::from_fn(|index| ProjectScaffoldFileV1 {
        path: PROJECT_SCAFFOLD_INVENTORY[index],
        sha256: ordinary_sha256(&rendered[index]),
        bytes: rendered[index].clone(),
    });
    validate_rendered_project(&files)?;
    let mut artifact = ProjectScaffoldV1 {
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
    validate_template(template)?;
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
                && root.get("template").and_then(Value::as_str)
                    == Some(PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR)
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

fn validate_template(template: &str) -> Result<(), Vec<Diagnostic>> {
    if template == PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR {
        Ok(())
    } else {
        Err(scaffold_error(
            "unknown project scaffold template; expected calculator",
        ))
    }
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
    files: &[ProjectScaffoldFileV1; PROJECT_SCAFFOLD_FILE_COUNT],
) -> Result<(), Vec<Diagnostic>> {
    let manifest = files[1].utf8();
    let app = files[2].utf8();
    let tests = files[3].utf8();
    let execution = validate_owned_project_test(
        manifest,
        &[(files[2].path, app), (files[3].path, tests)],
        &ProjectExecutionOptions::default(),
    )
    .map_err(|_| scaffold_error("built-in calculator project failed exact check or test"))?;
    if execution.command_succeeded() {
        Ok(())
    } else {
        Err(scaffold_error(
            "built-in calculator project tests did not pass",
        ))
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
        quote_json(PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR),
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
