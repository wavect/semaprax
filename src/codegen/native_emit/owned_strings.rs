//! Physical inline String ownership, deliberately separate from CleanupPlan.
//! Ordinary/stream-transcript String functions and the length-delimited V10
//! provider enable this bookkeeping; frozen command/provider routes do not.

use std::collections::BTreeMap;

pub(super) enum FunctionOutput<'a, O> {
    Direct(&'a mut O),
    Staged(crate::bounded_output::CappedString),
}

impl<O: super::COutput> std::fmt::Write for FunctionOutput<'_, O> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        super::COutput::push_str(self, value);
        Ok(())
    }
}

impl<O: super::COutput> super::COutput for FunctionOutput<'_, O> {
    fn push_str(&mut self, value: &str) {
        match self {
            Self::Direct(output) => output.push_str(value),
            Self::Staged(output) => output.push_str(value),
        }
    }
    fn push(&mut self, value: char) {
        match self {
            Self::Direct(output) => output.push(value),
            Self::Staged(output) => output.push(value),
        }
    }
}

#[derive(Default)]
pub(super) struct OwnedStrings {
    // True means the pointer itself must be declared in the function prologue.
    cells: BTreeMap<String, bool>,
}

impl OwnedStrings {
    pub(super) fn register(&mut self, name: &str, declare: bool) -> Result<(), super::Diagnostic> {
        if name.is_empty() || self.cells.insert(name.to_owned(), declare).is_some() {
            return Err(super::backend_error(
                "invalid inline String storage identity",
            ));
        }
        Ok(())
    }

    pub(super) fn declarations(&self) -> String {
        let mut out = crate::bounded_output::CappedString::new();
        for (name, declare) in &self.cells {
            if *declare {
                out.push_str(&format!("    char *{name} = NULL;\n"));
            }
            out.push_str(&format!("    bool {name}_live = false;\n"));
        }
        out.into_string()
    }

    pub(super) fn names(&self) -> Vec<String> {
        self.cells.keys().cloned().collect()
    }
}

