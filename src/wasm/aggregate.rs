//! Deterministic Wasm32 lowering for executable aggregate records v1.
//!
//! This is deliberately isolated from the scalar encoder so existing scalar,
//! owned-resource, callable, and Component byte contracts remain unchanged.

use std::collections::HashMap;

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::{BinaryOp, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};

use super::{
    function_import, intern_type, section, write_bytes, write_i64, write_name, write_u32,
    Signature, I32, I64, SCALAR_IMPORT_COUNT,
};

const SHADOW_STACK_TOP: u32 = 65_536;
const STATUS_SUCCESS: i32 = 0;
const STATUS_ADD_OVERFLOW: i32 = 1;
const STATUS_SUB_OVERFLOW: i32 = 2;
const STATUS_MUL_OVERFLOW: i32 = 3;
const STATUS_DIV_ZERO: i32 = 4;
const STATUS_DIV_OVERFLOW: i32 = 5;
const STATUS_REM_ZERO: i32 = 6;
const STATUS_REM_OVERFLOW: i32 = 7;
const STATUS_NEG_OVERFLOW: i32 = 8;
const STATUS_REQUIRES_FALSE: i32 = 9;
const STATUS_ENSURES_FALSE: i32 = 10;

#[derive(Clone, Copy)]
struct Pointer {
    local: u32,
    offset: u32,
}

#[derive(Clone)]
enum Value {
    Scalar { local: u32, ty: ResolvedType },
    ScalarMemory { pointer: Pointer, ty: ResolvedType },
    Aggregate { pointer: Pointer, ty: ResolvedType },
}

#[derive(Default)]
struct FrameAllocator {
    size: u32,
    align: u32,
}

impl FrameAllocator {
    fn allocate(&mut self, size: u32, align: u32) -> Result<u32, Diagnostic> {
        if !align.is_power_of_two() {
            return Err(error("aggregate frame alignment is not a power of two"));
        }
        let mask = align - 1;
        let offset = self
            .size
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| error("aggregate frame alignment overflows u32"))?;
        self.size = offset
            .checked_add(size)
            .ok_or_else(|| error("aggregate frame size overflows u32"))?;
        self.align = self.align.max(align);
        Ok(offset)
    }

    fn finish(&self) -> Result<u32, Diagnostic> {
        let align = self.align.max(8);
        let mask = align - 1;
        self.size
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| error("aggregate frame rounding overflows u32"))
    }
}

struct FunctionPlan {
    local_types: Vec<u8>,
    old_stack: u32,
    frame_base: u32,
    status: u32,
    result_out: u32,
    result_stage_scalar: Option<u32>,
    result_stage_aggregate: Option<u32>,
    scalar_expressions: HashMap<ExpressionId, u32>,
    scalar_bindings: HashMap<ValueId, u32>,
    aggregate_expressions: HashMap<ExpressionId, u32>,
    aggregate_bindings: HashMap<ValueId, u32>,
    call_out: HashMap<ExpressionId, u32>,
    frame_size: u32,
}

