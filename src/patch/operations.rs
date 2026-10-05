//! Parsed semantic patch operations.
//!
//! [`SemanticPatch`] owns one typed operation sequence; canonical rendering,
//! selector keys, counts and the planner's per-family views all derive from
//! it. Fields stay private so only the admitted constructor builds a patch.

use crate::hir;

/// Borrowed planner view of one authored `rename` operation.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rename<'a> {
    pub(super) stable_id: &'a str,
    pub(super) new_name: &'a str,
    pub(super) operation_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PatchSchema {
    V1,
    V2,
    V3,
}

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum PatchSelector {
    AssignFunctionId(String),
    Rename(String),
    RenameMember(String, String),
    RenameCase(String, String),
    ReplaceCallTypeArgument(String, u32),
    RequireNoNewEffects,
}

impl PatchSelector {
    pub(super) fn label(&self) -> String {
        match self {
            Self::AssignFunctionId(target) => format!("assign:{target}"),
            Self::Rename(target) => format!("rename:{target}"),
            Self::RenameMember(owner, member) => format!("member:{owner}:{member}"),
            Self::RenameCase(owner, case) => format!("case:{owner}:{case}"),
            Self::ReplaceCallTypeArgument(expression, index) => {
                format!("call:{expression}:{index}")
            }
            Self::RequireNoNewEffects => "require:no-new-effects".to_owned(),
        }
    }
}

/// Borrowed view of the one v3 `assign-function-id` operation.
#[derive(Clone, Copy, Debug)]
pub(super) struct AssignFunctionId<'a> {
    pub(super) repair_id: &'a str,
    pub(super) target: &'a str,
    pub(super) name: &'a str,
    pub(super) to: &'a str,
}

/// Borrowed planner view of one authored `rename-member` operation.
#[derive(Clone, Copy, Debug)]
pub(super) struct RenameMember<'a> {
    pub(super) owner: &'a str,
    pub(super) member: &'a str,
    pub(super) new_name: &'a str,
    pub(super) operation_index: usize,
}

/// Borrowed planner view of one authored `rename-case` operation.
#[derive(Clone, Copy, Debug)]
pub(super) struct RenameCase<'a> {
    pub(super) owner: &'a str,
    pub(super) case: &'a str,
    pub(super) new_name: &'a str,
    pub(super) operation_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarType {
    I64,
    Bool,
}

impl ScalarType {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            _ => None,
        }
    }

    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    pub(super) fn resolved(self) -> hir::ResolvedType {
        match self {
            Self::I64 => hir::ResolvedType::I64,
            Self::Bool => hir::ResolvedType::Bool,
        }
    }
}

/// Borrowed planner view of one authored `replace-call-type-argument` operation.
#[derive(Clone, Copy, Debug)]
pub(super) struct ReplaceCallTypeArgument<'a> {
    pub(super) expression: &'a str,
    pub(super) template: &'a str,
    pub(super) old_instance: &'a str,
    pub(super) index: u32,
    pub(super) from: ScalarType,
    pub(super) to: ScalarType,
    pub(super) operation_index: usize,
}

/// One parsed semantic patch.
///
/// `operations` is the only owned copy of the admitted instructions, in
/// authored order; canonical rendering walks it directly. The planner's
/// per-family views below are borrowed projections that preserve authored
/// order within each family, so preflight keeps its grouped family order and
/// first-error selection without a second, separately mutable payload.
#[derive(Debug)]
pub(super) struct SemanticPatch {
    schema: PatchSchema,
    base: String,
    operations: Vec<PreflightOperation>,
}

impl SemanticPatch {
    /// The only constructor: every operation's `index` is its authored position.
    pub(super) fn admitted(
        schema: PatchSchema,
        base: String,
        operations: Vec<PreflightOperation>,
    ) -> Self {
        debug_assert!(operations
            .iter()
            .enumerate()
            .all(|(position, operation)| operation.index() == position));
        debug_assert!(
            (schema == PatchSchema::V3)
                == operations.iter().any(|operation| matches!(
                    operation,
                    PreflightOperation::AssignFunctionId { .. }
                ))
        );
        Self {
            schema,
            base,
            operations,
        }
    }

    pub(super) fn schema(&self) -> PatchSchema {
        self.schema
    }

    pub(super) fn base(&self) -> &str {
        &self.base
    }

    /// Every admitted instruction, in authored order.
    pub(super) fn operations(&self) -> &[PreflightOperation] {
        &self.operations
    }

    pub(super) fn renames(&self) -> impl Iterator<Item = Rename<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                PreflightOperation::Rename { index, target, to } => Some(Rename {
                    stable_id: target,
                    new_name: to,
                    operation_index: *index,
                }),
                _ => None,
            })
    }

    pub(super) fn member_renames(&self) -> impl Iterator<Item = RenameMember<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                PreflightOperation::RenameMember {
                    index,
                    owner,
                    member,
                    to,
                } => Some(RenameMember {
                    owner,
                    member,
                    new_name: to,
                    operation_index: *index,
                }),
                _ => None,
            })
    }

    pub(super) fn case_renames(&self) -> impl Iterator<Item = RenameCase<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                PreflightOperation::RenameCase {
                    index,
                    owner,
                    case,
                    to,
                } => Some(RenameCase {
                    owner,
                    case,
                    new_name: to,
                    operation_index: *index,
                }),
                _ => None,
            })
    }

    pub(super) fn call_type_argument_replacements(
        &self,
    ) -> impl Iterator<Item = ReplaceCallTypeArgument<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                PreflightOperation::ReplaceCallTypeArgument {
                    index,
                    expression,
                    template,
                    old_instance,
                    argument_index,
                    from,
                    to,
                } => Some(ReplaceCallTypeArgument {
                    expression,
                    template,
                    old_instance,
                    index: *argument_index,
                    from: *from,
                    to: *to,
                    operation_index: *index,
                }),
                _ => None,
            })
    }

    pub(super) fn no_new_effects(&self) -> bool {
        self.operations
            .iter()
            .any(|operation| matches!(operation, PreflightOperation::RequireNoNewEffects { .. }))
    }

    pub(super) fn assign_function_id(&self) -> Option<AssignFunctionId<'_>> {
        self.operations
            .iter()
            .find_map(|operation| match operation {
                PreflightOperation::AssignFunctionId {
                    repair_id,
                    target,
                    name,
                    to,
                    ..
                } => Some(AssignFunctionId {
                    repair_id,
                    target,
                    name,
                    to,
                }),
                _ => None,
            })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PreflightOperation {
    AssignFunctionId {
        index: usize,
        repair_id: String,
        target: String,
        name: String,
        to: String,
    },
    Rename {
        index: usize,
        target: String,
        to: String,
    },
    RenameMember {
        index: usize,
        owner: String,
        member: String,
        to: String,
    },
    RenameCase {
        index: usize,
        owner: String,
        case: String,
        to: String,
    },
    ReplaceCallTypeArgument {
        index: usize,
        expression: String,
        template: String,
        old_instance: String,
        argument_index: u32,
        from: ScalarType,
        to: ScalarType,
    },
    RequireNoNewEffects {
        index: usize,
    },
}

impl PreflightOperation {
    /// Authored position of this operation within its patch.
    pub(crate) fn index(&self) -> usize {
        match self {
            Self::AssignFunctionId { index, .. }
            | Self::Rename { index, .. }
            | Self::RenameMember { index, .. }
            | Self::RenameCase { index, .. }
            | Self::ReplaceCallTypeArgument { index, .. }
            | Self::RequireNoNewEffects { index } => *index,
        }
    }
}
