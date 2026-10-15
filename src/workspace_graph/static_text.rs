//! Source-site authority retained only for selected static-text modules.
//! Construction reads the live authenticated AST before it is discarded.
use super::*;
use std::fmt::Write;

pub(super) mod primitive;
pub(super) mod record_closure;
pub(super) mod scalar_helpers;
pub(super) mod scalar_lexical;

const DEPTH: usize = 514;
const WORK: usize = 1_048_576;

#[derive(Default)]
pub(super) struct SelectedModules(Vec<usize>);
impl SelectedModules {
    pub(super) fn record(
        &mut self,
        index: usize,
        count: usize,
        selected: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        if !selected {
            return Ok(());
        }
        if self.0.capacity() == 0 {
            let bytes = count
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(capacity)?;
            reserve_builder_structure(bytes)?;
            self.0 = Vec::with_capacity(count);
            let actual = self
                .0
                .capacity()
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(capacity)?;
            if actual > bytes {
                reserve_builder_structure(actual - bytes)?;
            }
        }
        if index >= count || self.0.len() == self.0.capacity() || self.0.contains(&index) {
            return Err(fail());
        }
        self.0.push(index);
        Ok(())
    }
    pub(super) fn contains(&self, index: usize) -> bool {
        self.0.contains(&index)
    }
}

pub(super) struct Inventory {
    pub(super) module: String,
    pub(super) path: String,
    pub(super) sites: Vec<Site>,
    pub(super) lexical: Option<Box<primitive::Inventory>>,
}
pub(super) struct Site {
    pub(super) owner: hir::DeclarationId,
    pub(super) explicit: bool,
    pub(super) expression: hir::ExpressionId,
    pub(super) path: String,
    pub(super) span: Span,
    pub(super) text: hir::StaticUtf8,
}
struct SourceSite<'a> {
    owner: &'a Function,
    expression: &'a Expr,
    seen: bool,
}
fn fail() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G410",
        "static literal retained source/HIR authority disagrees",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![limit_error("builder_bytes", active_builder_limit())]
}

struct Prefix<'a>(&'a str);
impl Write for Prefix<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.strip_prefix(text).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
/// Extract only the exact monomorphic identity's canonical path; no allocation.
fn path<'a>(owner: &str, expression: &'a hir::ExpressionId) -> Option<&'a str> {
    let mut prefix = Prefix(expression.as_str());
    write!(prefix, "declaration:{}:{}:expression:", owner.len(), owner).ok()?;
    let (length, path) = prefix.0.split_once(':')?;
    let length: usize = length.parse().ok()?;
    (length == path.len()).then_some(path)
}