impl FunctionPlan {
    fn build(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<Self, Diagnostic> {
        let parameter_count = u32::try_from(function.params.len())
            .map_err(|_| error("aggregate function has too many parameters"))?;
        let result_out = parameter_count;
        let mut local_types = Vec::new();
        let mut add_local = |ty: u8| -> Result<u32, Diagnostic> {
            let relative = u32::try_from(local_types.len())
                .map_err(|_| error("aggregate function has too many locals"))?;
            local_types.push(ty);
            parameter_count
                .checked_add(1)
                .and_then(|base| base.checked_add(relative))
                .ok_or_else(|| error("aggregate local index overflows u32"))
        };
        let old_stack = add_local(I32)?;
        let frame_base = add_local(I32)?;
        let status = add_local(I32)?;

        let mut frame = FrameAllocator::default();
        let (result_stage_scalar, result_stage_aggregate) =
            if is_record(program, &function.return_type)? {
                let layout = layout(program, &function.return_type)?;
                (None, Some(frame.allocate(layout.size, layout.align)?))
            } else {
                (
                    Some(add_local(scalar_wasm_type(&function.return_type)?)?),
                    None,
                )
            };
        let mut plan = Self {
            local_types,
            old_stack,
            frame_base,
            status,
            result_out,
            result_stage_scalar,
            result_stage_aggregate,
            scalar_expressions: HashMap::new(),
            scalar_bindings: HashMap::new(),
            aggregate_expressions: HashMap::new(),
            aggregate_bindings: HashMap::new(),
            call_out: HashMap::new(),
            frame_size: 0,
        };
        for contract in &function.requires {
            plan.collect_expr(program, contract, parameter_count, &mut frame)?;
        }
        plan.collect_expr(program, &function.body, parameter_count, &mut frame)?;
        for contract in &function.ensures {
            plan.collect_expr(program, contract, parameter_count, &mut frame)?;
        }
        plan.frame_size = frame.finish()?;
        Ok(plan)
    }

    fn add_local(&mut self, parameter_count: u32, ty: u8) -> Result<u32, Diagnostic> {
        let relative = u32::try_from(self.local_types.len())
            .map_err(|_| error("aggregate function has too many locals"))?;
        self.local_types.push(ty);
        parameter_count
            .checked_add(1)
            .and_then(|base| base.checked_add(relative))
            .ok_or_else(|| error("aggregate local index overflows u32"))
    }

    fn collect_expr(
        &mut self,
        program: &ResolvedProgram,
        expr: &ResolvedExpr,
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        if is_record(program, &expr.ty)? {
            let record = layout(program, &expr.ty)?;
            let offset = frame.allocate(record.size, record.align)?;
            if self
                .aggregate_expressions
                .insert(expr.id.clone(), offset)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate aggregate expression identity `{}`",
                    expr.id
                )));
            }
        } else {
            let ty = scalar_wasm_type(&expr.ty)?;
            let local = self.add_local(parameter_count, ty)?;
            if self
                .scalar_expressions
                .insert(expr.id.clone(), local)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate scalar expression identity `{}`",
                    expr.id
                )));
            }
            if matches!(expr.kind, ResolvedExprKind::Call { .. }) {
                let (size, align) = scalar_size_align(&expr.ty)?;
                self.call_out
                    .insert(expr.id.clone(), frame.allocate(size, align)?);
            }
        }

        match &expr.kind {
            ResolvedExprKind::Call { args, .. } => {
                for arg in args {
                    self.collect_expr(program, arg, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::Unary { value, .. } => {
                self.collect_expr(program, value, parameter_count, frame)?;
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                self.collect_expr(program, left, parameter_count, frame)?;
                self.collect_expr(program, right, parameter_count, frame)?;
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    self.collect_expr(program, value, parameter_count, frame)?;
                    if is_record(program, &binding.ty)? {
                        let record = layout(program, &binding.ty)?;
                        let offset = frame.allocate(record.size, record.align)?;
                        if self
                            .aggregate_bindings
                            .insert(binding.id.clone(), offset)
                            .is_some()
                        {
                            return Err(error(format!(
                                "duplicate aggregate binding identity `{}`",
                                binding.id
                            )));
                        }
                    } else {
                        let local =
                            self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                        if self
                            .scalar_bindings
                            .insert(binding.id.clone(), local)
                            .is_some()
                        {
                            return Err(error(format!(
                                "duplicate scalar binding identity `{}`",
                                binding.id
                            )));
                        }
                    }
                }
                self.collect_expr(program, tail, parameter_count, frame)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(program, condition, parameter_count, frame)?;
                self.collect_expr(program, then_branch, parameter_count, frame)?;
                self.collect_expr(program, else_branch, parameter_count, frame)?;
            }
            ResolvedExprKind::ConstructRecord { fields, .. } => {
                for field in fields {
                    self.collect_expr(program, &field.value, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::Project { base, .. } => {
                self.collect_expr(program, base, parameter_count, frame)?;
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                self.collect_expr(program, base, parameter_count, frame)?;
                for field in fields {
                    self.collect_expr(program, &field.value, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        }
        Ok(())
    }

    fn expr_scalar(&self, expr: &ResolvedExpr) -> Result<u32, Diagnostic> {
        self.scalar_expressions
            .get(&expr.id)
            .copied()
            .ok_or_else(|| error(format!("missing scalar local for `{}`", expr.id)))
    }

    fn expr_pointer(&self, expr: &ResolvedExpr) -> Result<Pointer, Diagnostic> {
        self.aggregate_expressions
            .get(&expr.id)
            .copied()
            .map(|offset| Pointer {
                local: self.frame_base,
                offset,
            })
            .ok_or_else(|| error(format!("missing aggregate slot for `{}`", expr.id)))
    }
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W110", message)
}

fn resource_gate() -> Diagnostic {
    Diagnostic::io(
        "SPX-W111",
        "WebAssembly aggregate resource execution remains private and is not admitted by the public backend",
    )
}

fn is_record(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !arguments.is_empty() {
        return Err(error(format!(
            "generic aggregate type `{}` is outside executable records v1",
            ty.identity_key()
        )));
    }
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| error(format!("unknown aggregate type `{declaration}`")))?;
    Ok(matches!(
        item.kind,
        ResolvedTypeDeclarationKind::Record { .. }
    ))
}

fn layout(program: &ResolvedProgram, ty: &ResolvedType) -> Result<AggregateLayout, Diagnostic> {
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return Err(error(format!(
            "aggregate layout requested for scalar `{}`",
            ty.identity_key()
        )));
    };
    let layout = AggregateLayout::for_record(program, AggregateTarget::Wasm32, declaration)?;
    layout.validate(program)?;
    Ok(layout)
}

fn scalar_wasm_type(ty: &ResolvedType) -> Result<u8, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok(I64),
        ResolvedType::Bool => Ok(I32),
        _ => Err(error(format!(
            "non-scalar type `{}` reached scalar aggregate lowering",
            ty.identity_key()
        ))),
    }
}

fn scalar_size_align(ty: &ResolvedType) -> Result<(u32, u32), Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok((8, 8)),
        ResolvedType::Bool => Ok((4, 4)),
        _ => Err(error(format!(
            "non-scalar type `{}` has no Wasm32 scalar layout",
            ty.identity_key()
        ))),
    }
}

pub(super) fn emit(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    emit_profile(program, false)
}

