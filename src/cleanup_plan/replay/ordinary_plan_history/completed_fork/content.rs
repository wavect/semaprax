//! Full-content lexicographic cursors over independently constructed arenas.
use super::*;

pub(super) struct Cursor<'a> {
    chunks: &'a [Chunk],
    tail: &'a [Rc<SkeletonObservation>],
    links: Vec<usize>,
    segment: usize,
    item: usize,
}
const CURSOR: usize = 4 * size_of::<Cursor<'static>>()
    + 4 * size_of::<&Histories>()
    + 4 * size_of::<&Path>()
    + 6 * size_of::<usize>()
    + 4 * size_of::<Option<usize>>()
    + 4 * size_of::<Option<&SkeletonObservation>>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&Chunk>()
    + 4 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&mut Vec<usize>>()
    + 4 * size_of::<&mut Cursor<'static>>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<Ordering>()
    + size_of::<Result<Ordering, Diagnostic>>()
    + 4 * PAYMENT;
const STEP: usize = 2 * size_of::<&mut Cursor<'static>>()
    + 2 * size_of::<Option<&SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 2 * size_of::<&Chunk>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + PAYMENT;
const EQUALITY: usize = 4 * size_of::<&Histories>()
    + 2 * size_of::<Vec<usize>>()
    + 2 * size_of::<std::ops::Range<usize>>()
    + size_of::<Result<bool, Diagnostic>>()
    + 6 * size_of::<bool>()
    + 8 * size_of::<usize>()
    + 2 * size_of::<Ordering>()
    + 4 * size_of::<&mut [usize]>()
    + 4 * size_of::<&[usize]>()
    + 4 * size_of::<&usize>()
    + 2 * size_of::<Result<Vec<usize>, Diagnostic>>()
    + 4 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<std::iter::Rev<std::ops::Range<usize>>>()
    + 4 * size_of::<Option<usize>>()
    + 4 * PAYMENT;

