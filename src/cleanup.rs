//! Deterministic cleanup storage inventory.
//!
//! This is deliberately smaller than the executable `CleanupPlan` specified by
//! RFC 0003. It catalogs every storage candidate and resource leaf that a later
//! control-flow plan must govern. Beyond owned-parameter entry state, it does
//! not describe path-sensitive liveness, transfers, initialization/finalization
//! order, failure status, or exits.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind, ValueId,
};

#[cfg(test)]
thread_local! {
    static INVENTORY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_capacity_high_water() {
    INVENTORY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn capacity_high_water() -> usize {
    INVENTORY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_capacity_high_water(bytes: usize) {
    INVENTORY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str().len()
                + arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn shape_owned_capacity(shape: &FieldLivenessShape) -> usize {
    match shape {
        FieldLivenessShape::NoDrop => 0,
        FieldLivenessShape::Leaf { lifecycle, .. } => lifecycle.as_str().len(),
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            declaration.as_str().len()
                + fields.capacity() * std::mem::size_of::<FieldLiveness>()
                + fields
                    .iter()
                    .map(|field| field.field.as_str().len() + shape_owned_capacity(&field.shape))
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn slot_owned_capacity(slot: &CleanupStorageSlot) -> usize {
    let origin = match &slot.origin {
        CleanupStorageOrigin::Parameter { value, .. }
        | CleanupStorageOrigin::Binding { value }
        | CleanupStorageOrigin::ProvisionalResult { value } => value.as_str().len(),
        CleanupStorageOrigin::Temporary { expression } => expression.as_str().len(),
    };
    origin + resolved_type_owned_capacity(&slot.ty) + shape_owned_capacity(&slot.shape)
}

#[cfg(test)]
fn flag_owned_capacity(flag: &CleanupFlag) -> usize {
    flag.lifecycle.as_str().len()
        + flag.place.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + flag
            .place
            .projections
            .iter()
            .map(|id| id.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
fn inventory_builder_live_capacity(builder: &InventoryBuilder<'_>) -> usize {
    builder.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
        + builder.slots.iter().map(slot_owned_capacity).sum::<usize>()
        + builder.flags.capacity() * std::mem::size_of::<CleanupFlag>()
        + builder.flags.iter().map(flag_owned_capacity).sum::<usize>()
        + builder.live_owned_parameters.capacity() * std::mem::size_of::<CleanupStorageId>()
}

pub const CLEANUP_INVENTORY_SCHEMA_V1: &str = "semaprax.cleanup-inventory.v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CleanupStorageId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LivenessFlagId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupInventory {
    pub schema: &'static str,
    pub entry_state: CleanupEntryState,
    pub slots: Vec<CleanupStorageSlot>,
    pub flags: Vec<CleanupFlag>,
}

impl CleanupInventory {
    pub(crate) fn unresolved() -> Self {
        Self {
            schema: CLEANUP_INVENTORY_SCHEMA_V1,
            entry_state: CleanupEntryState {
                live_owned_parameters: Vec::new(),
            },
            slots: Vec::new(),
            flags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupEntryState {
    pub live_owned_parameters: Vec<CleanupStorageId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupStorageSlot {
    pub id: CleanupStorageId,
    /// Canonical structural discovery order, not runtime initialization order.
    pub discovery_index: u32,
    pub origin: CleanupStorageOrigin,
    pub ty: ResolvedType,
    pub shape: FieldLivenessShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CleanupStorageOrigin {
    Parameter {
        value: ValueId,
        parameter_index: u32,
    },
    Binding {
        value: ValueId,
    },
    Temporary {
        expression: ExpressionId,
    },
    ProvisionalResult {
        value: ValueId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FieldLivenessShape {
    NoDrop,
    Leaf {
        flag: LivenessFlagId,
        lifecycle: DeclarationId,
    },
    Record {
        declaration: DeclarationId,
        fields: Vec<FieldLiveness>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldLiveness {
    pub field: DeclarationId,
    pub field_index: u32,
    pub shape: FieldLivenessShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupFlag {
    pub id: LivenessFlagId,
    pub place: CleanupPlace,
    pub lifecycle: DeclarationId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CleanupPlace {
    pub storage: CleanupStorageId,
    pub projections: Vec<DeclarationId>,
}

pub(crate) fn build_inventory(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<CleanupInventory, Diagnostic> {
    let mut builder = InventoryBuilder {
        program,
        slots: Vec::new(),
        flags: Vec::new(),
        live_owned_parameters: Vec::new(),
    };

    for (parameter_index, parameter) in function.params.iter().enumerate() {
        if parameter.ownership == OwnershipMode::Own && builder.needs_drop(&parameter.ty)? {
            let storage = builder.add_slot(
                CleanupStorageOrigin::Parameter {
                    value: parameter.id.clone(),
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| cleanup_error("too many function parameters"))?,
                },
                parameter.ty.clone(),
            )?;
            builder.live_owned_parameters.push(storage);
        }
    }

    for expression in &function.requires {
        builder.collect_expression(expression)?;
    }
    builder.collect_expression(&function.body)?;
    if builder.needs_drop(&function.return_type)? {
        builder.add_slot(
            CleanupStorageOrigin::ProvisionalResult {
                value: function.result_id.clone(),
            },
            function.return_type.clone(),
        )?;
    }
    for expression in &function.ensures {
        builder.collect_expression(expression)?;
    }

    #[cfg(test)]
    note_capacity_high_water(inventory_builder_live_capacity(&builder));

    Ok(CleanupInventory {
        schema: CLEANUP_INVENTORY_SCHEMA_V1,
        entry_state: CleanupEntryState {
            live_owned_parameters: builder.live_owned_parameters,
        },
        slots: builder.slots,
        flags: builder.flags,
    })
}

pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in &program.functions {
        let expected = build_inventory(program, function)?;
        if function.cleanup != expected {
            return Err(cleanup_error(format!(
                "function `{}` has a non-canonical cleanup inventory",
                function.id
            )));
        }
    }
    for instance in &program.function_instances {
        let expected = build_inventory(program, &instance.function)?;
        if instance.function.cleanup != expected {
            return Err(cleanup_error(format!(
                "function instance `{}` has a non-canonical cleanup inventory",
                instance.id
            )));
        }
    }
    Ok(())
}

struct InventoryBuilder<'a> {
    program: &'a ResolvedProgram,
    slots: Vec<CleanupStorageSlot>,
    flags: Vec<CleanupFlag>,
    live_owned_parameters: Vec<CleanupStorageId>,
}

impl InventoryBuilder<'_> {
    fn needs_drop(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| facts.needs_drop)
            .ok_or_else(|| {
                cleanup_error(format!("type `{}` has no cleanup facts", ty.identity_key()))
            })
    }

    fn add_slot(
        &mut self,
        origin: CleanupStorageOrigin,
        ty: ResolvedType,
    ) -> Result<CleanupStorageId, Diagnostic> {
        let index = u32::try_from(self.slots.len())
            .map_err(|_| cleanup_error("too many cleanup storage candidates"))?;
        let storage = CleanupStorageId(index);
        let shape = self.shape_for_type(&ty, storage)?;
        self.slots.push(CleanupStorageSlot {
            id: storage,
            discovery_index: index,
            origin,
            ty,
            shape,
        });
        Ok(storage)
    }

    fn shape_for_type(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Children(
                &'a DeclarationId,
                &'a [crate::hir::ResolvedFieldDeclaration],
                usize,
            ),
            FinishField(&'a crate::hir::ResolvedFieldDeclaration),
            FinishRecord(&'a DeclarationId, usize),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 40) };

        let mut frames = vec![Frame::Enter(ty)];
        let mut projections = Vec::<DeclarationId>::new();
        let mut shapes = Vec::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + projections.capacity() * std::mem::size_of::<DeclarationId>()
                    + projections
                        .iter()
                        .map(|id| id.as_str().len())
                        .sum::<usize>()
                    + shapes.capacity() * std::mem::size_of::<FieldLivenessShape>()
                    + self.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
                    + self.flags.capacity() * std::mem::size_of::<CleanupFlag>()
                    + self.live_owned_parameters.capacity()
                        * std::mem::size_of::<CleanupStorageId>()
                    + self.slots.iter().map(slot_owned_capacity).sum::<usize>()
                    + self.flags.iter().map(flag_owned_capacity).sum::<usize>()
                    + shapes.iter().map(shape_owned_capacity).sum::<usize>(),
            );
            match frame {
                Frame::Enter(ty) => {
                    if !self.needs_drop(ty)? {
                        shapes.push(FieldLivenessShape::NoDrop);
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return Err(cleanup_error(format!(
                            "droppable type `{}` is not nominal",
                            ty.identity_key()
                        )));
                    };
                    if !arguments.is_empty() {
                        return Err(cleanup_error(format!(
                            "droppable type `{}` has unsupported generic arguments",
                            ty.identity_key()
                        )));
                    }
                    let declaration_item = self
                        .program
                        .types
                        .iter()
                        .find(|item| item.id == *declaration)
                        .ok_or_else(|| {
                            cleanup_error(format!("unknown cleanup type `{declaration}`"))
                        })?;
                    match &declaration_item.kind {
                        ResolvedTypeDeclarationKind::Resource { drop } => {
                            let flag_index = u32::try_from(self.flags.len())
                                .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                            let flag = LivenessFlagId(flag_index);
                            self.flags.push(CleanupFlag {
                                id: flag,
                                place: CleanupPlace {
                                    storage,
                                    projections: projections.clone(),
                                },
                                lifecycle: drop.id.clone(),
                            });
                            shapes.push(FieldLivenessShape::Leaf {
                                flag,
                                lifecycle: drop.id.clone(),
                            });
                        }
                        ResolvedTypeDeclarationKind::Record { fields } => {
                            frames.try_reserve(2).map_err(|_| {
                                cleanup_error("cleanup shape capacity exceeds address space")
                            })?;
                            frames.push(Frame::FinishRecord(declaration, fields.len()));
                            frames.push(Frame::Children(declaration, fields, 0));
                        }
                        ResolvedTypeDeclarationKind::Variant { .. } => {
                            return Err(cleanup_error(
                                "droppable variant cleanup is outside the copy-only v1 slice",
                            ));
                        }
                    }
                }
                Frame::Children(declaration, fields, index) => {
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    frames.try_reserve(3).map_err(|_| {
                        cleanup_error("cleanup shape capacity exceeds address space")
                    })?;
                    frames.push(Frame::Children(declaration, fields, index + 1));
                    frames.push(Frame::FinishField(field));
                    projections.push(field.id.clone());
                    frames.push(Frame::Enter(&field.ty));
                }
                Frame::FinishField(field) => {
                    let shape = shapes
                        .pop()
                        .ok_or_else(|| cleanup_error("cleanup field shape is absent"))?;
                    if projections.pop().as_ref() != Some(&field.id) {
                        return Err(cleanup_error("cleanup projection stack is inconsistent"));
                    }
                    shapes.push(FieldLivenessShape::Record {
                        declaration: field.id.clone(),
                        fields: vec![FieldLiveness {
                            field: field.id.clone(),
                            field_index: field.index,
                            shape,
                        }],
                    });
                }
                Frame::FinishRecord(declaration, field_count) => {
                    let split = shapes
                        .len()
                        .checked_sub(field_count)
                        .ok_or_else(|| cleanup_error("cleanup record shapes are incomplete"))?;
                    let mut fields = Vec::with_capacity(field_count);
                    if fields.capacity() != field_count {
                        return Err(cleanup_error("cleanup record field capacity is not exact"));
                    }
                    for shape in shapes.drain(split..) {
                        let field = match shape {
                            FieldLivenessShape::Record {
                                declaration: field,
                                mut fields,
                            } if fields.len() == 1 && fields[0].field == field => fields.remove(0),
                            _ => unreachable!("field wrapper is internal to shape construction"),
                        };
                        if fields.len() == fields.capacity() {
                            return Err(cleanup_error(
                                "cleanup record field capacity was exhausted",
                            ));
                        }
                        fields.push(field);
                    }
                    if fields.len() != field_count || fields.capacity() != field_count {
                        return Err(cleanup_error(
                            "cleanup record field capacity disagrees with its shape",
                        ));
                    }
                    #[cfg(test)]
                    note_capacity_high_water(
                        frames.capacity() * std::mem::size_of::<Frame<'_>>()
                            + projections.capacity() * std::mem::size_of::<DeclarationId>()
                            + projections
                                .iter()
                                .map(|id| id.as_str().len())
                                .sum::<usize>()
                            + shapes.capacity() * std::mem::size_of::<FieldLivenessShape>()
                            + shapes.iter().map(shape_owned_capacity).sum::<usize>()
                            + fields.capacity() * std::mem::size_of::<FieldLiveness>()
                            + fields
                                .iter()
                                .map(|field| {
                                    field.field.as_str().len() + shape_owned_capacity(&field.shape)
                                })
                                .sum::<usize>()
                            + declaration.as_str().len(),
                    );
                    shapes.push(FieldLivenessShape::Record {
                        declaration: declaration.clone(),
                        fields,
                    });
                }
            }
        }
        if shapes.len() != 1 || !projections.is_empty() {
            return Err(cleanup_error("cleanup shape traversal did not settle"));
        }
        Ok(shapes.pop().expect("shape count checked above"))
    }

    fn collect_expression(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedExpr),
            Children(&'a ResolvedExpr, usize),
            Finish(&'a ResolvedExpr),
            AddBinding(&'a ResolvedBinding),
            AddUpdateBase(&'a ResolvedExpr),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 24) };

        let mut frames = vec![Frame::Enter(expression)];
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + self.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
                    + self.flags.capacity() * std::mem::size_of::<CleanupFlag>()
                    + self.slots.iter().map(slot_owned_capacity).sum::<usize>()
                    + self.flags.iter().map(flag_owned_capacity).sum::<usize>(),
            );
            match frame {
                Frame::Enter(expression) => {
                    frames.try_reserve(2).map_err(|_| {
                        cleanup_error("cleanup traversal capacity exceeds address space")
                    })?;
                    frames.push(Frame::Finish(expression));
                    frames.push(Frame::Children(expression, 0));
                }
                Frame::Children(expression, index) => {
                    frames.try_reserve(2).map_err(|_| {
                        cleanup_error("cleanup traversal capacity exceeds address space")
                    })?;
                    let mut enter = None;
                    let mut action = None;
                    match &expression.kind {
                        ResolvedExprKind::Call { args, .. } => {
                            enter = args.get(index);
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::Unary { value, .. }
                        | ResolvedExprKind::Project { base: value, .. }
                        | ResolvedExprKind::Try { operand: value, .. }
                        | ResolvedExprKind::TryOption { operand: value, .. } => {
                            enter = (index == 0).then_some(value.as_ref());
                        }
                        ResolvedExprKind::Binary { left, right, .. } => {
                            enter = match index {
                                0 => Some(left),
                                1 => Some(right),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::Block { statements, tail } => {
                            let statement_index = index / 2;
                            if let Some(statement) = statements.get(statement_index) {
                                if index % 2 == 0 {
                                    enter = Some(statement.value());
                                } else {
                                    // Only `let` and assignment statements
                                    // carry bindings; unsafe boundaries add
                                    // none.
                                    if let ResolvedStatement::Let { binding, .. } = statement {
                                        action = Some(Frame::AddBinding(binding));
                                    }
                                }
                            } else if index == statements.len() * 2 {
                                enter = Some(tail);
                            }
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            enter = match index {
                                0 => Some(condition),
                                1 => Some(then_branch),
                                2 => Some(else_branch),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::ConstructRecord { fields, .. }
                        | ResolvedExprKind::ConstructVariant { fields, .. } => {
                            enter = fields.get(index).map(|field| &field.value);
                        }
                        ResolvedExprKind::Match { scrutinee, arms } => {
                            enter = if index == 0 {
                                Some(scrutinee)
                            } else {
                                arms.get(index - 1).map(|arm| &arm.value)
                            };
                        }
                        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                            if index == 0 {
                                enter = Some(base);
                            } else if index == 1 {
                                action = Some(Frame::AddUpdateBase(base));
                            } else {
                                enter = fields.get(index - 2).map(|field| &field.value);
                            }
                        }
                        ResolvedExprKind::Int(_)
                        | ResolvedExprKind::Int32(_)
                        | ResolvedExprKind::Char(_)
                        | ResolvedExprKind::Uint8(_)
                        | ResolvedExprKind::Float32(_)
                        | ResolvedExprKind::Float64(_)
                        | ResolvedExprKind::Bool(_)
                        | ResolvedExprKind::Place(_) => {}
                    }
                    if enter.is_some() || action.is_some() {
                        frames.push(Frame::Children(expression, index + 1));
                        if let Some(action) = action {
                            frames.push(action);
                        }
                        if let Some(child) = enter {
                            frames.push(Frame::Enter(child));
                        }
                    }
                }
                Frame::Finish(expression) => self.collect_owned_temporary(expression)?,
                Frame::AddBinding(binding) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: binding.id.clone(),
                            },
                            binding.ty.clone(),
                        )?;
                    }
                }
                Frame::AddUpdateBase(base) => {
                    // A place expression normally needs no temporary. Record
                    // update deliberately materializes an owned base epoch.
                    if matches!(base.kind, ResolvedExprKind::Place(_))
                        && base.ownership == OwnershipMode::Own
                        && self.needs_drop(&base.ty)?
                    {
                        self.add_slot(
                            CleanupStorageOrigin::Temporary {
                                expression: base.id.clone(),
                            },
                            base.ty.clone(),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_owned_temporary(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        if expression.ownership == OwnershipMode::Own
            && self.needs_drop(&expression.ty)?
            && !matches!(expression.kind, ResolvedExprKind::Place(_))
        {
            self.add_slot(
                CleanupStorageOrigin::Temporary {
                    expression: expression.id.clone(),
                },
                expression.ty.clone(),
            )?;
        }
        Ok(())
    }
}

fn cleanup_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}