fn emit_profile(program: &ResolvedProgram, test_exports: bool) -> Result<Vec<u8>, Diagnostic> {
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(resource_gate());
    }
    for item in &program.types {
        if matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. }) {
            let ty = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            };
            let facts = program
                .declarations
                .type_facts(&ty)
                .ok_or_else(|| error(format!("missing type facts for `{}`", item.id)))?;
            if facts.contains_resource {
                return Err(resource_gate());
            }
            layout(program, &ty)?;
        }
    }

    let main_index = program
        .functions
        .iter()
        .position(|function| function.id == program.entrypoint)
        .ok_or_else(|| error("web target requires a main function"))?;
    let main = &program.functions[main_index];
    if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
        return Err(error(
            "resolved web entry point must have type `fn main() -> i64`",
        ));
    }

    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let binary_checked = intern_type(
        Signature {
            params: vec![I64, I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let unary_checked = intern_type(
        Signature {
            params: vec![I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let contract_fail = intern_type(
        Signature {
            params: Vec::new(),
            results: Vec::new(),
        },
        &mut types,
        &mut type_indexes,
    );

    let mut function_types = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for param in &function.params {
            params.push(if is_record(program, &param.ty)? {
                I32
            } else {
                scalar_wasm_type(&param.ty)?
            });
        }
        params.push(I32);
        function_types.push(intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    let wrapper_type = intern_type(
        Signature {
            params: Vec::new(),
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let function_indexes = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                function.id.clone(),
                SCALAR_IMPORT_COUNT + u32::try_from(index).unwrap_or(u32::MAX),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut imports = Vec::new();
    write_u32(&mut imports, SCALAR_IMPORT_COUNT);
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    section(&mut module, 2, imports);

    let mut function_section = Vec::new();
    write_u32(
        &mut function_section,
        u32::try_from(function_types.len() + 1)
            .map_err(|_| error("too many aggregate functions"))?,
    );
    for ty in function_types {
        write_u32(&mut function_section, ty);
    }
    write_u32(&mut function_section, wrapper_type);
    section(&mut module, 3, function_section);

    let mut memory = Vec::new();
    write_u32(&mut memory, 1);
    memory.extend([0x00, 0x01]);
    section(&mut module, 5, memory);

    let mut globals = Vec::new();
    write_u32(&mut globals, 1);
    globals.extend([I32, 0x01, 0x41]);
    write_i64(&mut globals, i64::from(SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);

    let mut exports = Vec::new();
    let extra_exports = if test_exports {
        u32::try_from(program.functions.len())
            .map_err(|_| error("too many aggregate test exports"))?
            .checked_add(2)
            .ok_or_else(|| error("aggregate test export count overflows u32"))?
    } else {
        0
    };
    write_u32(&mut exports, 1 + extra_exports);
    write_name(&mut exports, "semaprax_main");
    exports.push(0x00);
    let wrapper_index = SCALAR_IMPORT_COUNT
        .checked_add(
            u32::try_from(program.functions.len()).map_err(|_| error("too many functions"))?,
        )
        .ok_or_else(|| error("aggregate wrapper index overflows u32"))?;
    write_u32(&mut exports, wrapper_index);
    if test_exports {
        write_name(&mut exports, "__spx_test_memory");
        exports.push(0x02);
        write_u32(&mut exports, 0);
        write_name(&mut exports, "__spx_test_shadow_stack");
        exports.push(0x03);
        write_u32(&mut exports, 0);
        for function in &program.functions {
            write_name(
                &mut exports,
                &format!("__spx_test_{}", hex_identity(&function.id)),
            );
            exports.push(0x00);
            write_u32(
                &mut exports,
                *function_indexes
                    .get(&function.id)
                    .ok_or_else(|| error("aggregate test function is not indexed"))?,
            );
        }
    }
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(
        &mut code,
        u32::try_from(program.functions.len() + 1)
            .map_err(|_| error("too many aggregate function bodies"))?,
    );
    for function in &program.functions {
        let body = emit_function(program, function, &function_indexes)?;
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    let wrapper = emit_wrapper(
        *function_indexes
            .get(&main.id)
            .ok_or_else(|| error("aggregate main function is not indexed"))?,
    );
    write_u32(&mut code, wrapper.len() as u32);
    code.extend(wrapper);
    section(&mut module, 10, code);
    Ok(module)
}

fn hex_identity(id: &DeclarationId) -> String {
    let mut output = String::new();
    for byte in id.as_str().bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn emit_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    function_indexes: &HashMap<DeclarationId, u32>,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = FunctionPlan::build(program, function)?;
    let mut body = Vec::new();
    write_u32(&mut body, plan.local_types.len() as u32);
    for ty in &plan.local_types {
        write_u32(&mut body, 1);
        body.push(*ty);
    }

    body.push(0x23);
    write_u32(&mut body, 0);
    body.push(0x21);
    write_u32(&mut body, plan.old_stack);
    if plan.frame_size != 0 {
        body.push(0x20);
        write_u32(&mut body, plan.old_stack);
        body.push(0x41);
        write_i64(&mut body, i64::from(plan.frame_size));
        body.push(0x49);
        body.extend([0x04, 0x40, 0x00, 0x0b]);
    }
    body.push(0x20);
    write_u32(&mut body, plan.old_stack);
    body.push(0x41);
    write_i64(&mut body, i64::from(plan.frame_size));
    body.push(0x6b);
    body.push(0x22);
    write_u32(&mut body, plan.frame_base);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x41);
    write_i64(&mut body, i64::from(STATUS_SUCCESS));
    body.push(0x21);
    write_u32(&mut body, plan.status);

    let mut bindings = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let local = u32::try_from(index).map_err(|_| error("parameter index overflows u32"))?;
        let value = if is_record(program, &param.ty)? {
            Value::Aggregate {
                pointer: Pointer { local, offset: 0 },
                ty: param.ty.clone(),
            }
        } else {
            Value::Scalar {
                local,
                ty: param.ty.clone(),
            }
        };
        bindings.insert(param.id.clone(), value);
    }
    body.extend([0x02, 0x40]);
    let mut emitter = Emitter {
        output: &mut body,
        program,
        function_indexes,
        plan: &plan,
        bindings,
        control_depth: 0,
    };
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_scalar(&condition, &ResolvedType::Bool, "precondition")?;
        emitter.get_scalar(&condition);
        emitter.output.push(0x45);
        emitter.fail_if(STATUS_REQUIRES_FALSE);
    }
    let result = emitter.emit_expr(&function.body)?;
    let staged = if let Some(local) = plan.result_stage_scalar {
        emitter.require_scalar(&result, &function.return_type, "function result")?;
        emitter.get_scalar(&result);
        emitter.output.push(0x21);
        write_u32(emitter.output, local);
        Value::Scalar {
            local,
            ty: function.return_type.clone(),
        }
    } else {
        let offset = plan
            .result_stage_aggregate
            .ok_or_else(|| error("missing aggregate result staging slot"))?;
        let staged = Value::Aggregate {
            pointer: Pointer {
                local: plan.frame_base,
                offset,
            },
            ty: function.return_type.clone(),
        };
        emitter.copy_value(&staged, &result, "function result")?;
        staged
    };
    emitter
        .bindings
        .insert(function.result_id.clone(), staged.clone());
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_scalar(&condition, &ResolvedType::Bool, "postcondition")?;
        emitter.get_scalar(&condition);
        emitter.output.push(0x45);
        emitter.fail_if(STATUS_ENSURES_FALSE);
    }
    let caller = if is_record(program, &function.return_type)? {
        Value::Aggregate {
            pointer: Pointer {
                local: plan.result_out,
                offset: 0,
            },
            ty: function.return_type.clone(),
        }
    } else {
        Value::Scalar {
            local: plan.result_out,
            ty: function.return_type.clone(),
        }
    };
    if matches!(caller, Value::Aggregate { .. }) {
        emitter.copy_value(&caller, &staged, "caller result publication")?;
    } else {
        let Value::Scalar { local: source, ty } = staged else {
            return Err(error(
                "scalar caller publication received aggregate staging",
            ));
        };
        emitter.emit_pointer(Pointer {
            local: plan.result_out,
            offset: 0,
        });
        emitter.output.push(0x20);
        write_u32(emitter.output, source);
        emitter.store_scalar(&ty);
    }
    drop(emitter);
    body.push(0x0b);
    body.push(0x20);
    write_u32(&mut body, plan.old_stack);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x20);
    write_u32(&mut body, plan.status);
    body.push(0x0b);
    Ok(body)
}

struct Emitter<'a> {
    output: &'a mut Vec<u8>,
    program: &'a ResolvedProgram,
    function_indexes: &'a HashMap<DeclarationId, u32>,
    plan: &'a FunctionPlan,
    bindings: HashMap<ValueId, Value>,
    control_depth: u32,
}

impl Emitter<'_> {
    fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Int(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x42);
                write_i64(self.output, *value);
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::I64,
                })
            }
            ResolvedExprKind::Bool(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x41);
                write_i64(self.output, i64::from(*value));
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::Bool,
                })
            }
            ResolvedExprKind::Place(place) => {
                let value = self.place_value(place)?;
                self.materialize(expr, &value)
            }
            ResolvedExprKind::Call { callee, args } => self.emit_call(expr, callee, args),
            ResolvedExprKind::Unary { op, value } => self.emit_unary(expr, *op, value),
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(expr, *op, left, right)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.bindings.clone();
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    let value = self.emit_expr(value)?;
                    let destination = if is_record(self.program, &binding.ty)? {
                        let offset = self
                            .plan
                            .aggregate_bindings
                            .get(&binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!("missing aggregate binding `{}`", binding.id))
                            })?;
                        Value::Aggregate {
                            pointer: Pointer {
                                local: self.plan.frame_base,
                                offset,
                            },
                            ty: binding.ty.clone(),
                        }
                    } else {
                        let local = self
                            .plan
                            .scalar_bindings
                            .get(&binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!("missing scalar binding `{}`", binding.id))
                            })?;
                        Value::Scalar {
                            local,
                            ty: binding.ty.clone(),
                        }
                    };
                    self.copy_value(&destination, &value, "local binding")?;
                    self.bindings.insert(binding.id.clone(), destination);
                }
                let tail = self.emit_expr(tail)?;
                let result = self.materialize(expr, &tail)?;
                self.bindings = saved;
                Ok(result)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.emit_if(expr, condition, then_branch, else_branch),
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                let record_layout = layout(self.program, &expr.ty)?;
                if record_layout.record != *record {
                    return Err(error(format!(
                        "constructor `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                for initializer in fields {
                    let field = record_layout
                        .field(&initializer.field)
                        .cloned()
                        .ok_or_else(|| {
                            error(format!("unknown constructor field `{}`", initializer.field))
                        })?;
                    let value = self.emit_expr(&initializer.value)?;
                    let field_destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(field.offset)
                                .ok_or_else(|| error("field pointer overflows u32"))?,
                        },
                        field.ty,
                        self.program,
                    )?;
                    self.copy_value(&field_destination, &value, "record field initializer")?;
                }
                Ok(destination)
            }
            ResolvedExprKind::Project { base, field } => {
                let base = self.emit_expr(base)?;
                let projected = self.project_value(&base, field)?;
                self.materialize(expr, &projected)
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.emit_expr(base)?;
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                self.copy_value(&destination, &base, "record update base")?;
                let record_layout = layout(self.program, &expr.ty)?;
                if record_layout.record != *record {
                    return Err(error(format!(
                        "record update `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                for replacement in fields {
                    let field = record_layout
                        .field(&replacement.field)
                        .cloned()
                        .ok_or_else(|| {
                            error(format!("unknown update field `{}`", replacement.field))
                        })?;
                    let value = self.emit_expr(&replacement.value)?;
                    let field_destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(field.offset)
                                .ok_or_else(|| error("field pointer overflows u32"))?,
                        },
                        field.ty,
                        self.program,
                    )?;
                    self.copy_value(&field_destination, &value, "record update field")?;
                }
                Ok(destination)
            }
        }
    }

    fn materialize(&mut self, expr: &ResolvedExpr, source: &Value) -> Result<Value, Diagnostic> {
        let destination = if is_record(self.program, &expr.ty)? {
            Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: expr.ty.clone(),
            }
        } else {
            Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            }
        };
        self.copy_value(&destination, source, "expression materialization")?;
        Ok(destination)
    }

    fn emit_call(
        &mut self,
        expr: &ResolvedExpr,
        callee: &DeclarationId,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let target = self
            .program
            .functions
            .iter()
            .find(|function| function.id == *callee)
            .ok_or_else(|| error(format!("unknown aggregate callee `{callee}`")))?;
        if target.params.len() != args.len() {
            return Err(error(format!(
                "aggregate call `{callee}` has {} arguments; expected {}",
                args.len(),
                target.params.len()
            )));
        }
        let mut values = Vec::with_capacity(args.len());
        for (argument, parameter) in args.iter().zip(&target.params) {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), &parameter.ty, "call argument")?;
            values.push(value);
        }
        for value in &values {
            match value {
                Value::Scalar { local, .. } => {
                    self.output.push(0x20);
                    write_u32(self.output, *local);
                }
                Value::ScalarMemory { .. } => self.get_scalar(value),
                Value::Aggregate { pointer, .. } => self.emit_pointer(*pointer),
            }
        }
        let (result, result_pointer) =
            if is_record(self.program, &expr.ty)? {
                let pointer = self.plan.expr_pointer(expr)?;
                (
                    Value::Aggregate {
                        pointer,
                        ty: expr.ty.clone(),
                    },
                    pointer,
                )
            } else {
                let offset =
                    self.plan.call_out.get(&expr.id).copied().ok_or_else(|| {
                        error(format!("missing scalar call out slot `{}`", expr.id))
                    })?;
                (
                    Value::Scalar {
                        local: self.plan.expr_scalar(expr)?,
                        ty: expr.ty.clone(),
                    },
                    Pointer {
                        local: self.plan.frame_base,
                        offset,
                    },
                )
            };
        self.emit_pointer(result_pointer);
        self.output.push(0x10);
        write_u32(
            self.output,
            *self
                .function_indexes
                .get(callee)
                .ok_or_else(|| error(format!("callee `{callee}` is not indexed")))?,
        );
        self.output.push(0x22);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.output.push(0x0c);
        write_u32(self.output, self.control_depth + 1);
        self.output.push(0x0b);
        if let Value::Scalar { local, ty } = &result {
            let offset = self.plan.call_out[&expr.id];
            self.emit_pointer(Pointer {
                local: self.plan.frame_base,
                offset,
            });
            self.load_scalar(ty);
            self.output.push(0x21);
            write_u32(self.output, *local);
        }
        Ok(result)
    }

    fn emit_unary(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        operand: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let operand = self.emit_expr(operand)?;
        let destination = self.plan.expr_scalar(expr)?;
        match op {
            UnaryOp::Not => {
                self.require_scalar(&operand, &ResolvedType::Bool, "logical not")?;
                self.get_scalar(&operand);
                self.output.push(0x45);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            UnaryOp::Neg => {
                self.require_scalar(&operand, &ResolvedType::I64, "numeric negation")?;
                self.get_scalar(&operand);
                self.output.push(0x42);
                write_i64(self.output, i64::MIN);
                self.output.push(0x51);
                self.fail_if(STATUS_NEG_OVERFLOW);
                self.output.push(0x42);
                write_i64(self.output, 0);
                self.get_scalar(&operand);
                self.output.push(0x7d);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn emit_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let left = self.emit_expr(left)?;
        let destination = self.plan.expr_scalar(expr)?;
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            self.require_scalar(&left, &ResolvedType::Bool, "lazy left operand")?;
            self.get_scalar(&left);
            self.output.extend([0x04, 0x40]);
            self.control_depth += 1;
            if op == BinaryOp::And {
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
                self.output.push(0x05);
                self.output.extend([0x41, 0x00, 0x21]);
                write_u32(self.output, destination);
            } else {
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, destination);
                self.output.push(0x05);
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            self.control_depth -= 1;
            self.output.push(0x0b);
            return Ok(Value::Scalar {
                local: destination,
                ty: ResolvedType::Bool,
            });
        }

        let right = self.emit_expr(right)?;
        match op {
            BinaryOp::Add => self.emit_checked_add(&left, &right, destination)?,
            BinaryOp::Sub => self.emit_checked_sub(&left, &right, destination)?,
            BinaryOp::Mul => self.emit_checked_mul(&left, &right, destination)?,
            BinaryOp::Div => self.emit_checked_div_rem(&left, &right, destination, false)?,
            BinaryOp::Rem => self.emit_checked_div_rem(&left, &right, destination, true)?,
            BinaryOp::Eq | BinaryOp::Ne => {
                if is_record(self.program, value_type(&left))? {
                    return Err(error("record equality is outside executable records v1"));
                }
                require_type(value_type(&left), value_type(&right), "equality operands")?;
                self.get_scalar(&left);
                self.get_scalar(&right);
                self.output.push(match (value_type(&left), op) {
                    (ResolvedType::I64, BinaryOp::Eq) => 0x51,
                    (ResolvedType::I64, BinaryOp::Ne) => 0x52,
                    (_, BinaryOp::Eq) => 0x46,
                    (_, BinaryOp::Ne) => 0x47,
                    _ => unreachable!(),
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                self.require_scalar(&left, &ResolvedType::I64, "ordered left operand")?;
                self.require_scalar(&right, &ResolvedType::I64, "ordered right operand")?;
                self.get_scalar(&left);
                self.get_scalar(&right);
                self.output.push(match op {
                    BinaryOp::Lt => 0x53,
                    BinaryOp::Gt => 0x55,
                    BinaryOp::Le => 0x57,
                    BinaryOp::Ge => 0x59,
                    _ => unreachable!(),
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn emit_if(
        &mut self,
        expr: &ResolvedExpr,
        condition: &ResolvedExpr,
        then_branch: &ResolvedExpr,
        else_branch: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let condition = self.emit_expr(condition)?;
        self.require_scalar(&condition, &ResolvedType::Bool, "if condition")?;
        let destination = if is_record(self.program, &expr.ty)? {
            Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: expr.ty.clone(),
            }
        } else {
            Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            }
        };
        self.get_scalar(&condition);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        let then_value = self.emit_expr(then_branch)?;
        self.copy_value(&destination, &then_value, "then branch")?;
        self.output.push(0x05);
        let else_value = self.emit_expr(else_branch)?;
        self.copy_value(&destination, &else_value, "else branch")?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(destination)
    }

    fn emit_checked_add(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "addition")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7c);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.get_scalar(right);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.output.push(0x83);
        self.output.push(0x42);
        write_i64(self.output, 0);
        self.output.push(0x53);
        self.fail_if(STATUS_ADD_OVERFLOW);
        Ok(())
    }

    fn emit_checked_sub(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "subtraction")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7d);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x85);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.output.push(0x83);
        self.output.push(0x42);
        write_i64(self.output, 0);
        self.output.push(0x53);
        self.fail_if(STATUS_SUB_OVERFLOW);
        Ok(())
    }

    fn emit_checked_mul(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "multiplication")?;
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.output.push(0x72);
        self.fail_if(STATUS_MUL_OVERFLOW);

        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7e);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(right);
        self.output.push(0x50);
        self.output.push(0x45);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.get_scalar(right);
        self.output.push(0x7f);
        self.get_scalar(left);
        self.output.push(0x52);
        self.fail_if(STATUS_MUL_OVERFLOW);
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }

    fn emit_checked_div_rem(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
        remainder: bool,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(
            left,
            right,
            if remainder { "remainder" } else { "division" },
        )?;
        self.get_scalar(right);
        self.output.push(0x50);
        self.fail_if(if remainder {
            STATUS_REM_ZERO
        } else {
            STATUS_DIV_ZERO
        });
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.fail_if(if remainder {
            STATUS_REM_OVERFLOW
        } else {
            STATUS_DIV_OVERFLOW
        });
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(if remainder { 0x81 } else { 0x7f });
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    fn require_i64_pair(
        &self,
        left: &Value,
        right: &Value,
        context: &str,
    ) -> Result<(), Diagnostic> {
        self.require_scalar(left, &ResolvedType::I64, context)?;
        self.require_scalar(right, &ResolvedType::I64, context)
    }

    fn place_value(&self, place: &crate::hir::Place) -> Result<Value, Diagnostic> {
        let mut value =
            self.bindings.get(&place.root).cloned().ok_or_else(|| {
                error(format!("aggregate value `{}` is not in scope", place.root))
            })?;
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(error(
                    "variant-field place projection is outside executable records v1",
                ));
            };
            value = self.project_value(&value, field)?;
        }
        Ok(value)
    }

    fn project_value(&self, base: &Value, field: &DeclarationId) -> Result<Value, Diagnostic> {
        let Value::Aggregate { pointer, ty } = base else {
            return Err(error("record projection base is not aggregate storage"));
        };
        let record = layout(self.program, ty)?;
        let field = record
            .field(field)
            .cloned()
            .ok_or_else(|| error(format!("record `{}` has no field `{field}`", record.record)))?;
        value_at(
            Pointer {
                local: pointer.local,
                offset: pointer
                    .offset
                    .checked_add(field.offset)
                    .ok_or_else(|| error("projection pointer overflows u32"))?,
            },
            field.ty,
            self.program,
        )
    }

    fn copy_value(
        &mut self,
        destination: &Value,
        source: &Value,
        context: &str,
    ) -> Result<(), Diagnostic> {
        require_type(value_type(source), value_type(destination), context)?;
        match destination {
            Value::Scalar { local, ty } => {
                self.get_scalar(source);
                self.output.push(0x21);
                write_u32(self.output, *local);
                scalar_wasm_type(ty)?;
            }
            Value::ScalarMemory { pointer, ty } => {
                self.emit_pointer(*pointer);
                self.get_scalar(source);
                self.store_scalar(ty);
            }
            Value::Aggregate {
                pointer: destination,
                ty,
            } => {
                let Value::Aggregate {
                    pointer: source, ..
                } = source
                else {
                    return Err(error(format!(
                        "{context} mixes scalar and aggregate values"
                    )));
                };
                let record = layout(self.program, ty)?;
                self.emit_pointer(*destination);
                self.emit_pointer(*source);
                self.output.push(0x41);
                write_i64(self.output, i64::from(record.size));
                self.output.extend([0xfc, 0x0a, 0x00, 0x00]);
            }
        }
        Ok(())
    }

    fn require_scalar(
        &self,
        value: &Value,
        ty: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if matches!(value, Value::Aggregate { .. }) {
            return Err(error(format!("{context} requires a scalar")));
        }
        require_type(value_type(value), ty, context)
    }

    fn get_scalar(&mut self, value: &Value) {
        match value {
            Value::Scalar { local, .. } => {
                self.output.push(0x20);
                write_u32(self.output, *local);
            }
            Value::ScalarMemory { pointer, ty } => {
                self.emit_pointer(*pointer);
                self.load_scalar(ty);
            }
            Value::Aggregate { .. } => unreachable!("validated scalar value"),
        }
    }

    fn emit_pointer(&mut self, pointer: Pointer) {
        self.output.push(0x20);
        write_u32(self.output, pointer.local);
        if pointer.offset != 0 {
            self.output.push(0x41);
            write_i64(self.output, i64::from(pointer.offset));
            self.output.push(0x6a);
        }
    }

    fn load_scalar(&mut self, ty: &ResolvedType) {
        match ty {
            ResolvedType::I64 => self.output.extend([0x29, 0x03, 0x00]),
            ResolvedType::Bool => self.output.extend([0x28, 0x02, 0x00]),
            _ => unreachable!("validated scalar load"),
        }
    }

    fn store_scalar(&mut self, ty: &ResolvedType) {
        match ty {
            ResolvedType::I64 => self.output.extend([0x37, 0x03, 0x00]),
            ResolvedType::Bool => self.output.extend([0x36, 0x02, 0x00]),
            _ => unreachable!("validated scalar store"),
        }
    }

    fn fail_if(&mut self, status: i32) {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(status));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.push(0x0c);
        write_u32(self.output, self.control_depth + 1);
        self.output.push(0x0b);
    }
}

