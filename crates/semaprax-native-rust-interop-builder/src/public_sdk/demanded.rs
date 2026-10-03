//! Inert concrete Rust demand plans attached to checked Semaprax call sites.
//!
//! This renderer has no process or publication authority. The generated Rust
//! must be compiled against the selected package before any call can execute.
use super::*;
use crate::indexed_binding::SelectedPackage;
use semaprax::ast::Span;
use semaprax::hir::{ResolvedExpr, ResolvedExprKind};
use semaprax::native_rust_binding::rust_api_path_tokens;
use semaprax_rust_api_index::{
    resolve_demanded_associated_types, resolve_demanded_instantiations, AssociatedTypeRequest,
    ConstArgument, GenericParameterKind, InstantiationRequest, RustApiIndex, TypeRecordKind,
    Visibility,
};
#[cfg(test)]
use semaprax_rust_api_index::{ConcreteType, DemandError};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDemandSelection {
    pub import_id: String,
    pub request: InstantiationRequest,
    /// Optional associated result projection; rustc proves both this projection
    /// and its equality to the declared Semaprax scalar result.
    pub associated_result: Option<AssociatedTypeRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteRustBindingPlan {
    pub import_id: String,
    pub item_path: String,
    pub rust_signature: String,
    pub index_digest: String,
    pub instantiation_identity: String,
    pub physical_symbol: String,
    pub source_path: String,
    pub use_sites: Vec<Span>,
    wrapper_line: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandedNativeRust {
    pub bindings: Vec<ConcreteRustBindingPlan>,
    pub c_source: String,
    pub header: String,
    pub safe_rust: String,
    pub ffi_rust: String,
    pub rust_adapter: String,
    pub descriptor: String,
}

fn refusal(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error("SPX-B153", message, span)
}

/// Explicit requests bind existing monomorphic Semaprax import declarations.
/// No public-generic ABI admission, source inference, trait implementation,
/// source embedding, package publication, or Rust layout projection occurs.
pub fn prepare_demanded_native_rust(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    selections: &[RustDemandSelection],
) -> Result<DemandedNativeRust, Vec<Diagnostic>> {
    prepare(
        source,
        source_path,
        options,
        index_bytes,
        package,
        selections,
    )
    .map_err(|error| vec![error.at_path(source_path.display().to_string())])
}

fn prepare(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    selections: &[RustDemandSelection],
) -> Result<DemandedNativeRust, Diagnostic> {
    let fail = |message| refusal(message, Span::default());
    if source.len() > MAX_SOURCE_BYTES || selections.is_empty() || selections.len() > MAX_IMPORTS {
        return Err(fail("concrete Rust demand exceeds its bounded input"));
    }
    let options = NativeRustSdkOptions {
        exports: canonical_values(options.exports, MAX_EXPORTS)?,
        imports: canonical_values(options.imports, MAX_IMPORTS)?,
        capabilities: canonical_values(options.capabilities, MAX_EFFECTS)?,
    };
    let mut selections = selections.iter().collect::<Vec<_>>();
    selections.sort_by(|a, b| a.import_id.cmp(&b.import_id));
    if selections.len() != options.imports.len()
        || selections
            .iter()
            .zip(&options.imports)
            .any(|(s, id)| &s.import_id != id)
    {
        return Err(fail(
            "concrete Rust demands must cover the exact import selection",
        ));
    }
    let index =
        RustApiIndex::replay(index_bytes).map_err(|_| fail("concrete Rust index replay failed"))?;
    index
        .require_package_identity(
            package.name,
            package.version,
            package.source_sha256,
            package.target,
            package.feature_digest,
        )
        .and_then(|_| index.require_cargo_alias_identity(package.cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(package.stable_rustc_version))
        .map_err(|_| fail("concrete Rust package/toolchain identity disagrees with the index"))?;
    if Some(package.target) != target_triple() {
        return Err(fail("concrete Rust demand target is unsupported"));
    }
    let program = semaprax::check(source, source_path).map_err(|mut errors| errors.remove(0))?;
    let resolved = semaprax::hir::resolve(&program).map_err(|mut errors| errors.remove(0))?;
    semaprax::hir::validate(&resolved)?;
    let canonical = semaprax::format::canonical(&program);
    let revision = domain_digest(SOURCE_DOMAIN, canonical.as_bytes());
    let spec = descriptor::canonical_spec(&program.module, &revision, package.target, &options)?;
    let prepared = crate::implementation::prepare_native_rust_interop(&program, spec.as_bytes())
        .map_err(|mut errors| errors.remove(0))?;
    let facts = descriptor::parse_descriptor(
        prepared.descriptor().as_bytes(),
        &program.module,
        &revision,
        package.target,
        &options,
    )?;
    let mut wrappers = String::new();
    let mut methods = String::new();
    let mut bindings = Vec::new();
    let mut emitted = BTreeMap::<String, (String, u64)>::new();
    for selection in selections {
        let import = resolved
            .interfaces
            .iter()
            .flat_map(|i| &i.imports)
            .find(|i| i.id.as_str() == selection.import_id)
            .ok_or_else(|| fail("concrete Rust import identity is absent"))?;
        let at = |message| refusal(message, import.span);
        if import.index_selected || import.rust_path.is_some() {
            return Err(at(
                "concrete Rust demands require an explicit monomorphic import declaration",
            ));
        }
        for argument in &selection.request.const_arguments {
            if ConstArgument::decimal(&argument.ty, &argument.value).as_ref() != Ok(argument) {
                return Err(at("concrete Rust const argument is not canonical"));
            }
        }
        let path = rust_api_path_tokens(&selection.request.item_path)
            .filter(|_| {
                selection
                    .request
                    .item_path
                    .starts_with(&format!("{}::", package.cargo_alias))
            })
            .ok_or_else(|| at("concrete Rust item path is outside the selected package"))?;
        for argument in &selection.request.type_arguments {
            let ty = argument.as_str();
            let primitive = matches!(ty, "i64" | "bool" | "u8" | "i32" | "usize");
            if rust_api_path_tokens(ty).is_none()
                || (!primitive
                    && !index.types().iter().any(|t| {
                        t.path == ty
                            && t.visibility == Visibility::Public
                            && t.generics.parameters.is_empty()
                            && matches!(t.kind, TypeRecordKind::Struct | TypeRecordKind::Enum)
                    }))
            {
                return Err(at(
                    "concrete Rust type is private, unavailable, or outside the selected index",
                ));
            }
        }
        let demand =
            resolve_demanded_instantiations(&index, std::slice::from_ref(&selection.request))
                .map_err(|_| at("concrete Rust item or type/const arguments are unsupported"))?
                .remove(0);
        let item = index
            .items()
            .iter()
            .find(|item| item.path == demand.item_path)
            .expect("resolved item");
        if !item.signature.starts_with("fn ") {
            return Err(at(
                "unsafe or foreign Rust functions require an authored adapter",
            ));
        }
        let mut types = demand.type_arguments.iter();
        let mut constants = demand.const_arguments.iter();
        let args = item
            .generics
            .parameters
            .iter()
            .map(|p| match p.kind {
                GenericParameterKind::Type => {
                    rust_api_path_tokens(types.next().expect("checked arity").as_str())
                        .expect("checked type tokens")
                }
                GenericParameterKind::Const => {
                    constants.next().expect("checked arity").value.clone()
                }
                GenericParameterKind::Lifetime => unreachable!("resolver refuses lifetime demands"),
            })
            .collect::<Vec<_>>()
            .join(",");
        let target = if args.is_empty() {
            path
        } else {
            format!("{path}::<{args}>")
        };
        let fact = facts
            .imports
            .iter()
            .find(|f| f.id == selection.import_id)
            .expect("descriptor import");
        let result = fact.result.rust();
        let (rust_result, associated_identity) = if let Some(request) = &selection.associated_result
        {
            if !selection
                .request
                .type_arguments
                .contains(&request.implementor)
            {
                return Err(at(
                    "associated result implementor is not a demanded type argument",
                ));
            }
            let projection =
                resolve_demanded_associated_types(&index, std::slice::from_ref(request))
                    .map_err(|_| {
                        at("associated result is private, sealed, generic, or unavailable")
                    })?
                    .remove(0);
            (projection.projection, projection.identity)
        } else {
            (result.to_owned(), String::new())
        };
        let types = fact
            .parameters
            .iter()
            .map(|p| p.ty.rust())
            .collect::<Vec<_>>()
            .join(",");
        let args = (0..fact.parameters.len())
            .map(|i| format!("arg_{i}"))
            .collect::<Vec<_>>();
        let declarations = args
            .iter()
            .zip(&fact.parameters)
            .map(|(a, p)| format!("{a}:{}", p.ty.rust()))
            .collect::<Vec<_>>()
            .join(",");
        let args = args.join(",");
        let key = format!("{}|{associated_identity}|{types}|{result}", demand.identity);
        let identity = domain_digest(b"semaprax.concrete-rust-binding.v1\0", key.as_bytes());
        let (symbol, line) = if let Some(pair) = emitted.get(&identity) {
            pair.clone()
        } else {
            let symbol = format!(
                "spx_ri07_{}",
                identity.strip_prefix("sha256:").expect("digest")
            );
            let line = wrappers.lines().count() as u64 + 1;
            writeln!(wrappers,"fn {symbol}({declarations})->{result}{{let target:fn({types})->{rust_result}={target};let value:{rust_result}=target({args});let result:{result}=value;result}}").unwrap();
            emitted.insert(identity.clone(), (symbol.clone(), line));
            (symbol, line)
        };
        write!(methods,"fn {}(&mut self{}{})->NativeRustImportResult<{result}>{{NativeRustImportResult::Success({symbol}({args}))}}",fact.inner_method,if declarations.is_empty(){""}else{","},declarations).unwrap();
        let mut sites = Vec::new();
        for function in resolved
            .functions
            .iter()
            .filter(|f| prepared.closure().iter().any(|id| id == f.id.as_str()))
        {
            collect_sites(&function.body, &selection.import_id, &mut sites)?;
        }
        if sites.is_empty() {
            return Err(at("concrete Rust request has no checked Semaprax use site"));
        }
        bindings.push(ConcreteRustBindingPlan {
            import_id: selection.import_id.clone(),
            item_path: demand.item_path,
            rust_signature: item.signature.clone(),
            index_digest: index.digest().to_owned(),
            instantiation_identity: identity,
            physical_symbol: symbol,
            source_path: source_path.display().to_string(),
            use_sites: sites,
            wrapper_line: line,
        });
    }
    let rust_adapter = format!("{wrappers}pub struct DemandedRustHost;\nimpl NativeRustImports for DemandedRustHost{{{methods}}}\n");
    if rust_adapter.len() > MAX_GENERATED_RUST_BYTES {
        return Err(fail("concrete Rust wrapper exceeds its output bound"));
    }
    Ok(DemandedNativeRust {
        bindings,
        c_source: prepared.generated_c().into(),
        header: prepared.generated_header().into(),
        safe_rust: prepared.generated_rust().into(),
        ffi_rust: prepared.private_ffi_source().into(),
        rust_adapter,
        descriptor: prepared.descriptor().into(),
    })
}

fn collect_sites(expr: &ResolvedExpr, id: &str, out: &mut Vec<Span>) -> Result<(), Diagnostic> {
    let mut pending = vec![expr];
    let mut visited = 0usize;
    while let Some(expr) = pending.pop() {
        visited += 1;
        if visited > 65536 {
            return Err(refusal(
                "concrete Rust call-site traversal exceeds its bound",
                expr.span,
            ));
        }
        match &expr.kind {
            ResolvedExprKind::NativeRustImportCall(call) => {
                if call.import.as_str() == id {
                    out.push(expr.span);
                }
                pending.extend(call.args.iter().rev());
            }
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for s in statements.iter().rev() {
                    for i in (0..s.child_count()).rev() {
                        pending.push(s.child(i).expect("child"));
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
            _ => {
                return Err(refusal(
                    "concrete Rust use-site traversal is outside the scalar profile",
                    expr.span,
                ))
            }
        }
    }
    Ok(())
}

impl DemandedNativeRust {
    /// Map captured, untrusted rustc JSON to exact generated wrapper lines.
    /// This is diagnostic data, never proof that a wrapper was compiled.
    pub fn map_captured_rustc_errors(
        &self,
        generated_file: &str,
        bytes: &[u8],
    ) -> Result<Vec<Diagnostic>, Diagnostic> {
        if bytes.is_empty() || bytes.len() > 262144 || generated_file.is_empty() {
            return Err(refusal(
                "captured concrete Rust diagnostics exceed their bound",
                Span::default(),
            ));
        }
        let mut output = Vec::new();
        for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let value: Value = serde_json::from_slice(line).map_err(|_| {
                refusal(
                    "captured concrete Rust diagnostic is malformed",
                    Span::default(),
                )
            })?;
            if value["level"] != "error" {
                continue;
            }
            let code = value["code"]["code"].as_str().unwrap_or("");
            if !matches!(code, "E0277" | "E0308" | "E0599" | "E0603" | "E0271") {
                continue;
            }
            let text = value["message"]
                .as_str()
                .filter(|text| text.len() <= 2048)
                .ok_or_else(|| {
                    refusal("captured Rust message exceeds its bound", Span::default())
                })?;
            for binding in &self.bindings {
                if !value["spans"].as_array().is_some_and(|spans| {
                    spans.iter().any(|span| {
                        span["is_primary"] == true
                            && span["file_name"] == generated_file
                            && span["line_start"].as_u64() == Some(binding.wrapper_line)
                    })
                }) {
                    continue;
                }
                for span in &binding.use_sites {
                    if output.len() == 64 {
                        return Err(refusal(
                            "captured Rust diagnostic mapping exceeds its bound",
                            *span,
                        ));
                    }
                    output.push(
                        Diagnostic::error(
                            "SPX-B150",
                            format!(
                                "Rust obligation for {} [{}] ({code}) rejected: {text}",
                                binding.item_path, binding.rust_signature
                            ),
                            *span,
                        )
                        .at_path(&binding.source_path),
                    );
                }
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "demanded_tests.rs"]
mod tests;
