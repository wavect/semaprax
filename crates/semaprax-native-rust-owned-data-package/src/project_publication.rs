//! Held filesystem authority for `semaprax new`.
//!
//! This lives beside the other native package publication code so the root
//! crate can reuse the safe cross-platform handle-relative platform facade
//! without acquiring raw platform authority or introducing unsafe code.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

use semaprax_native_rust_interop_platform as platform;

const ROOT_NAMES: [&str; 3] = ["README.md", "AGENTS.md", "semaprax.toml"];
const SERVICE_ROOT_NAMES: [&str; 6] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "service-config.schema.json",
    "service.config.json",
    "service-host-adapter-request.json",
];
const CALCULATOR_SOURCE_NAMES: [&str; 3] = ["app.spx", "core.spx", "tests.spx"];
const LIBRARY_SOURCE_NAMES: [&str; 3] = ["examples.spx", "lib.spx", "tests.spx"];

#[derive(Clone, Copy)]
enum TemplateInventory {
    Calculator,
    Library,
    Service,
}

#[allow(clippy::large_enum_variant)]
enum SourceInventory {
    Calculator(platform::PreparedDiscardInventory<3>),
    Library(platform::PreparedDiscardInventory<3>),
    Service(platform::PreparedDiscardInventory<3>),
}

enum RootInventory {
    Basic(Box<platform::PreparedDiscardInventory<3>>),
    Service(Box<platform::PreparedDiscardInventory<6>>),
}

impl RootInventory {
    fn file(&self, name: &str) -> Result<&platform::HeldRegularFile, NewProjectAuthorityError> {
        match self {
            Self::Basic(files) => files.file(name),
            Self::Service(files) => files.file(name),
        }
        .map_err(map_changed)
    }

