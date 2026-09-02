//! Physical inline String ownership, deliberately separate from CleanupPlan.
//! Ordinary/stream-transcript and owned-data-provider String functions enable
//! this bookkeeping. V10 retains its always-on gate; command routes stay frozen.

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
#[path = "owned_strings/tests.rs"]
mod tests;
