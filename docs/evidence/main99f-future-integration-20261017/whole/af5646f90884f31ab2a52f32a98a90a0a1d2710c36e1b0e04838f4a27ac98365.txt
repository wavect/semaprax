//! Prefix-certified lookup over the existing immutable ordered AVL tree.
//! No index, allocation, hash, key normalization or insertion-policy change.
use super::*;

// Three node/key/child visits retain the original lookup charge. Ten extra
// units cover four bracket reads/updates, four comparator fields and the
// two scalar return fields. Byte-loop work retains the old byte-pair rate.
const BOUND_UNITS: usize = 10;
struct LookupControl {
    lower: usize,
    upper: usize,
    at: usize,
    last: usize,
}
struct Comparison<'a> {
    left: &'a [u8],
    right: &'a [u8],
    end: usize,
    common: usize,
}
const COMPARISON_BYTES: usize = std::mem::size_of::<Comparison<'static>>()
    + std::mem::size_of::<Result<(Ordering, usize), Diagnostic>>()
    + std::mem::size_of::<(Ordering, usize)>();
// Complete new initialized controls and returning/receiving result overlap.
// No previous lookup local/header is credited against these full layouts.
const LOOKUP_BYTES: usize = std::mem::size_of::<LookupControl>()
    + 2 * std::mem::size_of::<Result<Option<&'static Option<CallFact<'static>>>, Diagnostic>>();

fn compare(
    function: &ResolvedFunction,
    query: &ExpressionId,
    candidate: &ExpressionId,
    prefix: usize,
    budget: &mut ReplayBudget,
) -> Result<(Ordering, usize), Diagnostic> {
    budget.reserve_bytes(
        function,
        COMPARISON_BYTES,
        "expression-fact comparison controls",
    )?;
    budget.charge(
        function,
        BOUND_UNITS,
        "expression-fact prefix control bookkeeping",
    )?;
    let mut comparison = Comparison {
        left: query.as_str().as_bytes(),
        right: candidate.as_str().as_bytes(),
        end: query.as_str().len().min(candidate.as_str().len()),
        common: prefix,
    };
    let units = comparison
        .end
        .checked_sub(prefix)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| {
            budget.budget_refusal(
                function,
                "expression-fact prefix comparison work overflow",
                "",
            )
        })?;
    // Same one-unit-per-byte-pair comparison rate as the complete old lookup,
    // including its end comparison. Only certified bytes are no longer read.
    budget.charge(function, units, "expression-fact certified suffix lookup")?;
    while comparison.common < comparison.end {
        let a = comparison.left[comparison.common];
        let b = comparison.right[comparison.common];
        if a != b {
            return Ok((a.cmp(&b), comparison.common));
        }
        comparison.common += 1;
    }
    Ok((
        comparison.left.len().cmp(&comparison.right.len()),
        comparison.common,
    ))
}

pub(super) fn find<'a, 'hir>(
    arena: &'a Arena<'hir>,
    function: &ResolvedFunction,
    query: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a Option<CallFact<'hir>>>, Diagnostic> {
    budget.reserve_bytes(function, 0, "expression-fact arena lookup authentication")?;
    budget.charge(function, 0, "expression-fact arena lookup authentication")?;
    budget.reserve_bytes(
        function,
        LOOKUP_BYTES,
        "expression-fact full lookup controls",
    )?;
    budget.charge(function, 4, "expression-fact lookup control initialization")?;
    let mut control = LookupControl {
        lower: 0,
        upper: 0,
        at: arena.root,
        last: arena.root,
    };
    // Two paid full comparisons seed useful bounds even for endpoint-heavy
    // queries. The first key is already the producer's minimum; finding the
    // maximum follows only actual right children and pays before each read.
    if arena.nodes.len() >= 16 {
        budget.charge(function, 3, "expression-fact minimum key access")?;
        let first = &arena.nodes[arena.first];
        match compare(function, query, &first.key, 0, budget)? {
            (Ordering::Less, _) => return Ok(None),
            (Ordering::Equal, _) => return Ok(Some(&first.fact)),
            (Ordering::Greater, common) => control.lower = common,
        }
        loop {
            budget.charge(function, 3, "expression-fact maximum key access")?;
            let node = &arena.nodes[control.last];
            if node.right == NIL {
                match compare(function, query, &node.key, 0, budget)? {
                    (Ordering::Greater, _) => return Ok(None),
                    (Ordering::Equal, _) => return Ok(Some(&node.fact)),
                    (Ordering::Less, common) => control.upper = common,
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
        budget.charge(function, 3, "expression-fact certified node access")?;
        let node = &arena.nodes[control.at];
        match compare(function, query, &node.key, prefix, budget)? {
            (Ordering::Equal, _) => return Ok(Some(&node.fact)),
            (Ordering::Less, common) => {
                control.upper = common;
                control.at = node.left;
            }
            (Ordering::Greater, common) => {
                control.lower = common;
                control.at = node.right;
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "prefix/source_main8a.rs"]
mod source_main8a;

#[cfg(test)]
#[path = "prefix/source_maina3.rs"]
mod source_maina3;
// One paid state for the complete immutable search, never a memo or identity.
struct ReceivingComparison<'a> {
    bytes: Comparison<'a>,
    order: Ordering,
    units: usize,
    a: u8,
    b: u8,
}

pub(super) fn find_receiving<'a, 'hir>(
    arena: &'a Arena<'hir>,
    function: &ResolvedFunction,
    query: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a Option<CallFact<'hir>>>, Diagnostic> {
    budget.reserve_bytes(function, 0, "expression-fact arena lookup authentication")?;
    budget.charge(function, 0, "expression-fact arena lookup authentication")?;
    budget.reserve_bytes(
        function,
        LOOKUP_BYTES,
        "expression-fact full lookup controls",
    )?;
    budget.charge(function, 4, "expression-fact lookup control initialization")?;
    let mut control = LookupControl {
        lower: 0,
        upper: 0,
        at: arena.root,
        last: arena.root,
    };
    // Complete new comparison state and initialized/receiving overlap. The
    // original lookup controls above retain their full independent charge.
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<ReceivingComparison<'_>>(),
        "expression-fact receiving comparison state",
    )?;
    budget.charge(
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
            budget.charge(
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
                    budget.budget_refusal(
                        function,
                        "expression-fact prefix comparison work overflow",
                        "",
                    )
                })?;
            budget.charge(
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
        budget.charge(function, 3, "expression-fact minimum key access")?;
        let first = &arena.nodes[arena.first];
        compare_into!(&first.key, 0);
        match compared.order {
            Ordering::Less => return Ok(None),
            Ordering::Equal => return Ok(Some(&first.fact)),
            Ordering::Greater => control.lower = compared.bytes.common,
        }
        loop {
            budget.charge(function, 3, "expression-fact maximum key access")?;
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
        budget.charge(function, 3, "expression-fact certified node access")?;
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
