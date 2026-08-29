//! Deterministic npm facade for the Useful Text Consumer v1 project profile.
//!
//! Rendering is authority-neutral: callers provide authenticated Wasm bytes
//! and the already-admitted public ABI. This module neither reads nor writes a
//! path and never launches npm, Node, or another process.

mod carrier;
mod command;
mod command_v2;
mod command_v3;
mod command_v4;
mod data;
mod owned_data;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
mod publication;
mod semantic_recipe_v8;

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::{ProjectManifest, PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1};

use carrier::{
    artifact, json_string, payload_digest, payload_digest_artifacts_v2,
    payload_digest_artifacts_v3, payload_digest_artifacts_v4, payload_digest_artifacts_v5,
    payload_digest_artifacts_v6, render_carrier, render_carrier_artifacts, require_exact_keys,
    trusted_binding, validate_carrier_limit, NpmArtifact, NpmBuildIdentity,
};
pub use carrier::{
    ProjectNpmBuild, MAX_PROJECT_NPM_BUILD_BYTES, PROJECT_NPM_BUILD_SCHEMA,
    PROJECT_NPM_BUILD_SCHEMA_V2, PROJECT_NPM_BUILD_SCHEMA_V3, PROJECT_NPM_BUILD_SCHEMA_V4,
    PROJECT_NPM_BUILD_SCHEMA_V5, PROJECT_NPM_BUILD_SCHEMA_V6, PROJECT_NPM_BUILD_SCHEMA_V7,
};

pub(crate) fn prepare_owned_data(
    program: &crate::hir::ResolvedProgram,
    descriptor: &crate::project::PublicApiDescriptor,
    package: &str,
    version: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    owned_data::prepare(program, descriptor, package, version, max_bytes)
}

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

/// Exact six-file, pathless npm package inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsefulTextNpmPackage {
    artifacts: [NpmArtifact; 6],
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
    if manifest.is_v8() {
        let version = manifest
            .package_version()
            .ok_or_else(|| package_error("owned-data npm facade requires a package version"))?;
        let subject = crate::project::PublicApiSubject {
            project_schema: manifest.schema(),
            project_revision,
            workspace_revision,
            project_graph_digest,
        };
        let derived =
            crate::project::derive_public_api_descriptor(program, manifest.web_exports(), subject)?;
        let replayed = crate::project::replay_public_api_descriptor(
            program,
            manifest.web_exports(),
            subject,
            &derived.canonical_bytes(),
            &derived.digest(),
        )?;
        if replayed != derived {
            return Err(package_error(
                "owned-data npm descriptor derivation and replay disagree",
            ));
        }
        return owned_data::prepare(program, &replayed, manifest.name(), version, max_bytes);
    }
    if manifest.is_v7() {
        return command_v4::prepare(
            manifest,
            program,
            project_revision,
            workspace_revision,
            project_graph_digest,
            max_bytes,
        );
    }
    if manifest.is_v6() {
        return command_v3::prepare(
            manifest,
            program,
            project_revision,
            workspace_revision,
            project_graph_digest,
            max_bytes,
        );
    }
    if manifest.is_v5() {
        return command_v2::prepare(
            manifest,
            program,
            project_revision,
            workspace_revision,
            project_graph_digest,
            max_bytes,
        );
    }
    if manifest.is_v4() {
        return command::prepare(
            manifest,
            program,
            project_revision,
            workspace_revision,
            project_graph_digest,
            max_bytes,
        );
    }
    if manifest.is_v3() {
        return data::prepare(
            manifest,
            program,
            project_revision,
            workspace_revision,
            project_graph_digest,
            max_bytes,
        );
    }
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

pub(super) fn render_semantic_recipe(
    program: &crate::hir::ResolvedProgram,
) -> Result<String, Diagnostic> {
    render_semantic_recipe_profile(program, false)
}

pub(crate) fn render_owned_data_semantic_recipe(
    program: &crate::hir::ResolvedProgram,
) -> Result<String, Diagnostic> {
    semantic_recipe_v8::render(program)
}

pub(crate) fn replay_owned_data_semantic_recipe(
    linked: &crate::hir::ResolvedProgram,
    recipe: &str,
) -> Result<crate::hir::ResolvedProgram, Diagnostic> {
    semantic_recipe_v8::replay_against(linked, recipe)
}

