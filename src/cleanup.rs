//! Deterministic cleanup storage inventory.
//!
//! This is deliberately smaller than the executable `CleanupPlan` specified by
//! RFC 0003. It catalogs every storage candidate and resource leaf that a later
//! control-flow plan must govern. Beyond owned-parameter entry state, it does
//! not describe path-sensitive liveness, transfers, initialization/finalization
//! order, failure status, or exits.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};

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
        let shape = self.shape_for_type(&ty, storage, &mut Vec::new())?;
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
        projections: &mut Vec<DeclarationId>,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        if !self.needs_drop(ty)? {
            return Ok(FieldLivenessShape::NoDrop);
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
            .ok_or_else(|| cleanup_error(format!("unknown cleanup type `{declaration}`")))?;
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
                Ok(FieldLivenessShape::Leaf {
                    flag,
                    lifecycle: drop.id.clone(),
                })
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                let mut shapes = Vec::with_capacity(fields.len());
                for field in fields {
                    projections.push(field.id.clone());
                    let shape = self.shape_for_type(&field.ty, storage, projections)?;
                    projections.pop();
                    shapes.push(FieldLiveness {
                        field: field.id.clone(),
                        field_index: field.index,
                        shape,
                    });
                }
                Ok(FieldLivenessShape::Record {
                    declaration: declaration.clone(),
                    fields: shapes,
                })
            }
        }
    }

    fn collect_expression(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                for argument in args {
                    self.collect_expression(argument)?;
                }
            }
            ResolvedExprKind::Unary { value, .. } => self.collect_expression(value)?,
            ResolvedExprKind::Binary { left, right, .. } => {
                self.collect_expression(left)?;
                self.collect_expression(right)?;
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            self.collect_expression(value)?;
                            if binding.ownership == OwnershipMode::Own
                                && self.needs_drop(&binding.ty)?
                            {
                                self.add_slot(
                                    CleanupStorageOrigin::Binding {
                                        value: binding.id.clone(),
                                    },
                                    binding.ty.clone(),
                                )?;
                            }
                        }
                    }
                }
                self.collect_expression(tail)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expression(condition)?;
                self.collect_expression(then_branch)?;
                self.collect_expression(else_branch)?;
            }
            ResolvedExprKind::ConstructRecord { fields, .. } => {
                for field in fields {
                    self.collect_expression(&field.value)?;
                }
            }
            ResolvedExprKind::Project { base, .. } => self.collect_expression(base)?,
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        }
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
