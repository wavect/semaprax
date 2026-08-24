//! Deterministic npm facade for the Useful Text Consumer v1 project profile.
//!
//! Rendering is authority-neutral: callers provide authenticated Wasm bytes
//! and the already-admitted public ABI. This module neither reads nor writes a
//! path and never launches npm, Node, or another process.

use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::{ProjectManifest, PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1};

pub(crate) const USEFUL_TEXT_PACKAGE_PATHS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.text-exports.json",
    "package.json",
];

const MAX_WASM_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXPORTS: usize = 32;
const MAX_PARAMETERS: usize = 8;

pub const PROJECT_NPM_BUILD_SCHEMA: &str = "semaprax.project-npm-build.v1";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-npm-build.payload.v1\0";
pub const MAX_PROJECT_NPM_BUILD_BYTES: usize = 40 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextPackageType {
    Str,
    I64,
    Bool,
}

impl TextPackageType {
    fn json(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    fn typescript(self) -> &'static str {
        match self {
            Self::Str => "string",
            Self::I64 => "bigint",
            Self::Bool => "boolean",
        }
    }
}

/// One authenticated public adapter fact supplied by the Wasm profile
/// planner. Stable IDs, symbols, and types are rendered without inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextPackageExport {
    stable_id: String,
    wasm_export: String,
    parameters: Vec<TextPackageType>,
    result: TextPackageType,
}