fn value_type(value: &Value) -> &ResolvedType {
    match value {
        Value::Scalar { ty, .. } | Value::ScalarMemory { ty, .. } | Value::Aggregate { ty, .. } => {
            ty
        }
    }
}

fn value_at(
    pointer: Pointer,
    ty: ResolvedType,
    program: &ResolvedProgram,
) -> Result<Value, Diagnostic> {
    if is_record(program, &ty)? {
        Ok(Value::Aggregate { pointer, ty })
    } else {
        scalar_wasm_type(&ty)?;
        Ok(Value::ScalarMemory { pointer, ty })
    }
}

fn require_type(
    actual: &ResolvedType,
    expected: &ResolvedType,
    context: &str,
) -> Result<(), Diagnostic> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(format!(
            "inconsistent HIR type for {context}: expected `{}`, found `{}`",
            expected.identity_key(),
            actual.identity_key()
        )))
    }
}

fn emit_wrapper(main_index: u32) -> Vec<u8> {
    let old_stack = 0_u32;
    let frame_base = 1_u32;
    let status = 2_u32;
    let mut body = Vec::new();
    write_u32(&mut body, 3);
    write_u32(&mut body, 1);
    body.push(I32);
    write_u32(&mut body, 1);
    body.push(I32);
    write_u32(&mut body, 1);
    body.push(I32);

    body.push(0x23);
    write_u32(&mut body, 0);
    body.push(0x22);
    write_u32(&mut body, old_stack);
    body.push(0x41);
    write_i64(&mut body, 8);
    body.push(0x49);
    body.extend([0x04, 0x40, 0x00, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, old_stack);
    body.push(0x41);
    write_i64(&mut body, 8);
    body.push(0x6b);
    body.push(0x22);
    write_u32(&mut body, frame_base);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x20);
    write_u32(&mut body, frame_base);
    body.push(0x10);
    write_u32(&mut body, main_index);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, old_stack);
    body.push(0x24);
    write_u32(&mut body, 0);

    emit_arithmetic_trap_case(&mut body, status, STATUS_ADD_OVERFLOW, 0, i64::MAX, 1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_SUB_OVERFLOW, 1, i64::MIN, 1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_MUL_OVERFLOW, 2, i64::MAX, 2);
    emit_arithmetic_trap_case(&mut body, status, STATUS_DIV_ZERO, 3, 1, 0);
    emit_arithmetic_trap_case(&mut body, status, STATUS_DIV_OVERFLOW, 3, i64::MIN, -1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_REM_ZERO, 4, 1, 0);
    emit_arithmetic_trap_case(&mut body, status, STATUS_REM_OVERFLOW, 4, i64::MIN, -1);
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(STATUS_NEG_OVERFLOW));
    body.push(0x46);
    body.extend([0x04, 0x40, 0x42]);
    write_i64(&mut body, i64::MIN);
    body.push(0x10);
    write_u32(&mut body, 5);
    body.extend([0x1a, 0x00, 0x0b]);

    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x45);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    body.push(0x10);
    write_u32(&mut body, 6);
    body.push(0x00);
    body.push(0x0b);

    body.push(0x20);
    write_u32(&mut body, frame_base);
    body.extend([0x29, 0x03, 0x00, 0x0b]);
    body
}

