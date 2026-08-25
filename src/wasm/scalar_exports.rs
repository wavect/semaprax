//! Admission and wrapper planning for Public Scalar Export Profile v1.
//!
//! The profile is intentionally stricter than the ordinary core-Wasm path:
//! every executable function is direct, monomorphic, effect-free scalar HIR,
//! while only explicitly selected stable identities receive public wrappers.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    self, DeclarationId, FunctionExecutionId, IdentityOrigin, OwnershipMode, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
};

use super::{write_u32, ByteOutput, I32, I64};

const MAX_EXPORTS: usize = 32;
const MAX_EXECUTABLE_FUNCTIONS: usize = 256;
const MAX_STABLE_ID_BYTES: usize = 128;
const MAX_PARAMETERS: usize = 8;

/// One public wrapper selected by its persistent declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScalarExportPlan {
    pub(super) stable_id: String,
    pub(super) wasm_export: String,
    pub(super) function_id: DeclarationId,
    pub(super) params: Vec<ScalarType>,
    pub(super) result: ScalarType,
}

impl ScalarExportPlan {
    pub(super) fn manifest_json(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|ty| quote_json(ty.text()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
            quote_json(&self.stable_id),
            quote_json(&self.wasm_export),
            params,
            quote_json(self.result.text()),
        )
    }

    /// Emit the direct adapter body. The profile admits no adapters that need
    /// locals, conversions, capabilities, or result transport.
    pub(super) fn emit_wrapper_body(
        &self,
        body: &mut impl ByteOutput,
        function_indexes: &HashMap<FunctionExecutionId, u32>,
    ) -> Result<(), Diagnostic> {
        let result_local = self.params.len() as u32;
        if self.result == ScalarType::Bool {
            write_u32(body, 1); // one i32 group for the canonical result
            write_u32(body, 1);
            body.push(I32);
        } else {
            write_u32(body, 0);
        }
        for (index, parameter) in self.params.iter().enumerate() {
            if *parameter == ScalarType::Bool {
                emit_bool_trap(body, index as u32);
            }
            body.push(0x20); // local.get
            write_u32(body, index as u32);
        }
        let execution = FunctionExecutionId::Monomorphic(self.function_id.clone());
        let target = function_indexes.get(&execution).copied().ok_or_else(|| {
            admission(format!(
                "selected scalar export `{}` has no monomorphic Wasm target",
                self.stable_id
            ))
        })?;
        body.push(0x10); // call
        write_u32(body, target);
        if self.result == ScalarType::Bool {
            body.push(0x21); // local.set
            write_u32(body, result_local);
            emit_bool_trap(body, result_local);
            body.push(0x20); // local.get
            write_u32(body, result_local);
        }
        body.push(0x0b); // end
        Ok(())
    }
}

/// The sole ABI values admitted by the public scalar profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScalarType {
    I64,
    Bool,
}

impl ScalarType {
    pub(super) const fn text(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    pub(super) const fn typescript_type(self) -> &'static str {
        match self {
            Self::I64 => "bigint",
            Self::Bool => "boolean",
        }
    }

    pub(super) const fn wasm_type(self) -> u8 {
        match self {
            Self::I64 => I64,
            Self::Bool => I32,
        }
    }
}

/// Validate the public scalar-export profile and produce wrappers in canonical
/// stable-ID byte order.
pub(super) fn prepare(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<ScalarExportPlan>, Diagnostic> {
    validate_selection(export_ids)?;
    hir::validate(program)?;
    validate_program_profile(program)?;

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut sorted_ids = export_ids.to_vec();
    sorted_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));

    let mut symbols = BTreeSet::new();
    sorted_ids
        .into_iter()
        .map(|stable_id| {
            let function = functions.get(stable_id.as_str()).copied().ok_or_else(|| {
                admission(format!(
                    "selected scalar export identity `{stable_id}` does not name a monomorphic function"
                ))
            })?;
            let declaration = program.declarations.declaration(&function.id).ok_or_else(|| {
                admission(format!(
                    "selected scalar export identity `{stable_id}` is absent from the declaration index"
                ))
            })?;
            if declaration.identity_origin != IdentityOrigin::Explicit {
                return Err(admission(format!(
                    "selected scalar export identity `{stable_id}` must be explicit"
                )));
            }

            let wasm_export = raw_symbol(&stable_id);
            if !symbols.insert(wasm_export.clone()) {
                return Err(admission(format!(
                    "selected scalar export identity `{stable_id}` collides with another raw export symbol"
                )));
            }
            Ok(ScalarExportPlan {
                wasm_export,
                function_id: function.id.clone(),
                params: function
                    .params
                    .iter()
                    .map(|parameter| scalar_type(&parameter.ty))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        admission(format!(
                            "selected scalar export identity `{stable_id}` has a non-scalar parameter"
                        ))
                    })?,
                result: scalar_type(&function.return_type).ok_or_else(|| {
                    admission(format!(
                        "selected scalar export identity `{stable_id}` has a non-scalar result"
                    ))
                })?,
                stable_id,
            })
        })
        .collect()
}

