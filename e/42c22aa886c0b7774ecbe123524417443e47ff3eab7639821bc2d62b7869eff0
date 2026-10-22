//! Exact reuse of function identities already held by one resolved program.

use std::borrow::Cow;

use super::{DeclarationId, ResolvedProgram};

impl ResolvedProgram {
    /// Return the exact identity held by the first matching resolved function.
    ///
    /// The owned fallback exists only to preserve legacy diagnostics for an
    /// absent selection. Callers must still pass it through their existing
    /// admission checks before treating the selected function as authority.
    pub(crate) fn selected_function_id(&self, selected: &str) -> Cow<'_, DeclarationId> {
        self.functions
            .iter()
            .find(|function| function.id.as_str() == selected)
            .map(|function| Cow::Borrowed(&function.id))
            .unwrap_or_else(|| Cow::Owned(DeclarationId::new(selected)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved() -> ResolvedProgram {
        let source = r#"
module test.selected_function;
@id("command.é") fn command() -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, "selected-function.spx").unwrap();
        crate::hir::resolve(&ast).unwrap()
    }

    #[test]
    fn selected_function_identity_is_the_exact_held_value() {
        let program = resolved();
        let detached = DeclarationId::new("command.é");
        let selected = program.selected_function_id(detached.as_str());
        let Cow::Borrowed(selected) = selected else {
            panic!("present function identity must be borrowed");
        };
        let held = &program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "command.é")
            .unwrap()
            .id;
        assert!(std::ptr::eq(selected, held));
        assert!(!std::ptr::eq(selected, &detached));
    }

    #[test]
    fn selected_function_lookup_and_full_clone_drop_allocate_nothing() {
        let program = resolved();
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let selected = program.selected_function_id("command.é").into_owned();
            assert_eq!(selected.as_str(), "command.é");
            drop(selected);
        });
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocated, 0);
        assert_eq!(observed.freed, 0);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }

    #[test]
    fn absent_text_uses_only_the_legacy_detached_diagnostic_fallback() {
        let program = resolved();
        for selected in ["", "missing\0command", "missing.λ"] {
            let id = program.selected_function_id(selected);
            assert!(matches!(&id, Cow::Owned(_)));
            assert_eq!(id.as_ref().as_str(), selected);
            assert!(program.functions.iter().all(|function| {
                !std::ptr::eq(id.as_ref(), &function.id) && function.id.as_str() != selected
            }));
        }
    }

    #[test]
    fn all_named_receivers_use_the_shared_held_identity_or_borrowed_suffix_probe() {
        let group_one = [
            include_str!("../project/admission/mod.rs"),
            include_str!("../project/admission/stdin_stream_command.rs"),
            include_str!("../project/admission/legacy.rs"),
            include_str!("../workspace_graph/retained_validation/stream_admission.rs"),
            include_str!("../codegen/native_stdin_stream.rs"),
        ];
        assert_eq!(
            group_one
                .iter()
                .map(|source| source.matches("selected_function_id(").count())
                .sum::<usize>(),
            16
        );
        for source in group_one {
            assert!(!source.contains("DeclarationId::new(command)"));
            assert!(!source.contains("DeclarationId::new(command_id)"));
            assert!(!source.contains("DeclarationId::new(\n                    manifest.command()"));
        }

        let group_two = [
            include_str!("collection_outcome/nested.rs"),
            include_str!("collection_outcome/stream_input.rs"),
            include_str!("projected_string_view.rs"),
            include_str!("../aggregate_layout.rs"),
        ];
        assert_eq!(
            group_two
                .iter()
                .map(|source| source.matches("has_record_invariant(").count())
                .sum::<usize>(),
            5
        );
        for source in group_two {
            assert!(!source.contains("format!(\"{declaration}#invariant\")"));
        }
    }
}
