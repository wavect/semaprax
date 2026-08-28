//! Target-neutral admission for the closed Useful Data Command v2 boundary.
//!
//! Native and Wasm projections consume this one authenticated plan.  The
//! command is deliberately not a general process API: its two borrowed byte
//! roots are supplied by a fixed adapter and its only semantic authority is a
//! bounded, success-published stdout transcript.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, ByteSliceExtent, ByteSliceRootKind, DeclarationId, IdentityOrigin, OwnershipMode,
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement,
    ResolvedType, ValueId,
};

const MAX_FUNCTIONS: usize = 256;
const MAX_STABLE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandProfilePlan {
    function_id: DeclarationId,
    stdout_capacity: u64,
}

impl CommandProfilePlan {
    pub(crate) fn prepare(program: &ResolvedProgram, command_id: &str) -> Result<Self, Diagnostic> {
        validate_id(command_id)?;
        hir::validate(program)?;
        crate::host_io_ops::validate_stdout_profile_authority(program)?;
        if program.functions.is_empty() || program.functions.len() > MAX_FUNCTIONS {
            return Err(capacity(format!(
                "Useful Data Command v2 admits 1..={MAX_FUNCTIONS} functions"
            )));
        }

        let functions = program
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        let command = functions.get(command_id).copied().ok_or_else(|| {
            admission(format!(
                "command identity `{command_id}` does not name a monomorphic function"
            ))
        })?;
        require_explicit(program, command)?;
        if command.params.len() != 2
            || command.params.iter().any(|parameter| {
                parameter.ty != ResolvedType::SliceU8
                    || parameter.ownership != OwnershipMode::Borrow
            })
            || command.return_type != ResolvedType::Bool
        {
            return Err(admission(format!(
                "command `{command_id}` must be exactly `(borrow Slice<u8>, borrow Slice<u8>) -> bool`"
            )));
        }
        if command.effects != [crate::host_io_ops::STDOUT_WRITE_EFFECT] {
            return Err(admission(format!(
                "command `{command_id}` must declare exactly `process.stdout.write`"
            )));
        }
        let external_roots = command
            .params
            .iter()
            .map(|parameter| parameter.id.clone())
            .collect::<BTreeSet<_>>();
        for parameter in &command.params {
            let provenance = program
                .declarations
                .byte_slice_provenance(&parameter.id)
                .ok_or_else(|| admission("command byte-slice parameter lacks provenance"))?;
            if provenance.root != parameter.id
                || provenance.root_kind != ByteSliceRootKind::FunctionParameter
                || provenance.root_length != ByteSliceExtent::ParameterLength
                || provenance.offset != ByteSliceExtent::Constant(0)
                || provenance.length != ByteSliceExtent::ParameterLength
                || provenance.producer.is_some()
            {
                return Err(admission(format!(
                    "command `{command_id}` parameter is not an exact full external root"
                )));
            }
        }

        let entry = functions
            .get(program.entrypoint.as_str())
            .copied()
            .ok_or_else(|| admission("command-profile entrypoint is absent"))?;
        if entry.name != "main"
            || !entry.params.is_empty()
            || entry.return_type != ResolvedType::I64
        {
            return Err(admission(
                "Useful Data Command v2 entrypoint must remain an exact `fn main() -> i64`",
            ));
        }

        let mut call_graph = BTreeMap::new();
        for function in &program.functions {
            if function.id != command.id && !function.effects.is_empty() {
                return Err(admission(format!(
                    "non-command function `{}` must be effect-free",
                    function.id
                )));
            }
            let mut callees = Vec::new();
            validate_function(
                program,
                function,
                &functions,
                &mut callees,
                (function.id == command.id).then_some(&external_roots),
            )?;
            callees.sort();
            callees.dedup();
            call_graph.insert(function.id.clone(), callees);
        }
        reject_call_cycles(&call_graph)?;

        Ok(Self {
            function_id: command.id.clone(),
            stdout_capacity: crate::host_io_ops::MAX_STDOUT_TRANSCRIPT_BYTES,
        })
    }

    pub(crate) fn function_id(&self) -> &DeclarationId {
        &self.function_id
    }

