//! Private structural-cost accumulators for the expected projection.
//!
//! Fixed inline HIR/cleanup footprints retain their original structural proof.
//! Retained ExpressionId heap carriers have a separate checked prebound: three
//! per visited expression cover direct ordinary/native/host/intrinsic lowering.
//! `super` methods charge four, conservatively including the discarded receiver;
//! sixteen per authored traversal statement cover `for` (16) and `for own`
//! (15), independently of their source/body children. Raw AST has no source
//! type: a `Slice<u8>` `for` receiver is outside this count, and its lowering
//! debits its own carriers and backing on the live ledger before construction
//! (`hir::resolve_for::bytes`). Cloned identities share
//! backing. Defaults, imported wrappers and generic materializations propagate
//! their own counts before resolution. This bounds retained/direct lowering,
//! not cumulative constructor attempts: upcast retries can revisit subtrees.
//! Every constructor independently reserves its carrier before allocating, so
//! all repeated/transient attempts also consume the live monotonic ledger.
//! Payloads retain independent identity/string bounds; heap carriers never
//! spend bookkeeping allowance.

use crate::diagnostic::Diagnostic;

use super::super::{active_builder_limit, checked_usage};

#[derive(Clone, Copy)]
pub(super) struct ExpandedDefaultCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
    pub(super) identity_carriers: usize,
}

#[derive(Clone, Copy)]
pub(super) struct GenericInstanceCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
    pub(super) identity_carriers: usize,
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
    pub(super) identity_carriers: usize,
    pub(super) single_identity_carriers: usize,
    pub(super) exact_fixed_discount: usize,
    pub(super) exact_expressions: usize,
    pub(super) exact_absent_roots: [usize; 3],
    pub(super) exact_carrier_free_roots: usize,
    pub(super) exact_carrier_free_ifs: usize,
    pub(super) exact_carrier_free_vars: usize,
    pub(super) exact_carrier_free_blocks: usize,
    pub(super) exact_carrier_free_calls: usize,
}

/// Per-kind census of authored roots whose root storage is proved absent.
/// Exact uncached modes 5/6 and the cached constructor-exact profile consult
/// it. Earlier modes retain their literal-only calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AbsentStorageRoot {
    Literal = 0,
    Unary = 1,
    Binary = 2,
}

// The final uncached profile uses the complete asserted expression bundle,
// retaining every map/tree allowance and live debit. Only the same four
// independently absent root storage footprints are excluded, once per authored
// root, plus the six auxiliary carriers of authored carrier-free
// literal/unary/binary/`if` roots and variable/block roots outside closures.
// Authenticated authored ordinary call roots in those same positions omit
// only the six carriers and keep all four storage footprints.
// Runtime/default expansions retain their complete storage bundles.
pub(super) fn fixed_hir_discount(
    mode: u8,
    raw: &StructuralCost,
    runtime: &StructuralCost,
) -> Result<usize, Vec<Diagnostic>> {
    if mode < 5 {
        return Ok(raw.literal_fixed_discount);
    }
    let absent = if matches!(mode, 5 | 6) {
        exact_absent_storage(raw)?
    } else {
        raw.literal_fixed_discount
    };
    checked_usage(
        absent,
        checked_usage(
            raw.exact_fixed_discount,
            runtime.exact_fixed_discount,
            "builder_bytes",
            active_builder_limit(),
        )?,
        "builder_bytes",
        active_builder_limit(),
    )
}

