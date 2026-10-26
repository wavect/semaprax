//! Complete prefix-certified search with narrow H006 transport.
use super::*;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
// Two narrow return/receive roles, plus the surviving outer public Diagnostic
// Result. The complete old scratch and every original work event stay paid.
pub(super) const LOOKUP_BYTES: usize = std::mem::size_of::<LookupControl>()
    + 2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, ByteFailure>>()
    + std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>();
const _: () = assert!(
    super::LOOKUP_BYTES - LOOKUP_BYTES
        == std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>()
            - 2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, ByteFailure>>()
);
pub(in crate::cleanup_plan::replay::expression_facts) fn find_receiving<'a, 'hir>(
    arena: &'a Arena<'hir>,
    function: &ResolvedFunction,
    query: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a Option<CallFact<'hir>>>, ByteFailure> {
    budget.reserve_compact_bytes(function, 0, "expression-fact arena lookup authentication")?;
    budget.charge_compact(function, 0, "expression-fact arena lookup authentication")?;
    budget.reserve_compact_bytes(
        function,
        LOOKUP_BYTES,
        "expression-fact full lookup controls",
    )?;
    budget.charge_compact(function, 4, "expression-fact lookup control initialization")?;
    let mut control = LookupControl {
        lower: 0,
        upper: 0,
        at: arena.root,
        last: arena.root,
    };
    // Complete new comparison state and initialized/receiving overlap. The
    // original lookup controls above retain their full independent charge.
    budget.reserve_compact_bytes(
        function,
        2 * std::mem::size_of::<ReceivingComparison<'_>>(),
        "expression-fact receiving comparison state",
    )?;
    budget.charge_compact(
        function,
        8,
        "expression-fact receiving state initialization",
    )?;
    let mut compared = ReceivingComparison {
        bytes: Comparison {
            left: query.as_str().as_bytes(),
            right: &[],
            end: 0,
            common: 0,
        },
        order: Ordering::Equal,
        units: 0,
        a: 0,
        b: 0,
    };
    // Update existing fields; no comparison Result or tuple is constructed.
    // Every original bookkeeping and byte-pair work event remains.
    macro_rules! compare_into {
        ($candidate:expr, $prefix:expr) => {{
            budget.charge_compact(
                function,
                BOUND_UNITS,
                "expression-fact prefix control bookkeeping",
            )?;
            compared.bytes.right = ($candidate).as_str().as_bytes();
            compared.bytes.end = compared.bytes.left.len().min(compared.bytes.right.len());
            compared.bytes.common = $prefix;
            compared.units = compared
                .bytes
                .end
                .checked_sub(compared.bytes.common)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| {
                    budget.compact_refusal(
                        function,
                        "expression-fact prefix comparison work overflow",
                        "",
                    )
                })?;
            budget.charge_compact(
                function,
                compared.units,
                "expression-fact certified suffix lookup",
            )?;
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
    // Two paid full comparisons seed useful bounds even for endpoint-heavy
    // queries. The first key is already the producer's minimum; finding the
    // maximum follows only actual right children and pays before each read.
    if arena.nodes.len() >= 16 {
        budget.charge_compact(function, 3, "expression-fact minimum key access")?;
        let first = &arena.nodes[arena.first];
        compare_into!(&first.key, 0);
        match compared.order {
            Ordering::Less => return Ok(None),
            Ordering::Equal => return Ok(Some(&first.fact)),
            Ordering::Greater => control.lower = compared.bytes.common,
        }
        loop {
            budget.charge_compact(function, 3, "expression-fact maximum key access")?;
            let node = &arena.nodes[control.last];
            if node.right == NIL {
                compare_into!(&node.key, 0);
                match compared.order {
                    Ordering::Greater => return Ok(None),
                    Ordering::Equal => return Ok(Some(&node.fact)),
                    Ordering::Less => control.upper = compared.bytes.common,
                }
                break;
            }
            control.last = node.right;
        }
    }
    while control.at != NIL {
        // Every node reached is between the last lower and upper
        // bound keys. If the query shares p bytes with both, every intervening
        // byte-lexicographic key shares those p bytes. Endpoints were compared
        // explicitly above. With a missing bound its certificate is zero.
        let prefix = control.lower.min(control.upper);
        budget.charge_compact(function, 3, "expression-fact certified node access")?;
        let node = &arena.nodes[control.at];
        compare_into!(&node.key, prefix);
        match compared.order {
            Ordering::Equal => return Ok(Some(&node.fact)),
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
    Ok(None)
}
