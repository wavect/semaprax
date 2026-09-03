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

use super::{write_i32, write_u32, ByteOutput, F32, F64, I32, I64};

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
        if self.result.needs_boundary_trap() {
            write_u32(body, 1); // one i32 group for the canonical result
            write_u32(body, 1);
            body.push(I32);
        } else {
            write_u32(body, 0);
        }
        for (index, parameter) in self.params.iter().enumerate() {
            emit_boundary_trap(body, *parameter, index as u32);
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
        if self.result.needs_boundary_trap() {
            body.push(0x21); // local.set
            write_u32(body, result_local);
            emit_boundary_trap(body, self.result, result_local);
            body.push(0x20); // local.get
            write_u32(body, result_local);
        }
        body.push(0x0b); // end
        Ok(())
    }
}

/// The Copy-scalar ABI values admitted by the public scalar profile: the same
/// surface the reference interpreter, the interop projections, and the schema
/// projections already admit, minus `usize`, whose host width is not a public
/// fact of this profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScalarType {
    I64,
    I32,
    U8,
    Char,
    F32,
    F64,
    Bool,
}

impl ScalarType {
    /// Canonical widening order for every projection that renders one row per
    /// admitted scalar. `I64` and `Bool` lead because they are the frozen v1
    /// base that already-published artifacts encode.
    pub(super) const WIDENED: [Self; 5] = [Self::I32, Self::U8, Self::Char, Self::F32, Self::F64];

    pub(super) const fn text(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I32 => "i32",
            Self::U8 => "u8",
            Self::Char => "char",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }

    /// `i64` is the only admitted scalar whose Wasm value type exceeds the
    /// exact integer range of a JavaScript number, so it alone is a `bigint`.
    pub(super) const fn typescript_type(self) -> &'static str {
        match self {
            Self::I64 => "bigint",
            Self::I32 | Self::U8 | Self::Char | Self::F32 | Self::F64 => "number",
            Self::Bool => "boolean",
        }
    }

    /// Exactly the Core-Wasm value-type lowering the monomorphic callee already
    /// uses (`wasm_type` in the backend), so an adapter forwards its parameters
    /// without conversion: `i32`, `u8`, `char`, and `bool` ride the `i32` lane.
    pub(super) const fn wasm_type(self) -> u8 {
        match self {
            Self::I64 => I64,
            Self::I32 | Self::U8 | Self::Char | Self::Bool => I32,
            Self::F32 => F32,
            Self::F64 => F64,
        }
    }

    /// True when the Wasm value type is wider than the SEMAPRAX type, so a
    /// host caller can present a representation the language does not admit.
    /// `i64`, `i32`, `f32`, and `f64` occupy their value type exactly.
    const fn needs_boundary_trap(self) -> bool {
        matches!(self, Self::Bool | Self::U8 | Self::Char)
    }

    /// The JavaScript predicate that rejects every host value outside the exact
    /// admitted range of one widened scalar. `Number.isInteger` already rejects
    /// non-numbers, bigints, and non-integral doubles, and requiring
    /// `Math.fround` identity makes an `f32` narrowing explicit at the call
    /// site instead of silently rounding a double at the boundary.
    const fn javascript_rejection(self) -> &'static str {
        match self {
            Self::I32 => "!Number.isInteger(value) || value < -2147483648 || value > 2147483647",
            Self::U8 => "!Number.isInteger(value) || value < 0 || value > 255",
            Self::Char => {
                "!Number.isInteger(value) || value < 0 || value > 1114111 || (value >= 55296 && value <= 57343)"
            }
            Self::F32 => {
                "typeof value !== \"number\" || (!Number.isNaN(value) && Math.fround(value) !== value)"
            }
            Self::F64 => "typeof value !== \"number\"",
            // The frozen base guards live in the runtime template itself.
            Self::I64 | Self::Bool => "true",
        }
    }

    const fn javascript_expectation(self) -> &'static str {
        match self {
            Self::I32 => "a signed 32-bit integer number",
            Self::U8 => "an integer number in 0..=255",
            Self::Char => "a Unicode scalar value number",
            Self::F32 => "a number exactly representable as f32",
            Self::F64 => "a number",
            Self::I64 | Self::Bool => "unreachable",
        }
    }

    /// `non-canonical` names a value the Wasm lane can hold but the SEMAPRAX
    /// type cannot, exactly as the frozen `bool` result guard already says.
    const fn javascript_defect(self) -> &'static str {
        match self {
            Self::U8 | Self::Char => "non-canonical",
            _ => "invalid",
        }
    }
}

/// The admitted ABI type spellings, as they appear in a rendered manifest.
pub(super) fn is_admitted_abi_text(value: Option<&str>) -> bool {
    value.is_some_and(|text| {
        [ScalarType::I64, ScalarType::Bool]
            .into_iter()
            .chain(ScalarType::WIDENED)
            .any(|ty| ty.text() == text)
    })
}

/// The widened scalars this package actually projects, in canonical order.
fn widened_in_use(plans: &[ScalarExportPlan]) -> impl Iterator<Item = ScalarType> + '_ {
    ScalarType::WIDENED.into_iter().filter(|ty| {
        plans
            .iter()
            .any(|plan| plan.result == *ty || plan.params.contains(ty))
    })
}