    fn settle_for_publish(&mut self) -> Result<(), NewProjectAuthorityError> {
        match self {
            Self::Basic(files) => files.settle_for_publish(),
            Self::Service(files) => files.settle_for_publish(),
        }
        .map_err(map_changed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum NewProjectAuthorityError {
    Exists,
    StageExists,
    Changed,
    Invalid,
}

#[doc(hidden)]
pub struct NewProjectAuthority {
    parent: platform::HeldDirectory,
    stage: platform::HeldDirectory,
    source: Option<platform::HeldDirectory>,
    stage_name: platform::PreparedStageName,
    source_name: platform::PreparedStageName,
    output_name: OsString,
    parent_path: PathBuf,
    output_path: PathBuf,
    root: RootInventory,
    source_files: SourceInventory,
    published: bool,
    #[cfg(test)]
    before_rename: Option<Box<dyn FnOnce()>>,
    #[cfg(test)]
    after_rename: Option<Box<dyn FnOnce()>>,
}

impl NewProjectAuthority {
    pub fn create(
        parent_path: &Path,
        output_name: &OsStr,
        stage_name: &OsStr,
    ) -> Result<Self, NewProjectAuthorityError> {
        Self::create_for_template(
            parent_path,
            output_name,
            stage_name,
            TemplateInventory::Calculator,
        )
    }

    /// Create the held authority for the closed library inventory. The
    /// caller selects the template before this filesystem boundary.
    pub fn create_library(
        parent_path: &Path,
        output_name: &OsStr,
        stage_name: &OsStr,
    ) -> Result<Self, NewProjectAuthorityError> {
        Self::create_for_template(
            parent_path,
            output_name,
            stage_name,
            TemplateInventory::Library,
        )
    }

    /// Hold the service's three source files and six root files as one exact
    /// staged publication inventory.
    pub fn create_service(
        parent_path: &Path,
        output_name: &OsStr,
        stage_name: &OsStr,
    ) -> Result<Self, NewProjectAuthorityError> {
        Self::create_for_template(
            parent_path,
            output_name,
            stage_name,
            TemplateInventory::Service,
        )
    }

    fn create_for_template(
        parent_path: &Path,
        output_name: &OsStr,
        stage_name: &OsStr,
        template: TemplateInventory,
    ) -> Result<Self, NewProjectAuthorityError> {
        let parent = platform::hold_directory(parent_path).map_err(map_changed)?;
        let output = platform::prepare_child_name(output_name).map_err(map_invalid)?;
        if !platform::child_absent_prepared(&parent, &output).map_err(map_changed)? {
            return Err(NewProjectAuthorityError::Exists);
        }
        let prepared_stage = platform::prepare_stage_name(stage_name).map_err(map_invalid)?;
        if stage_name
            .as_encoded_bytes()
            .eq_ignore_ascii_case(output_name.as_encoded_bytes())
        {
            return Err(NewProjectAuthorityError::Invalid);
        }
        let stage_name = prepared_stage;
        let source_name = platform::prepare_stage_name(OsStr::new("src")).map_err(map_invalid)?;
        let empty = platform::prepare_discard_inventory([]).map_err(map_invalid)?;
        let root = match template {
            TemplateInventory::Service => RootInventory::Service(Box::new(
                platform::prepare_discard_inventory(SERVICE_ROOT_NAMES.map(OsStr::new))
                    .map_err(map_invalid)?,
            )),
            _ => RootInventory::Basic(Box::new(
                platform::prepare_discard_inventory(ROOT_NAMES.map(OsStr::new))
                    .map_err(map_invalid)?,
            )),
        };
        let source_files = match template {
            TemplateInventory::Library => SourceInventory::Library(
                platform::prepare_discard_inventory(LIBRARY_SOURCE_NAMES.map(OsStr::new))
                    .map_err(map_invalid)?,
            ),
            TemplateInventory::Calculator => SourceInventory::Calculator(
                platform::prepare_discard_inventory(CALCULATOR_SOURCE_NAMES.map(OsStr::new))
                    .map_err(map_invalid)?,
            ),
            TemplateInventory::Service => SourceInventory::Service(
                platform::prepare_discard_inventory(CALCULATOR_SOURCE_NAMES.map(OsStr::new))
                    .map_err(map_invalid)?,
            ),
        };
        // Prepare expected namespace bindings before creating any directory.
        let parent_path = parent_path.to_path_buf();
        let output_path = parent_path.join(output_name);
        let output_name = output_name.to_os_string();
        let stage = platform::create_directory_new_prepared(&parent, &stage_name, 0o700).map_err(
            |error| {
                if error == platform::Error::Exists {
                    NewProjectAuthorityError::StageExists
                } else {
                    NewProjectAuthorityError::Changed
                }
            },
        )?;
        let source = match platform::create_directory_new_prepared(&stage, &source_name, 0o700) {
            Ok(source) => source,
            Err(error) => {
                let _ =
                    platform::discard_owned_stage_prepared(&parent, &stage, &stage_name, &empty);
                return Err(map_create(error));
            }
        };
        Ok(Self {
            parent,
            stage,
            source: Some(source),
            stage_name,
            source_name,
            output_name,
            parent_path,
            output_path,
            root,
            source_files,
            published: false,
            #[cfg(test)]
            before_rename: None,
            #[cfg(test)]
            after_rename: None,
        })
    }

    pub fn ambient_paths_still_bind(
        &self,
        parent_path: &Path,
        stage_path: &Path,
    ) -> Result<bool, NewProjectAuthorityError> {
        let parent =
            platform::same_directory_path(&self.parent, parent_path).map_err(map_changed)?;
        let stage = platform::same_directory_path(&self.stage, stage_path).map_err(map_changed)?;
        Ok(parent && stage)
    }

    pub fn write(
        &mut self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<(), NewProjectAuthorityError> {
        let source = self
            .source
            .as_ref()
            .ok_or(NewProjectAuthorityError::Changed)?;
        // The root and source inventories have different arities, so each
        // branch names its own inventory instead of sharing one tuple.
        match (relative_path, &mut self.source_files) {
            ("README.md" | "AGENTS.md" | "semaprax.toml", _) => {
                write_root(&self.stage, &mut self.root, relative_path, bytes)
            }
            (
                "service-config.schema.json"
                | "service.config.json"
                | "service-host-adapter-request.json",
                SourceInventory::Service(_),
            ) => write_root(&self.stage, &mut self.root, relative_path, bytes),
            ("src/app.spx", SourceInventory::Calculator(files)) => {
                platform::write_file_new_prepared(source, files, "app.spx", bytes, 0o600)
            }
            ("src/core.spx", SourceInventory::Calculator(files)) => {
                platform::write_file_new_prepared(source, files, "core.spx", bytes, 0o600)
            }
            ("src/examples.spx", SourceInventory::Library(files)) => {
                platform::write_file_new_prepared(source, files, "examples.spx", bytes, 0o600)
            }
            ("src/lib.spx", SourceInventory::Library(files)) => {
                platform::write_file_new_prepared(source, files, "lib.spx", bytes, 0o600)
            }
            ("src/tests.spx", SourceInventory::Calculator(files)) => {
                platform::write_file_new_prepared(source, files, "tests.spx", bytes, 0o600)
            }
            ("src/tests.spx", SourceInventory::Library(files)) => {
                platform::write_file_new_prepared(source, files, "tests.spx", bytes, 0o600)
            }
            ("src/app.spx", SourceInventory::Service(files)) => {
                platform::write_file_new_prepared(source, files, "app.spx", bytes, 0o600)
            }
            ("src/core.spx", SourceInventory::Service(files)) => {
                platform::write_file_new_prepared(source, files, "core.spx", bytes, 0o600)
            }
            ("src/tests.spx", SourceInventory::Service(files)) => {
                platform::write_file_new_prepared(source, files, "tests.spx", bytes, 0o600)
            }
            _ => return Err(NewProjectAuthorityError::Invalid),
        }
        .map_err(map_create)
    }

    pub fn authenticate(&self, files: &[(&str, &[u8])]) -> Result<(), NewProjectAuthorityError> {
        let expected_names: &[&str] = match &self.source_files {
            SourceInventory::Calculator(_) => &[
                "README.md",
                "AGENTS.md",
                "semaprax.toml",
                "src/app.spx",
                "src/core.spx",
                "src/tests.spx",
            ],
            SourceInventory::Library(_) => &[
                "README.md",
                "AGENTS.md",
                "semaprax.toml",
                "src/examples.spx",
                "src/lib.spx",
                "src/tests.spx",
            ],
            SourceInventory::Service(_) => &[
                "README.md",
                "AGENTS.md",
                "semaprax.toml",
                "src/app.spx",
                "src/core.spx",
                "src/tests.spx",
                "service-config.schema.json",
                "service.config.json",
                "service-host-adapter-request.json",
            ],
        };
        if files
            .iter()
            .map(|(name, _)| *name)
            .ne(expected_names.iter().copied())
        {
            return Err(NewProjectAuthorityError::Invalid);
        }
        let source = self
            .source
            .as_ref()
            .ok_or(NewProjectAuthorityError::Changed)?;
        authenticate_file(self.root.file("README.md")?, files[0].1)?;
        authenticate_file(self.root.file("AGENTS.md")?, files[1].1)?;
        authenticate_file(self.root.file("semaprax.toml")?, files[2].1)?;
        match &self.source_files {
            SourceInventory::Calculator(source_files) | SourceInventory::Service(source_files) => {
                authenticate_file(
                    source_files.file("app.spx").map_err(map_changed)?,
                    files[3].1,
                )?;
                authenticate_file(
                    source_files.file("core.spx").map_err(map_changed)?,
                    files[4].1,
                )?;
                authenticate_file(
                    source_files.file("tests.spx").map_err(map_changed)?,
                    files[5].1,
                )?;
            }
            SourceInventory::Library(source_files) => {
                authenticate_file(
                    source_files.file("examples.spx").map_err(map_changed)?,
                    files[3].1,
                )?;
                authenticate_file(
                    source_files.file("lib.spx").map_err(map_changed)?,
                    files[4].1,
                )?;
                authenticate_file(
                    source_files.file("tests.spx").map_err(map_changed)?,
                    files[5].1,
                )?;
            }
        }
        if matches!(self.source_files, SourceInventory::Service(_)) {
            for (index, name) in SERVICE_ROOT_NAMES.iter().enumerate().skip(3) {
                authenticate_file(self.root.file(name)?, files[index + 3].1)?;
            }
        }
        match &self.source_files {
            SourceInventory::Calculator(source_files) | SourceInventory::Service(source_files) => {
                let mut source_scan =
                    platform::prepare_inventory_exact(source_files).map_err(map_invalid)?;
                platform::inventory_exact_prepared(&mut source_scan, source, source_files)
                    .map_err(map_changed)?;
            }
            SourceInventory::Library(source_files) => {
                let mut source_scan =
                    platform::prepare_inventory_exact(source_files).map_err(map_invalid)?;
                platform::inventory_exact_prepared(&mut source_scan, source, source_files)
                    .map_err(map_changed)?;
            }
        }
        match &self.root {
            RootInventory::Basic(root) => {
                let mut scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("README.md"),
                        OsStr::new("AGENTS.md"),
                        OsStr::new("semaprax.toml"),
                        OsStr::new("src"),
                    ],
                    3,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut scan,
                    &self.stage,
                    [
                        root.file("README.md").map_err(map_changed)?,
                        root.file("AGENTS.md").map_err(map_changed)?,
                        root.file("semaprax.toml").map_err(map_changed)?,
                    ],
                    [source],
                )
                .map_err(map_changed)
            }
            RootInventory::Service(root) => {
                let mut scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("README.md"),
                        OsStr::new("AGENTS.md"),
                        OsStr::new("semaprax.toml"),
                        OsStr::new("service-config.schema.json"),
                        OsStr::new("service.config.json"),
                        OsStr::new("service-host-adapter-request.json"),
                        OsStr::new("src"),
                    ],
                    6,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut scan,
                    &self.stage,
                    [
                        root.file("README.md").map_err(map_changed)?,
                        root.file("AGENTS.md").map_err(map_changed)?,
                        root.file("semaprax.toml").map_err(map_changed)?,
                        root.file("service-config.schema.json")
                            .map_err(map_changed)?,
                        root.file("service.config.json").map_err(map_changed)?,
                        root.file("service-host-adapter-request.json")
                            .map_err(map_changed)?,
                    ],
                    [source],
                )
                .map_err(map_changed)
            }
        }
    }