    pub(crate) const fn stdout_capacity(&self) -> u64 {
        self.stdout_capacity
    }
}

fn validate_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    functions: &BTreeMap<&str, &ResolvedFunction>,
    callees: &mut Vec<DeclarationId>,
    stdout_external_roots: Option<&BTreeSet<ValueId>>,
) -> Result<(), Diagnostic> {
    if !function.requires.is_empty() || !function.ensures.is_empty() {
        return Err(admission(format!(
            "Useful Data Command v2 function `{}` must be contract-free",
            function.id
        )));
    }
    for parameter in &function.params {
        if !internal_parameter(&parameter.ty, parameter.ownership) {
            return Err(admission(format!(
                "Useful Data Command v2 function `{}` has an unsupported parameter",
                function.id
            )));
        }
    }
    if !internal_result(&function.return_type) {
        return Err(admission(format!(
            "Useful Data Command v2 function `{}` has an unsupported result",
            function.id
        )));
    }

    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if !internal_expression_type(&expression.ty) {
            return Err(admission(format!(
                "Useful Data Command v2 function `{}` reaches an unsupported type",
                function.id
            )));
        }
        match &expression.kind {
            ResolvedExprKind::String(_)
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_) => {
                return Err(admission(format!(
                    "Useful Data Command v2 function `{}` reaches text allocation, an import, or command I/O",
                    function.id
                )));
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(admission(format!(
                        "Useful Data Command v2 function `{}` reaches a generic call",
                        function.id
                    )));
                }
                pending.extend(args);
                if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                    let roots = stdout_external_roots.ok_or_else(|| {
                        admission(format!(
                            "non-command function `{}` may not write stdout",
                            function.id
                        ))
                    })?;
                    validate_stdout_external_argument(program, function, args, roots)?;
                } else if crate::byte_ops::by_id(callee.as_str()).is_none() {
                    if !functions.contains_key(callee.as_str()) {
                        return Err(admission(format!(
                            "Useful Data Command v2 function `{}` reaches unavailable call `{callee}`",
                            function.id
                        )));
                    }
                    callees.push(callee.clone());
                }
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return Err(admission(format!(
                            "Useful Data Command v2 function `{}` reaches an unsafe boundary",
                            function.id
                        )));
                    }
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
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
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                pending.push(operand);
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Project { base, .. } => pending.push(base),
            ResolvedExprKind::Upcast { source } => pending.push(source),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push(end);
                pending.push(start);
                pending.push(source);
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::Place(_) => {}
            ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_) => {
                return Err(admission(format!(
                    "Useful Data Command v2 function `{}` reaches a non-profile scalar",
                    function.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_stdout_external_argument(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    args: &[ResolvedExpr],
    external_roots: &BTreeSet<ValueId>,
) -> Result<(), Diagnostic> {
    let [argument] = args else {
        return Err(admission("stdout_write argument inventory is not exact"));
    };
    let place = match &argument.kind {
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. }
            if place.projections.is_empty() =>
        {
            place
        }
        _ => {
            return Err(admission(format!(
                "command `{}` must write an external Slice parameter or immutable alias",
                function.id
            )));
        }
    };
    let provenance = program
        .declarations
        .byte_slice_provenance(&place.root)
        .ok_or_else(|| admission("stdout_write operand lacks authenticated byte provenance"))?;
    if provenance.root_kind != ByteSliceRootKind::FunctionParameter
        || provenance.root_length != ByteSliceExtent::ParameterLength
        || provenance.offset != ByteSliceExtent::Constant(0)
        || provenance.length != ByteSliceExtent::ParameterLength
        || !external_roots.contains(&provenance.root)
    {
        return Err(admission(format!(
            "command `{}` stdout_write operand is not rooted in an external Slice parameter",
            function.id
        )));
    }
    Ok(())
}

fn internal_parameter(ty: &ResolvedType, ownership: OwnershipMode) -> bool {
    match ty {
        ResolvedType::Bytes => ownership == OwnershipMode::Own,
        ResolvedType::SliceU8 => ownership == OwnershipMode::Borrow,
        _ => ownership == OwnershipMode::Value && internal_result(ty),
    }
}

fn internal_result(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::Bool
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::Bytes
    ) || ty.is_compiler_byte_option()
}

fn internal_expression_type(ty: &ResolvedType) -> bool {
    *ty == ResolvedType::Unit || internal_result(ty) || *ty == ResolvedType::SliceU8
}

fn require_explicit(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    if program
        .declarations
        .declaration(&function.id)
        .is_none_or(|declaration| declaration.identity_origin != IdentityOrigin::Explicit)
    {
        return Err(admission(format!(
            "command `{}` must have an explicit stable identity",
            function.id
        )));
    }
    Ok(())
}

fn reject_call_cycles(
    call_graph: &BTreeMap<DeclarationId, Vec<DeclarationId>>,
) -> Result<(), Diagnostic> {
    fn visit(
        id: &DeclarationId,
        call_graph: &BTreeMap<DeclarationId, Vec<DeclarationId>>,
        active: &mut BTreeSet<DeclarationId>,
        complete: &mut BTreeSet<DeclarationId>,
    ) -> Result<(), Diagnostic> {
        if complete.contains(id) {
            return Ok(());
        }
        if !active.insert(id.clone()) {
            return Err(admission(format!(
                "Useful Data Command v2 reaches a recursive call cycle at `{id}`"
            )));
        }
        if let Some(callees) = call_graph.get(id) {
            for callee in callees {
                visit(callee, call_graph, active, complete)?;
            }
        }
        active.remove(id);
        complete.insert(id.clone());
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in call_graph.keys() {
        visit(id, call_graph, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), Diagnostic> {
    if !(1..=MAX_STABLE_ID_BYTES).contains(&id.len()) {
        return Err(capacity(format!(
            "command IDs must contain 1..={MAX_STABLE_ID_BYTES} bytes"
        )));
    }
    if !id.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    }) {
        return Err(admission(format!(
            "command ID `{id}` must use lowercase [a-z0-9._-]"
        )));
    }
    Ok(())
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W121", message)
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W122", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::CommandProfilePlan;

    const COMMAND: &str = r#"
module test.command_v2;
permit { process.stdout.write }
@id("command.run")
fn run(input: borrow Slice<u8>, needle: borrow Slice<u8>) -> bool
    uses { process.stdout.write }
{
    if byte_len(needle) == 0usize {
        stdout_write(input) == byte_len(input)
    } else {
        false
    }
}
@id("main") fn main() -> i64 { 0 }
"#;

    fn resolved(source: &str) -> crate::hir::ResolvedProgram {
        let parsed = crate::parse(source, Path::new("command-v2.spx")).unwrap();
        crate::hir::resolve(&parsed).unwrap()
    }

    #[test]
    fn exact_two_slice_bool_boundary_is_admitted() {
        let program = resolved(COMMAND);
        let plan = CommandProfilePlan::prepare(&program, "command.run").unwrap();
        assert_eq!(plan.function_id().as_str(), "command.run");
        assert_eq!(plan.stdout_capacity(), 65_536);
    }

    #[test]
    fn signature_and_non_command_stdout_authority_fail_closed() {
        let wrong_signature = resolved(&COMMAND.replace(
            "input: borrow Slice<u8>, needle: borrow Slice<u8>",
            "input: borrow Slice<u8>, needle: borrow Slice<u8>, extra: borrow Slice<u8>",
        ));
        let error = CommandProfilePlan::prepare(&wrong_signature, "command.run").unwrap_err();
        assert_eq!(error.code, "SPX-W121");
        assert!(error.message.contains("must be exactly"));

        let helper = COMMAND
            .replace(
                "@id(\"command.run\")",
                r#"@id("command.helper")
fn helper(input: borrow Slice<u8>) -> usize uses { process.stdout.write } {
    stdout_write(input)
}
@id("command.run")"#,
            )
            .replace("stdout_write(input)", "helper(input)");
        let error = CommandProfilePlan::prepare(&resolved(&helper), "command.run").unwrap_err();
        assert_eq!(error.code, "SPX-W121");
        assert!(error.message.contains("non-command function"));
    }
}