impl TextPackageExport {
    pub(crate) fn new(
        stable_id: String,
        wasm_export: String,
        parameters: Vec<TextPackageType>,
        result: TextPackageType,
    ) -> Self {
        Self {
            stable_id,
            wasm_export,
            parameters,
            result,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NpmArtifact {
    path: &'static str,
    bytes: Vec<u8>,
}

impl NpmArtifact {
    pub(crate) fn path(&self) -> &'static str {
        self.path
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact six-file, pathless npm package inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsefulTextNpmPackage {
    artifacts: [NpmArtifact; 6],
}

/// Canonical, replayable carrier for one exact six-file npm package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNpmBuild {
    envelope: String,
    payload_digest: String,
    artifact_bytes: usize,
    max_bytes: usize,
    trusted: TrustedNpmBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrustedNpmBinding {
    project_schema: String,
    package: String,
    version: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    semantic_recipe: String,
}

struct ReplayedNpmEnvelope {
    canonical: String,
    payload_digest: String,
    artifact_bytes: usize,
    trusted: TrustedNpmBinding,
}

impl ProjectNpmBuild {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn artifact_bytes(&self) -> usize {
        self.artifact_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn verify(&self) -> Result<(), Diagnostic> {
        let replayed = Self::replay_envelope(&self.envelope, self.max_bytes)?;
        if replayed.payload_digest != self.payload_digest
            || replayed.artifact_bytes != self.artifact_bytes
            || replayed.canonical != self.envelope
            || replayed.trusted != self.trusted
        {
            return Err(package_error(
                "npm build disagrees with its context-bound trusted Project facts",
            ));
        }
        Ok(())
    }

    /// Inspect an untrusted serialized envelope for canonical compiler
    /// consistency. Success does not authenticate its claimed Project facts,
    /// construct a publishable build, or grant publication authority.
    pub fn inspect_envelope(envelope: &str, max_bytes: usize) -> Result<(), Diagnostic> {
        Self::replay_envelope(envelope, max_bytes).map(|_| ())
    }

    fn replay_envelope(
        envelope: &str,
        max_bytes: usize,
    ) -> Result<ReplayedNpmEnvelope, Diagnostic> {
        validate_carrier_limit(envelope.len(), max_bytes)?;
        let value: serde_json::Value = serde_json::from_str(envelope)
            .map_err(|_| package_error("npm build envelope is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| package_error("npm build envelope must be one JSON object"))?;
        require_exact_keys(
            object,
            &[
                "artifact_bytes",
                "artifacts",
                "package",
                "payload_digest",
                "project_graph_digest",
                "project_revision",
                "project_schema",
                "schema",
                "semantic_recipe",
                "version",
                "workspace_revision",
            ],
        )?;
        let schema = json_string(object, "schema")?;
        if schema != PROJECT_NPM_BUILD_SCHEMA {
            return Err(package_error("npm build schema is unsupported"));
        }
        let identity = NpmBuildIdentity {
            project_schema: json_string(object, "project_schema")?,
            package: json_string(object, "package")?,
            version: json_string(object, "version")?,
            project_revision: json_string(object, "project_revision")?,
            workspace_revision: json_string(object, "workspace_revision")?,
            project_graph_digest: json_string(object, "project_graph_digest")?,
            semantic_recipe: json_string(object, "semantic_recipe")?,
        };
        for value in [
            identity.project_schema,
            identity.package,
            identity.version,
            identity.project_revision,
            identity.workspace_revision,
            identity.project_graph_digest,
        ] {
            if value.is_empty() || value.len() > 512 {
                return Err(package_error("npm build identity fact is unbounded"));
            }
        }
        if identity.semantic_recipe.is_empty() || identity.semantic_recipe.len() > 1024 * 1024 {
            return Err(package_error("npm build semantic recipe is unbounded"));
        }
        let rows = object
            .get("artifacts")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| package_error("npm build artifacts are invalid"))?;
        if rows.len() != USEFUL_TEXT_PACKAGE_PATHS.len() {
            return Err(package_error("npm build artifact inventory is not exact"));
        }
        let mut artifacts = Vec::with_capacity(rows.len());
        let mut total = 0_usize;
        for (row, expected_path) in rows.iter().zip(USEFUL_TEXT_PACKAGE_PATHS) {
            let row = row
                .as_object()
                .ok_or_else(|| package_error("npm build artifact row is invalid"))?;
            require_exact_keys(row, &["hex", "path", "sha256"])?;
            let path = json_string(row, "path")?;
            if path != expected_path {
                return Err(package_error("npm build artifact order is not canonical"));
            }
            let bytes = decode_hex(json_string(row, "hex")?, max_bytes.saturating_sub(total))?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| package_error("npm build artifact byte count overflowed"))?;
            if total > max_bytes {
                return Err(package_error(
                    "npm build artifacts exceed the trusted limit",
                ));
            }
            let digest = format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(&bytes))
            );
            if json_string(row, "sha256")? != digest {
                return Err(package_error("npm build artifact digest disagrees"));
            }
            artifacts.push(NpmArtifact {
                path: expected_path,
                bytes,
            });
        }
        let declared_total = object
            .get("artifact_bytes")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| package_error("npm build artifact_bytes is invalid"))?;
        if declared_total != total {
            return Err(package_error("npm build artifact byte count disagrees"));
        }
        let package = UsefulTextNpmPackage {
            artifacts: artifacts
                .try_into()
                .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
        };
        validate_replayed_package(identity, &package)?;
        let payload_digest = payload_digest(identity, &package);
        if json_string(object, "payload_digest")? != payload_digest {
            return Err(package_error("npm build payload digest disagrees"));
        }
        let canonical = render_carrier(identity, &package, total, &payload_digest);
        if canonical != envelope {
            return Err(package_error("npm build envelope is not canonical"));
        }
        Ok(ReplayedNpmEnvelope {
            canonical,
            payload_digest,
            artifact_bytes: total,
            trusted: trusted_binding(identity),
        })
    }

    /// Publish into a destination that must not already exist. Files use
    /// create-new semantics, so this route never replaces foreign bytes.
    /// Publication is deliberately fail-stop, not atomic: after a write or
    /// settlement error, the newly created destination may contain the exact
    /// canonical artifact prefix already reported as successful. This method
    /// never guesses at cleanup authority or claims that prefix was removed.
    pub fn publish(&self, output: &Path) -> Result<(), Diagnostic> {
        self.verify()?;
        let package = decode_carrier_artifacts(&self.envelope, self.max_bytes)?;
        match std::fs::symlink_metadata(output) {
            Ok(_) => return Err(package_error("npm package destination already exists")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(package_error(format!(
                    "cannot inspect npm package destination {}: {error}",
                    output.display()
                )))
            }
        }
        std::fs::create_dir(output).map_err(|error| {
            package_error(format!(
                "cannot create npm package destination {}: {error}",
                output.display()
            ))
        })?;
        for artifact in package.artifacts() {
            let path = output.join(artifact.path());
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| {
                    package_error(format!(
                        "cannot create npm package artifact {}; publication stopped and the destination may contain an exact canonical prefix: {error}",
                        path.display()
                    ))
                })?;
            file.write_all(artifact.bytes()).map_err(|error| {
                package_error(format!(
                    "cannot write npm package artifact {}; publication stopped and the destination may contain an exact canonical prefix: {error}",
                    path.display()
                ))
            })?;
            file.sync_all().map_err(|error| {
                package_error(format!(
                    "cannot settle npm package artifact {}; publication stopped and the destination may contain an exact canonical prefix: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct NpmBuildIdentity<'a> {
    project_schema: &'a str,
    package: &'a str,
    version: &'a str,
    project_revision: &'a str,
    workspace_revision: &'a str,
    project_graph_digest: &'a str,
    semantic_recipe: &'a str,
}

fn trusted_binding(identity: NpmBuildIdentity<'_>) -> TrustedNpmBinding {
    TrustedNpmBinding {
        project_schema: identity.project_schema.to_owned(),
        package: identity.package.to_owned(),
        version: identity.version.to_owned(),
        project_revision: identity.project_revision.to_owned(),
        workspace_revision: identity.workspace_revision.to_owned(),
        project_graph_digest: identity.project_graph_digest.to_owned(),
        semantic_recipe: identity.semantic_recipe.to_owned(),
    }
}

impl UsefulTextNpmPackage {
    pub(crate) fn artifacts(&self) -> &[NpmArtifact; 6] {
        &self.artifacts
    }

    pub(crate) fn artifact(&self, path: &str) -> Option<&[u8]> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.path == path)
            .map(NpmArtifact::bytes)
    }
}

/// Render the exact Useful Text Consumer v1 npm payload.
pub(crate) fn render_useful_text_npm_package(
    manifest: &ProjectManifest,
    wasm_bytes: &[u8],
    exports: &[TextPackageExport],
) -> Result<UsefulTextNpmPackage, Diagnostic> {
    let version = require_useful_text_project(manifest)?;
    validate_inputs(wasm_bytes, exports)?;

    let wasm_sha256 = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(wasm_bytes))
    );
    let runtime = render_runtime();
    let bindings = render_bindings(exports);
    let declarations = render_declarations(exports);
    let metadata = render_metadata(manifest.name(), version, &wasm_sha256, exports);
    let package = render_package_json(manifest.name(), version);
    let artifacts = [
        artifact("app.wasm", wasm_bytes),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.text-exports.json", metadata.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ];
    debug_assert_eq!(
        artifacts.each_ref().map(|artifact| artifact.path),
        USEFUL_TEXT_PACKAGE_PATHS
    );
    Ok(UsefulTextNpmPackage { artifacts })
}

/// Prepare a pathless npm carrier from one authenticated, linked HIR program.
/// Text-profile admission and Wasm emission happen before any carrier becomes
/// observable.
pub(crate) fn prepare(
    manifest: &ProjectManifest,
    program: &crate::hir::ResolvedProgram,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    let version = require_useful_text_project(manifest)?;
    validate_carrier_limit(0, max_bytes)?;
    let wasm_bytes =
        crate::wasm::emit_resolved_module_with_text_exports(program, manifest.web_exports())?;
    let semantic_recipe = render_semantic_recipe(program)?;
    let exports = derive_exports(program, manifest.web_exports())?;
    let package = render_useful_text_npm_package(manifest, &wasm_bytes, &exports)?;
    let artifact_bytes = package
        .artifacts
        .iter()
        .try_fold(0_usize, |total, artifact| {
            total
                .checked_add(artifact.bytes.len())
                .filter(|value| *value <= max_bytes)
                .ok_or_else(|| package_error("npm build artifacts exceed the trusted limit"))
        })?;
    let identity = NpmBuildIdentity {
        project_schema: manifest.schema(),
        package: manifest.name(),
        version,
        project_revision,
        workspace_revision,
        project_graph_digest,
        semantic_recipe: &semantic_recipe,
    };
    let payload_digest = payload_digest(identity, &package);
    let envelope = render_carrier(identity, &package, artifact_bytes, &payload_digest);
    validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

fn require_useful_text_project(manifest: &ProjectManifest) -> Result<&str, Diagnostic> {
    if !manifest.is_v2() || manifest.profile() != Some(PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1) {
        return Err(package_error(
            "npm facade requires the useful-text-consumer.v1 Project profile",
        ));
    }
    manifest
        .package_version()
        .ok_or_else(|| package_error("npm facade requires a manifest package version"))
}

fn derive_exports(
    program: &crate::hir::ResolvedProgram,
    selected: &[String],
) -> Result<Vec<TextPackageExport>, Diagnostic> {
    use crate::hir::{OwnershipMode, ResolvedType};

    selected
        .iter()
        .map(|stable_id| {
            let function = program
                .functions
                .iter()
                .find(|function| function.id.as_str() == stable_id)
                .ok_or_else(|| {
                    package_error(format!("selected npm export `{stable_id}` is absent"))
                })?;
            let parameters = function
                .params
                .iter()
                .map(|parameter| match (&parameter.ty, parameter.ownership) {
                    (ResolvedType::Str, OwnershipMode::Borrow) => Ok(TextPackageType::Str),
                    (ResolvedType::I64, OwnershipMode::Value) => Ok(TextPackageType::I64),
                    (ResolvedType::Bool, OwnershipMode::Value) => Ok(TextPackageType::Bool),
                    _ => Err(package_error(format!(
                        "selected npm export `{stable_id}` has an unsupported parameter"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = match function.return_type {
                ResolvedType::I64 => TextPackageType::I64,
                ResolvedType::Bool => TextPackageType::Bool,
                _ => {
                    return Err(package_error(format!(
                        "selected npm export `{stable_id}` has an unsupported result"
                    )))
                }
            };
            Ok(TextPackageExport::new(
                stable_id.clone(),
                raw_symbol(stable_id),
                parameters,
                result,
            ))
        })
        .collect()
}

fn render_semantic_recipe(program: &crate::hir::ResolvedProgram) -> Result<String, Diagnostic> {
    use std::collections::BTreeMap;

    if program.functions.is_empty() || program.functions.len() > 256 {
        return Err(package_error(
            "npm semantic recipe function inventory is unbounded",
        ));
    }
    let mut names = BTreeMap::new();
    for (index, function) in program.functions.iter().enumerate() {
        let name = if function.id == program.entrypoint {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        if names.insert(function.id.as_str(), name).is_some() {
            return Err(package_error(
                "npm semantic recipe duplicates a function identity",
            ));
        }
    }
    let mut output = String::from("module semaprax_npm_recipe;\n\n");
    for function in &program.functions {
        if !function.effects.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
        {
            return Err(package_error(
                "npm semantic recipe does not admit effects or contracts",
            ));
        }
        let mut values = BTreeMap::<String, String>::new();
        let parameters = function
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let name = format!("p{index}");
                values.insert(parameter.id.as_str().to_owned(), name.clone());
                let ty = recipe_type(&parameter.ty)?;
                let mode = match parameter.ownership {
                    crate::hir::OwnershipMode::Borrow => "borrow ",
                    crate::hir::OwnershipMode::Value => "",
                    crate::hir::OwnershipMode::Own | crate::hir::OwnershipMode::Shared => {
                        return Err(package_error(
                            "npm semantic recipe parameter ownership is unsupported",
                        ))
                    }
                };
                Ok(format!("{name}: {mode}{ty}"))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .join(", ");
        let name = names
            .get(function.id.as_str())
            .ok_or_else(|| package_error("npm semantic recipe function name is absent"))?;
        output.push_str(&format!(
            "@id({})\nfn {name}({parameters}) -> {}\n",
            quote_source(function.id.as_str()),
            recipe_type(&function.return_type)?,
        ));
        let mut local_index = 0_usize;
        output.push_str(&render_recipe_expr(
            &function.body,
            &names,
            &mut values,
            &mut local_index,
        )?);
        output.push_str("\n\n");
        if output.len() > 1024 * 1024 {
            return Err(package_error("npm semantic recipe exceeds its byte limit"));
        }
    }
    Ok(output)
}

fn recipe_type(ty: &crate::hir::ResolvedType) -> Result<&'static str, Diagnostic> {
    match ty {
        crate::hir::ResolvedType::I64 => Ok("i64"),
        crate::hir::ResolvedType::Bool => Ok("bool"),
        crate::hir::ResolvedType::Str => Ok("str"),
        _ => Err(package_error("npm semantic recipe type is unsupported")),
    }
}

fn render_recipe_expr(
    expression: &crate::hir::ResolvedExpr,
    functions: &std::collections::BTreeMap<&str, String>,
    values: &mut std::collections::BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    use crate::hir::{ResolvedExprKind, ResolvedStatement};

    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(value.to_string()),
        ResolvedExprKind::Bool(value) => Ok(value.to_string()),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => values
            .get(place.root.as_str())
            .cloned()
            .ok_or_else(|| package_error("npm semantic recipe place is unavailable")),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } if type_arguments.is_empty() && instance.is_none() => {
            let name = crate::str_ops::by_id(callee.as_str())
                .map(|operation| operation.name().to_owned())
                .or_else(|| functions.get(callee.as_str()).cloned())
                .ok_or_else(|| package_error("npm semantic recipe callee is unavailable"))?;
            let args = args
                .iter()
                .map(|argument| render_recipe_expr(argument, functions, values, local_index))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{name}({args})"))
        }
        ResolvedExprKind::Unary { op, value } => {
            let operator = match op {
                crate::ast::UnaryOp::Neg => "-",
                crate::ast::UnaryOp::Not => "!",
            };
            Ok(format!(
                "({operator}{})",
                render_recipe_expr(value, functions, values, local_index)?
            ))
        }
        ResolvedExprKind::Binary { op, left, right } => Ok(format!(
            "({} {} {})",
            render_recipe_expr(left, functions, values, local_index)?,
            op.text(),
            render_recipe_expr(right, functions, values, local_index)?,
        )),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => Ok(format!(
            "if {} {} else {}",
            render_recipe_expr(condition, functions, values, local_index)?,
            render_recipe_expr(then_branch, functions, values, local_index)?,
            render_recipe_expr(else_branch, functions, values, local_index)?,
        )),
        ResolvedExprKind::Block { statements, tail } => {
            let mut rendered = String::from("{ ");
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } => {
                        let value = render_recipe_expr(value, functions, values, local_index)?;
                        let name = format!("v{}", *local_index);
                        *local_index += 1;
                        values.insert(binding.id.as_str().to_owned(), name.clone());
                        rendered.push_str(&format!(
                            "let {}{name} = {value}; ",
                            if *mutable { "mut " } else { "" }
                        ));
                    }
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } => {
                        let value = render_recipe_expr(value, functions, values, local_index)?;
                        let name = values.get(binding.id.as_str()).ok_or_else(|| {
                            package_error("npm semantic recipe assignment target is unavailable")
                        })?;
                        rendered.push_str(&format!("{name} = {value}; "));
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        let body = render_recipe_expr(body, functions, values, local_index)?;
                        rendered.push_str(&format!(
                            "@audit(\"canonical npm semantic replay\") unsafe {body}; "
                        ));
                    }
                    ResolvedStatement::Assign { field: Some(_), .. }
                    | ResolvedStatement::While { .. } => {
                        return Err(package_error(
                            "npm semantic recipe statement is unsupported",
                        ))
                    }
                }
            }
            rendered.push_str(&render_recipe_expr(tail, functions, values, local_index)?);
            rendered.push_str(" }");
            Ok(rendered)
        }
        _ => Err(package_error(
            "npm semantic recipe expression is unsupported",
        )),
    }
}

fn quote_source(value: &str) -> String {
    // Stable IDs admitted by the profile are ASCII and exclude quotes and
    // escapes, so JSON quoting is also exact SEMAPRAX source quoting.
    quote_json(value)
}

fn raw_symbol(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut symbol = String::with_capacity(9 + stable_id.len() * 2);
    symbol.push_str("spx_text_");
    for byte in stable_id.bytes() {
        symbol.push(HEX[(byte >> 4) as usize] as char);
        symbol.push(HEX[(byte & 0x0f) as usize] as char);
    }
    symbol
}

fn artifact(path: &'static str, bytes: &[u8]) -> NpmArtifact {
    NpmArtifact {
        path,
        bytes: bytes.to_vec(),
    }
}

fn validate_inputs(wasm_bytes: &[u8], exports: &[TextPackageExport]) -> Result<(), Diagnostic> {
    if wasm_bytes.is_empty() || wasm_bytes.len() > MAX_WASM_BYTES {
        return Err(package_error(format!(
            "npm facade Wasm must contain 1..={MAX_WASM_BYTES} bytes"
        )));
    }
    if !(1..=MAX_EXPORTS).contains(&exports.len()) {
        return Err(package_error(format!(
            "npm facade requires 1..={MAX_EXPORTS} exports"
        )));
    }
    let mut previous: Option<&str> = None;
    for export in exports {
        if previous.is_some_and(|value| value.as_bytes() >= export.stable_id.as_bytes()) {
            return Err(package_error(
                "npm facade exports must be strictly sorted and unique",
            ));
        }
        previous = Some(&export.stable_id);
        if export.parameters.len() > MAX_PARAMETERS
            || !export.parameters.contains(&TextPackageType::Str)
        {
            return Err(package_error(
                "npm facade export ABI is outside Useful Text Consumer v1",
            ));
        }
        if export.stable_id.is_empty()
            || !export.stable_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
            || export.wasm_export.is_empty()
            || !export.wasm_export.is_ascii()
        {
            return Err(package_error("npm facade export identity is invalid"));
        }
    }
    Ok(())
}

fn render_runtime() -> &'static str {
    r#"const MIN_I64 = -(1n << 63n);
const MAX_I64 = (1n << 63n) - 1n;
function checked(value, operation) {
  if (value < MIN_I64 || value > MAX_I64) throw new RangeError(`SEMAPRAX checked arithmetic failure: ${operation}`);
  return value;
}
export const imports = Object.freeze({ env: Object.freeze({
  spx_add: (a, b) => checked(a + b, "addition overflow"),
  spx_sub: (a, b) => checked(a - b, "subtraction overflow"),
  spx_mul: (a, b) => checked(a * b, "multiplication overflow"),
  spx_div: (a, b) => { if (b === 0n || (a === MIN_I64 && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid division"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === MIN_I64 && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid remainder"); return a % b; },
  spx_neg: value => checked(-value, "negation overflow"),
  spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
}) });
export function instantiateCore(input) {
  let module;
  if (input instanceof WebAssembly.Module) module = input;
  else if (input instanceof ArrayBuffer) module = new WebAssembly.Module(new Uint8Array(input));
  else if (ArrayBuffer.isView(input)) module = new WebAssembly.Module(new Uint8Array(input.buffer, input.byteOffset, input.byteLength));
  else throw new TypeError("SEMAPRAX instantiate requires caller-owned WebAssembly bytes or a WebAssembly.Module");
  return new WebAssembly.Instance(module, imports);
}
"#
}

fn render_bindings(exports: &[TextPackageExport]) -> String {
    let facts = exports
        .iter()
        .map(|export| {
            format!(
                "[{},{{raw:{},params:[{}],result:{}}}]",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|ty| quote_json(ty.json()))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.json()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"import {{ instantiateCore }} from "./semaprax.js";
const MIN_I64 = -(1n << 63n), MAX_I64 = (1n << 63n) - 1n;
const ENTRIES = Object.freeze([{facts}]);
const ENCODER = new TextEncoder();
export class SemapraxTextError extends Error {{
  constructor(code, message) {{ super(message); this.name = "SemapraxTextError"; this.code = code; }}
}}
function rejectLoneSurrogate(value, index) {{
  for (let offset = 0; offset < value.length; offset++) {{
    const unit = value.charCodeAt(offset);
    if (unit >= 0xd800 && unit <= 0xdbff) {{
      if (offset + 1 >= value.length) throw new SemapraxTextError(2, `argument ${{index}} contains a lone UTF-16 surrogate`);
      const low = value.charCodeAt(++offset);
      if (low < 0xdc00 || low > 0xdfff) throw new SemapraxTextError(2, `argument ${{index}} contains a lone UTF-16 surrogate`);
    }} else if (unit >= 0xdc00 && unit <= 0xdfff) {{
      throw new SemapraxTextError(2, `argument ${{index}} contains a lone UTF-16 surrogate`);
    }}
  }}
}}
function globalNumber(value, name) {{
  const raw = value instanceof WebAssembly.Global ? value.value : value;
  if (!Number.isSafeInteger(raw) || raw < 0 || raw > 0xffffffff) throw new Error(`invalid SEMAPRAX ${{name}} export`);
  return raw;
}}
function scalarArgument(value, type, index) {{
  if (type === "i64") {{
    if (typeof value !== "bigint" || value < MIN_I64 || value > MAX_I64) throw new TypeError(`argument ${{index}} must be a signed 64-bit bigint`);
    return value;
  }}
  if (typeof value !== "boolean") throw new TypeError(`argument ${{index}} must be boolean`);
  return value ? 1 : 0;
}}
function scalarResult(value, type) {{
  if (type === "i64") {{
    if (typeof value !== "bigint" || value < MIN_I64 || value > MAX_I64) throw new TypeError("SEMAPRAX adapter returned invalid i64");
    return value;
  }}
  if (value !== 0 && value !== 1) throw new TypeError("SEMAPRAX adapter returned non-canonical bool");
  return value === 1;
}}
export function instantiate(input) {{
  const instance = instantiateCore(input), e = instance.exports;
  if (!(e.memory instanceof WebAssembly.Memory) || typeof e.__spx_text_status_v1 === "undefined") throw new Error("SEMAPRAX text ABI metadata is absent");
  const base = globalNumber(e.__spx_text_scratch_base_v1, "scratch base");
  const capacity = globalNumber(e.__spx_text_scratch_capacity_v1, "scratch capacity");
  if (base + capacity > e.memory.buffer.byteLength) throw new Error("SEMAPRAX text scratch range is invalid");
  let busy = false;
  function invoke(id, values) {{
    const row = ENTRIES.find(([candidate]) => candidate === id);
    if (row === undefined) throw new RangeError(`unknown SEMAPRAX text export: ${{id}}`);
    const fact = row[1];
    if (values.length !== fact.params.length) throw new TypeError(`SEMAPRAX text export ${{id}} expects ${{fact.params.length}} arguments`);
    if (busy) throw new SemapraxTextError(3, "SEMAPRAX text scratch arena is busy");
    busy = true;
    let memory, used = 0;
    try {{
      const encoded = new Array(values.length), offsets = new Array(values.length);
      for (let index = 0; index < values.length; index++) {{
        if (fact.params[index] !== "str") continue;
        if (typeof values[index] !== "string") throw new TypeError(`argument ${{index}} must be string`);
        rejectLoneSurrogate(values[index], index);
        if (values[index].length > capacity) throw new SemapraxTextError(1, `argument ${{index}} exceeds SEMAPRAX text scratch capacity`);
        const bytes = ENCODER.encode(values[index]);
        if (bytes.byteLength > capacity) throw new SemapraxTextError(1, `argument ${{index}} exceeds SEMAPRAX text scratch capacity`);
        if (!Number.isSafeInteger(used + bytes.byteLength) || used + bytes.byteLength > capacity) throw new SemapraxTextError(1, "arguments exceed cumulative SEMAPRAX text scratch capacity");
        offsets[index] = used; encoded[index] = bytes; used += bytes.byteLength;
      }}
      memory = new Uint8Array(e.memory.buffer, base, capacity);
      const rawArgs = [];
      for (let index = 0; index < values.length; index++) {{
        const type = fact.params[index];
        if (type === "str") {{ memory.set(encoded[index], offsets[index]); rawArgs.push(base + offsets[index], encoded[index].byteLength); }}
        else rawArgs.push(scalarArgument(values[index], type, index));
      }}
      const raw = e[fact.raw];
      if (typeof raw !== "function") throw new Error(`SEMAPRAX text adapter missing: ${{fact.raw}}`);
      const value = raw(...rawArgs);
      const status = globalNumber(e.__spx_text_status_v1, "status");
      if (status !== 0) throw new SemapraxTextError(status, `SEMAPRAX text adapter failed with code ${{status}}`);
      return scalarResult(value, fact.result);
    }} finally {{
      if (memory !== undefined && used !== 0) memory.fill(0, 0, used);
      busy = false;
    }}
  }}
  const functions = Object.create(null);
  for (const [id] of ENTRIES) Object.defineProperty(functions, id, {{ enumerable: true, value: (...values) => invoke(id, values) }});
  return Object.freeze({{ functions: Object.freeze(functions), call: (id, ...values) => invoke(id, values) }});
}}
export const exportIds = Object.freeze(ENTRIES.map(([id]) => id));
export default instantiate;
"#
    )
}

fn render_declarations(exports: &[TextPackageExport]) -> String {
    let properties = exports
        .iter()
        .map(|export| {
            let parameters = export
                .parameters
                .iter()
                .enumerate()
                .map(|(index, ty)| format!("arg{index}: {}", ty.typescript()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "  readonly {}: ({parameters}) => {};",
                quote_json(&export.stable_id),
                export.result.typescript(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "export type TextFailureCode = 1 | 2 | 3;\nexport declare class SemapraxTextError extends Error {{ readonly code: TextFailureCode; }}\nexport interface UsefulTextFunctions {{\n{properties}\n}}\nexport interface UsefulTextRuntime {{ readonly functions: Readonly<UsefulTextFunctions>; call<I extends keyof UsefulTextFunctions>(id: I, ...args: Parameters<UsefulTextFunctions[I]>): ReturnType<UsefulTextFunctions[I]>; }}\nexport declare function instantiate(input: ArrayBuffer | ArrayBufferView | WebAssembly.Module): UsefulTextRuntime;\nexport declare const exportIds: readonly (keyof UsefulTextFunctions)[];\nexport default instantiate;\n"
    )
}

fn render_metadata(
    package_name: &str,
    version: &str,
    wasm_sha256: &str,
    exports: &[TextPackageExport],
) -> String {
    let functions = exports
        .iter()
        .map(|export| {
            format!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|ty| quote_json(ty.json()))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.json()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"semaprax.text-exports.v1\",\"package\":{},\"version\":{},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}},\"scratch\":{{\"status\":\"__spx_text_status_v1\",\"base\":\"__spx_text_scratch_base_v1\",\"capacity\":\"__spx_text_scratch_capacity_v1\",\"busy_code\":3}},\"functions\":[{}]}}\n",
        quote_json(package_name),
        quote_json(version),
        quote_json(wasm_sha256),
        functions,
    )
}

fn render_package_json(name: &str, version: &str) -> String {
    format!(
        "{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.text-exports.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.text-exports.json\"],\"engines\":{{\"node\":\">=22\"}}}}\n",
        quote_json(name),
        quote_json(version),
    )
}

fn validate_replayed_package(
    identity: NpmBuildIdentity<'_>,
    package: &UsefulTextNpmPackage,
) -> Result<(), Diagnostic> {
    if identity.project_schema != super::PROJECT_SCHEMA_V2
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
    {
        return Err(package_error("npm build identity facts are not canonical"));
    }
    let wasm = package
        .artifact("app.wasm")
        .ok_or_else(|| package_error("npm build app.wasm is absent"))?;
    wasmparser::Validator::new()
        .validate_all(wasm)
        .map_err(|_| package_error("npm build app.wasm is not structurally valid"))?;
    let metadata_bytes = package
        .artifact("semaprax.text-exports.json")
        .ok_or_else(|| package_error("npm build text metadata is absent"))?;
    let metadata: serde_json::Value = serde_json::from_slice(metadata_bytes)
        .map_err(|_| package_error("npm build text metadata is not valid JSON"))?;
    let object = metadata
        .as_object()
        .ok_or_else(|| package_error("npm build text metadata must be one object"))?;
    require_exact_keys(
        object,
        &[
            "functions",
            "package",
            "schema",
            "scratch",
            "version",
            "wasm",
        ],
    )?;
    if json_string(object, "schema")? != "semaprax.text-exports.v1"
        || json_string(object, "package")? != identity.package
        || json_string(object, "version")? != identity.version
    {
        return Err(package_error("npm build text metadata identity disagrees"));
    }
    let wasm_fact = object
        .get("wasm")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| package_error("npm build text metadata wasm fact is invalid"))?;
    require_exact_keys(wasm_fact, &["path", "sha256"])?;
    let wasm_sha256 = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(wasm)));
    if json_string(wasm_fact, "path")? != "app.wasm"
        || json_string(wasm_fact, "sha256")? != wasm_sha256
    {
        return Err(package_error(
            "npm build text metadata Wasm binding disagrees",
        ));
    }
    let scratch = object
        .get("scratch")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| package_error("npm build text scratch metadata is invalid"))?;
    require_exact_keys(scratch, &["base", "busy_code", "capacity", "status"])?;
    if json_string(scratch, "status")? != "__spx_text_status_v1"
        || json_string(scratch, "base")? != "__spx_text_scratch_base_v1"
        || json_string(scratch, "capacity")? != "__spx_text_scratch_capacity_v1"
        || scratch.get("busy_code").and_then(serde_json::Value::as_u64) != Some(3)
    {
        return Err(package_error("npm build text scratch metadata disagrees"));
    }
    let functions = object
        .get("functions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm build text function metadata is invalid"))?;
    let exports = functions
        .iter()
        .map(parse_metadata_export)
        .collect::<Result<Vec<_>, _>>()?;
    validate_inputs(wasm, &exports)?;
    validate_wasm_export_inventory(wasm, &exports)?;
    let recipe_ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-recipe.spx"),
    )
    .map_err(|_| package_error("npm build semantic recipe does not parse"))?;
    let replayed = crate::hir::resolve(&recipe_ast)
        .map_err(|_| package_error("npm build semantic recipe does not resolve"))?;
    let replayed_recipe = render_semantic_recipe(&replayed)?;
    if replayed_recipe != identity.semantic_recipe {
        return Err(package_error("npm build semantic recipe is not canonical"));
    }
    let selected = exports
        .iter()
        .map(|export| export.stable_id.clone())
        .collect::<Vec<_>>();
    let replayed_exports = derive_exports(&replayed, &selected)?;
    if replayed_exports != exports {
        return Err(package_error(
            "npm build semantic recipe ABI disagrees with text metadata",
        ));
    }
    let expected_wasm = crate::wasm::emit_resolved_module_with_text_exports(&replayed, &selected)?;
    if expected_wasm != wasm {
        return Err(package_error(
            "npm build app.wasm disagrees with independent semantic recipe replay",
        ));
    }

    let expected = [
        ("semaprax.js", render_runtime().as_bytes().to_vec()),
        (
            "semaprax.bindings.js",
            render_bindings(&exports).into_bytes(),
        ),
        (
            "semaprax.bindings.d.ts",
            render_declarations(&exports).into_bytes(),
        ),
        (
            "semaprax.text-exports.json",
            render_metadata(identity.package, identity.version, &wasm_sha256, &exports)
                .into_bytes(),
        ),
        (
            "package.json",
            render_package_json(identity.package, identity.version).into_bytes(),
        ),
    ];
    for (path, bytes) in expected {
        if package.artifact(path) != Some(bytes.as_slice()) {
            return Err(package_error(format!(
                "npm build generated artifact `{path}` disagrees with semantic replay"
            )));
        }
    }
    Ok(())
}

fn parse_metadata_export(value: &serde_json::Value) -> Result<TextPackageExport, Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| package_error("npm build text function row is invalid"))?;
    require_exact_keys(
        object,
        &["parameters", "result", "stable_id", "wasm_export"],
    )?;
    let parameters = object
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm build text function parameters are invalid"))?
        .iter()
        .map(parse_package_type)
        .collect::<Result<Vec<_>, _>>()?;
    let result = parse_package_type(
        object
            .get("result")
            .ok_or_else(|| package_error("npm build text function result is absent"))?,
    )?;
    if result == TextPackageType::Str {
        return Err(package_error(
            "npm build text function result is unsupported",
        ));
    }
    let stable_id = json_string(object, "stable_id")?.to_owned();
    let wasm_export = json_string(object, "wasm_export")?.to_owned();
    if wasm_export != raw_symbol(&stable_id) {
        return Err(package_error(
            "npm build raw text symbol disagrees with stable ID",
        ));
    }
    Ok(TextPackageExport::new(
        stable_id,
        wasm_export,
        parameters,
        result,
    ))
}

fn parse_package_type(value: &serde_json::Value) -> Result<TextPackageType, Diagnostic> {
    match value.as_str() {
        Some("str") => Ok(TextPackageType::Str),
        Some("i64") => Ok(TextPackageType::I64),
        Some("bool") => Ok(TextPackageType::Bool),
        _ => Err(package_error("npm build text ABI type is unsupported")),
    }
}

fn validate_wasm_export_inventory(
    wasm: &[u8],
    exports: &[TextPackageExport],
) -> Result<(), Diagnostic> {
    use std::collections::BTreeMap;
    use wasmparser::{ExternalKind, Parser, Payload};

    let mut actual = BTreeMap::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ExportSection(section) =
            payload.map_err(|_| package_error("npm build app.wasm is not parseable"))?
        {
            for export in section {
                let export = export
                    .map_err(|_| package_error("npm build app.wasm export is not parseable"))?;
                actual.insert(export.name.to_owned(), export.kind);
            }
        }
    }
    let mut expected = BTreeMap::from([
        ("memory".to_owned(), ExternalKind::Memory),
        ("__spx_text_status_v1".to_owned(), ExternalKind::Global),
        (
            "__spx_text_scratch_base_v1".to_owned(),
            ExternalKind::Global,
        ),
        (
            "__spx_text_scratch_capacity_v1".to_owned(),
            ExternalKind::Global,
        ),
    ]);
    for export in exports {
        expected.insert(export.wasm_export.clone(), ExternalKind::Func);
    }
    if actual != expected {
        return Err(package_error(
            "npm build app.wasm export inventory disagrees with text metadata",
        ));
    }
    Ok(())
}