    pub fn publish_and_verify(
        &mut self,
        files: &[(&str, &[u8])],
    ) -> Result<(), NewProjectAuthorityError> {
        self.authenticate(files)?;
        match &mut self.source_files {
            SourceInventory::Calculator(source_files) => source_files.settle_for_publish(),
            SourceInventory::Library(source_files) => source_files.settle_for_publish(),
            SourceInventory::Service(source_files) => source_files.settle_for_publish(),
        }
        .map_err(map_changed)?;
        self.root.settle_for_publish()?;
        drop(self.source.take());
        let mut publish =
            platform::prepare_publish_directory(&self.output_name).map_err(map_invalid)?;
        #[cfg(test)]
        if let Some(hook) = self.before_rename.take() {
            hook();
        }
        let published = platform::publish_directory_new_prepared(
            &mut publish,
            &self.parent,
            &self.stage,
            &self.stage_name,
            &self.output_name,
        );
        if published.is_ok() {
            // A successful rename irrevocably ends staging cleanup authority.
            self.published = true;
            #[cfg(test)]
            if let Some(hook) = self.after_rename.take() {
                hook();
            }
        }
        // Descendant release ended our original source-directory ownership.
        // A failed rename must not reacquire cleanup authority from whatever
        // now occupies `src`, even if its files retain the original identities.
        // Leave inert staging residue and preserve the selected publish error.
        published.map_err(map_create)?;
        self.source = Some(
            platform::hold_child_directory(&self.stage, OsStr::new("src")).map_err(map_changed)?,
        );
        self.authenticate_published(files)?;
        if !self.ambient_paths_still_bind(&self.parent_path, &self.output_path)? {
            return Err(NewProjectAuthorityError::Changed);
        }
        Ok(())
    }

