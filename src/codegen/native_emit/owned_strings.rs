//! Physical inline String ownership, deliberately separate from CleanupPlan.
//! Only the length-delimited V10 provider enables this bookkeeping.

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
    fn inline_owner_cells_are_exclusive_to_v10_provider() {
        let checked = crate::check(
            "module test.inline; @id(\"word\") fn word() -> string { \"value\" } @id(\"main\") fn main() -> i64 { 0 }",
            "inline.spx",
        ).unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let ordinary = crate::codegen::emit_hir_c(&program).unwrap();
        let older = crate::codegen::emit_hir_c_for_owned_data_provider(&program).unwrap();
        for source in [&ordinary, &older] {
            assert!(!source.contains("spx_result_live"));
            assert!(!source.contains("invalid String transfer"));
            assert!(!source.contains("live String overwritten"));
        }
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
}
