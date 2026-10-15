//! Original raw structural formulas with typed refusal custody. No old visitor
//! or StructuralCost method is called on this actual constructor route.
use super::{absent_storage_root, literal_fixed_discount, StructuralCost};
use crate::workspace_graph::constructor_failure::receiving::{Receiver, Setup};
use std::mem::{size_of, size_of_val};

pub(in crate::workspace_graph) struct Cost<'r> {
    pub(in crate::workspace_graph) raw: StructuralCost,
    owner: &'r mut Receiver,
}
impl<'r> Cost<'r> {
    pub(in crate::workspace_graph) fn new(
        owner: &'r mut Receiver,
        inline: bool,
    ) -> Result<Self, Setup> {
        owner.reserve(
            size_of::<Result<Self, Setup>>()
                + size_of::<Self>()
                + size_of::<&mut Receiver>()
                + size_of::<bool>(),
        )?;
        Ok(Self {
            raw: StructuralCost::raw_ast(true).with_inline_values(inline),
            owner,
        })
    }
    pub(in crate::workspace_graph) fn frame(&mut self, arguments: usize) -> Result<(), Setup> {
        self.owner.reserve(
            size_of::<Result<(), Setup>>()
                + size_of::<&mut Self>()
                + size_of::<usize>()
                + arguments,
        )
    }
    pub(in crate::workspace_graph) fn is_raw_fallback(&self) -> bool {
        self.raw.is_raw_fallback()
    }
    pub(in crate::workspace_graph) fn pay(&mut self) -> Result<(), Setup> {
        self.owner.reserve(self.raw.total)
    }
    pub(in crate::workspace_graph) fn teardown(&mut self) -> Result<(), Setup> {
        self.frame(size_of::<Option<usize>>() + size_of::<usize>())?;
        let bytes = self
            .raw
            .total
            .checked_mul(4)
            .ok_or_else(|| self.owner.limit())?;
        self.owner.reserve(bytes)
    }
    pub(in crate::workspace_graph) fn note_exact_expr_fixed_bundle(&mut self) -> Result<(), Setup> {
        self.frame(0)?;
        self.raw.exact_expressions = self.owner.sum(self.raw.exact_expressions, 1)?;
        let slack = crate::workspace_graph::HIR_FIXED_EXPANSION_FACTOR
            * std::mem::size_of::<crate::ast::Expr>()
            - crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE;
        self.raw.exact_fixed_discount = self.owner.sum(self.raw.exact_fixed_discount, slack)?;
        Ok(())
    }

    pub(in crate::workspace_graph) fn note_single_identity_carrier(&mut self) -> Result<(), Setup> {
        self.frame(0)?;
        self.raw.single_identity_carriers = self.owner.sum(self.raw.single_identity_carriers, 1)?;
        Ok(())
    }