fn validate_selection(export_ids: &[String]) -> Result<(), Diagnostic> {
    if !(1..=MAX_EXPORTS).contains(&export_ids.len()) {
        return Err(capacity(format!(
            "Public Scalar Export Profile v1 requires 1..={MAX_EXPORTS} selected stable IDs"
        )));
    }

    let mut seen = BTreeSet::new();
    for stable_id in export_ids {
        if !seen.insert(stable_id.as_str()) {
            return Err(admission(format!(
                "Public Scalar Export Profile v1 selected stable ID `{stable_id}` more than once"
            )));
        }
    }
    for stable_id in export_ids {
        if !(1..=MAX_STABLE_ID_BYTES).contains(&stable_id.len()) {
            return Err(capacity(format!(
                "Public Scalar Export Profile v1 stable IDs must contain 1..={MAX_STABLE_ID_BYTES} bytes"
            )));
        }
        if !is_profile_id(stable_id) {
            return Err(admission(format!(
                "Public Scalar Export Profile v1 stable ID `{stable_id}` must use lowercase [a-z0-9._-]"
            )));
        }
    }
    Ok(())
}

fn validate_program_profile(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if !program.permits.is_empty() {
        return Err(admission(
            "Public Scalar Export Profile v1 does not admit module permits",
        ));
    }
    if !program.interfaces.is_empty() {
        return Err(admission(
            "Public Scalar Export Profile v1 does not admit imports or interfaces",
        ));
    }
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(admission(
            "Public Scalar Export Profile v1 does not admit generic function templates or instances",
        ));
    }
    if program.functions.len() > MAX_EXECUTABLE_FUNCTIONS {
        return Err(capacity(format!(
            "Public Scalar Export Profile v1 admits at most {MAX_EXECUTABLE_FUNCTIONS} monomorphic executable functions"
        )));
    }
    if program.types.iter().any(|declaration| {
        program
            .declarations
            .declaration(&declaration.id)
            .is_none_or(|item| item.identity_origin != IdentityOrigin::CompilerOwned)
    }) {
        return Err(admission(
            "Public Scalar Export Profile v1 does not admit authored resource, record, or variant declarations",
        ));
    }
    for function in &program.functions {
        if program
            .declarations
            .declaration(&function.id)
            .is_none_or(|declaration| declaration.identity_origin != IdentityOrigin::Explicit)
        {
            return Err(admission(format!(
                "Public Scalar Export Profile v1 function `{}` must have an explicit stable identity",
                function.id
            )));
        }
        validate_function_profile(function)?;
    }
    Ok(())
}

fn validate_function_profile(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(capacity(format!(
            "Public Scalar Export Profile v1 function `{}` exceeds the {MAX_PARAMETERS}-parameter limit",
            function.id
        )));
    }
    if !function.effects.is_empty() {
        return Err(admission(format!(
            "Public Scalar Export Profile v1 function `{}` declares effects",
            function.id
        )));
    }
    for parameter in &function.params {
        if parameter.ownership != OwnershipMode::Value || scalar_type(&parameter.ty).is_none() {
            return Err(admission(format!(
                "Public Scalar Export Profile v1 function `{}` has a non-value scalar parameter",
                function.id
            )));
        }
    }
    if scalar_type(&function.return_type).is_none() {
        return Err(admission(format!(
            "Public Scalar Export Profile v1 function `{}` has a non-scalar result",
            function.id
        )));
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
    {
        validate_expression_profile(expression, &function.id)?;
    }
    Ok(())
}

