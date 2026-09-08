//! Private structural-cost accumulators for the expected projection.

use crate::diagnostic::Diagnostic;

use super::super::{active_builder_limit, checked_usage};

#[derive(Clone, Copy)]
pub(super) struct ExpandedDefaultCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
}

#[derive(Clone, Copy)]
pub(super) struct GenericInstanceCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
}

/// Bytes the resolver clones from one source tree, split into the fixed node
/// footprints that expand by the structural factor and the string contents
/// that expand by the copy factor. `string_bytes` is always part of `total`.
pub(in crate::workspace_graph) struct StructuralCost {
    pub(super) total: usize,
    pub(super) string_bytes: usize,
    embedded_headers: bool,
    inline_values: bool,
    pub(super) scalar_identity_discount: usize,
    pub(super) literal_fixed_discount: usize,
}

impl StructuralCost {
    pub(super) const fn new() -> Self {
        Self {
            total: 0,
            string_bytes: 0,
            embedded_headers: false,
            inline_values: false,
            scalar_identity_discount: 0,
            literal_fixed_discount: 0,
        }
    }

    /// Raw AST fallback only. Every payload-only call must follow charging the
    /// enclosing Rust value, whose inline layout already includes this header.
    /// Fixed HIR bundle assertions use the full Expr/Function/TypeDeclaration
    /// footprints, all still charged; payload and identity multipliers are
    /// unchanged. Vec<String> elements and synthesized strings use `string`.
    pub(super) fn raw_ast(embedded_headers: bool) -> Self {
        Self {
            embedded_headers,
            ..Self::new()
        }
    }

    pub(super) fn is_raw_fallback(&self) -> bool {
        self.inline_values
    }

    pub(super) fn with_inline_values(mut self, enabled: bool) -> Self {
        self.inline_values = enabled;
        self
    }

    pub(super) fn account_scalar_identity(
        &mut self,
        kind: &crate::ast::ExprKind,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.inline_values {
            self.literal_fixed_discount = checked_usage(
                self.literal_fixed_discount,
                literal_fixed_discount(kind),
                "builder_bytes",
                active_builder_limit(),
            )?;
            self.scalar_identity_discount = checked_usage(
                self.scalar_identity_discount,
                super::identity_slots::scalar_expression_identity_discount(kind),
                "builder_bytes",
                active_builder_limit(),
            )?;
        }
        Ok(())
    }

    pub(super) fn inline_pattern_parent<T, P>(
        &mut self,
        parent: &T,
        pattern: &P,
    ) -> Result<(), Vec<Diagnostic>> {
        let bytes = std::mem::size_of_val(parent);
        self.add(if self.inline_values {
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
    pub(super) fn inline_expr_parent<T>(&mut self, parent: &T) -> Result<(), Vec<Diagnostic>> {
        let bytes = std::mem::size_of_val(parent);
        self.add(if self.inline_values {
            bytes
                .checked_sub(std::mem::size_of::<crate::ast::Expr>())
                .expect("audited parent contains one inline Expr")
        } else {
            bytes
        })
    }

    pub(super) fn embedded_string(&mut self, value: &str) -> Result<(), Vec<Diagnostic>> {
        if self.embedded_headers {
            self.add_split(value.len(), value.len())
        } else {
            self.string(value)
        }
    }

    pub(super) fn function_signature(
        &mut self,
        function: &crate::ast::Function,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.inline_values {
            self.add(
                std::mem::size_of_val(function)
                    - std::mem::size_of::<crate::ast::Expr>()
                    - std::mem::size_of::<crate::ast::Type>(),
            )
        } else {
            self.value(function)
        }
    }

    pub(super) fn inline_type_parent<T>(&mut self, parent: &T) -> Result<(), Vec<Diagnostic>> {
        self.add(if self.inline_values {
            std::mem::size_of_val(parent)
                .checked_sub(std::mem::size_of::<crate::ast::Type>())
                .expect("audited parent contains an inline Type")
        } else {
            std::mem::size_of_val(parent)
        })
    }

    pub(super) fn match_arm(&mut self, arm: &crate::ast::MatchArm) -> Result<(), Vec<Diagnostic>> {
        if self.inline_values {
            self.add(
                std::mem::size_of_val(arm)
                    - std::mem::size_of::<crate::ast::Expr>()
                    - std::mem::size_of_val(&arm.pattern),
            )
        } else {
            self.value(arm)
        }
    }

    pub(super) const fn structure(bytes: usize) -> Self {
        Self {
            total: bytes,
            string_bytes: 0,
            embedded_headers: false,
            inline_values: false,
            scalar_identity_discount: 0,
            literal_fixed_discount: 0,
        }
    }

    pub(super) fn add(&mut self, bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.total = checked_usage(self.total, bytes, "builder_bytes", active_builder_limit())?;
        Ok(())
    }

    /// Absorbs a cost whose `string_bytes` are already part of `bytes`.
    pub(super) fn add_split(
        &mut self,
        bytes: usize,
        string_bytes: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        self.add(bytes)?;
        self.string_bytes = checked_usage(
            self.string_bytes,
            string_bytes,
            "builder_bytes",
            active_builder_limit(),
        )?;
        Ok(())
    }

    pub(super) fn value<T>(&mut self, value: &T) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of_val(value))
    }

    pub(super) fn string(&mut self, value: &str) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of::<String>())?;
        self.add_split(value.len(), value.len())
    }

    pub(super) fn program(&mut self, program: &crate::ast::Program) -> Result<(), Vec<Diagnostic>> {
        self.add(program_carrier_bytes(std::slice::from_ref(program)).unwrap())?;
        for agent in &program.agents {
            self.embedded_string(&agent.stable_id)?;
            self.embedded_string(&agent.name)?;
            for role in &agent.types {
                self.value(role)?;
                self.embedded_string(&role.stable_id)?;
            }
            for operation in &agent.operations {
                self.value(operation)?;
                self.embedded_string(&operation.stable_id)?;
            }
            self.embedded_string(&agent.runtime_v1_json)?;
        }
        Ok(())
    }
}

