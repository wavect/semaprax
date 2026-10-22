//! Direct full-error-slot successor of the complete immediate retained query.
use super::*;
use crate::cleanup_plan::replay::ordinary_receiving::{
    charge_into, prepare_into, refuse_into, reserve_into, Failure,
};
impl<'arena, 'hir, 'budget> Session<'arena, 'hir, 'budget> {
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_query_bytes() -> usize {
        2 * size_of::<(
            &mut Self,
            &ExpressionId,
            &mut Option<&Option<CallFact<'_>>>,
            &mut Failure<'_>,
            bool,
        )>()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_edge_query_bytes() -> usize
    {
        2 * size_of::<(
            &mut Self,
            &ExpressionId,
            &mut Option<&Option<CallFact<'_>>>,
            &mut Failure<'_>,
            bool,
        )>()
    }
    // The original self loan (and edge query-expression loan) moves to the
    // caller's whole argument packet. Each old role remains, once; no equal-
    // width unrelated slot is substituted or credited against another role.
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_local_bytes() -> usize {
        size_of::<(
            &Arena<'_>,
            &ResolvedFunction,
            &mut ReplayBudget,
            &mut Control,
            &mut Compared<'_>,
        )>() + 2 * size_of::<&mut ReplayBudget>()
            + size_of::<&crate::cleanup_plan::StatusSource>()
            + 2 * size_of::<bool>()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_edge_local_bytes() -> usize
    {
        size_of::<(
            &Arena<'_>,
            &ResolvedFunction,
            &mut ReplayBudget,
            &mut Control,
            &mut Compared<'_>,
        )>() + 2 * size_of::<&mut ReplayBudget>()
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::expression_facts) const fn added_status_query_bytes(
    ) -> usize {
        Self::into_query_bytes() + Self::into_local_bytes() - Self::query_bytes()
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::expression_facts) const fn added_edge_query_bytes() -> usize
    {
        Self::into_edge_query_bytes() + Self::into_edge_local_bytes() - Self::EDGE_QUERY_BYTES
    }
    pub(in crate::cleanup_plan::replay::expression_facts) fn fact_into(
        &mut self,
        query: &'arena ExpressionId,
        output: &mut Option<&'arena Option<CallFact<'hir>>>,
        slot: &mut Failure<'_>,
    ) -> bool {
        #[cfg(test)]
        crate::cleanup_plan::replay::ordinary_receiving::observe_site(2);
        if slot.error.is_some() {
            return false;
        }
        // The sole production caller preserves successful HeldPrefix::insert
        // immediately before this query. That existing guard checks the exact
        // function/budget/current ledger on EVERY row; the closed continuation
        // cannot switch ACTIVE or invoke a callback between guard and query.
        if !reserve_into!(
            self.budget,
            self.function,
            slot,
            0,
            "expression-fact arena lookup authentication"
        ) {
            return false;
        }
        if !charge_into!(
            self.budget,
            self.function,
            slot,
            0,
            "expression-fact arena lookup authentication"
        ) {
            return false;
        }
        // The two wide Result objects cease to exist. All inherited query
        // borrowers and the new output/error loans remain paid on each query.
        if !reserve_into!(
            self.budget,
            self.function,
            slot,
            Self::into_local_bytes(),
            "retained expression-fact query result controls"
        ) {
            return false;
        }
        let function = self.function;
        let budget = &mut *self.budget;
        *output = None;
        // Original ordered work and complete query reset. No verdict is reused. Slice loans remain bound to the held immutable
        // function/arena; every field is reset before the next query.
        if !charge_into!(
            budget,
            function,
            slot,
            4,
            "expression-fact lookup control initialization"
        ) {
            return false;
        }
        self.control.lower = 0;
        self.control.upper = 0;
        self.control.at = self.arena.root;
        self.control.last = self.arena.root;
        if !charge_into!(
            budget,
            function,
            slot,
            8,
            "expression-fact receiving state initialization"
        ) {
            return false;
        }
        self.compared.bytes.left = query.as_str().as_bytes();
        self.compared.bytes.right = &[];
        self.compared.bytes.end = 0;
        self.compared.bytes.common = 0;
        self.compared.order = Ordering::Equal;
        self.compared.units = 0;
        self.compared.a = 0;
        self.compared.b = 0;
        let arena = self.arena;
        let control = &mut self.control;
        let compared = &mut self.compared;
        macro_rules! compare_into {
            ($candidate:expr, $prefix:expr) => {{
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    10,
                    "expression-fact prefix control bookkeeping"
                ) {
                    return false;
                }
                compared.bytes.right = ($candidate).as_str().as_bytes();
                compared.bytes.end = compared.bytes.left.len().min(compared.bytes.right.len());
                compared.bytes.common = $prefix;
                compared.units = match compared
                    .bytes
                    .end
                    .checked_sub(compared.bytes.common)
                    .and_then(|n| n.checked_add(1))
                {
                    Some(units) => units,
                    None => {
                        return refuse_into!(
                            budget,
                            function,
                            slot,
                            "expression-fact prefix comparison work overflow",
                            ""
                        );
                    }
                };
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    compared.units,
                    "expression-fact certified suffix lookup"
                ) {
                    return false;
                }
                loop {
                    if compared.bytes.common == compared.bytes.end {
                        compared.order = compared.bytes.left.len().cmp(&compared.bytes.right.len());
                        break;
                    }
                    compared.a = compared.bytes.left[compared.bytes.common];
                    compared.b = compared.bytes.right[compared.bytes.common];
                    if compared.a != compared.b {
                        compared.order = compared.a.cmp(&compared.b);
                        break;
                    }
                    compared.bytes.common += 1;
                }
            }};
        }
        if arena.nodes.len() >= 16 {
            if !charge_into!(
                budget,
                function,
                slot,
                3,
                "expression-fact minimum key access"
            ) {
                return false;
            }
            let first = &arena.nodes[arena.first];
            compare_into!(&first.key, 0);
            match compared.order {
                Ordering::Less => return true,
                Ordering::Equal => {
                    *output = Some(&first.fact);
                    return true;
                }
                Ordering::Greater => control.lower = compared.bytes.common,
            }
            loop {
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    3,
                    "expression-fact maximum key access"
                ) {
                    return false;
                }
                let node = &arena.nodes[control.last];
                if node.right == NIL {
                    compare_into!(&node.key, 0);
                    match compared.order {
                        Ordering::Greater => return true,
                        Ordering::Equal => {
                            *output = Some(&node.fact);
                            return true;
                        }
                        Ordering::Less => control.upper = compared.bytes.common,
                    }
                    break;
                }
                control.last = node.right;
            }
        }
        while control.at != NIL {
            let prefix = control.lower.min(control.upper);
            if !charge_into!(
                budget,
                function,
                slot,
                3,
                "expression-fact certified node access"
            ) {
                return false;
            }
            let node = &arena.nodes[control.at];
            compare_into!(&node.key, prefix);
            match compared.order {
                Ordering::Equal => {
                    *output = Some(&node.fact);
                    return true;
                }
                Ordering::Less => {
                    control.upper = compared.bytes.common;
                    control.at = node.left;
                }
                Ordering::Greater => {
                    control.lower = compared.bytes.common;
                    control.at = node.right;
                }
            }
        }
        true
    }
    pub(in crate::cleanup_plan::replay::expression_facts) fn fact_edge_into_prepaid(
        &mut self,
        query: &'arena ExpressionId,
        output: &mut Option<&'arena Option<CallFact<'hir>>>,
        slot: &mut Failure<'_>,
    ) -> bool {
        #[cfg(test)]
        crate::cleanup_plan::replay::ordinary_receiving::observe_site(3);
        let function = self.function;
        let budget = &mut *self.budget;
        if slot.error.is_some() {
            return false;
        }
        *output = None;
        // Original ordered work and complete query reset. No verdict is reused. Slice loans remain bound to the held immutable
        // function/arena; every field is reset before the next query.
        if !charge_into!(
            budget,
            function,
            slot,
            4,
            "expression-fact lookup control initialization"
        ) {
            return false;
        }
        self.control.lower = 0;
        self.control.upper = 0;
        self.control.at = self.arena.root;
        self.control.last = self.arena.root;
        if !charge_into!(
            budget,
            function,
            slot,
            8,
            "expression-fact receiving state initialization"
        ) {
            return false;
        }
        self.compared.bytes.left = query.as_str().as_bytes();
        self.compared.bytes.right = &[];
        self.compared.bytes.end = 0;
        self.compared.bytes.common = 0;
        self.compared.order = Ordering::Equal;
        self.compared.units = 0;
        self.compared.a = 0;
        self.compared.b = 0;
        let arena = self.arena;
        let control = &mut self.control;
        let compared = &mut self.compared;
        macro_rules! compare_into {
            ($candidate:expr, $prefix:expr) => {{
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    10,
                    "expression-fact prefix control bookkeeping"
                ) {
                    return false;
                }
                compared.bytes.right = ($candidate).as_str().as_bytes();
                compared.bytes.end = compared.bytes.left.len().min(compared.bytes.right.len());
                compared.bytes.common = $prefix;
                compared.units = match compared
                    .bytes
                    .end
                    .checked_sub(compared.bytes.common)
                    .and_then(|n| n.checked_add(1))
                {
                    Some(units) => units,
                    None => {
                        return refuse_into!(
                            budget,
                            function,
                            slot,
                            "expression-fact prefix comparison work overflow",
                            ""
                        );
                    }
                };
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    compared.units,
                    "expression-fact certified suffix lookup"
                ) {
                    return false;
                }
                loop {
                    if compared.bytes.common == compared.bytes.end {
                        compared.order = compared.bytes.left.len().cmp(&compared.bytes.right.len());
                        break;
                    }
                    compared.a = compared.bytes.left[compared.bytes.common];
                    compared.b = compared.bytes.right[compared.bytes.common];
                    if compared.a != compared.b {
                        compared.order = compared.a.cmp(&compared.b);
                        break;
                    }
                    compared.bytes.common += 1;
                }
            }};
        }
        if arena.nodes.len() >= 16 {
            if !charge_into!(
                budget,
                function,
                slot,
                3,
                "expression-fact minimum key access"
            ) {
                return false;
            }
            let first = &arena.nodes[arena.first];
            compare_into!(&first.key, 0);
            match compared.order {
                Ordering::Less => return true,
                Ordering::Equal => {
                    *output = Some(&first.fact);
                    return true;
                }
                Ordering::Greater => control.lower = compared.bytes.common,
            }
            loop {
                if !charge_into!(
                    budget,
                    function,
                    slot,
                    3,
                    "expression-fact maximum key access"
                ) {
                    return false;
                }
                let node = &arena.nodes[control.last];
                if node.right == NIL {
                    compare_into!(&node.key, 0);
                    match compared.order {
                        Ordering::Greater => return true,
                        Ordering::Equal => {
                            *output = Some(&node.fact);
                            return true;
                        }
                        Ordering::Less => control.upper = compared.bytes.common,
                    }
                    break;
                }
                control.last = node.right;
            }
        }
        while control.at != NIL {
            let prefix = control.lower.min(control.upper);
            if !charge_into!(
                budget,
                function,
                slot,
                3,
                "expression-fact certified node access"
            ) {
                return false;
            }
            let node = &arena.nodes[control.at];
            compare_into!(&node.key, prefix);
            match compared.order {
                Ordering::Equal => {
                    *output = Some(&node.fact);
                    return true;
                }
                Ordering::Less => {
                    control.upper = compared.bytes.common;
                    control.at = node.left;
                }
                Ordering::Greater => {
                    control.lower = compared.bytes.common;
                    control.at = node.right;
                }
            }
        }
        true
    }
}