fn validate_expression_profile(
    expression: &ResolvedExpr,
    function_id: &DeclarationId,
) -> Result<(), Diagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if expression.ownership != OwnershipMode::Value || scalar_type(&expression.ty).is_none() {
            return Err(admission(format!(
                "Public Scalar Export Profile v1 function `{function_id}` contains a non-value scalar expression"
            )));
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_) => {}
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(admission(format!(
                        "Public Scalar Export Profile v1 function `{function_id}` projects an aggregate value"
                    )));
                }
            }
            ResolvedExprKind::Call {
                type_arguments,
                instance,
                args,
                ..
            } => {
                if !type_arguments.is_empty() || instance.is_some() {
                    return Err(admission(format!(
                        "Public Scalar Export Profile v1 function `{function_id}` calls a generic function"
                    )));
                }
                pending.extend(args.iter());
            }
            ResolvedExprKind::NativeRustImportCall(_) => {
                return Err(admission(format!(
                    "Public Scalar Export Profile v1 function `{function_id}` calls a native import"
                )));
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    if let ResolvedStatement::Let { binding, .. } = statement {
                        if binding.ownership != OwnershipMode::Value
                            || scalar_type(&binding.ty).is_none()
                        {
                            return Err(admission(format!(
                                "Public Scalar Export Profile v1 function `{function_id}` binds a non-value scalar"
                            )));
                        }
                    }
                    pending.push(statement.value());
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. } => {
                return Err(admission(format!(
                    "Public Scalar Export Profile v1 function `{function_id}` contains an aggregate, variant, or result expression"
                )));
            }
        }
    }
    Ok(())
}

fn scalar_type(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        ResolvedType::Unit
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::String
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::Nominal { .. } => None,
    }
}

fn is_profile_id(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    })
}

/// A fixed-width hexadecimal encoding is injective over the exact ID bytes and
/// is therefore collision-free without normalisation or display-name input.
pub(super) fn raw_symbol(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut symbol = String::with_capacity("spx_scalar_".len() + stable_id.len() * 2);
    symbol.push_str("spx_scalar_");
    for byte in stable_id.bytes() {
        symbol.push(HEX[(byte >> 4) as usize] as char);
        symbol.push(HEX[(byte & 0x0f) as usize] as char);
    }
    symbol
}

/// Trap if the given i32 local is not a canonical Wasm boolean (zero or one).
fn emit_bool_trap(body: &mut impl ByteOutput, local: u32) {
    body.push(0x20); // local.get
    write_u32(body, local);
    body.extend_bytes(&[0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    // i32.const 1; i32.gt_u; if (empty); unreachable; end
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W115", message)
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W116", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn resolve(source: &str) -> ResolvedProgram {
        let program = crate::parse(source, Path::new("scalar-exports.spx")).unwrap();
        crate::hir::resolve(&program).unwrap()
    }

    #[test]
    fn prepares_explicit_ids_in_bytewise_order_with_exact_raw_symbols() {
        let program = resolve(
            "module scalar.exports;\n@id(\"scalar.main\") fn main() -> i64 { helper(1) }\n@id(\"scalar.helper\") fn helper(value: i64) -> i64 { value }\n",
        );
        let plans = prepare(
            &program,
            &["scalar.main".to_owned(), "scalar.helper".to_owned()],
        )
        .unwrap();
        assert_eq!(plans[0].stable_id, "scalar.helper");
        assert_eq!(
            plans[0].wasm_export,
            "spx_scalar_7363616c61722e68656c706572"
        );
        assert_eq!(plans[1].stable_id, "scalar.main");
        assert_eq!(plans[0].params, vec![ScalarType::I64]);
    }

    #[test]
    fn rejects_duplicate_selection_before_planning() {
        let program =
            resolve("module scalar.exports;\n@id(\"scalar.main\") fn main() -> i64 { 0 }\n");
        let error = prepare(
            &program,
            &["scalar.main".to_owned(), "scalar.main".to_owned()],
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W115");
    }

    #[test]
    fn rejects_more_than_eight_parameters_as_capacity() {
        let program = resolve(
            "module scalar.exports;\n@id(\"scalar.main\") fn main() -> i64 { 0 }\n@id(\"scalar.wide\") fn wide(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64, i: i64) -> i64 { a }\n",
        );
        let error = prepare(&program, &["scalar.main".to_owned()]).unwrap_err();
        assert_eq!(error.code, "SPX-W116");
    }
}