struct AstCursor<'a> {
    frames: [Option<(&'a Expr, usize)>; DEPTH],
    work: usize,
}
impl<'a> AstCursor<'a> {
    fn new() -> Result<Self, Vec<Diagnostic>> {
        reserve_builder_structure(std::mem::size_of::<Self>())?;
        Ok(Self {
            frames: [None; DEPTH],
            work: WORK,
        })
    }
    fn visit(
        &mut self,
        root: &'a Expr,
        mut visit: impl FnMut(&'a Expr) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.frames[0] = Some((root, usize::MAX));
        let mut depth = 1;
        while depth != 0 {
            self.work = self.work.checked_sub(1).ok_or_else(fail)?;
            let (expression, next) = self.frames[depth - 1].ok_or_else(fail)?;
            if next == usize::MAX {
                visit(expression)?;
                self.frames[depth - 1] = Some((expression, 0));
                continue;
            }
            let width = match &expression.kind {
                ExprKind::Block { statements, .. } => statements.len(),
                ExprKind::Match { arms, .. } => arms.len(),
                _ => 0,
            };
            self.work = self.work.checked_sub(width).ok_or_else(fail)?;
            if let Some(child) = expression.child(next) {
                if depth == DEPTH {
                    return Err(fail());
                }
                self.frames[depth - 1] = Some((expression, next + 1));
                self.frames[depth] = Some((child, usize::MAX));
                depth += 1;
            } else {
                self.frames[depth - 1] = None;
                depth -= 1;
            }
        }
        Ok(())
    }
}
fn roots(function: &Function) -> impl Iterator<Item = &Expr> {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
}

pub(super) fn retain(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    let mut cursor = AstCursor::new()?;
    let mut count = 0usize;
    let mut lexical = false;
    for function in &program.functions {
        for root in roots(function) {
            cursor.visit(root, |expression| {
                lexical |= matches!(expression.kind, ExprKind::LexicalExit { .. });
                if matches!(expression.kind, ExprKind::BorrowedUtf8Literal(_)) {
                    count = count.checked_add(1).ok_or_else(capacity)?;
                }
                Ok(())
            })?;
        }
    }
    // Imported synthetic bodies can select the full checked program. They do
    // not authorize this module: their providers retain their own source sites.
    if count == 0 {
        return Ok(None);
    }
    let scratch = count
        .checked_mul(std::mem::size_of::<SourceSite<'_>>())
        .ok_or_else(capacity)?;
    reserve_builder_structure(scratch)?;
    let mut source = Vec::with_capacity(count);
    if source.capacity() > count {
        reserve_builder_structure(
            (source.capacity() - count)
                .checked_mul(std::mem::size_of::<SourceSite<'_>>())
                .ok_or_else(capacity)?,
        )?;
    }
    for function in &program.functions {
        for root in roots(function) {
            cursor.visit(root, |expression| {
                if matches!(expression.kind, ExprKind::BorrowedUtf8Literal(_)) {
                    source.push(SourceSite {
                        owner: function,
                        expression,
                        seen: false,
                    });
                }
                Ok(())
            })?;
        }
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            + count
                .checked_mul(std::mem::size_of::<Site>())
                .ok_or_else(capacity)?,
    )?;
    let mut sites = Vec::with_capacity(count);
    if sites.capacity() > count {
        reserve_builder_structure(
            (sites.capacity() - count)
                .checked_mul(std::mem::size_of::<Site>())
                .ok_or_else(capacity)?,
        )?;
    }
    let mut work = WORK;
    let mut matching_work = WORK;
    for (owner, roots) in resolved
        .functions
        .iter()
        .map(|function| {
            (
                &function.id,
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures),
            )
        })
        .chain(resolved.function_templates.iter().map(|function| {
            (
                &function.id,
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures),
            )
        }))
    {
        let Some(origin) = authored.get(owner.as_str()) else {
            continue;
        };
        if origin.path != program.path
            || origin.module != program.module
            || origin.function.is_none()
        {
            continue;
        }
        hir::static_text::visit_roots_with_work(roots, &mut work, |expression, text, literal| {
            if !literal {
                return Ok(());
            }
            matching_work = matching_work.checked_sub(source.len()).ok_or_else(|| {
                graph_error("SPX-G410", "static source-site matching exceeds work limit")
            })?;
            let mut matching = source.iter_mut().filter(|site| {
                site.owner.stable_id == owner.as_str() && site.expression.span == expression.span
            });
            let site = matching.next().ok_or_else(|| {
                graph_error(
                    "SPX-G410",
                    "static producer lacks exact authored source site",
                )
            })?;
            if matching.next().is_some() || site.seen {
                return Err(graph_error("SPX-G410", "static source site is duplicated"));
            }
            let ExprKind::BorrowedUtf8Literal(expected) = &site.expression.kind else {
                unreachable!()
            };
            if expected != text {
                return Err(graph_error(
                    "SPX-G410",
                    "static producer content disagrees with source",
                ));
            }
            let literal_path = path(owner.as_str(), &expression.id).ok_or_else(|| {
                graph_error(
                    "SPX-G410",
                    "static producer identity has wrong source owner",
                )
            })?;
            // String/Arc payload reservation is before construction. Cloning
            // the expression identity retains its already-charged backing.
            if !crate::bounded_output::reserve_active_required(
                owner
                    .as_str()
                    .len()
                    .checked_add(literal_path.len())
                    .ok_or_else(|| graph_error("SPX-G171", "static identity size overflows"))?,
            ) {
                return Err(graph_error(
                    "SPX-G171",
                    "static retained identities exceed builder budget",
                ));
            }
            let text = hir::StaticUtf8::from_text(expected);
            if !text.is_backed() {
                return Err(graph_error(
                    "SPX-G171",
                    "static source backing reservation was refused",
                ));
            }
            sites.push(Site {
                owner: owner.clone(),
                explicit: origin.explicit,
                expression: expression.id.clone(),
                path: literal_path.to_owned(),
                span: expression.span,
                text,
            });
            site.seen = true;
            Ok(())
        })
        .map_err(|e| vec![e])?;
    }
    if sites.len() != count || source.iter().any(|site| !site.seen) {
        return Err(fail());
    }
    let lexical = if lexical {
        primitive::capture(program, resolved, authored, authenticated_sources)?
    } else {
        None
    };
    let module = crate::bounded_output::budgeted_clone(&program.module);
    let path = crate::bounded_output::budgeted_clone(&program.path);
    Ok(Some(Box::new(Inventory {
        module,
        path,
        sites,
        lexical,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_inventory_scratch_reserves_before_construction_and_absence_is_empty() {
        let bytes = std::mem::size_of::<AstCursor<'_>>();
        let (cursor, refused, used) =
            crate::bounded_output::with_limit_usage(bytes, AstCursor::new);
        assert!(cursor.is_ok());
        assert!(!refused);
        assert_eq!(used, bytes);
        let (cursor, refused, used, first_refusal) =
            crate::bounded_output::with_limit_usage_refusal(bytes - 1, AstCursor::new);
        let error = cursor
            .err()
            .unwrap_or_else(|| panic!("one-short cursor backing refuses"));
        assert!(refused);
        let first_refusal = first_refusal.expect("cursor precharge records refusal");
        assert_eq!(first_refusal.requested, bytes);
        assert_eq!(first_refusal.remaining, bytes - 1);
        assert_eq!(first_refusal.floor, 0);
        assert_eq!(first_refusal.stage, None);
        // Refusal reporting is charged too. An independent diagnostic-only
        // control proves that no cursor backing was debited after refusal.
        let (report, report_refused, report_used) =
            crate::bounded_output::with_limit_usage(bytes - 1, capacity);
        assert!(!report_refused);
        assert_eq!(error, report);
        assert_eq!(used, report_used);
        let ((result, selected), refused, used) =
            crate::bounded_output::with_limit_usage(0, || {
                let mut selected = SelectedModules::default();
                let result = selected.record(0, 32, false);
                (result, selected)
            });
        assert!(result.is_ok());
        assert!(!refused);
        assert_eq!(used, 0);
        assert_eq!(selected.0.capacity(), 0);
        let required = 32 * std::mem::size_of::<usize>();
        let (result, refused, used, first_refusal) =
            crate::bounded_output::with_limit_usage_refusal(required - 1, || {
                let mut selected = SelectedModules::default();
                let result = selected.record(0, 32, true);
                assert_eq!(selected.0.capacity(), 0);
                result
            });
        let error = result
            .err()
            .unwrap_or_else(|| panic!("one-short selected vector backing refuses"));
        assert!(refused);
        let first_refusal = first_refusal.expect("selected vector precharge records refusal");
        assert_eq!(first_refusal.requested, required);
        assert_eq!(first_refusal.remaining, required - 1);
        let (report, report_refused, report_used) =
            crate::bounded_output::with_limit_usage(required - 1, capacity);
        assert!(!report_refused);
        assert_eq!(error, report);
        assert_eq!(used, report_used);
    }
}