pub(super) fn prepare<'a>(
    set: &'a Histories,
    path: &'a Path,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Cursor<'a>, Diagnostic> {
    let mut cursor = Cursor {
        chunks: &set.chunks,
        tail: &path.tail.observations.items,
        links: Vec::new(),
        segment: 0,
        item: 0,
    };
    let mut previous = path.previous;
    while let Some(index) = previous {
        work.budget
            .charge(work.function, 1, "completed history cursor links")?;
        let chunk = match set.chunks.get(index) {
            Some(chunk) => chunk,
            None => {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "completed history chunk is absent",
                    "",
                ));
            }
        };
        if matches!(chunk.previous, Some(parent) if parent >= index) {
            return Err(work.budget.budget_refusal(
                work.function,
                "completed history chunk is cyclic",
                "",
            ));
        }
        let old = match chunk.previous {
            Some(parent) => set.chunks[parent].length,
            None => 0,
        };
        if old.checked_add(chunk.items.len()) != Some(chunk.length) {
            return Err(work.budget.budget_refusal(
                work.function,
                "completed history length is invalid",
                "",
            ));
        }
        work.budget.reserve_vec(
            work.function,
            &mut cursor.links,
            1,
            "completed history cursor backing",
        )?;
        cursor.links.push(index);
        previous = chunk.previous;
    }
    // Iterative storage: arena Drop only releases Vec/Rc observation handles;
    // it follows no owned parent edges and allocates no destruction frontier.
    work.budget.charge(
        work.function,
        cursor.links.len(),
        "completed history cursor link reversal",
    )?;
    cursor.links.reverse();
    Ok(cursor)
}
pub(super) fn next<'a>(
    cursor: &mut Cursor<'a>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Option<&'a SkeletonObservation>, Diagnostic> {
    loop {
        work.budget
            .reserve_bytes(work.function, STEP, "completed history cursor step")?;
        work.budget
            .charge(work.function, 1, "completed history content traversal")?;
        let items = match cursor.links.get(cursor.segment) {
            Some(index) => cursor.chunks[*index].items.as_slice(),
            None => cursor.tail,
        };
        if let Some(value) = items.get(cursor.item) {
            cursor.item += 1;
            return Ok(Some(value.as_ref()));
        }
        if cursor.segment >= cursor.links.len() {
            return Ok(None);
        }
        cursor.segment += 1;
        cursor.item = 0;
    }
}
fn order(
    left: &Histories,
    a: usize,
    right: &Histories,
    b: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Ordering, Diagnostic> {
    work.budget
        .reserve_bytes(work.function, CURSOR, "completed history pair cursors")?;
    let a = &left.paths[a];
    let b = &right.paths[b];
    let mut x = prepare(left, a, work)?;
    let mut y = prepare(right, b, work)?;
    loop {
        // Content, never arena indices, pointer equality or a digest, decides.
        match (next(&mut x, work)?, next(&mut y, work)?) {
            (Some(x), Some(y)) => match x.cmp(y) {
                Ordering::Equal => {}
                found => return Ok(found),
            },
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
            (None, None) => {
                return Ok((
                    a.tail.failed,
                    a.tail.residual,
                    &a.tail.owned_source,
                    &a.tail.continuation,
                )
                    .cmp(&(
                        b.tail.failed,
                        b.tail.residual,
                        &b.tail.owned_source,
                        &b.tail.continuation,
                    )));
            }
        }
    }
}
// Paid heapsort changes only ephemeral path indices, never cleanup-plan order.
fn sorted(set: &Histories, work: &mut SkeletonWork<'_, '_>) -> Result<Vec<usize>, Diagnostic> {
    let mut indices = Vec::new();
    for index in 0..set.paths.len() {
        work.budget
            .charge(work.function, 1, "completed history sort index")?;
        work.budget
            .reserve_bytes(work.function, EQUALITY, "completed history sort index")?;
        work.budget.reserve_vec(
            work.function,
            &mut indices,
            1,
            "completed history sort backing",
        )?;
        indices.push(index);
    }
    let len = indices.len();
    for start in (0..len / 2).rev() {
        sift(set, &mut indices, start, len, work)?;
    }
    for end in (1..len).rev() {
        indices.swap(0, end);
        sift(set, &mut indices, 0, end, work)?;
    }
    Ok(indices)
}
fn sift(
    set: &Histories,
    indices: &mut [usize],
    mut root: usize,
    end: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    loop {
        work.budget
            .charge(work.function, 1, "completed history heap step")?;
        work.budget
            .reserve_bytes(work.function, EQUALITY, "completed history sift controls")?;
        // root < end and the existing path ceiling bound this arithmetic; use
        // checked operations as well so a forged host test cannot wrap it.
        let doubled = match root.checked_mul(2) {
            Some(value) => value,
            None => {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "completed history sort overflowed",
                    "",
                ));
            }
        };
        let mut child = match doubled.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "completed history sort overflowed",
                    "",
                ));
            }
        };
        if child >= end {
            return Ok(());
        }
        if child + 1 < end
            && order(set, indices[child], set, indices[child + 1], work)? == Ordering::Less
        {
            child += 1;
        }
        if order(set, indices[root], set, indices[child], work)? != Ordering::Less {
            return Ok(());
        }
        indices.swap(root, child);
        root = child;
    }
}
pub(super) fn equivalent(
    left: &Histories,
    right: &Histories,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    work.budget.reserve_bytes(
        work.function,
        EQUALITY,
        "completed independent history equality",
    )?;
    if left.paths.len() != right.paths.len() {
        return Ok(false);
    }
    let a = sorted(left, work)?;
    let b = sorted(right, work)?;
    for index in 0..a.len() {
        work.budget
            .reserve_bytes(work.function, EQUALITY, "completed history final pair")?;
        if order(left, a[index], right, b[index], work)? != Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}