/// Extra `argument` guards for the generated JavaScript facade.
///
/// The `i64` and `bool` guards are frozen in the runtime template, so a
/// package that projects only those two scalars renders the empty string here
/// and reproduces every already-published v1 facade byte for byte.
pub(super) fn javascript_argument_guards(plans: &[ScalarExportPlan]) -> String {
    widened_in_use(plans)
        .map(|ty| {
            javascript_guard(
                ty,
                &format!(
                    "`argument ${{index}} must be {}`",
                    ty.javascript_expectation()
                ),
            )
        })
        .collect()
}

/// Extra `result` guards for the generated JavaScript facade, which reject a
/// raw adapter return the SEMAPRAX result type cannot contain.
pub(super) fn javascript_result_guards(plans: &[ScalarExportPlan]) -> String {
    widened_in_use(plans)
        .map(|ty| {
            javascript_guard(
                ty,
                &format!(
                    "\"SEMAPRAX adapter returned {} {}\"",
                    ty.javascript_defect(),
                    ty.text()
                ),
            )
        })
        .collect()
}

fn javascript_guard(ty: ScalarType, failure: &str) -> String {
    format!(
        "  if (type === \"{}\") {{\n    if ({}) throw new TypeError({failure});\n    return value;\n  }}\n",
        ty.text(),
        ty.javascript_rejection(),
    )
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
            ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::ByteRange { .. } => {
                return Err(admission(format!(
                    "Public Scalar Export Profile v1 function `{function_id}` contains portable byte data"
                )));
            }
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
            ResolvedExprKind::HostCommandCall(_) => {
                return Err(admission(format!(
                    "Public Scalar Export Profile v1 function `{function_id}` uses command I/O"
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
                    for index in 0..statement.child_count() {
                        pending.push(
                            statement
                                .child(index)
                                .expect("resolved statement child count is canonical"),
                        );
                    }
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
        ResolvedType::I32 => Some(ScalarType::I32),
        ResolvedType::U8 => Some(ScalarType::U8),
        ResolvedType::Char => Some(ScalarType::Char),
        ResolvedType::F32 => Some(ScalarType::F32),
        ResolvedType::F64 => Some(ScalarType::F64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        // `usize` is deliberately excluded: its width is a host fact, not a
        // public fact of this profile. Every other exclusion needs the
        // memory/ownership ABI the owned-data programme owns.
        ResolvedType::Unit
        | ResolvedType::Usize
        | ResolvedType::String
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::ArrayU8(_)
        | ResolvedType::Bytes
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

/// Trap unless the given local already holds a canonical representation of the
/// scalar it claims to be. The check runs on the export edge, so a host caller
/// that presents an inadmissible number fails closed instead of reaching a
/// verified body with a value its type does not contain. Scalars that occupy
/// their Wasm value type exactly need no check and emit no bytes.
fn emit_boundary_trap(body: &mut impl ByteOutput, ty: ScalarType, local: u32) {
    match ty {
        ScalarType::Bool => emit_unsigned_ceiling_trap(body, local, 1),
        ScalarType::U8 => emit_unsigned_ceiling_trap(body, local, 0xff),
        ScalarType::Char => {
            emit_unsigned_ceiling_trap(body, local, 0x0010_ffff);
            emit_surrogate_trap(body, local);
        }
        ScalarType::I64 | ScalarType::I32 | ScalarType::F32 | ScalarType::F64 => {}
    }
}

/// Trap if the given i32 local exceeds `ceiling` when read as an unsigned
/// 32-bit number, which rejects every negative host value as well.
fn emit_unsigned_ceiling_trap(body: &mut impl ByteOutput, local: u32, ceiling: i32) {
    body.push(0x20); // local.get
    write_u32(body, local);
    body.push(0x41); // i32.const
    write_i32(body, ceiling);
    body.extend_bytes(&[0x4b, 0x04, 0x40, 0x00, 0x0b]);
    // i32.gt_u; if (empty); unreachable; end
}

/// Trap if the given i32 local names a UTF-16 surrogate code point, which is
/// a code point but never a Unicode scalar value.
fn emit_surrogate_trap(body: &mut impl ByteOutput, local: u32) {
    body.push(0x20); // local.get
    write_u32(body, local);
    body.push(0x41); // i32.const
    write_i32(body, 0xd800);
    body.push(0x6b); // i32.sub
    body.push(0x41); // i32.const
    write_i32(body, 0x800);
    body.extend_bytes(&[0x49, 0x04, 0x40, 0x00, 0x0b]);
    // i32.lt_u; if (empty); unreachable; end
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

    /// The backend's ABI spellings and the linker's Copy-scalar vocabulary
    /// describe one surface. A widening that reaches only one of them would
    /// let a linked program lose its manifest, or the reverse.
    #[test]
    fn abi_spellings_equal_the_shared_copy_scalar_vocabulary() {
        let backend = [ScalarType::I64, ScalarType::Bool]
            .into_iter()
            .chain(ScalarType::WIDENED)
            .map(ScalarType::text)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            backend,
            crate::hir::COPY_SCALAR_NAMES.into_iter().collect(),
            "the Wasm ABI vocabulary drifted from the shared Copy-scalar names"
        );
        for name in crate::hir::COPY_SCALAR_NAMES {
            assert!(is_admitted_abi_text(Some(name)), "{name} is not admitted");
        }
        for excluded in ["usize", "string", "str", "Bytes", "", "I64"] {
            assert!(!is_admitted_abi_text(Some(excluded)), "{excluded} admitted");
        }
        assert!(!is_admitted_abi_text(None));
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
