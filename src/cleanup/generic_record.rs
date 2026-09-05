//! Exact cleanup shapes for bounded concrete generic record instances.

use super::*;

impl InventoryBuilder<'_> {
    /// Derive exact substituted cleanup paths for a concrete generic record.
    /// This stays iterative because a finite instance may reuse one generic
    /// template at multiple depths without being recursive by value.
    pub(super) fn shape_for_concrete_generic_record(
        &mut self,
        root: &ResolvedType,
        storage: CleanupStorageId,
        root_projections: &[DeclarationId],
    ) -> Result<FieldLivenessShape, Diagnostic> {
        enum Frame {
            Enter(ResolvedType, Vec<DeclarationId>, usize),
            FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
            Leave(String),
        }

        let mut frames = vec![Frame::Enter(root.clone(), root_projections.to_vec(), 1)];
        let mut shapes = Vec::new();
        let mut active = BTreeSet::new();
        let mut visited_fields = 0usize;
        let mut owned_leaves = 0usize;
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty, projections, depth) => {
                    if !self.needs_drop(&ty)? {
                        shapes.push(FieldLivenessShape::NoDrop);
                        continue;
                    }
                    if ty == ResolvedType::Bytes {
                        owned_leaves = owned_leaves.checked_add(1).ok_or_else(|| {
                            cleanup_error("generic cleanup owned-leaf count overflowed")
                        })?;
                        if owned_leaves > MAX_CLEANUP_OWNED_LEAVES {
                            return Err(cleanup_error(
                                "generic cleanup shape exceeds its owned-leaf limit",
                            ));
                        }
                        let flag = LivenessFlagId(
                            u32::try_from(self.flags.len())
                                .map_err(|_| cleanup_error("too many cleanup liveness flags"))?,
                        );
                        let lifecycle = DeclarationId::new(BYTES_DROP_LIFECYCLE_ID);
                        self.flags.push(CleanupFlag {
                            id: flag,
                            place: CleanupPlace {
                                storage,
                                projections,
                            },
                            lifecycle: lifecycle.clone(),
                        });
                        shapes.push(FieldLivenessShape::Leaf { flag, lifecycle });
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = &ty
                    else {
                        return Err(cleanup_error(format!(
                            "droppable generic field `{}` is not nominal",
                            ty.identity_key()
                        )));
                    };
                    if depth > MAX_CLEANUP_SHAPE_DEPTH {
                        return Err(cleanup_error(
                            "generic cleanup shape exceeds its record-depth limit",
                        ));
                    }
                    let item = self
                        .program
                        .types
                        .iter()
                        .find(|item| item.id == *declaration)
                        .ok_or_else(|| {
                            cleanup_error(format!("unknown cleanup type `{declaration}`"))
                        })?;
                    let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
                        return Err(cleanup_error(
                            "generic cleanup descendant is not an admitted record",
                        ));
                    };
                    let identity = ty.identity_key();
                    if !active.insert(identity.clone()) {
                        return Err(cleanup_error("generic cleanup shape is recursive"));
                    }
                    visited_fields = visited_fields.checked_add(fields.len()).ok_or_else(|| {
                        cleanup_error("generic cleanup visited-field count overflowed")
                    })?;
                    if visited_fields > MAX_CLEANUP_VISITED_FIELDS {
                        return Err(cleanup_error(
                            "generic cleanup shape exceeds its visited-field limit",
                        ));
                    }
                    let metadata = fields
                        .iter()
                        .map(|field| (field.id.clone(), field.index))
                        .collect::<Vec<_>>();
                    frames.push(Frame::FinishRecord(declaration.clone(), metadata));
                    frames.push(Frame::Leave(identity));
                    for field in fields.iter().rev() {
                        let mut field_projections = projections.clone();
                        field_projections.push(field.id.clone());
                        frames.push(Frame::Enter(
                            crate::hir::substitute_type(&field.ty, declaration, arguments)?,
                            field_projections,
                            depth + 1,
                        ));
                    }
                }
                Frame::FinishRecord(declaration, metadata) => {
                    let start = shapes.len().checked_sub(metadata.len()).ok_or_else(|| {
                        cleanup_error("generic cleanup record field shape is absent")
                    })?;
                    let children = shapes.split_off(start);
                    let fields = metadata
                        .into_iter()
                        .zip(children)
                        .map(|((field, field_index), shape)| FieldLiveness {
                            field,
                            field_index,
                            shape,
                        })
                        .collect();
                    shapes.push(FieldLivenessShape::Record {
                        declaration,
                        fields,
                    });
                }
                Frame::Leave(identity) => {
                    active.remove(&identity);
                }
            }
        }
        if shapes.len() != 1 {
            return Err(cleanup_error("generic cleanup shape did not settle"));
        }
        Ok(shapes.remove(0))
    }
}