fn emit_arithmetic_trap_case(
    body: &mut Vec<u8>,
    status_local: u32,
    expected: i32,
    import: u32,
    left: i64,
    right: i64,
) {
    body.push(0x20);
    write_u32(body, status_local);
    body.push(0x41);
    write_i64(body, i64::from(expected));
    body.push(0x46);
    body.extend([0x04, 0x40, 0x42]);
    write_i64(body, left);
    body.push(0x42);
    write_i64(body, right);
    body.push(0x10);
    write_u32(body, import);
    body.extend([0x1a, 0x00, 0x0b]);
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        emit_profile, function_import, hex_identity, intern_type, section, write_bytes, write_i64,
        write_name, write_u32, Signature, I32, SHADOW_STACK_TOP,
    };
    use crate::codegen::native_aggregate::{
        resource_harness_scenario, wasm_address, HarnessAction, ResourceHarnessScenario,
    };
    use crate::hir::{self, DeclarationId};
    use crate::parse;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    const SOURCE: &str = r#"
module test.aggregate_wasm;
@id("inner.type")
record Inner {
    @id("inner.value") value: i64,
    @id("inner.flag") flag: bool,
}
@id("outer.type")
record Outer {
    @id("outer.inner") inner: Inner,
    @id("outer.other") other: i64,
}
@id("case.ok")
fn ok(base: Outer) -> Outer {
    base with { inner: base.inner with { value: base.inner.value + 2 } }
}
@id("case.fail.base")
fn fail_base() -> Outer requires false {
    Outer { inner: Inner { value: 1, flag: true }, other: 2 }
}
@id("case.base.first")
fn base_first() -> Outer {
    fail_base() with { other: 9223372036854775807 + 1 }
}
@id("case.replacements")
fn replacements(base: Outer) -> Outer {
    base with {
        inner: base.inner with { value: 9223372036854775807 + 1 },
        other: 1 / 0,
    }
}
@id("case.post")
fn post(base: Outer) -> Outer ensures false { ok(base) }
@id("app.main")
fn main() -> i64 {
    let value = Outer {
        inner: Inner { value: 18, flag: true },
        other: 22,
    };
    let changed = ok(value);
    if changed.inner.flag { changed.inner.value + changed.other } else { 0 }
}
"#;

    #[test]
    fn node_executes_aggregate_status_out_poison_order_and_shadow_stack_reentry() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(SOURCE, Path::new("aggregate-wasm.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true).unwrap();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-aggregate-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();

        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = () => new Uint8Array(memory.buffer, output, 24).fill(poison);
const assertPoison = () => {{
  for (const byte of new Uint8Array(memory.buffer, output, 24)) if (byte !== poison) throw new Error("aggregate failure published output");
}};
view.setBigInt64(input, 18n, true);
view.setInt32(input + 8, 1, true);
view.setBigInt64(input + 16, 22n, true);
poisonOutput();
if (instance.exports["{ok}"](input, output) !== 0) throw new Error("success status");
if (view.getBigInt64(output, true) !== 20n || view.getInt32(output + 8, true) !== 1 || view.getBigInt64(output + 16, true) !== 22n) throw new Error("success aggregate");
if (stack.value !== {stack_top}) throw new Error("success stack restore");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput();
  if (instance.exports["{base_first}"](output) !== 9) throw new Error("base-first status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("base-first stack restore");
  if (instance.exports["{replacements}"](input, output) !== 1) throw new Error("replacement-order status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("replacement stack restore");
  if (instance.exports["{post}"](input, output) !== 10) throw new Error("postcondition status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("postcondition stack restore");
}}
if (instance.exports.semaprax_main() !== 42n) throw new Error("public aggregate result");
if (stack.value !== {stack_top}) throw new Error("public stack restore");
console.log("aggregate-wasm-v1-ok");
"#,
            ok = export("case.ok"),
            base_first = export("case.base.first"),
            replacements = export("case.replacements"),
            post = export("case.post"),
            stack_top = SHADOW_STACK_TOP,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node aggregate runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "aggregate-wasm-v1-ok"
        );
    }

    #[test]
    fn private_node_resource_records_follow_plan_order_and_finish_with_zero_liveness() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let scenario = resource_harness_scenario();
        let bytes = private_resource_harness_wasm(&scenario);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-resource-aggregate-wasm-{}-{id}",
            std::process::id()
        );
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        let expected = scenario
            .expected_trace
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let live = scenario
            .actions
            .iter()
            .filter_map(|action| match action {
                HarnessAction::Store(_, value) => Some(value.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const expected = [{expected}];
const live = new Set([{live}]);
const log = [];
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  finalize(handle) {{
    if (!live.delete(handle)) throw new Error(`duplicate/unknown finalizer ${{handle}}`);
    log.push(handle);
  }},
}} }});
if (instance.exports.run() !== 1) throw new Error("resource aggregate poison check failed");
if (live.size !== 0) throw new Error(`resource aggregate liveness ${{[...live]}}`);
if (log.join(",") !== expected.join(",")) throw new Error(`resource aggregate order ${{log}}`);
console.log("aggregate-resource-wasm-v1-ok");
"#
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node private resource aggregate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "aggregate-resource-wasm-v1-ok"
        );
    }

    fn private_resource_harness_wasm(scenario: &ResourceHarnessScenario) -> Vec<u8> {
        let mut types = Vec::new();
        let mut indexes = std::collections::HashMap::new();
        let finalize_type = intern_type(
            Signature {
                params: vec![I32],
                results: vec![],
            },
            &mut types,
            &mut indexes,
        );
        let run_type = intern_type(
            Signature {
                params: vec![],
                results: vec![I32],
            },
            &mut types,
            &mut indexes,
        );
        let mut module = b"\0asm\x01\0\0\0".to_vec();
        let mut type_section = Vec::new();
        write_u32(&mut type_section, types.len() as u32);
        for signature in &types {
            type_section.push(0x60);
            write_bytes(&mut type_section, &signature.params);
            write_bytes(&mut type_section, &signature.results);
        }
        section(&mut module, 1, type_section);
        let mut imports = Vec::new();
        write_u32(&mut imports, 1);
        function_import(&mut imports, "env", "finalize", finalize_type);
        section(&mut module, 2, imports);
        let mut functions = Vec::new();
        write_u32(&mut functions, 1);
        write_u32(&mut functions, run_type);
        section(&mut module, 3, functions);
        let mut memory = Vec::new();
        write_u32(&mut memory, 1);
        memory.extend([0x00, 0x01]);
        section(&mut module, 5, memory);
        let mut exports = Vec::new();
        write_u32(&mut exports, 1);
        write_name(&mut exports, "run");
        exports.push(0x00);
        write_u32(&mut exports, 1);
        section(&mut module, 7, exports);

        let mut body = vec![0x00];
        for action in &scenario.actions {
            match *action {
                HarnessAction::Store(slot, value) => {
                    store(&mut body, wasm_address(slot), value as i32)
                }
                HarnessAction::Transfer(source, destination) => {
                    transfer(&mut body, wasm_address(source), wasm_address(destination))
                }
                HarnessAction::Finalize(slot) => finalize(&mut body, wasm_address(slot)),
                HarnessAction::PoisonPartialResult => {
                    // Wasm32 Pair is exactly two four-byte resource leaves;
                    // poison the entire caller result slot, not just field 0.
                    store(&mut body, 2048, 0x7f7f_7f7f);
                    store(&mut body, 2052, 0x7f7f_7f7f);
                }
            }
        }
        load(&mut body, 2048);
        body.push(0x41);
        write_i64(&mut body, 0x7f7f_7f7f);
        body.push(0x46);
        load(&mut body, 2052);
        body.push(0x41);
        write_i64(&mut body, 0x7f7f_7f7f);
        body.extend([0x46, 0x71, 0x0b]);
        let mut code = Vec::new();
        write_u32(&mut code, 1);
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
        section(&mut module, 10, code);
        module
    }

    fn store(body: &mut Vec<u8>, address: i32, value: i32) {
        body.push(0x41);
        write_i64(body, i64::from(address));
        body.push(0x41);
        write_i64(body, i64::from(value));
        body.extend([0x36, 0x02, 0x00]);
    }

    fn load(body: &mut Vec<u8>, address: i32) {
        body.push(0x41);
        write_i64(body, i64::from(address));
        body.extend([0x28, 0x02, 0x00]);
    }

    fn transfer(body: &mut Vec<u8>, source: i32, destination: i32) {
        body.push(0x41);
        write_i64(body, i64::from(destination));
        load(body, source);
        body.extend([0x36, 0x02, 0x00]);
        store(body, source, 0);
    }

    fn finalize(body: &mut Vec<u8>, address: i32) {
        load(body, address);
        body.push(0x10);
        write_u32(body, 0);
        store(body, address, 0);
    }
}