impl<O: super::COutput> super::CEmitter<'_, O> {
    pub(super) fn string_initialize(&mut self, name: &str) {
        if self.owned_strings.is_some() {
            self.line(&format!("{name}_live = true;"));
        }
    }

    pub(super) fn string_move(&mut self, destination: &str, source: &str) {
        if self.owned_strings.is_some() {
            self.line(&format!("if (!{source}_live || {destination}_live) spx_runtime_invariant_failure(\"invalid String transfer\");"));
        }
        self.line(&format!("{destination} = {source};"));
        if self.owned_strings.is_some() {
            self.line(&format!("{destination}_live = {source}_live;"));
            self.line(&format!("{source}_live = false;"));
            self.line(&format!("{source} = NULL;"));
        }
    }

    pub(super) fn string_require_dead(&mut self, name: &str) {
        if self.owned_strings.is_some() {
            self.line(&format!(
                "if ({name}_live) spx_runtime_invariant_failure(\"live String overwritten\");"
            ));
        }
    }

    pub(super) fn string_drop(&mut self, name: &str) {
        if self.owned_strings.is_some() {
            self.line(&format!("if ({name}_live) {{"));
            self.indent += 1;
            self.line(&format!("{name}_live = false;"));
            self.line(&format!("spx_string_drop({name});"));
            self.line(&format!("{name} = NULL;"));
            self.indent -= 1;
            self.line("}");
        } else {
            self.line(&format!("spx_string_drop({name});"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::COutput;

    #[test]
    fn direct_sink_preserves_bytes_and_exact_budget_charges() {
        let render = |wrapped| {
            crate::bounded_output::with_limit_usage(100, || {
                let mut output = crate::bounded_output::CappedString::new();
                if wrapped {
                    let mut direct = FunctionOutput::Direct(&mut output);
                    direct.push_str("ordinary body");
                    direct.push('\n');
                } else {
                    output.push_str("ordinary body");
                    output.push('\n');
                }
                output.into_string()
            })
        };
        assert_eq!(render(false), render(true));
    }

    #[test]
    fn exhausted_or_duplicate_cell_identity_is_diagnostic_not_panic() {
        let mut cells = OwnedStrings::default();
        cells.register("spx_internal_0", true).unwrap();
        assert!(cells.register("spx_internal_0", true).is_err());
        let (result, overflowed) = crate::bounded_output::with_limit(0, || {
            let name = crate::bounded_output::budgeted_format(format_args!("spx_internal_{}", 1));
            cells.register(&name, true)
        });
        assert!(overflowed);
        assert!(result.is_err());
    }

    #[test]
    fn inline_owner_cells_preserve_frozen_provider_and_cover_ordinary_native() {
        let checked = crate::check(
            "module test.inline; @id(\"word\") fn word() -> string { \"value\" } @id(\"main\") fn main() -> i64 { 0 }",
            "inline.spx",
        ).unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let ordinary = crate::codegen::emit_hir_c(&program).unwrap();
        let older = crate::codegen::emit_hir_c_for_owned_data_provider(&program).unwrap();
        assert!(!older.contains("spx_result_live"));
        assert!(!older.contains("invalid String transfer"));
        assert!(!older.contains("live String overwritten"));
        assert!(ordinary.contains("spx_result_live"));
        assert!(ordinary.contains("invalid String transfer"));
        assert!(ordinary.contains("strlen(spx_source)"));
        assert!(!ordinary.contains("struct spx_string_v10"));
        let current = crate::codegen::emit_hir_c_for_owned_utf8_provider(&program).unwrap();
        assert!(current.contains("char *spx_internal_0 = NULL;"));
        assert!(current.contains("bool spx_internal_0_live = false;"));
        assert!(current.contains("invalid String transfer"));
        assert!(current.contains("if (!spx_result_live)"));
        let publish = current
            .find("*spx_result_out = spx_result;\n    spx_result_live = false;")
            .unwrap();
        assert!(publish > current.find("if (!spx_result_live)").unwrap());
    }

    fn resolved(source: &str) -> crate::hir::ResolvedProgram {
        crate::hir::resolve(&crate::check(source, "inline-presence.spx").unwrap()).unwrap()
    }

    #[test]
    fn presence_checks_signature_body_and_both_contract_phases() {
        use super::super::NativeOutputProfile as Profile;
        let program = resolved(
            r#"module test.presence;
@id("param") fn parameter(text: string) -> i64 { 1 }
@id("result") fn result_text() -> string { "result" }
@id("body") fn body() -> i64 { let text = "body"; 1 }
@id("requires") fn before() -> i64 requires string_is_empty("") { 1 }
@id("ensures") fn after() -> i64 ensures string_is_empty("") { 1 }
@id("main") fn main() -> i64 { 0 }
"#,
        );
        for function in &program.functions {
            let has_strings = function.id.as_str() != "main";
            assert_eq!(super::super::function_uses_strings(function), has_strings);
            for profile in [Profile::Legacy, Profile::StdoutTranscript] {
                assert_eq!(profile.tracks_strings(function), has_strings);
            }
            // V10 deliberately retains its previous always-on selection,
            // including zero-String functions, while all frozen profiles stay off.
            assert!(Profile::OwnedUtf8Provider.tracks_strings(function));
            for profile in [
                Profile::OwnedDataProvider,
                Profile::UsefulDataCommand,
                Profile::LanguageCommandIo,
                Profile::LineCommandIo,
            ] {
                assert!(!profile.tracks_strings(function));
                assert!(!profile.corrects_ordinary_strings());
            }
        }
    }

    #[test]
    fn ordinary_discovery_includes_instantiated_string_runtime_groups_only_when_selected() {
        let program = resolved(
            r#"module test.instance_strings;
@id("measure") fn measure<T>(value: T) -> i64 { string_len_chars("hé") }
@id("main") fn main() -> i64 { measure<i64>(1) }
"#,
        );
        assert_eq!(program.function_instances.len(), 1);
        assert!(!super::super::program_uses_strings(&program, false));
        assert!(!super::super::program_uses_string_ops(&program, false));
        assert!(!super::super::program_uses_string_ops_v2(&program, false));
        assert!(super::super::program_uses_strings(&program, true));
        assert!(super::super::program_uses_string_ops(&program, true));
        assert!(super::super::program_uses_string_ops_v2(&program, true));
        let native = crate::codegen::emit_hir_c(&program).unwrap();
        assert!(native.contains("static __attribute__((unused)) char *spx_string_from_literal("));
        assert!(native.contains("spx_string_len_chars(const char *"));
        assert!(native.contains("live String overwritten"));
    }

    #[test]
    fn string_free_function_emission_matches_frozen_route_bytes_and_budget() {
        use super::super::NativeOutputProfile as Profile;
        let program = resolved("module test.scalar; @id(\"main\") fn main() -> i64 { 40 + 2 }");
        let emit = |profile| {
            crate::bounded_output::with_limit_usage(1_000_000, || {
                super::super::emit_hir_c_with_labels(
                    &program,
                    &std::collections::HashMap::new(),
                    profile,
                    None,
                )
                .unwrap()
            })
        };
        // These profiles shared the exact scalar projection before correction.
        assert_eq!(emit(Profile::Legacy), emit(Profile::OwnedDataProvider));
    }
}