pub(crate) fn program_carrier_bytes(programs: &[crate::ast::Program]) -> Option<usize> {
    let base = std::mem::size_of::<crate::ast::Program>()
        - std::mem::size_of::<Vec<crate::ast::AgentDeclaration>>();
    programs.len().checked_mul(base)?.checked_add(
        programs
            .iter()
            .map(|program| program.agents.len())
            .try_fold(0usize, usize::checked_add)?
            .checked_mul(std::mem::size_of::<crate::ast::AgentDeclaration>())?,
    )
}

pub(super) fn is_builder_refusal(errors: &[Diagnostic]) -> bool {
    !errors.is_empty()
        && errors.iter().all(|error| {
            error.code == "SPX-G171"
                && error.message
                    == format!(
                        "Workspace Semantic Graph `builder_bytes` exceeds {}",
                        active_builder_limit()
                    )
        })
}

#[cfg(test)]
mod tests {
    use super::StructuralCost;

    #[test]
    fn identity_prebound_embedded_headers_keep_parent_and_payload() {
        let expression = crate::ast::Expr {
            kind: crate::ast::ExprKind::Var("input".to_owned()),
            span: crate::ast::Span::default(),
        };
        let mut legacy = StructuralCost::new();
        let mut tight = StructuralCost::raw_ast(true);
        super::super::declaration_cost::ast_expr_cost(&expression, &mut legacy).unwrap();
        super::super::declaration_cost::ast_expr_cost(&expression, &mut tight).unwrap();
        assert_eq!(tight.total, std::mem::size_of_val(&expression) + 5);
        assert_eq!(legacy.total - tight.total, std::mem::size_of::<String>());
        assert_eq!(legacy.string_bytes, tight.string_bytes);
        // The fixed HIR bundle is bounded by Expr's footprint alone, not by
        // the duplicate header. Removing it does not alter that assertion.
        assert!(
            super::super::super::HIR_EXPR_FIXED_BUNDLE
                <= super::super::super::HIR_FIXED_EXPANSION_FACTOR
                    * (tight.total - tight.string_bytes)
        );
    }