    pub(in crate::workspace_graph) fn add_identity_carriers(
        &mut self,
        count: usize,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&count))?;
        self.raw.identity_carriers = self.owner.sum(self.raw.identity_carriers, count)?;
        Ok(())
    }

    pub(in crate::workspace_graph) fn account_scalar_identity(
        &mut self,
        kind: &crate::ast::ExprKind,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&kind))?;
        if self.raw.inline_values {
            self.raw.literal_fixed_discount = self.owner.sum(
                self.raw.literal_fixed_discount,
                literal_fixed_discount(kind),
            )?;
            self.raw.scalar_identity_discount = self.owner.sum(self.raw.scalar_identity_discount, crate::workspace_graph::expected_projection::identity_slots::scalar_expression_identity_discount(kind))?;
        }
        Ok(())
    }

    /// Counted beside the root's complete exact bundle, on the same raw-AST
    /// accumulator as the literal discount. Runtime/synthetic accumulators
    /// never enable `inline_values` and so keep every storage footprint.
    pub(in crate::workspace_graph) fn account_exact_absent_storage(
        &mut self,
        kind: &crate::ast::ExprKind,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&kind))?;
        if let (true, Some(root)) = (self.raw.inline_values, absent_storage_root(kind)) {
            let count = &mut self.raw.exact_absent_roots[root as usize];
            *count = self.owner.sum(*count, 1)?;
        }
        Ok(())
    }

    /// Counted beside the storage census, for the same root kinds plus `if`
    /// and variable/block roots, only when the parent proves the position carrier-free (see
    /// `ROOT_AUXILIARY_CARRIERS`). An `if` root keeps its storage, so it is
    /// censused separately. Variables also retain storage, and their census
    /// excludes every closure subtree, where capture lowering adds bindings.
    /// Runtime/synthetic accumulators never count.
    pub(in crate::workspace_graph) fn account_carrier_free_root(
        &mut self,
        kind: &crate::ast::ExprKind,
        carrier_free: bool,
        outside_closure: bool,
    ) -> Result<(), Setup> {
        self.frame(
            size_of_val(&kind) + size_of_val(&carrier_free) + size_of_val(&outside_closure),
        )?;
        if !(self.raw.inline_values && carrier_free) {
            return Ok(());
        }
        let count = if absent_storage_root(kind).is_some() {
            &mut self.raw.exact_carrier_free_roots
        } else if matches!(kind, crate::ast::ExprKind::If { .. }) {
            &mut self.raw.exact_carrier_free_ifs
        } else if outside_closure && matches!(kind, crate::ast::ExprKind::Var(_)) {
            &mut self.raw.exact_carrier_free_vars
        } else if outside_closure && matches!(kind, crate::ast::ExprKind::Block { .. }) {
            &mut self.raw.exact_carrier_free_blocks
        } else {
            return Ok(());
        };
        *count = self.owner.sum(*count, 1)?;
        Ok(())
    }

    pub(in crate::workspace_graph) fn inline_pattern_parent<T, P>(
        &mut self,
        parent: &T,
        pattern: &P,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&parent) + size_of_val(&pattern))?;
        let bytes = std::mem::size_of_val(parent);
        self.add(if self.raw.inline_values {
            bytes
                .checked_sub(std::mem::size_of_val(pattern))
                .expect("audited parent contains the inline pattern")
        } else {
            bytes
        })
    }

    /// Used only for parents with one inline Expr that the visitor also visits.
    /// Every complete child Expr remains charged by ast_expr_cost. In
    /// particular, boxed children never use this correction. Its HIR bundle
    /// already includes the statement, arm, and field-initializer carriers.
    pub(in crate::workspace_graph) fn inline_expr_parent<T>(
        &mut self,
        parent: &T,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&parent))?;
        let bytes = std::mem::size_of_val(parent);
        self.add(if self.raw.inline_values {
            bytes
                .checked_sub(std::mem::size_of::<crate::ast::Expr>())
                .expect("audited parent contains one inline Expr")
        } else {
            bytes
        })
    }

    pub(in crate::workspace_graph) fn embedded_string(&mut self, value: &str) -> Result<(), Setup> {
        self.frame(size_of_val(&value))?;
        if self.raw.embedded_headers {
            self.add_split(value.len(), value.len())
        } else {
            self.string(value)
        }
    }

    pub(in crate::workspace_graph) fn function_signature(
        &mut self,
        function: &crate::ast::Function,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&function))?;
        if self.raw.inline_values {
            self.add(
                std::mem::size_of_val(function)
                    - std::mem::size_of::<crate::ast::Expr>()
                    - std::mem::size_of::<crate::ast::Type>(),
            )
        } else {
            self.value(function)
        }
    }

    pub(in crate::workspace_graph) fn inline_type_parent<T>(
        &mut self,
        parent: &T,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&parent))?;
        self.add(if self.raw.inline_values {
            std::mem::size_of_val(parent)
                .checked_sub(std::mem::size_of::<crate::ast::Type>())
                .expect("audited parent contains an inline Type")
        } else {
            std::mem::size_of_val(parent)
        })
    }

    pub(in crate::workspace_graph) fn match_arm(
        &mut self,
        arm: &crate::ast::MatchArm,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&arm))?;
        if self.raw.inline_values {
            self.add(
                std::mem::size_of_val(arm)
                    - std::mem::size_of::<crate::ast::Expr>()
                    - std::mem::size_of_val(&arm.pattern),
            )
        } else {
            self.value(arm)
        }
    }

    pub(in crate::workspace_graph) fn add(&mut self, bytes: usize) -> Result<(), Setup> {
        self.frame(size_of_val(&bytes))?;
        self.raw.total = self.owner.sum(self.raw.total, bytes)?;
        Ok(())
    }

    /// Absorbs a cost whose `string_bytes` are already part of `bytes`.
    pub(in crate::workspace_graph) fn add_split(
        &mut self,
        bytes: usize,
        string_bytes: usize,
    ) -> Result<(), Setup> {
        self.frame(size_of_val(&bytes) + size_of_val(&string_bytes))?;
        self.add(bytes)?;
        self.raw.string_bytes = self.owner.sum(self.raw.string_bytes, string_bytes)?;
        Ok(())
    }

    pub(in crate::workspace_graph) fn value<T>(&mut self, value: &T) -> Result<(), Setup> {
        self.frame(size_of_val(&value))?;
        self.add(std::mem::size_of_val(value))
    }

    pub(in crate::workspace_graph) fn string(&mut self, value: &str) -> Result<(), Setup> {
        self.frame(size_of_val(&value))?;
        self.add(std::mem::size_of::<String>())?;
        self.add_split(value.len(), value.len())
    }
}