pub(super) fn identity_carriers(
    mode: u8,
    raw: &StructuralCost,
    runtime: &StructuralCost,
) -> Result<usize, Vec<Diagnostic>> {
    let total = checked_usage(
        raw.identity_carriers,
        runtime.identity_carriers,
        "builder_bytes",
        active_builder_limit(),
    )?;
    if mode < 5 {
        return Ok(total);
    }
    let single = checked_usage(
        raw.single_identity_carriers,
        runtime.single_identity_carriers,
        "builder_bytes",
        active_builder_limit(),
    )?;
    Ok(total
        .checked_sub(single.checked_mul(2).ok_or_else(|| {
            vec![super::super::limit_error(
                "builder_bytes",
                active_builder_limit(),
            )]
        })?)
        .expect("single-root branches retain their independently charged root"))
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
            identity_carriers: 0,
            single_identity_carriers: 0,
            exact_fixed_discount: 0,
            exact_expressions: 0,
            exact_absent_roots: [0; 3],
            exact_carrier_free_roots: 0,
            exact_carrier_free_ifs: 0,
            exact_carrier_free_vars: 0,
            exact_carrier_free_blocks: 0,
            exact_carrier_free_calls: 0,
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

    pub(super) fn note_exact_expr_fixed_bundle(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.exact_expressions = checked_usage(
            self.exact_expressions,
            1,
            "builder_bytes",
            active_builder_limit(),
        )?;
        let slack = super::super::HIR_FIXED_EXPANSION_FACTOR
            * std::mem::size_of::<crate::ast::Expr>()
            - super::super::HIR_EXPR_FIXED_BUNDLE;
        self.exact_fixed_discount = checked_usage(
            self.exact_fixed_discount,
            slack,
            "builder_bytes",
            active_builder_limit(),
        )?;
        Ok(())
    }

    pub(super) fn note_single_identity_carrier(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.single_identity_carriers = checked_usage(
            self.single_identity_carriers,
            1,
            "builder_bytes",
            active_builder_limit(),
        )?;
        Ok(())
    }

    pub(super) fn add_identity_carriers(&mut self, count: usize) -> Result<(), Vec<Diagnostic>> {
        self.identity_carriers = checked_usage(
            self.identity_carriers,
            count,
            "builder_bytes",
            active_builder_limit(),
        )?;
        Ok(())
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

    /// Counted beside the root's complete exact bundle, on the same raw-AST
    /// accumulator as the literal discount. Runtime/synthetic accumulators
    /// never enable `inline_values` and so keep every storage footprint.
    pub(super) fn account_exact_absent_storage(
        &mut self,
        kind: &crate::ast::ExprKind,
    ) -> Result<(), Vec<Diagnostic>> {
        if let (true, Some(root)) = (self.inline_values, absent_storage_root(kind)) {
            let count = &mut self.exact_absent_roots[root as usize];
            *count = checked_usage(*count, 1, "builder_bytes", active_builder_limit())?;
        }
        Ok(())
    }

    /// Counted beside the storage census, for the same root kinds plus `if`
    /// and variable/block roots, only when the parent proves the position carrier-free (see
    /// `ROOT_AUXILIARY_CARRIERS`). An `if` root keeps its storage, so it is
    /// censused separately. Variables also retain storage, and their census
    /// excludes every closure subtree, where capture lowering adds bindings.
    /// Runtime/synthetic accumulators never count.
    pub(super) fn account_carrier_free_root(
        &mut self,
        kind: &crate::ast::ExprKind,
        carrier_free: bool,
        outside_closure: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        if !(self.inline_values && carrier_free) {
            return Ok(());
        }
        let count = if absent_storage_root(kind).is_some() {
            &mut self.exact_carrier_free_roots
        } else if matches!(kind, crate::ast::ExprKind::If { .. }) {
            &mut self.exact_carrier_free_ifs
        } else if outside_closure && matches!(kind, crate::ast::ExprKind::Var(_)) {
            &mut self.exact_carrier_free_vars
        } else if outside_closure && matches!(kind, crate::ast::ExprKind::Block { .. }) {
            &mut self.exact_carrier_free_blocks
        } else {
            return Ok(());
        };
        *count = checked_usage(*count, 1, "builder_bytes", active_builder_limit())?;
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
            identity_carriers: 0,
            single_identity_carriers: 0,
            exact_fixed_discount: 0,
            exact_expressions: 0,
            exact_absent_roots: [0; 3],
            exact_carrier_free_roots: 0,
            exact_carrier_free_ifs: 0,
            exact_carrier_free_vars: 0,
            exact_carrier_free_blocks: 0,
            exact_carrier_free_calls: 0,
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
            if agent.has_execution_metadata() {
                // The sealed HIR origin is a distinct retained allocation;
                // account it before constructing/cloning the synthetic HIR.
                self.add(std::mem::size_of::<
                    crate::hir::AgentExecutionSourceAssociation,
                >())?;
                self.embedded_string(&program.module)?;
                for operation in &agent.operations {
                    self.add(std::mem::size_of::<crate::hir::ResolvedAgentOperationRole>())?;
                    self.embedded_string(&operation.stable_id)?;
                }
                if let Some(binding) = &agent.model_wait {
                    self.add(std::mem::size_of::<crate::hir::ResolvedAgentModelWaitBinding>())?;
                    self.embedded_string(&binding.helper_id)?;
                }
            }
            if let Some(binding) = &agent.model_wait {
                self.value(binding.as_ref())?;
                self.embedded_string(&binding.helper_id)?;
            }
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

// Both traversal forms retain their full authored Statement carrier before
// visiting source/body children. Its fixed allowance covers the expression
// inline bundle. Traversal heap carriers are separately precharged.
const _: () = assert!(
    std::mem::size_of::<crate::ast::Statement>() >= std::mem::size_of::<crate::ast::Expr>()
);

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

// The same four storage footprints are absent for every authored unary and
// binary root. Both resolvers (`finish_unary`, `FinishBinary` and the recursive
// reference) build the root itself with `OwnershipMode::Value` and a primitive
// type, including lazy `&&`/`||` and checked arithmetic. The inventory creates
// a temporary only for `Own`, and flags/places only under a slot's shape; plan
// lowering returns `owned_source: None`, so no call-argument epoch can key the
// root. Each child keeps its own bundle and census. Checked status sources,
// lazy/failure blocks, transitions, edges, regions, entry states, reachability
// and all HIR carriers stay inside the charged remainder of the bundle.
fn absent_storage_root(kind: &crate::ast::ExprKind) -> Option<AbsentStorageRoot> {
    use crate::ast::ExprKind;
    match kind {
        ExprKind::Unary { .. } => Some(AbsentStorageRoot::Unary),
        ExprKind::Binary { .. } => Some(AbsentStorageRoot::Binary),
        _ if literal_fixed_discount(kind) == LITERAL_ABSENT_STORAGE => {
            Some(AbsentStorageRoot::Literal)
        }
        _ => None,
    }
}

/// Bytes of proved-absent root storage and carrier-free auxiliary carriers in
/// one retained exact HIR copy. Fails closed unless the storage census is
/// covered by the raw accumulator's own bundles, every carrier-free storage
/// root is also a censused storage root, and the disjoint `if`/variable/block censuses
/// fit beside the storage census within those bundles.
pub(super) fn exact_absent_storage(raw: &StructuralCost) -> Result<usize, Vec<Diagnostic>> {
    let overflow = || {
        vec![super::super::limit_error(
            "builder_bytes",
            active_builder_limit(),
        )]
    };
    let roots = raw
        .exact_absent_roots
        .iter()
        .try_fold(0usize, |sum, count| sum.checked_add(*count))
        .ok_or_else(overflow)?;
    if roots > raw.exact_expressions
        || raw.exact_carrier_free_roots > roots
        || roots
            .checked_add(raw.exact_carrier_free_ifs)
            .and_then(|count| count.checked_add(raw.exact_carrier_free_vars))
            .and_then(|count| count.checked_add(raw.exact_carrier_free_blocks))
            .and_then(|count| count.checked_add(raw.exact_carrier_free_calls))
            .is_none_or(|censused| censused > raw.exact_expressions)
    {
        return Err(overflow());
    }
    let carrier_free = raw
        .exact_carrier_free_roots
        .checked_add(raw.exact_carrier_free_ifs)
        .and_then(|count| count.checked_add(raw.exact_carrier_free_vars))
        .and_then(|count| count.checked_add(raw.exact_carrier_free_blocks))
        .and_then(|count| count.checked_add(raw.exact_carrier_free_calls))
        .ok_or_else(overflow)?;
    roots
        .checked_mul(LITERAL_ABSENT_STORAGE)
        .and_then(|storage| {
            carrier_free
                .checked_mul(ROOT_AUXILIARY_CARRIERS)
                .and_then(|carriers| storage.checked_add(carriers))
        })
        .ok_or_else(overflow)
}

// The six auxiliary HIR carriers of the asserted expression bundle. A literal
// root is one `ResolvedExpr` (`resolve_expr.rs` literal arms); a unary root is
// one `ResolvedExpr` boxing its child (`finish::finish_unary`); a binary root
// is one `ResolvedExpr` boxing both children (`Frame::FinishBinary`); an `if`
// root is one `ResolvedExpr` boxing its condition and branches
// (`finish::finish_if`, `control::if_expression`). A variable outside every
// authored closure lowers to `Place` or `FunctionReference` (`named_value`),
// retaining its binding type and ownership without constructing auxiliary
// carriers. A carrier-free authored block likewise moves its already charged
// statements and boxes its already charged tail (`finish::finish_block`),
// without constructing an additional auxiliary carrier at the root. All four
// storage footprints remain charged for variables and blocks. Closure
// subtrees are excluded because capture rewriting can add binding carriers,
// including through delegated statement/field/arm children. An authenticated
// ordinary monomorphic call (`call_carriers`) likewise moves its arguments
// in FinishCall; bind_row_loans mutates their metadata without auxiliary
// carriers. Call roots keep all four storage footprints, including owned
// results. Its proof invoice stays separate from retained HIR copies.
// None constructs a
// binding, statement, field initializer, arm or pattern field. Upcasts, the
// only resolver wrapper of an authored root, re-enter only `let` values and
// method receivers. An `if` may own its result, so it keeps all storage.
//
// Absence at the root is not sufficient: `let`/assignment statements, field
// initializers and match arms embed their child `ResolvedExpr` inline, and
// `inline_expr_parent`/`match_arm` charge those parents without it, so that
// child's bundle is their payer. `for`, closure, `yield` and lexical-exit
// lowering likewise wrap authored children. Only a root the parent places in a
// `Box` (unary/binary operands, `if` condition and branches, block tail) or in
// an ordinary call's argument vector, none of which constructs these types, is
// counted. Every other root, every runtime/default/generic expansion, and the
// root's own `ResolvedExpr`, cleanup, plan, reachability and bookkeeping
// footprints all stay charged.
const ROOT_AUXILIARY_CARRIERS: usize = std::mem::size_of::<crate::hir::ResolvedBinding>()
    + std::mem::size_of::<crate::hir::ResolvedStatement>()
    + std::mem::size_of::<crate::hir::ResolvedFieldInitializer>()
    + std::mem::size_of::<crate::hir::ResolvedMatchArm>()
    + std::mem::size_of::<crate::hir::ResolvedMatchPatternField>()
    + std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>();
// A carrier-free root still keeps its complete `ResolvedExpr` and every
// cleanup/plan/reachability footprint except the four absent storage ones; a
// carrier-free `if`, variable or block keeps those four as well.
const _: () = assert!(
    super::super::HIR_EXPR_FIXED_BUNDLE
        >= LITERAL_ABSENT_STORAGE
            + ROOT_AUXILIARY_CARRIERS
            + std::mem::size_of::<crate::hir::ResolvedExpr>()
);

// Everything except the four absent storage footprints stays charged per root.
#[cfg(test)]
pub(super) const EXACT_ABSENT_ROOT_STORAGE: usize = LITERAL_ABSENT_STORAGE;
// Carrier-free roots additionally omit exactly these six HIR carriers.
#[cfg(test)]
pub(super) const EXACT_ROOT_AUXILIARY_CARRIERS: usize = ROOT_AUXILIARY_CARRIERS;

#[cfg(test)]
#[path = "carrier_absence_tests.rs"]
mod carrier_absence_tests;

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

#[cfg(test)]
#[test]
fn identity_prebound_carriers_include_traversal_statements_and_every_child() {
    for traversal in ["for item in values", "for own item in values"] {
        let source = std::format!(
            "module carrier.fixture; @id(\"carrier.main\") fn main() -> i64 {{ {traversal} {{ 0 }} 0 }}"
        );
        let program = crate::parse(&source, std::path::Path::new("carrier.spx")).unwrap();
        let mut cost = StructuralCost::new();
        super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut cost).unwrap();
        // Enclosing block, source variable, body block, body tail, outer tail;
        // the sixteen synthesized traversal identities are additional.
        assert_eq!(cost.identity_carriers, 5 * 3 + 16);
    }
}

#[cfg(test)]
#[test]
fn identity_prebound_carriers_propagate_defaults_and_generic_materializations() {
    let source = "module carrier.defaults; @id(\"carrier.generic\") fn marker<T>(value: T) -> i64 { 0 } @id(\"carrier.main\") fn main() -> i64 { marker<i64>(1) }";
    let program = crate::parse(source, std::path::Path::new("carrier.spx")).unwrap();
    let mut template = StructuralCost::new();
    super::declaration_cost::ast_function_cost(&program.functions[0], &mut template).unwrap();
    let generic = super::generic_instance_source_cost(&program).unwrap();
    assert_eq!(generic.identity_carriers, template.identity_carriers);
    assert!(generic.identity_carriers > 0);
    for (ty, owned, expected) in [
        (crate::ast::Type::I64, false, 3),
        (crate::ast::Type::Bytes, true, 6),
    ] {
        let default = super::defaults::default_expr_expanded_cost(
            &ty,
            "carrier.defaults",
            &program,
            &std::collections::BTreeMap::new(),
            &[],
            &mut [
                std::collections::BTreeMap::new(),
                std::collections::BTreeMap::new(),
            ],
            &mut std::collections::BTreeSet::new(),
            owned,
        )
        .unwrap();
        assert_eq!(default.identity_carriers, expected);
    }
}

#[cfg(test)]
#[test]
fn identity_prebound_carriers_include_transient_super_receivers_in_wide_calls() {
    use crate::ast::{Expr, ExprKind, Span};
    let expression = Expr {
        kind: ExprKind::Call {
            name: "wide".to_owned(),
            type_arguments: Vec::new(),
            args: (0..6)
                .map(|_| Expr {
                    kind: ExprKind::SuperMethod {
                        method: "inherited".to_owned(),
                        method_span: Span::default(),
                        args: Vec::new(),
                    },
                    span: Span::default(),
                })
                .collect(),
        },
        span: Span::default(),
    };
    let mut cost = StructuralCost::new();
    super::declaration_cost::ast_expr_cost(&expression, &mut cost).unwrap();
    // Six independent four-constructor expansions defeat any apparent spare
    // allowance supplied by the enclosing ordinary call's single constructor.
    assert_eq!(cost.identity_carriers, 3 + 6 * 4);
    assert!(cost.identity_carriers > 6 * 4);
}

#[cfg(test)]
mod exact_bundle_tests {
    use super::{fixed_hir_discount, identity_carriers, StructuralCost};

    #[test]
    fn final_profile_retains_complete_fixed_bundle_and_single_root_carrier() {
        let program = crate::parse(
            "module exact; @id(\"exact.main\") fn main() -> i64 { 1 }",
            std::path::Path::new("exact.spx"),
        )
        .unwrap();
        let expression = match &program.functions[0].body.kind {
            crate::ast::ExprKind::Block { tail, .. } => tail.as_ref(),
            _ => panic!("literal fixture has a block tail"),
        };
        let mut raw = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_expr_cost(expression, &mut raw).unwrap();
        let runtime = StructuralCost::new();
        assert_eq!(identity_carriers(4, &raw, &runtime).unwrap(), 3);
        assert_eq!(identity_carriers(5, &raw, &runtime).unwrap(), 1);
        let discount = crate::workspace_graph::HIR_FIXED_EXPANSION_FACTOR
            * std::mem::size_of::<crate::ast::Expr>()
            - crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE;
        assert_eq!(raw.exact_fixed_discount, discount);
        assert_eq!(
            fixed_hir_discount(4, &raw, &runtime).unwrap(),
            raw.literal_fixed_discount
        );
        assert_eq!(
            fixed_hir_discount(5, &raw, &runtime).unwrap(),
            raw.literal_fixed_discount + discount
        );
    }
}

#[cfg(test)]
#[path = "uncached_absence_tests.rs"]
mod uncached_absence_tests;

#[cfg(test)]
#[path = "cost/generic_rewrite_test_bridge.rs"]
pub(in crate::workspace_graph) mod generic_rewrite_test_bridge;

#[path = "cost/constructor.rs"]
pub(in crate::workspace_graph) mod constructor;

#[cfg(test)]
#[path = "call_carriers_historical_tests.rs"]
mod call_carriers_historical_tests;