    #[test]
    fn identity_prebound_standalone_and_runtime_headers_remain_charged() {
        for mut cost in [StructuralCost::new(), StructuralCost::raw_ast(true)] {
            cost.string("effect").unwrap();
            assert_eq!(cost.total, std::mem::size_of::<String>() + 6);
            assert_eq!(cost.string_bytes, 6);
        }
        // Synthetic/runtime accumulators never enable the AST-only fallback,
        // even when they share an AST visitor for a generated wrapper.
        let mut runtime = StructuralCost::structure(0);
        runtime.embedded_string("alias").unwrap();
        assert_eq!(runtime.total, std::mem::size_of::<String>() + 5);
    }
    #[test]
    fn identity_prebound_inline_expr_layout_preserves_each_child_bundle() {
        let program = crate::parse(
            "module test; @id(\"test.main\") fn main() -> i64 { let x = 1; x }",
            std::path::Path::new("inline.spx"),
        )
        .unwrap();
        let expression = &program.functions[0].body;
        let mut headers_only = StructuralCost::raw_ast(true);
        let mut inline = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_expr_cost(expression, &mut headers_only).unwrap();
        super::super::declaration_cost::ast_expr_cost(expression, &mut inline).unwrap();
        assert_eq!(
            headers_only.total - inline.total,
            std::mem::size_of::<crate::ast::Expr>()
        );
        assert_eq!(headers_only.string_bytes, inline.string_bytes);
        let program = crate::parse(
            "module test; @id(\"test.main\") fn main() -> i64 { while false { 0 } 0 }",
            std::path::Path::new("boxed.spx"),
        )
        .unwrap();
        let mut headers_only = StructuralCost::raw_ast(true);
        let mut inline = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_expr_cost(
            &program.functions[0].body,
            &mut headers_only,
        )
        .unwrap();
        super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut inline)
            .unwrap();
        assert_eq!(
            headers_only.total, inline.total,
            "boxed while children retain their own allocation"
        );
    }
    #[test]
    fn identity_prebound_raw_fallback_charges_literal_and_field_payloads() {
        for (short, long) in [
            ("\"x\"", "\"xxxxx\""),
            (
                "{ let mut value = 0; value.x = 1; 0 }",
                "{ let mut value = 0; value.xxxxx = 1; 0 }",
            ),
        ] {
            let costs = [short, long].map(|body| {
                let source =
                    format!("module test; @id(\"test.main\") fn main() -> i64 {{ {body} }}");
                let program = crate::parse(&source, std::path::Path::new("payload.spx")).unwrap();
                let mut legacy = StructuralCost::new();
                let mut tight = StructuralCost::raw_ast(true).with_inline_values(true);
                super::super::declaration_cost::ast_expr_cost(
                    &program.functions[0].body,
                    &mut legacy,
                )
                .unwrap();
                super::super::declaration_cost::ast_expr_cost(
                    &program.functions[0].body,
                    &mut tight,
                )
                .unwrap();
                (legacy.total, tight.total, tight.string_bytes)
            });
            assert_eq!(costs[0].0, costs[1].0, "legacy receipt is frozen");
            assert_eq!(costs[1].1 - costs[0].1, 4);
            assert_eq!(costs[1].2 - costs[0].2, 4);
        }
    }

    #[test]
    fn identity_prebound_raw_fallback_charges_callable_and_local_types() {
        use crate::ast::Type;
        let ty = Type::Function {
            parameters: vec![Type::Named {
                name: "Input".to_owned(),
                arguments: vec![],
            }],
            result: Box::new(Type::I64),
        };
        let mut legacy = StructuralCost::new();
        let mut tight = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_type_cost(&ty, &mut legacy).unwrap();
        super::super::declaration_cost::ast_type_cost(&ty, &mut tight).unwrap();
        assert_eq!(legacy.total, std::mem::size_of::<Type>());
        assert_eq!(tight.total, 3 * std::mem::size_of::<Type>() + 5);
        assert_eq!(tight.string_bytes, 5);
        let totals = ["Input", "InputLong"].map(|name| {
            let source = format!(
                "module test; @id(\"test.main\") fn main() -> i64 {{ let x: {name} = 0; 0 }}"
            );
            let program = crate::parse(&source, std::path::Path::new("annotation.spx")).unwrap();
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut cost)
                .unwrap();
            cost.total
        });
        assert_eq!(totals[1] - totals[0], 4);
    }
    #[test]
    fn identity_prebound_scalar_discount_preserves_calls_and_expression_ids() {
        for (body, expected) in [("1 + -2", 10), ("callee(1)", 2), ("input", 0)] {
            let source = format!("module test; @id(\"test.main\") fn main() -> i64 {{ {body} }}");
            let program = crate::parse(&source, std::path::Path::new("scalar-slots.spx")).unwrap();
            let mut legacy = StructuralCost::new();
            let mut tight = StructuralCost::raw_ast(true).with_inline_values(true);
            super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut legacy)
                .unwrap();
            super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut tight)
                .unwrap();
            assert_eq!(legacy.scalar_identity_discount, 0);
            assert_eq!(tight.scalar_identity_discount, expected);
            // BASE=3: the per-expression identity is never removed.
            assert_eq!(
                super::super::identity_slots::scalar_expression_identity_discount(
                    &crate::ast::ExprKind::Bool(true)
                ),
                2
            );
        }
    }
    #[test]
    fn identity_prebound_function_and_parameter_inline_children_count_once() {
        let program = crate::parse(
            "module test; @id(\"test.value\") fn value(input: i64) -> i64 { input }",
            std::path::Path::new("function-layout.spx"),
        )
        .unwrap();
        let function = &program.functions[0];
        let mut legacy = StructuralCost::new();
        let mut tight = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_function_cost(function, &mut legacy).unwrap();
        super::super::declaration_cost::ast_function_cost(function, &mut tight).unwrap();
        assert_eq!(legacy.string_bytes, tight.string_bytes);
        assert_eq!(
            legacy.total - tight.total,
            4 * std::mem::size_of::<String>()
                + std::mem::size_of::<crate::ast::Expr>()
                + 2 * std::mem::size_of::<crate::ast::Type>()
        );
        assert!(
            tight.total
                >= std::mem::size_of::<crate::ast::Expr>()
                    + 2 * std::mem::size_of::<crate::ast::Type>()
        );
    }
}