fn valid_package_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (byte == b'-' && index != 0)
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn valid_sha256_fact(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_package_semver(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || !value.is_ascii() {
        return false;
    }
    let mut build = value.split('+');
    let Some(core_pre) = build.next() else {
        return false;
    };
    if build
        .next()
        .is_some_and(|part| !valid_semver_ids(part, false))
        || build.next().is_some()
    {
        return false;
    }
    let (core, pre) = core_pre
        .split_once('-')
        .map_or((core_pre, None), |(a, b)| (a, Some(b)));
    if pre.is_some_and(|part| !valid_semver_ids(part, true)) {
        return false;
    }
    let mut core = core.split('.');
    let parts = [core.next(), core.next(), core.next()];
    core.next().is_none()
        && parts
            .into_iter()
            .all(|part| part.is_some_and(valid_semver_number))
}

fn valid_semver_ids(value: &str, numeric_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(numeric_zero
                    && part.bytes().all(|byte| byte.is_ascii_digit())
                    && part.len() > 1
                    && part.starts_with('0'))
        })
}

fn valid_semver_number(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn payload_digest(identity: NpmBuildIdentity<'_>, package: &UsefulTextNpmPackage) -> String {
    let mut digest = Sha256::new();
    digest.update(PROJECT_NPM_BUILD_DIGEST_DOMAIN);
    for value in [
        identity.project_schema,
        identity.package,
        identity.version,
        identity.project_revision,
        identity.workspace_revision,
        identity.project_graph_digest,
        identity.semantic_recipe,
    ] {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    for artifact in package.artifacts() {
        digest.update((artifact.path.len() as u64).to_le_bytes());
        digest.update(artifact.path.as_bytes());
        digest.update((artifact.bytes.len() as u64).to_le_bytes());
        digest.update(&artifact.bytes);
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn render_carrier(
    identity: NpmBuildIdentity<'_>,
    package: &UsefulTextNpmPackage,
    artifact_bytes: usize,
    payload_digest: &str,
) -> String {
    let artifacts = package
        .artifacts()
        .iter()
        .map(|artifact| {
            format!(
                "{{\"path\":{},\"sha256\":\"sha256:{:x}\",\"hex\":\"{}\"}}",
                quote_json(artifact.path),
                crate::digest_hex::LowerHex(Sha256::digest(&artifact.bytes)),
                encode_hex(&artifact.bytes),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"project_schema\":{},\"package\":{},\"version\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"semantic_recipe\":{},\"artifact_bytes\":{},\"payload_digest\":{},\"artifacts\":[{}]}}",
        quote_json(PROJECT_NPM_BUILD_SCHEMA),
        quote_json(identity.project_schema),
        quote_json(identity.package),
        quote_json(identity.version),
        quote_json(identity.project_revision),
        quote_json(identity.workspace_revision),
        quote_json(identity.project_graph_digest),
        quote_json(identity.semantic_recipe),
        artifact_bytes,
        quote_json(payload_digest),
        artifacts,
    )
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str, remaining: usize) -> Result<Vec<u8>, Diagnostic> {
    if value.len() & 1 == 1 || value.len() / 2 > remaining {
        return Err(package_error(
            "npm build artifact hex exceeds the trusted limit",
        ));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let encoded = value.as_bytes();
    let mut offset = 0;
    while offset < encoded.len() {
        let high = hex_nibble(encoded[offset])
            .ok_or_else(|| package_error("npm build artifact hex is not lowercase"))?;
        let low = hex_nibble(encoded[offset + 1])
            .ok_or_else(|| package_error("npm build artifact hex is not lowercase"))?;
        bytes.push((high << 4) | low);
        offset += 2;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn require_exact_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
) -> Result<(), Diagnostic> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(package_error(
            "npm build object has an unknown or missing field",
        ));
    }
    Ok(())
}

fn json_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error(format!("npm build {key} is invalid")))
}

fn validate_carrier_limit(length: usize, max_bytes: usize) -> Result<(), Diagnostic> {
    if max_bytes == 0 || max_bytes > MAX_PROJECT_NPM_BUILD_BYTES || length > max_bytes {
        return Err(package_error("npm build exceeds the trusted carrier limit"));
    }
    Ok(())
}

fn decode_carrier_artifacts(
    envelope: &str,
    max_bytes: usize,
) -> Result<UsefulTextNpmPackage, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|_| package_error("npm build envelope is not valid JSON"))?;
    let rows = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm build artifacts are invalid"))?;
    let mut total = 0_usize;
    let mut artifacts = Vec::with_capacity(6);
    for (row, path) in rows.iter().zip(USEFUL_TEXT_PACKAGE_PATHS) {
        let encoded = row
            .get("hex")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| package_error("npm build artifact hex is invalid"))?;
        let bytes = decode_hex(encoded, max_bytes.saturating_sub(total))?;
        total += bytes.len();
        artifacts.push(NpmArtifact { path, bytes });
    }
    Ok(UsefulTextNpmPackage {
        artifacts: artifacts
            .try_into()
            .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
    })
}

fn package_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W120", message)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn manifest() -> ProjectManifest {
        ProjectManifest::parse(
            "schema = \"semaprax.project.v2\"\nname = \"config-validator\"\nversion = \"1.2.3\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.valid\"]\ntests = [\"config.tests\"]\n",
        )
        .unwrap()
    }

    fn export() -> TextPackageExport {
        TextPackageExport::new(
            "config.valid".into(),
            "spx_text_636f6e6669672e76616c6964".into(),
            vec![TextPackageType::Str, TextPackageType::Bool],
            TextPackageType::Bool,
        )
    }

    fn runtime_manifest() -> ProjectManifest {
        ProjectManifest::parse(
            "schema = \"semaprax.project.v2\"\nname = \"config-validator\"\nversion = \"1.2.3\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.contains\", \"config.fail\", \"config.len\"]\ntests = [\"config.tests\"]\n",
        )
        .unwrap()
    }

    fn runtime_package() -> UsefulTextNpmPackage {
        let source = crate::parse(
            "module config.app;\n@id(\"config.contains\") fn contains(value: borrow str, needle: borrow str) -> bool { str_contains(value, needle) }\n@id(\"config.fail\") fn fail(value: borrow str) -> i64 { str_len_bytes(value) / 0 }\n@id(\"config.len\") fn len(value: borrow str) -> i64 { str_len_bytes(value) }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("config-runtime.spx"),
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let wasm = crate::wasm::emit_resolved_module_with_text_exports(
            &program,
            runtime_manifest().web_exports(),
        )
        .unwrap();
        let exports = derive_exports(&program, runtime_manifest().web_exports()).unwrap();
        render_useful_text_npm_package(&runtime_manifest(), &wasm, &exports).unwrap()
    }

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("semaprax-{label}-{}-{nonce}", std::process::id()))
    }

    fn write_package(root: &Path, package: &UsefulTextNpmPackage) {
        std::fs::create_dir(root).unwrap();
        for artifact in package.artifacts() {
            std::fs::write(root.join(artifact.path()), artifact.bytes()).unwrap();
        }
    }

    #[test]
    fn useful_text_npm_inventory_and_metadata_are_exact_and_deterministic() {
        let first = render_useful_text_npm_package(&manifest(), b"\0asm", &[export()]).unwrap();
        let second = render_useful_text_npm_package(&manifest(), b"\0asm", &[export()]).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.artifacts().each_ref().map(|artifact| artifact.path()),
            USEFUL_TEXT_PACKAGE_PATHS
        );
        let package: serde_json::Value =
            serde_json::from_slice(first.artifact("package.json").unwrap()).unwrap();
        assert_eq!(package["name"], "config-validator");
        assert_eq!(package["version"], "1.2.3");
        assert_eq!(package["sideEffects"], false);
        assert_eq!(package["exports"]["./app.wasm"], "./app.wasm");
        assert_eq!(
            package["exports"]["./manifest"],
            "./semaprax.text-exports.json"
        );
        assert_eq!(package["engines"]["node"], ">=22");
        for forbidden in ["private", "scripts", "dependencies", "devDependencies"] {
            assert!(package.get(forbidden).is_none(), "unexpected {forbidden}");
        }
        let bindings =
            std::str::from_utf8(first.artifact("semaprax.bindings.js").unwrap()).unwrap();
        assert!(bindings.contains("rejectLoneSurrogate"));
        assert!(bindings.contains("used + bytes.byteLength > capacity"));
        assert!(bindings.contains("SemapraxTextError(3"));
        assert!(!bindings.contains("new Uint8Array(e.memory.buffer, base, capacity);\n  let"));
    }

    #[test]
    fn npm_renderer_rejects_v1_unsorted_and_non_text_inputs() {
        let v1 = ProjectManifest::parse(
            "schema = \"semaprax.project.v1\"\nname = \"config-validator\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.valid\"]\ntests = [\"config.tests\"]\n",
        )
        .unwrap();
        assert_eq!(
            render_useful_text_npm_package(&v1, b"\0asm", &[export()])
                .unwrap_err()
                .code,
            "SPX-W120"
        );
        let scalar_source = crate::parse(
            "module config.app;\n@id(\"config.valid\") fn valid(value: bool) -> bool { value }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("config-v1.spx"),
        )
        .unwrap();
        let scalar_program = crate::hir::resolve(&scalar_source).unwrap();
        let admission = prepare(&v1, &scalar_program, "", "", "", 0).unwrap_err();
        assert_eq!(admission.code, "SPX-W120");
        assert_eq!(
            admission.message,
            "npm facade requires the useful-text-consumer.v1 Project profile"
        );
        let mut later = export();
        later.stable_id = "z.valid".into();
        assert!(render_useful_text_npm_package(&manifest(), b"\0asm", &[later, export()]).is_err());
        let scalar = TextPackageExport::new(
            "config.valid".into(),
            "spx_text_00".into(),
            vec![TextPackageType::Bool],
            TextPackageType::Bool,
        );
        assert!(render_useful_text_npm_package(&manifest(), b"\0asm", &[scalar]).is_err());
    }

    #[test]
    fn carrier_replay_rejects_a_canonical_self_resigned_generated_artifact_mutation() {
        let source = crate::parse(
            "module config.app;\n@id(\"config.valid\") fn valid(value: borrow str, expected: bool) -> bool { str_is_empty(value) == expected }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("config.spx"),
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let wasm = crate::wasm::emit_resolved_module_with_text_exports(
            &program,
            &["config.valid".to_owned()],
        )
        .unwrap();
        let mut package = render_useful_text_npm_package(&manifest(), &wasm, &[export()]).unwrap();
        let semantic_recipe = render_semantic_recipe(&program).unwrap();
        let identity = NpmBuildIdentity {
            project_schema: super::super::PROJECT_SCHEMA_V2,
            package: "config-validator",
            version: "1.2.3",
            project_revision:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            workspace_revision:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            project_graph_digest:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            semantic_recipe: &semantic_recipe,
        };
        let total = package
            .artifacts
            .iter()
            .map(|artifact| artifact.bytes.len())
            .sum();
        let digest = payload_digest(identity, &package);
        let envelope = render_carrier(identity, &package, total, &digest);
        ProjectNpmBuild::inspect_envelope(&envelope, envelope.len()).unwrap();
        let trusted_build = ProjectNpmBuild {
            envelope: envelope.clone(),
            payload_digest: digest.clone(),
            artifact_bytes: total,
            max_bytes: envelope.len(),
            trusted: trusted_binding(identity),
        };
        trusted_build.verify().unwrap();

        let attacker_source = crate::parse(
            "module config.app;\n@id(\"config.valid\") fn valid(value: borrow str, expected: bool) -> bool { !str_is_empty(value) == expected }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("attacker-config.spx"),
        )
        .unwrap();
        let attacker_program = crate::hir::resolve(&attacker_source).unwrap();
        let attacker_wasm = crate::wasm::emit_resolved_module_with_text_exports(
            &attacker_program,
            &["config.valid".to_owned()],
        )
        .unwrap();
        assert_ne!(attacker_wasm, wasm);
        let mut replaced = package.clone();
        replaced
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == "app.wasm")
            .unwrap()
            .bytes = attacker_wasm.clone();
        let attacker_sha = format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(&attacker_wasm))
        );
        replaced
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == "semaprax.text-exports.json")
            .unwrap()
            .bytes =
            render_metadata("config-validator", "1.2.3", &attacker_sha, &[export()]).into_bytes();
        let replaced_total = replaced
            .artifacts
            .iter()
            .map(|artifact| artifact.bytes.len())
            .sum();
        let replaced_digest = payload_digest(identity, &replaced);
        let resigned_wasm = render_carrier(identity, &replaced, replaced_total, &replaced_digest);
        let error =
            ProjectNpmBuild::inspect_envelope(&resigned_wasm, resigned_wasm.len()).unwrap_err();
        assert!(error.message.contains("semantic recipe replay"));

        // A fully self-consistent alternate recipe is inspectable as compiler
        // output, but cannot replace the context-bound recipe held by the
        // original prepared build or gain publication authority.
        let attacker_recipe = render_semantic_recipe(&attacker_program).unwrap();
        let attacker_identity = NpmBuildIdentity {
            semantic_recipe: &attacker_recipe,
            ..identity
        };
        let attacker_digest = payload_digest(attacker_identity, &replaced);
        let attacker_envelope = render_carrier(
            attacker_identity,
            &replaced,
            replaced_total,
            &attacker_digest,
        );
        ProjectNpmBuild::inspect_envelope(&attacker_envelope, attacker_envelope.len()).unwrap();
        let mut forged = trusted_build.clone();
        forged.envelope = attacker_envelope;
        forged.payload_digest = attacker_digest;
        forged.artifact_bytes = replaced_total;
        forged.max_bytes = forged.envelope.len();
        let error = forged.verify().unwrap_err();
        assert!(error
            .message
            .contains("context-bound trusted Project facts"));

        let package_json = package
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == "package.json")
            .unwrap();
        package_json.bytes = b"{\"name\":\"config-validator\",\"version\":\"1.2.3\",\"type\":\"module\",\"scripts\":{\"postinstall\":\"false\"}}\n".to_vec();
        let total = package
            .artifacts
            .iter()
            .map(|artifact| artifact.bytes.len())
            .sum();
        let digest = payload_digest(identity, &package);
        let resigned = render_carrier(identity, &package, total, &digest);
        let error = ProjectNpmBuild::inspect_envelope(&resigned, resigned.len()).unwrap_err();
        assert_eq!(error.code, "SPX-W120");
        assert!(error.message.contains("semantic replay"));
    }

    #[test]
    fn generated_facade_is_bounded_repeatable_and_offline_npm_types_are_strict() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let root = temporary_directory("npm-runtime-v1");
        write_package(&root, &runtime_package());
        let script = r#"import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import instantiate, { SemapraxTextError } from "./semaprax.bindings.js";
