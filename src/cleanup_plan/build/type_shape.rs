//! Canonical cleanup liveness shapes for direct and aggregate storage.

use super::*;

impl PlanBuilder<'_> {
    pub(super) fn shape_for_type(
        &mut self,
        ty: &ResolvedType,
        storage: &StorageId,
        projections: &mut Vec<DeclarationId>,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        if !self.needs_drop(ty)? {
            return Ok(FieldLivenessShape::NoDrop);
        }
        if matches!(ty, ResolvedType::Bytes) {
            let flag = LivenessFlagId(self.next_flag);
            self.next_flag = self
                .next_flag
                .checked_add(1)
                .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
            let lifecycle = DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID);
            self.leaves.insert(
                flag,
                LeafMetadata {
                    place: CleanupPlace {
                        storage: storage.clone(),
                        projections: projections.clone(),
                    },
                    lifecycle: lifecycle.clone(),
                },
            );
            return Ok(FieldLivenessShape::Leaf { flag, lifecycle });
        }
        if let Some(shape) = self.bounded_vec_shape(ty, storage, projections)? {
            return Ok(shape);
        }
        if let Some(shape) = self.bounded_box_shape(ty, storage, projections)? {
            return Ok(shape);
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Err(plan_error("droppable cleanup-plan type is not nominal"));
        };
        let item = self
            .program
            .types
            .iter()
            .find(|item| item.id == *declaration)
            .ok_or_else(|| plan_error(format!("unknown cleanup type `{declaration}`")))?;
        match &item.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if !arguments.is_empty() {
                    return Err(plan_error("generic cleanup-plan storage is unsupported"));
                }
                let flag = LivenessFlagId(self.next_flag);
                self.next_flag = self
                    .next_flag
                    .checked_add(1)
                    .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
                self.leaves.insert(
                    flag,
                    LeafMetadata {
                        place: CleanupPlace {
                            storage: storage.clone(),
                            projections: projections.clone(),
                        },
                        lifecycle: drop.id.clone(),
                    },
                );
                Ok(FieldLivenessShape::Leaf {
                    flag,
                    lifecycle: drop.id.clone(),
                })
            }
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                if !arguments.is_empty()
                    && !matches!(&item.kind, ResolvedTypeDeclarationKind::Record { .. })
                {
                    return Err(plan_error("generic cleanup-plan storage is unsupported"));
                }
                let mut shapes = Vec::with_capacity(fields.len());
                for field in fields {
                    projections.push(field.id.clone());
                    let field_ty = crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                    let shape = self.shape_for_type(&field_ty, storage, projections)?;
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
            ResolvedTypeDeclarationKind::Variant { cases } => {
                let mut case_shapes = Vec::with_capacity(cases.len());
                for case in cases {
                    let mut fields = Vec::with_capacity(case.fields.len());
                    for field in &case.fields {
                        let field_ty =
                            crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                        let shape = if self.needs_drop(&field_ty)? {
                            if field_ty != ResolvedType::Bytes {
                                return Err(plan_error(
                                    "droppable variant field is outside the direct-Bytes v1 slice",
                                ));
                            }
                            let flag = LivenessFlagId(self.next_flag);
                            self.next_flag = self
                                .next_flag
                                .checked_add(1)
                                .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
                            let lifecycle =
                                DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID);
                            let mut leaf_projections = projections.clone();
                            leaf_projections.push(case.id.clone());
                            leaf_projections.push(field.id.clone());
                            self.leaves.insert(
                                flag,
                                LeafMetadata {
                                    place: CleanupPlace {
                                        storage: storage.clone(),
                                        projections: leaf_projections,
                                    },
                                    lifecycle: lifecycle.clone(),
                                },
                            );
                            FieldLivenessShape::Leaf { flag, lifecycle }
                        } else {
                            FieldLivenessShape::NoDrop
                        };
                        fields.push(FieldLiveness {
                            field: field.id.clone(),
                            field_index: field.index,
                            shape,
                        });
                    }
                    case_shapes.push(crate::cleanup::VariantCaseLiveness {
                        case: case.id.clone(),
                        case_index: case.index,
                        fields,
                    });
                }
                Ok(FieldLivenessShape::Variant {
                    declaration: declaration.clone(),
                    cases: case_shapes,
                })
            }
        }
    }
}
