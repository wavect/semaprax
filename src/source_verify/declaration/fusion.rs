//! Exact finite construction inventory; large candidates remain diagnostic-only.
use crate::ast::program_source::ProgramSource;
use crate::ast::{
    BinaryOp, Expr, ExprKind as K, Function, Statement as S, Type, TypeDeclaration,
    TypeDeclarationKind,
};
mod consumers;
#[cfg(test)]
mod tests;

const MAX_HELP_BYTES: usize = 1024;
const MAX_IDENTITY_BYTES: usize = 128;
struct Help(String);
impl std::fmt::Write for Help {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self
            .0
            .len()
            .checked_add(value.len())
            .is_none_or(|len| len > MAX_HELP_BYTES)
        {
            return Err(std::fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}
pub(super) fn refusal_help(program: &dyn ProgramSource) -> Option<String> {
    use std::fmt::Write;
    let inventory = inventory(program)?;
    if inventory.function.stable_id.len() > MAX_IDENTITY_BYTES
        || inventory.record.name.len() > MAX_IDENTITY_BYTES
    {
        return None;
    }
    let debit = MAX_HELP_BYTES.checked_add(std::mem::size_of::<Help>())?;
    if !crate::bounded_output::reserve_active_preserving(debit, 0) {
        return None;
    }
    let mut help = Help(String::new());
    help.0.try_reserve_exact(MAX_HELP_BYTES).ok()?;
    if help.0.capacity() != MAX_HELP_BYTES {
        return None;
    }
    write!(&mut help, "Finite source candidate `{}`: {} `{}` rows; {} Bytes-producing executions, width {}, payload {} bytes, owned UTF-8 {} bytes (line {}). Still SPX-T267: source recognition is not fusion or partition authority. Keep the original fixture; #746/#747 need independent HIR proof and a complete selected executor. The legacy 32-event limit is unchanged.", inventory.function.stable_id, inventory.count, inventory.record.name, inventory.count, inventory.width, inventory.payload_bytes, inventory.string_bytes, inventory.byte_site.span.line).ok()?;
    Some(help.0)
}

pub(crate) struct Inventory<'a> {
    pub(crate) function: &'a Function,
    pub(crate) record: &'a TypeDeclaration,
    pub(crate) byte_site: &'a Expr,
    pub(crate) condition: &'a Expr,
    pub(crate) body: &'a Expr,
    pub(crate) count: u64,
    pub(crate) width: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) string_bytes: u64,
}
fn call<'a>(expr: &'a Expr, name: &str) -> Option<(&'a [Type], &'a [Expr])> {
    let K::Call {
        name: actual,
        type_arguments,
        args,
    } = &expr.kind
    else {
        return None;
    };
    (actual == name).then_some((type_arguments, args))
}
fn var(expr: &Expr, name: &str) -> bool {
    matches!(&expr.kind, K::Var(actual) if actual == name)
}
fn row_type(arguments: &[Type], record: &TypeDeclaration) -> bool {
    matches!(arguments, [Type::Named { name, arguments }] if name == &record.name && arguments.is_empty())
}
pub(crate) fn inventory(program: &dyn ProgramSource) -> Option<Inventory<'_>> {
    if program.functions().len() != 1
        || program.types().len() != 1
        || !program.module_uses().is_empty()
        || !program.permits().is_empty()
        || !program.interfaces().is_empty()
        || !program.protocols().is_empty()
        || !program.implementations().is_empty()
        || !program.session_protocols().is_empty()
        || !program.agents().is_empty()
    {
        return None;
    }
    let function = program.functions().get(0)?.payload();
    if !function.explicit_id
        || !function.type_parameters.is_empty()
        || !function.params.is_empty()
        || function.return_type != Type::I64
        || !function.effects.is_empty()
        || function.yields.is_some()
        || function.follows.is_some()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return None;
    }
    let record = program.types().get(0)?;
    if !record.explicit_id
        || !record.type_parameters.is_empty()
        || record.extends.is_some()
        || !record.invariants().is_empty()
        || record.stable_id == function.stable_id
        || record.name == "Option"
    {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return None;
    };
    if fields.len() != 3 {
        return None;
    }
    for (index, field) in fields.iter().enumerate() {
        if !field.explicit_id
            || field.stable_id == record.stable_id
            || field.stable_id == function.stable_id
            || fields[..index]
                .iter()
                .any(|other| other.name == field.name || other.stable_id == field.stable_id)
        {
            return None;
        }
    }
    if fields.iter().filter(|f| f.ty == Type::I64).count() != 1
        || fields.iter().filter(|f| f.ty == Type::String).count() != 1
        || fields.iter().filter(|f| f.ty == Type::Bytes).count() != 1
    {
        return None;
    }
    let K::Block { statements, tail } = &function.body.kind else {
        return None;
    };
    let [S::Let {
        name: vector,
        mutable: true,
        declared: None,
        value: allocation,
        ..
    }, S::Let {
        name: induction,
        mutable: true,
        declared: None,
        value: start,
        ..
    }, S::While {
        condition, body, ..
    }, suffix @ ..] = statements.as_slice()
    else {
        return None;
    };
    if vector == induction || !matches!(start.kind, K::Int(0)) {
        return None;
    }
    let (types, args) = call(allocation, "vec_with_capacity")?;
    if !row_type(types, record) {
        return None;
    }
    let [capacity] = args else {
        return None;
    };
    let K::Usize(capacity) = capacity.kind else {
        return None;
    };
    let K::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return None;
    };
    if !var(left, induction) {
        return None;
    }
    let K::Int(bound) = right.kind else {
        return None;
    };
    let count = u64::try_from(bound).ok()?;
    let row_capacity = crate::vec_ops::MAX_CAPACITY.min(
        crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
            / (2 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT),
    );
    if count == 0 || count != capacity || count > row_capacity {
        return None;
    }
    let K::Block {
        statements: construction,
        tail: construction_tail,
    } = &body.kind
    else {
        return None;
    };
    let [S::Let {
        name: item,
        mutable: false,
        declared: None,
        value: constructor,
        ..
    }, S::Assign {
        name: destination,
        field: None,
        value: push,
        ..
    }, S::Assign {
        name: counter,
        field: None,
        value: step,
        ..
    }] = construction.as_slice()
    else {
        return None;
    };
    if item == vector
        || item == induction
        || destination != vector
        || counter != induction
        || !matches!(construction_tail.kind, K::Int(0))
    {
        return None;
    }
    let K::ConstructRecord {
        type_name,
        type_arguments,
        fields: initializers,
        ..
    } = &constructor.kind
    else {
        return None;
    };
    if type_name != &record.name || !type_arguments.is_empty() || initializers.len() != fields.len()
    {
        return None;
    }
    let mut byte_site = None;
    let mut width = None;
    let mut string_bytes = None;
    for (field, initializer) in fields.iter().zip(initializers) {
        if field.name != initializer.name {
            return None;
        }
        match field.ty {
            Type::I64 if var(&initializer.value, induction) => {}
            Type::String => {
                let K::String(text) = &initializer.value.kind else {
                    return None;
                };
                string_bytes = Some(u64::try_from(text.len()).ok()?.checked_mul(count)?);
            }
            Type::Bytes => {
                let (types, args) = call(&initializer.value, "bytes_zeroed")?;
                let [extent] = args else {
                    return None;
                };
                if !types.is_empty() {
                    return None;
                }
                let K::Usize(extent) = extent.kind else {
                    return None;
                };
                if extent == 0 {
                    return None;
                }
                width = Some(extent);
                byte_site = Some(&initializer.value);
            }
            _ => return None,
        }
    }
    let width = width?;
    let payload_bytes = width.checked_mul(count)?;
    // One proposed ordinary backing must fit the existing single-Bytes extent.
    if payload_bytes > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
        || payload_bytes > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
    {
        return None;
    }
    let (types, args) = call(push, "vec_push")?;
    if !row_type(types, record)
        || !matches!(args, [old, fresh] if var(old, vector) && var(fresh, item))
    {
        return None;
    }
    if !matches!(&step.kind, K::Binary { op: BinaryOp::Add, left, right } if var(left, induction) && matches!(right.kind, K::Int(1)))
    {
        return None;
    }
    consumers::validate(record, vector, induction, suffix, tail)?;
    Some(Inventory {
        function,
        record,
        byte_site: byte_site?,
        condition,
        body,
        count,
        width,
        payload_bytes,
        string_bytes: string_bytes?,
    })
}

/// Ordinary finite capacity, restricted to the unchanged physical event cap.
pub(crate) fn finite_loop(program: &dyn ProgramSource, expression: &Expr) -> Option<u32> {
    if program.functions().len() != 1 {
        return None;
    }
    let function = program.functions().get(0)?.payload();
    let K::Block { statements, .. } = &function.body.kind else {
        return None;
    };
    let Some(S::While {
        condition, body, ..
    }) = statements.get(2)
    else {
        return None;
    };
    if !(std::ptr::eq(expression, condition.as_ref()) || std::ptr::eq(expression, body.as_ref())) {
        return None;
    }
    let proof = inventory(program)?;
    if proof.count > u64::from(crate::byte_data_capacity::MAX_BYTES_COPY_SITES)
        || !(std::ptr::eq(expression, proof.condition) || std::ptr::eq(expression, proof.body))
    {
        return None;
    }
    u32::try_from(proof.count).ok()
}
pub(crate) fn finite_allocation(program: &dyn ProgramSource, expression: &Expr) -> bool {
    inventory(program).is_some_and(|proof| {
        proof.count <= u64::from(crate::byte_data_capacity::MAX_BYTES_COPY_SITES)
            && std::ptr::eq(expression, proof.byte_site)
    })
}