const runtime = instantiate(readFileSync("./app.wasm"));
const f = runtime.functions;
assert.equal(f["config.len"]("repeat"), 6n);
assert.equal(f["config.len"]("repeat"), 6n);
assert.throws(() => f["config.len"](String.fromCharCode(0xd800)), error => error instanceof SemapraxTextError && error.code === 2);
const exact = "a".repeat(65536);
assert.equal(f["config.len"](exact), 65536n);
assert.throws(() => f["config.len"](exact + "a"), error => error instanceof SemapraxTextError && error.code === 1);
assert.throws(() => f["config.contains"]("a".repeat(32768), "b".repeat(32769)), error => error instanceof SemapraxTextError && error.code === 1);
assert.throws(() => f["config.fail"]("must-be-erased"), RangeError);
assert.equal(f["config.len"]("after-failure"), 13n);
const source = readFileSync("./semaprax.bindings.js", "utf8");
assert.match(source, /memory\.fill\(0, 0, used\)/);
assert.match(source, /SemapraxTextError\(3/);
"#;
        let node = Command::new("node")
            .current_dir(&root)
            .args(["--input-type=module", "--eval", script])
            .output()
            .unwrap();
        assert!(
            node.status.success(),
            "generated npm runtime failed:\n{}",
            String::from_utf8_lossy(&node.stderr)
        );

        if Command::new("npm").arg("--version").output().is_ok()
            && Command::new("tsc").arg("--version").output().is_ok()
        {
            let npm_cache = temporary_directory("npm-cache-v1");
            std::fs::create_dir(&npm_cache).unwrap();
            let packed = Command::new("npm")
                .current_dir(&root)
                .env("npm_config_cache", &npm_cache)
                .args(["pack", "--ignore-scripts", "--json"])
                .output()
                .unwrap();
            assert!(
                packed.status.success(),
                "offline npm pack failed:\n{}",
                String::from_utf8_lossy(&packed.stderr)
            );
            let report: serde_json::Value = serde_json::from_slice(&packed.stdout).unwrap();
            let mut files = report[0]["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["path"].as_str().unwrap())
                .collect::<Vec<_>>();
            files.sort_unstable();
            let mut expected = USEFUL_TEXT_PACKAGE_PATHS;
            expected.sort_unstable();
            assert_eq!(files, expected);

            let consumer = root.join("consumer");
            std::fs::create_dir(&consumer).unwrap();
            std::fs::write(
                consumer.join("package.json"),
                "{\"name\":\"consumer\",\"private\":true,\"type\":\"module\"}\n",
            )
            .unwrap();
            let tarball = root.join("config-validator-1.2.3.tgz");
            let installed = Command::new("npm")
                .current_dir(&consumer)
                .env("npm_config_cache", &npm_cache)
                .args([
                    "install",
                    "--offline",
                    "--ignore-scripts",
                    "--no-audit",
                    "--no-fund",
                    tarball.to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert!(
                installed.status.success(),
                "offline npm install failed:\n{}",
                String::from_utf8_lossy(&installed.stderr)
            );
            std::fs::write(
                consumer.join("consumer.ts"),
                "import instantiate, { exportIds, type UsefulTextRuntime } from \"config-validator\";\nconst bytes = new Uint8Array();\nconst runtime: UsefulTextRuntime = instantiate(bytes);\nconst length: bigint = runtime.functions[\"config.len\"](\"ok\");\nconst contained: boolean = runtime.functions[\"config.contains\"](\"ok\", \"k\");\nvoid [length, contained, exportIds];\n",
            )
            .unwrap();
            let typed = Command::new("tsc")
                .current_dir(&consumer)
                .args([
                    "--strict",
                    "--noEmit",
                    "--target",
                    "ES2022",
                    "--module",
                    "NodeNext",
                    "--moduleResolution",
                    "NodeNext",
                    "consumer.ts",
                ])
                .output()
                .unwrap();
            assert!(
                typed.status.success(),
                "strict TypeScript consumer failed:\n{}",
                String::from_utf8_lossy(&typed.stderr)
            );
            std::fs::remove_dir_all(npm_cache).unwrap();
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