    fn authenticate_published(
        &self,
        files: &[(&str, &[u8])],
    ) -> Result<(), NewProjectAuthorityError> {
        let source = self
            .source
            .as_ref()
            .ok_or(NewProjectAuthorityError::Changed)?;
        let readme = hold_matching(&self.stage, OsStr::new("README.md"), files[0].1)?;
        let agents = hold_matching(&self.stage, OsStr::new("AGENTS.md"), files[1].1)?;
        let manifest = hold_matching(&self.stage, OsStr::new("semaprax.toml"), files[2].1)?;
        match &self.source_files {
            SourceInventory::Calculator(_) | SourceInventory::Service(_) => {
                let app = hold_matching(source, OsStr::new("app.spx"), files[3].1)?;
                let core = hold_matching(source, OsStr::new("core.spx"), files[4].1)?;
                let tests = hold_matching(source, OsStr::new("tests.spx"), files[5].1)?;
                let mut source_scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("app.spx"),
                        OsStr::new("core.spx"),
                        OsStr::new("tests.spx"),
                    ],
                    3,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut source_scan,
                    source,
                    [&app, &core, &tests],
                    [],
                )
                .map_err(map_changed)?;
            }
            SourceInventory::Library(_) => {
                let examples = hold_matching(source, OsStr::new("examples.spx"), files[3].1)?;
                let library = hold_matching(source, OsStr::new("lib.spx"), files[4].1)?;
                let tests = hold_matching(source, OsStr::new("tests.spx"), files[5].1)?;
                let mut source_scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("examples.spx"),
                        OsStr::new("lib.spx"),
                        OsStr::new("tests.spx"),
                    ],
                    3,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut source_scan,
                    source,
                    [&examples, &library, &tests],
                    [],
                )
                .map_err(map_changed)?;
            }
        }
        match &self.root {
            RootInventory::Basic(_) => {
                let mut scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("README.md"),
                        OsStr::new("AGENTS.md"),
                        OsStr::new("semaprax.toml"),
                        OsStr::new("src"),
                    ],
                    3,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut scan,
                    &self.stage,
                    [&readme, &agents, &manifest],
                    [source],
                )
                .map_err(map_changed)?;
            }
            RootInventory::Service(_) => {
                let schema = hold_matching(
                    &self.stage,
                    OsStr::new("service-config.schema.json"),
                    files[6].1,
                )?;
                let config =
                    hold_matching(&self.stage, OsStr::new("service.config.json"), files[7].1)?;
                let request = hold_matching(
                    &self.stage,
                    OsStr::new("service-host-adapter-request.json"),
                    files[8].1,
                )?;
                let mut scan = platform::prepare_inventory_entries_exact(
                    [
                        OsStr::new("README.md"),
                        OsStr::new("AGENTS.md"),
                        OsStr::new("semaprax.toml"),
                        OsStr::new("service-config.schema.json"),
                        OsStr::new("service.config.json"),
                        OsStr::new("service-host-adapter-request.json"),
                        OsStr::new("src"),
                    ],
                    6,
                )
                .map_err(map_invalid)?;
                platform::inventory_entries_exact_prepared(
                    &mut scan,
                    &self.stage,
                    [&readme, &agents, &manifest, &schema, &config, &request],
                    [source],
                )
                .map_err(map_changed)?;
            }
        }
        platform::recheck_directory(&self.parent).map_err(map_changed)?;
        platform::recheck_directory(&self.stage).map_err(map_changed)?;
        platform::recheck_directory(source).map_err(map_changed)
    }
}

