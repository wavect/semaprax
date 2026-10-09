//! Explicit native execution artifacts for a source-command Project's declared
//! test module. The ordinary Project interpreter remains unavailable for these
//! profiles: this module never supplies argv or file-read authority to it.

use std::path::Path;

use crate::diagnostic::Diagnostic;

use super::{execution, native_publication, ProjectProfile, ProjectSnapshot};

/// One authenticated, zero-argument `i64` root of the manifest-declared test
/// module. `main` is first; named cases follow in linked stable-identity order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNativeTestRoot {
    stable_id: String,
    name: String,
    is_main: bool,
    result_is_exit_status: bool,
}

impl ProjectNativeTestRoot {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn is_main(&self) -> bool {
        self.is_main
    }

    /// Effectful source-command tests return the check code as process status;
    /// pure native tests print the exact `i64` result to stdout.
    pub const fn result_is_exit_status(&self) -> bool {
        self.result_is_exit_status
    }
}

impl ProjectSnapshot {
    /// List only roots already selected by the manifest-declared test module.
    /// This is a description of the retained Project, with no build or process
    /// authority. Every effect remains bounded by the manifest and the same
    /// native source-command admission used for an ordinary Project build.
    pub fn native_test_roots(&self) -> Result<Vec<ProjectNativeTestRoot>, Vec<Diagnostic>> {
        require_native_test_profile(self)?;
        let program = self.test_program();
        let result_is_exit_status = !program.permits.is_empty();
        if program.module != self.manifest().test_module() {
            return Err(vec![Diagnostic::io(
                "SPX-G173",
                "authenticated native test closure disagrees with the manifest test module",
            )]);
        }
        let main = program
            .functions
            .iter()
            .find(|function| function.id == program.entrypoint)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G173",
                    "authenticated native test main is absent from the linked test closure",
                )]
            })?;
        if main.name != "main"
            || !main.params.is_empty()
            || main.return_type != crate::hir::ResolvedType::I64
        {
            return Err(vec![Diagnostic::io(
                "SPX-G173",
                "authenticated native test main has an invalid declaration shape",
            )]);
        }
        let mut roots = vec![ProjectNativeTestRoot {
            stable_id: main.id.as_str().to_owned(),
            name: main.name.clone(),
            is_main: true,
            result_is_exit_status,
        }];
        roots.extend(
            execution::cases::case_selection(
                &self.revision,
                program,
                self.manifest().test_module(),
            )
            .map(|(stable_id, name)| ProjectNativeTestRoot {
                stable_id: stable_id.to_owned(),
                name: name.to_owned(),
                is_main: false,
                result_is_exit_status,
            }),
        );
        Ok(roots)
    }

    /// Compile one exact root returned by `native_test_roots` into a fresh
    /// executable. The emitted HIR comes from `test_program`, never the command
    /// entry program. Effectful tests use the existing manifest-bounded native
    /// SourceCommand adapter; the interpreter gains no provider.
    pub fn build_native_test(
        &mut self,
        stable_id: &str,
        output: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        let roots = self.native_test_roots()?;
        if !roots.iter().any(|root| root.stable_id == stable_id) {
            return Err(vec![Diagnostic::io(
                "SPX-G172",
                "native test root is not main or a named case in the manifest test module",
            )]);
        }
        let mut program = self.test_program().clone();
        if program
            .permits
            .iter()
            .any(|effect| !self.manifest().capabilities().contains(effect))
        {
            return Err(vec![Diagnostic::io(
                "SPX-J131",
                "native test closure requires a capability outside the exact source-command manifest grant",
            )]);
        }
        program.entrypoint = crate::hir::DeclarationId::new(stable_id);
        let prepared = match self.manifest().project_profile() {
            ProjectProfile::SourceCommandV1 if !program.permits.is_empty() => {
                crate::codegen::emit_hir_c_with_source_command(&program)
            }
            ProjectProfile::SourceCommandResourceOutputV1 if !program.permits.is_empty() => {
                crate::codegen::emit_hir_c_with_source_resource_command(&program)
            }
            _ => {
                if program
                    .functions
                    .iter()
                    .any(|function| !function.effects.is_empty())
                {
                    return Err(vec![Diagnostic::io(
                        "SPX-J131",
                        "native test closure has effects without declared module permits",
                    )]);
                }
                crate::codegen::emit_hir_c(&program)
            }
        }
        .map_err(|error| vec![error])?;
        self.recheck()?;
        let mut destination =
            native_publication::NativeOutput::prepare(output).map_err(|error| vec![error])?;
        crate::codegen::compile_native_executable_into(&prepared, destination.file())
            .map_err(|error| vec![error])?;
        destination.retain().map_err(|error| vec![error])?;
        self.published_subject = Some("native test executable");
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }
}

fn require_native_test_profile(snapshot: &ProjectSnapshot) -> Result<(), Vec<Diagnostic>> {
    if snapshot.manifest().project_profile().is_source_command() {
        Ok(())
    } else {
        Err(vec![Diagnostic::io(
            "SPX-B104",
            "native test compilation is admitted only for source-command Project profiles",
        )])
    }
}
