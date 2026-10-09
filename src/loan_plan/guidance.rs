//! Recovery context for the unchanged per-function checked-work refusal.

use crate::{diagnostic::Diagnostic, hir::ResolvedFunction};

pub(super) fn work_limit(mut diagnostic: Diagnostic, function: &ResolvedFunction) -> Diagnostic {
    if diagnostic.code != "SPX-H006"
        || diagnostic.message != "loan analysis exceeds 1,000,000 checked work"
    {
        return diagnostic;
    }
    let span = function.body.span;
    if diagnostic.span.is_none() && span.start < span.end && span.line > 0 && span.column > 0 {
        diagnostic.span = Some(span);
    }
    if diagnostic.help.is_none() {
        diagnostic.help = Some(format!(
            "function @id({:?}) reached the per-function loan-analysis limit; split large functions into smaller helpers and keep borrowed views local to their last use; the work limit remains 1,000,000",
            function.id.as_str()
        ));
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> crate::hir::ResolvedProgram {
        let source = include_str!("../../tests/language/while_loops/record_borrow_renewal.spx");
        let ast = crate::check(source, std::path::Path::new("loan-limit-context.spx")).unwrap();
        crate::hir::resolve(&ast).unwrap()
    }

    #[test]
    fn loan_work_limit_identifies_the_function_without_changing_the_guard() {
        let program = fixture();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "matcher.run")
            .unwrap();
        let mut work = crate::loan_plan::WorkCounter::new(0);
        let selected =
            crate::loan_plan::build_cfg_plan_counted(&program, function, &mut work).unwrap_err();
        assert_eq!(work.used, 1);
        let diagnostic = work_limit(selected, function);
        assert_eq!(diagnostic.code, "SPX-H006");
        assert_eq!(
            diagnostic.message,
            "loan analysis exceeds 1,000,000 checked work"
        );
        assert_eq!(diagnostic.span, Some(function.body.span));
        assert_eq!(diagnostic.path, None);
        let help = diagnostic.help.unwrap();
        assert!(help.contains("function @id(\"matcher.run\")"));
        assert!(help.contains("smaller helpers"));
        assert!(help.contains("work limit remains 1,000,000"));
    }

    #[test]
    fn loan_work_limit_preserves_selected_blame_and_missing_provenance() {
        let program = fixture();
        let function = &program.functions[0];
        let selected = Diagnostic::error(
            "SPX-H006",
            "move, mutation, or transfer overlaps an active shared loan",
            function.body.span,
        )
        .at_path("held.spx")
        .with_help("the active shared loan begins at 2:3");
        let diagnostic = work_limit(selected.clone(), function);
        assert_eq!(diagnostic.code, selected.code);
        assert_eq!(diagnostic.message, selected.message);
        assert_eq!(diagnostic.path, selected.path);
        assert_eq!(diagnostic.span, selected.span);
        assert_eq!(diagnostic.help, selected.help);
        let mut synthetic = function.clone();
        synthetic.body.span = crate::ast::Span::default();
        let selected = Diagnostic::io("SPX-H006", "loan analysis exceeds 1,000,000 checked work");
        let diagnostic = work_limit(selected, &synthetic);
        assert_eq!(diagnostic.span, None);
        assert_eq!(diagnostic.path, None);
    }
}