fn render_semantic_recipe_profile(
    program: &crate::hir::ResolvedProgram,
    preserve_public_names: bool,
) -> Result<String, Diagnostic> {
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
    let has_stdout = program
        .functions
        .iter()
        .any(|function| function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT]);
    let command_io = program.permits
        == [
            crate::command_io_ops::ARGS_READ_EFFECT,
            crate::command_io_ops::STDERR_WRITE_EFFECT,
            crate::command_io_ops::STDIN_READ_EFFECT,
            crate::host_io_ops::STDOUT_WRITE_EFFECT,
        ];
    let mut output = if command_io {
        format!(
            "module semaprax_npm_recipe;\n\npermit {{ {} }}\n\n",
            program.permits.join(", ")
        )
    } else if has_stdout {
        String::from("module semaprax_npm_recipe;\n\npermit { process.stdout.write }\n\n")
    } else {
        String::from("module semaprax_npm_recipe;\n\n")
    };
    for function in &program.functions {
        let effects_admitted = if command_io {
            function
                .effects
                .iter()
                .all(|effect| program.permits.iter().any(|permit| permit == effect))
        } else {
            function.effects.is_empty()
                || function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT]
        };
        if !effects_admitted || !function.requires.is_empty() || !function.ensures.is_empty() {
            return Err(package_error(
                "npm semantic recipe does not admit these effects or contracts",
            ));
        }
        let mut values = BTreeMap::<String, String>::new();
        let parameters = function
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let name = if preserve_public_names {
                    parameter.name.clone()
                } else {
                    format!("p{index}")
                };
                values.insert(parameter.id.as_str().to_owned(), name.clone());
                let ty = recipe_type(&parameter.ty)?;
                let mode = match parameter.ownership {
                    crate::hir::OwnershipMode::Borrow => "borrow ",
                    crate::hir::OwnershipMode::Value => "",
                    crate::hir::OwnershipMode::Own => "own ",
                    crate::hir::OwnershipMode::Shared => {
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
            "@id({})\nfn {name}({parameters}) -> {}\n{}",
            quote_source(function.id.as_str()),
            recipe_type(&function.return_type)?,
            if function.effects.is_empty() {
                "".to_owned()
            } else if command_io {
                format!("    uses {{ {} }}\n", function.effects.join(", "))
            } else {
                format!(
                    "    uses {{ {} }}\n",
                    crate::host_io_ops::STDOUT_WRITE_EFFECT
                )
            },
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

fn recipe_type(ty: &crate::hir::ResolvedType) -> Result<String, Diagnostic> {
    match ty {
        crate::hir::ResolvedType::I64 => Ok("i64".to_owned()),
        crate::hir::ResolvedType::Bool => Ok("bool".to_owned()),
        crate::hir::ResolvedType::U8 => Ok("u8".to_owned()),
        crate::hir::ResolvedType::Usize => Ok("usize".to_owned()),
        crate::hir::ResolvedType::Str => Ok("str".to_owned()),
        crate::hir::ResolvedType::SliceU8 => Ok("Slice<u8>".to_owned()),
        crate::hir::ResolvedType::Bytes => Ok("Bytes".to_owned()),
        crate::hir::ResolvedType::ArrayU8(length) => Ok(format!("[u8; {length}]")),
        crate::hir::ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == crate::prelude::OPTION_ID && arguments.len() == 1 => {
            Ok(format!("Option<{}>", recipe_type(&arguments[0])?))
        }
        crate::hir::ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == crate::prelude::RESULT_ID && arguments.len() == 2 => {
            Ok(format!(
                "Result<{}, {}>",
                recipe_type(&arguments[0])?,
                recipe_type(&arguments[1])?
            ))
        }
        _ => Err(package_error("npm semantic recipe type is unsupported")),
    }
}

fn render_recipe_expr(
    expression: &crate::hir::ResolvedExpr,
    functions: &std::collections::BTreeMap<&str, String>,
    values: &mut std::collections::BTreeMap<String, String>,
    local_index: &mut usize,
) -> Result<String, Diagnostic> {
    use crate::hir::{ResolvedExprKind, ResolvedMatchPattern, ResolvedStatement};

    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(value.to_string()),
        ResolvedExprKind::Uint8(value) => Ok(format!("{value}u8")),
        ResolvedExprKind::Usize(value) => Ok(format!("{value}usize")),
        ResolvedExprKind::ArrayU8(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(|value| format!("{value}u8"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        ResolvedExprKind::RepeatArrayU8 { value, count } => Ok(format!("[{value}u8; {count}]")),
        ResolvedExprKind::Bool(value) => Ok(value.to_string()),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => values
            .get(place.root.as_str())
            .cloned()
            .ok_or_else(|| package_error("npm semantic recipe place is unavailable")),
        ResolvedExprKind::BorrowPlace { operation, place } if place.projections.is_empty() => {
            let value = values
                .get(place.root.as_str())
                .cloned()
                .ok_or_else(|| package_error("npm semantic recipe place is unavailable"))?;
            let operation = crate::byte_ops::by_id(operation.as_str()).ok_or_else(|| {
                package_error("npm semantic recipe borrow operation is unavailable")
            })?;
            Ok(format!("{}({value})", operation.name()))
        }
        ResolvedExprKind::HostCommandCall(call) => {
            let args = call
                .args
                .iter()
                .map(|argument| render_recipe_expr(argument, functions, values, local_index))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!(
                "{}({args})",
                crate::command_io_ops::name(call.operation)
            ))
        }
        ResolvedExprKind::ByteRange {
            operation,
            source,
            start,
            end,
        } => {
            if operation.as_str() != crate::byte_ops::RANGE_ID {
                return Err(package_error(
                    "npm semantic recipe byte-range operation is unavailable",
                ));
            }
            Ok(format!(
                "{}({}, {}, {})",
                crate::byte_ops::RANGE_NAME,
                render_recipe_expr(source, functions, values, local_index)?,
                render_recipe_expr(start, functions, values, local_index)?,
                render_recipe_expr(end, functions, values, local_index)?,
            ))
        }
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } if type_arguments.is_empty() && instance.is_none() => {
            let name = crate::str_ops::by_id(callee.as_str())
                .map(|operation| operation.name().to_owned())
                .or_else(|| {
                    crate::byte_ops::by_id(callee.as_str())
                        .map(|operation| operation.name().to_owned())
                })
                .or_else(|| {
                    crate::host_io_ops::by_id(callee.as_str())
                        .map(|operation| operation.name().to_owned())
                })
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
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } if variant.as_str() == crate::prelude::OPTION_ID
            || variant.as_str() == crate::prelude::RESULT_ID =>
        {
            let (case_name, expected_field, field_name) = match case.as_str() {
                crate::prelude::OPTION_NONE_ID => ("None", None, None),
                crate::prelude::OPTION_SOME_ID => (
                    "Some",
                    Some(crate::prelude::OPTION_SOME_VALUE_ID),
                    Some("value"),
                ),
                crate::prelude::RESULT_OK_ID => (
                    "Ok",
                    Some(crate::prelude::RESULT_OK_VALUE_ID),
                    Some("value"),
                ),
                crate::prelude::RESULT_ERR_ID => (
                    "Err",
                    Some(crate::prelude::RESULT_ERR_ERROR_ID),
                    Some("error"),
                ),
                _ => {
                    return Err(package_error(
                        "npm semantic recipe variant case is unsupported",
                    ))
                }
            };
            let fields = match (expected_field, field_name, fields.as_slice()) {
                (None, None, []) => String::new(),
                (Some(expected), Some(name), [field]) if field.field.as_str() == expected => {
                    format!(
                        "{name}: {}",
                        render_recipe_expr(&field.value, functions, values, local_index)?
                    )
                }
                _ => {
                    return Err(package_error(
                        "npm semantic recipe variant fields are not exact",
                    ))
                }
            };
            Ok(format!(
                "{}::{case_name} {{ {fields} }}",
                recipe_type(&expression.ty)?
            ))
        }
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
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        let condition =
                            render_recipe_expr(condition, functions, values, local_index)?;
                        let body = render_recipe_expr(body, functions, values, local_index)?;
                        rendered.push_str(&format!("while {condition} {body} "));
                    }
                    ResolvedStatement::Assign { field: Some(_), .. } => {
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
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee = render_recipe_expr(scrutinee, functions, values, local_index)?;
            let mut rendered = format!("match {scrutinee} {{ ");
            for arm in arms {
                let mut arm_values = values.clone();
                let pattern = match &arm.pattern {
                    ResolvedMatchPattern::Variant { case, fields, .. }
                        if case.as_str() == crate::prelude::OPTION_SOME_ID && fields.len() == 1 =>
                    {
                        if fields[0].field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID {
                            return Err(package_error(
                                "npm semantic recipe Option::Some field is not exact",
                            ));
                        }
                        let name = format!("v{}", *local_index);
                        *local_index += 1;
                        arm_values.insert(fields[0].binding.id.as_str().to_owned(), name.clone());
                        format!("Option::Some {{ value: {name} }}")
                    }
                    ResolvedMatchPattern::Variant { case, fields, .. }
                        if case.as_str() == crate::prelude::OPTION_NONE_ID && fields.is_empty() =>
                    {
                        "Option::None {}".to_owned()
                    }
                    ResolvedMatchPattern::Variant { case, fields, .. }
                        if case.as_str() == crate::prelude::RESULT_OK_ID && fields.len() == 1 =>
                    {
                        let name = format!("v{}", *local_index);
                        *local_index += 1;
                        if fields[0].field.as_str() != crate::prelude::RESULT_OK_VALUE_ID {
                            return Err(package_error(
                                "npm semantic recipe Result::Ok field is not exact",
                            ));
                        }
                        arm_values.insert(fields[0].binding.id.as_str().to_owned(), name.clone());
                        format!("Result::Ok {{ value: {name} }}")
                    }
                    ResolvedMatchPattern::Variant { case, fields, .. }
                        if case.as_str() == crate::prelude::RESULT_ERR_ID && fields.len() == 1 =>
                    {
                        let name = format!("v{}", *local_index);
                        *local_index += 1;
                        if fields[0].field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID {
                            return Err(package_error(
                                "npm semantic recipe Result::Err field is not exact",
                            ));
                        }
                        arm_values.insert(fields[0].binding.id.as_str().to_owned(), name.clone());
                        format!("Result::Err {{ error: {name} }}")
                    }
                    _ => {
                        return Err(package_error(
                            "npm semantic recipe match pattern is unsupported",
                        ))
                    }
                };
                if arm.guard.is_some() {
                    return Err(package_error(
                        "npm semantic recipe match guard is unsupported",
                    ));
                }
                let value =
                    render_recipe_expr(&arm.value, functions, &mut arm_values, local_index)?;
                rendered.push_str(&format!("{pattern} => {value}, "));
            }
            rendered.push('}');
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

pub(super) fn valid_package_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (byte == b'-' && index != 0)
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

pub(super) fn valid_sha256_fact(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(super) fn valid_package_semver(value: &str) -> bool {
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

pub(super) fn package_error(message: impl Into<String>) -> Diagnostic {
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

    #[cfg(unix)]
    #[test]
    fn handle_relative_publication_never_writes_through_a_substituted_destination() {
        use std::os::unix::fs::symlink;

        let source = crate::parse(
            "module config.app;\n@id(\"config.contains\") fn contains(value: borrow str, needle: borrow str) -> bool { str_contains(value, needle) }\n@id(\"config.fail\") fn fail(value: borrow str) -> i64 { str_len_bytes(value) / 0 }\n@id(\"config.len\") fn len(value: borrow str) -> i64 { str_len_bytes(value) }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            Path::new("publication-race.spx"),
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let package = runtime_package();
        let recipe = render_semantic_recipe(&program).unwrap();
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
            semantic_recipe: &recipe,
        };
        let total = package.artifacts.iter().map(|item| item.bytes.len()).sum();
        let digest = payload_digest(identity, &package);
        let envelope = render_carrier(identity, &package, total, &digest);
        let build = ProjectNpmBuild {
            envelope,
            payload_digest: digest,
            artifact_bytes: total,
            max_bytes: MAX_PROJECT_NPM_BUILD_BYTES,
            trusted: trusted_binding(identity),
        };
        build.verify().unwrap();

        let root = temporary_directory("npm-handle-relative-race");
        let output = root.join("package");
        let moved = root.join("held-package");
        let foreign = root.join("foreign");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("marker"), b"foreign").unwrap();
        let output_for_hook = output.clone();
        let moved_for_hook = moved.clone();
        let foreign_for_hook = foreign.clone();
        publication::set_test_after_create(Box::new(move || {
            std::fs::rename(&output_for_hook, &moved_for_hook).unwrap();
            symlink(&foreign_for_hook, &output_for_hook).unwrap();
        }));
        let error = build.publish(&output).unwrap_err();
        assert!(
            error.message.contains("identity changed"),
            "{}",
            error.message
        );
        assert_eq!(std::fs::read(foreign.join("marker")).unwrap(), b"foreign");
        assert_eq!(std::fs::read_dir(&foreign).unwrap().count(), 1);
        assert!(std::fs::symlink_metadata(&output)
            .unwrap()
            .file_type()
            .is_symlink());
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
