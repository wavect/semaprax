//! Structural parser provenance only; typing remains independently mandatory.
#[derive(Clone, Copy)]
enum PatternFrame<'a> {
    Pattern(&'a super::MatchPattern),
    Records(&'a [super::RecordMatchPatternField], usize),
    Alternatives(&'a [super::MatchPattern], usize),
}

use super::{Expr, ExprKind, LetSyntax, Statement};
pub fn value(statement: &Statement) -> Option<&Expr> {
    let Statement::Let {
        syntax: LetSyntax::ExpressionStatement,
        name,
        name_span,
        mutable: false,
        declared: None,
        value,
        span,
    } = statement
    else {
        return None;
    };
    let suffix = name.strip_prefix("_if")?;
    if suffix.is_empty()
        || suffix.starts_with('0')
        || !suffix.bytes().all(|byte| byte.is_ascii_digit())
        || *name_span != value.span
        || span.start != value.span.start
        || span.end <= value.span.end
        || span.line != value.span.line
        || span.column != value.span.column
    {
        return None;
    }
    Some(value)
}

fn retain<'a>(
    names: &mut Vec<&'a str>,
    name: &'a str,
    reserve: &mut impl FnMut(usize) -> bool,
) -> bool {
    if !name.starts_with("_if") {
        return true;
    }
    if names.len() == names.capacity() {
        let Some(capacity) = names.capacity().max(2).checked_mul(2) else {
            return false;
        };
        let Some(bytes) = capacity.checked_mul(std::mem::size_of::<&str>()) else {
            return false;
        };
        if !reserve(bytes) || names.try_reserve_exact(capacity - names.len()).is_err() {
            return false;
        }
    }
    names.push(name);
    true
}
pub fn projection_is_hygienic(root: &Expr, reserve: impl FnMut(usize) -> bool) -> bool {
    projection_with_bindings(root, std::iter::empty(), reserve)
}
/// Presentation-only caller scope; supplied names grant no semantic authority.
pub fn projection_with_bindings<'a>(
    root: &'a Expr,
    bindings: impl Iterator<Item = &'a str>,
    reserve: impl FnMut(usize) -> bool,
) -> bool {
    projection_roots(std::iter::once(root), bindings, reserve)
}
pub fn projection_roots<'a>(
    roots: impl Iterator<Item = &'a Expr>,
    bindings: impl Iterator<Item = &'a str>,
    mut reserve: impl FnMut(usize) -> bool,
) -> bool {
    // Block children use statement/child positions rather than repeated prefix lookup.
    // Over-depth foreign ASTs conservatively retain the full let spelling.
    let mut stack = Vec::new();
    if !grow_frames(&mut stack, 1, &mut reserve) {
        return false;
    }
    if !reserve(std::mem::size_of::<Vec<Option<PatternFrame<'_>>>>()) {
        return false;
    }
    let mut patterns = Vec::new();

    let mut markers = Vec::new();
    let mut uses = Vec::new();
    for name in bindings {
        if !retain(&mut uses, name, &mut reserve) {
            return false;
        }
    }
    for root in roots {
        stack[0] = Some((root, 0usize, 0usize));
        let mut len = 1;
        while len != 0 {
            let (node, outer, inner) = stack[len - 1].expect("retained projection frame");
            if outer == 0 && inner == 0 {
                match &node.kind {
                    ExprKind::VecConstructor { .. } => {
                        let Ok(expansion) = super::vec_constructor::inspect(node) else {
                            return false;
                        };
                        for statement in expansion.statements {
                            let Statement::Let { name, .. } = statement else {
                                unreachable!("authenticated constructor");
                            };
                            if !retain(&mut markers, name, &mut reserve) {
                                return false;
                            }
                        }
                    }
                    ExprKind::Call { name, .. } => {
                        if !retain(&mut uses, name, &mut reserve) {
                            return false;
                        }
                    }
                    ExprKind::Closure { params, .. } => {
                        for param in params {
                            if !retain(&mut uses, &param.name, &mut reserve) {
                                return false;
                            }
                        }
                    }
                    ExprKind::Match { arms, .. } => {
                        for arm in arms {
                            if !pattern_names(&arm.pattern, &mut uses, &mut reserve, &mut patterns)
                            {
                                return false;
                            }
                        }
                    }
                    _ => {}
                }
                if let ExprKind::Var(name) = &node.kind {
                    if !retain(&mut uses, name, &mut reserve) {
                        return false;
                    }
                }
                if let ExprKind::Block { statements, .. } = &node.kind {
                    for statement in statements {
                        match statement {
                            Statement::Let { name, .. } => {
                                let target = if crate::ast::expression_statement::value(statement)
                                    .is_some()
                                {
                                    &mut markers
                                } else {
                                    &mut uses
                                };
                                if !retain(target, name, &mut reserve) {
                                    return false;
                                }
                            }
                            Statement::Assign { name, .. } => {
                                if !retain(&mut uses, name, &mut reserve) {
                                    return false;
                                }
                            }
                            Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                                if !retain(&mut uses, item, &mut reserve) {
                                    return false;
                                }
                            }
                            Statement::ForBorrow { item, index, .. } => {
                                if !retain(&mut uses, item, &mut reserve) {
                                    return false;
                                }
                                if let Some(index) = index {
                                    if !retain(&mut uses, &index.name, &mut reserve) {
                                        return false;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            let (child, next) = if matches!(node.kind, ExprKind::VecConstructor { .. }) {
                // Entry validated the whole chain once. Child advance must remain constant-time.
                let ExprKind::VecConstructor { value, .. } = &node.kind else {
                    unreachable!()
                };
                let ExprKind::Block { statements, .. } = &value.kind else {
                    unreachable!("validated constructor block")
                };
                let child = statements.get(outer + 1).map(|statement| {
                    let Statement::Let { value, .. } = statement else {
                        unreachable!("validated constructor binding")
                    };
                    let ExprKind::Call { args, .. } = &value.kind else {
                        unreachable!("validated constructor call")
                    };
                    &args[1]
                });
                (child, (outer + 1, 0))
            } else if let ExprKind::Block { statements, tail } = &node.kind {
                if let Some(statement) = statements.get(outer) {
                    if let Some(child) = statement.child(inner) {
                        (Some(child), (outer, inner + 1))
                    } else {
                        stack[len - 1] = Some((node, outer + 1, 0));
                        continue;
                    }
                } else if outer == statements.len() {
                    (Some(tail.as_ref()), (outer + 1, 0))
                } else {
                    (None, (outer, inner))
                }
            } else {
                (node.child(outer), (outer + 1, 0))
            };
            if let Some(child) = child {
                stack[len - 1] = Some((node, next.0, next.1));
                if len == 513 || !grow_frames(&mut stack, len + 1, &mut reserve) {
                    return false;
                }
                stack[len] = Some((child, 0, 0));
                len += 1;
            } else {
                len -= 1;
            }
        }
    }
    markers.sort_unstable();
    uses.sort_unstable();
    !markers.windows(2).any(|pair| pair[0] == pair[1])
        && markers.iter().all(|name| uses.binary_search(name).is_err())
}

// Grow only physical scratch; the independent 513-frame traversal limit remains.
// Pay the entire replacement while the previous allocation is still live. No
// capacity credit or refund is inferred from the allocator's choice of backing.
fn grow_frames<T: Copy>(
    frames: &mut Vec<Option<T>>,
    needed: usize,
    reserve: &mut impl FnMut(usize) -> bool,
) -> bool {
    if needed > 513 {
        return false;
    }
    if needed <= frames.len() {
        return true;
    }
    let capacity = frames.len().max(1).saturating_mul(2).min(513).max(needed);
    let Ok(layout) = std::alloc::Layout::array::<Option<T>>(capacity) else {
        return false;
    };
    let Some(bytes) = layout
        .size()
        .checked_add(2 * std::mem::size_of::<Vec<Option<T>>>() + std::mem::size_of::<Option<T>>())
    else {
        return false;
    };
    if !reserve(bytes) {
        return false;
    }
    let mut replacement = Vec::new();
    if replacement.try_reserve_exact(capacity).is_err() || replacement.capacity() != capacity {
        return false;
    }
    replacement.append(frames);
    replacement.resize(capacity, None);
    *frames = replacement;
    true
}

fn pattern_names<'a>(
    pattern: &'a super::MatchPattern,
    names: &mut Vec<&'a str>,
    reserve: &mut impl FnMut(usize) -> bool,
    frames: &mut Vec<Option<PatternFrame<'a>>>,
) -> bool {
    use super::{MatchPattern, RecordMatchFieldPattern};
    if !reserve(0) {
        return false;
    }
    if frames.is_empty() {
        // Acquire the complete original fixed pattern capacity only when a
        // pattern exists. The 129-slot walk and dispatcher remain unchanged.
        if !reserve(std::mem::size_of::<[Option<PatternFrame<'_>>; 129]>()) {
            return false;
        }
        if frames.try_reserve_exact(129).is_err() || frames.capacity() != 129 {
            return false;
        }
        frames.resize(129, None);
    }
    frames[0] = Some(PatternFrame::Pattern(pattern));
    let mut len = 1;
    while len != 0 {
        len -= 1;
        let frame = frames[len].take().expect("retained pattern frame");
        let mut first = None;
        let mut second = None;
        match frame {
            PatternFrame::Pattern(pattern) => match pattern {
                MatchPattern::Binding { name, .. } => {
                    if !retain(names, name, reserve) {
                        return false;
                    }
                }
                MatchPattern::Variant { fields, .. } => {
                    for field in fields {
                        if !retain(names, &field.binding, reserve) {
                            return false;
                        }
                    }
                }
                MatchPattern::Record { fields, .. } => {
                    first = Some(PatternFrame::Records(fields, 0));
                }
                MatchPattern::Or { alternatives, .. } => {
                    first = Some(PatternFrame::Alternatives(alternatives, 0));
                }
                MatchPattern::Literal { .. } | MatchPattern::Wildcard { .. } => {}
            },
            PatternFrame::Alternatives(values, index) => {
                if let Some(value) = values.get(index) {
                    first = Some(PatternFrame::Alternatives(values, index + 1));
                    second = Some(PatternFrame::Pattern(value));
                }
            }
            PatternFrame::Records(fields, index) => {
                if let Some(field) = fields.get(index) {
                    first = Some(PatternFrame::Records(fields, index + 1));
                    match &field.pattern {
                        RecordMatchFieldPattern::Binding { name, .. } => {
                            if !retain(names, name, reserve) {
                                return false;
                            }
                        }
                        RecordMatchFieldPattern::Record { fields, .. } => {
                            second = Some(PatternFrame::Records(fields, 0));
                        }
                        RecordMatchFieldPattern::Wildcard { .. } => {}
                    }
                }
            }
        }
        for next in [first, second].into_iter().flatten() {
            if len == frames.len() {
                return false;
            }
            frames[len] = Some(next);
            len += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests;