// Raw-AST fallback preserves these complete, independently charged children.
// The remaining Function carrier alone still covers its entire fixed HIR bundle.
const _: () = assert!(
    super::super::HIR_FUNCTION_FIXED_BUNDLE
        <= super::super::HIR_FIXED_EXPANSION_FACTOR
            * (std::mem::size_of::<crate::ast::Function>()
                - std::mem::size_of::<crate::ast::Expr>()
                - std::mem::size_of::<crate::ast::Type>())
);

// Primitive literal lowering returns the incoming block/state unchanged with
// owned_source=None; inventory requires Own && needs_drop to create a temporary.
// Keep all state/transition/block/edge/region and parent metadata: an enclosing
// contract or publication may still need them. Only these four owned-storage
// allocations are absent. All eight bookkeeping footprints remain reserved.
const LITERAL_ABSENT_STORAGE: usize = std::mem::size_of::<crate::cleanup::CleanupStorageSlot>()
    + std::mem::size_of::<crate::cleanup::CleanupFlag>()
    + std::mem::size_of::<crate::cleanup::CleanupPlace>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupSlot>();
const LITERAL_FIXED_BUNDLE: usize = super::super::HIR_EXPR_FIXED_BUNDLE - LITERAL_ABSENT_STORAGE;
const _: () = assert!(
    LITERAL_FIXED_BUNDLE
        <= super::super::HIR_FIXED_EXPANSION_FACTOR * std::mem::size_of::<crate::ast::Expr>()
            - LITERAL_ABSENT_STORAGE
);

fn literal_fixed_discount(kind: &crate::ast::ExprKind) -> usize {
    use crate::ast::ExprKind;
    match kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_) => LITERAL_ABSENT_STORAGE,
        _ => 0,
    }
}

#[cfg(test)]
#[test]
fn identity_prebound_literal_bundle_excludes_every_nonliteral_root() {
    use crate::ast::{Expr, ExprKind, Span, UnaryOp};
    let leaf = || {
        Box::new(Expr {
            kind: ExprKind::Int(1),
            span: Span::default(),
        })
    };
    for kind in [
        ExprKind::Int(1),
        ExprKind::Int32(1),
        ExprKind::Char(65),
        ExprKind::Uint8(1),
        ExprKind::Usize(1),
        ExprKind::Float32(0),
        ExprKind::Float64(0),
        ExprKind::Bool(false),
    ] {
        assert_eq!(literal_fixed_discount(&kind), LITERAL_ABSENT_STORAGE);
    }
    for kind in [
        ExprKind::String("owned".to_owned()),
        ExprKind::ArrayU8(vec![1]),
        ExprKind::Call {
            name: "bytes_copy".to_owned(),
            type_arguments: vec![],
            args: vec![],
        },
        ExprKind::Var("owned".to_owned()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value: leaf(),
        },
        ExprKind::Block {
            statements: vec![],
            tail: leaf(),
        },
    ] {
        assert_eq!(literal_fixed_discount(&kind), 0);
    }
}
