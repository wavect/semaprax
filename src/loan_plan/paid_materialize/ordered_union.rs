//! Two bounded k-way passes over internally ordered edge rows. Scratch is one
//! paid slice heap; output has exact union capacity. No per-ID tree or copy of
//! the input rows is retained. The materializer has no logical-work grant;
//! its enclosing producer's existing work units are unchanged.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;

pub(crate) fn merge<'a>(
    rows: impl Iterator<Item = &'a [LoanId]> + Clone,
    payer: &CfgReservation,
) -> Result<Vec<LoanId>, Diagnostic> {
    payer.reserve(0).map_err(|_| storage::marker())?;
    // Producer-owned rows are ordered. Preserve the complete previous union
    // behavior for an internally malformed row too, without sorting its input.
    if rows
        .clone()
        .any(|row| row.windows(2).any(|pair| pair[0] > pair[1]))
    {
        let mut values = cfg_directory::Set::new(payer)?;
        for row in rows {
            for id in row {
                cfg_directory::set_insert(&mut values, *id, Some(payer))?;
            }
        }
        return values.into_vec(payer);
    }
    // Pay both actual Vec carriers, even when there are no nonempty rows.
    let count = rows.clone().filter(|row| !row.is_empty()).count();
    if count == 0 {
        return cfg_directory::vector(0, payer);
    }
    let mut heap = cfg_directory::vector(count, payer)?;
    fill(&mut heap, rows.clone());
    let mut unique = 0;
    let mut previous = None;
    while let Some(id) = next(&mut heap) {
        payer.reserve(0).map_err(|_| storage::marker())?;
        if previous != Some(id) {
            unique += 1; // At most the entire u16 LoanId domain.
            previous = Some(id);
        }
    }
    let mut output = cfg_directory::vector(unique, payer)?;
    fill(&mut heap, rows);
    while let Some(id) = next(&mut heap) {
        payer.reserve(0).map_err(|_| storage::marker())?;
        if output.last() != Some(&id) {
            // Capacity is the first pass's exact unique count. No growth.
            output.push(id);
        }
    }
    Ok(output)
}
fn fill<'a>(heap: &mut Vec<&'a [LoanId]>, rows: impl Iterator<Item = &'a [LoanId]>) {
    debug_assert!(heap.is_empty());
    for row in rows.filter(|row| !row.is_empty()) {
        debug_assert!(row.windows(2).all(|pair| pair[0] <= pair[1]));
        heap.push(row);
        let mut child = heap.len() - 1;
        while child != 0 {
            let parent = (child - 1) / 2;
            if heap[parent][0] <= heap[child][0] {
                break;
            }
            heap.swap(parent, child);
            child = parent;
        }
    }
}
fn next(heap: &mut Vec<&[LoanId]>) -> Option<LoanId> {
    let first = *heap.first()?;
    let id = first[0];
    if first.len() == 1 {
        heap.swap_remove(0);
    } else {
        heap[0] = &first[1..];
    }
    let mut parent = 0usize;
    while let Some(left) = parent.checked_mul(2usize).and_then(|n| n.checked_add(1)) {
        if left >= heap.len() {
            break;
        }
        let right = left + 1;
        let child = if right < heap.len() && heap[right][0] < heap[left][0] {
            right
        } else {
            left
        };
        if heap[parent][0] <= heap[child][0] {
            break;
        }
        heap.swap(parent, child);
        parent = child;
    }
    Some(id)
}
