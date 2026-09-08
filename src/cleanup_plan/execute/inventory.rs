//! Checked cleanup leaf inventory for the trace executor.
use super::*;
pub(super) fn collect_leaves(
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), CleanupExecutionError> {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            let leaf = Leaf {
                place: CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.clone(),
                },
                lifecycle: lifecycle.clone(),
            };
            if leaves.insert(*flag, leaf).is_some() {
                return Err(invariant(format!(
                    "cleanup flag {} is declared more than once",
                    flag.0
                )));
            }
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                collect_leaves(storage, projections, &field.shape, leaves)?;
                projections.pop();
            }
        }
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                projections.push(case.case.clone());
                for field in &case.fields {
                    projections.push(field.field.clone());
                    collect_leaves(storage, projections, &field.shape, leaves)?;
                    projections.pop();
                }
                projections.pop();
            }
        }
    }
    Ok(())
}