fn write_root(
    stage: &platform::HeldDirectory,
    root: &mut RootInventory,
    name: &str,
    bytes: &[u8],
) -> Result<(), platform::Error> {
    match root {
        RootInventory::Basic(files) => {
            platform::write_file_new_prepared(stage, files, name, bytes, 0o600)
        }
        RootInventory::Service(files) => {
            platform::write_file_new_prepared(stage, files, name, bytes, 0o600)
        }
    }
}

impl Drop for NewProjectAuthority {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if self.source.as_ref().is_some_and(|source| {
            match &self.source_files {
                SourceInventory::Calculator(source_files) => {
                    platform::discard_owned_stage_prepared(
                        &self.stage,
                        source,
                        &self.source_name,
                        source_files,
                    )
                }
                SourceInventory::Library(source_files) => platform::discard_owned_stage_prepared(
                    &self.stage,
                    source,
                    &self.source_name,
                    source_files,
                ),
                SourceInventory::Service(source_files) => platform::discard_owned_stage_prepared(
                    &self.stage,
                    source,
                    &self.source_name,
                    source_files,
                ),
            }
            .is_ok()
        }) {
            match &self.root {
                RootInventory::Basic(root) => {
                    let _ = platform::discard_owned_stage_prepared(
                        &self.parent,
                        &self.stage,
                        &self.stage_name,
                        root,
                    );
                }
                RootInventory::Service(root) => {
                    let _ = platform::discard_owned_stage_prepared(
                        &self.parent,
                        &self.stage,
                        &self.stage_name,
                        root,
                    );
                }
            }
        }
    }
}

fn authenticate_file(
    file: &platform::HeldRegularFile,
    expected: &[u8],
) -> Result<(), NewProjectAuthorityError> {
    let mut scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
    if !platform::compare_exact(file, expected, &mut scratch).map_err(map_changed)? {
        return Err(NewProjectAuthorityError::Changed);
    }
    platform::recheck_regular_file(file).map_err(map_changed)
}

fn hold_matching(
    directory: &platform::HeldDirectory,
    name: &OsStr,
    expected: &[u8],
) -> Result<platform::HeldRegularFile, NewProjectAuthorityError> {
    let file = platform::hold_regular_file(directory, name).map_err(map_changed)?;
    authenticate_file(&file, expected)?;
    Ok(file)
}

fn map_create(error: platform::Error) -> NewProjectAuthorityError {
    if error == platform::Error::Exists {
        NewProjectAuthorityError::Exists
    } else {
        NewProjectAuthorityError::Changed
    }
}

fn map_changed(_error: platform::Error) -> NewProjectAuthorityError {
    NewProjectAuthorityError::Changed
}

fn map_invalid(_error: platform::Error) -> NewProjectAuthorityError {
    NewProjectAuthorityError::Invalid
}
