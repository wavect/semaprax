//! Held filesystem authority for `semaprax new`.
//!
//! This lives beside the other native package publication code so the root
//! crate can reuse the safe cross-platform handle-relative platform facade
//! without acquiring raw platform authority or introducing unsafe code.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use semaprax_native_rust_interop_platform as platform;

const ROOT_NAMES: [&str; 2] = ["README.md", "semaprax.toml"];
const SOURCE_NAMES: [&str; 2] = ["app.spx", "tests.spx"];

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
    source: platform::HeldDirectory,
    stage_name: platform::PreparedStageName,
    source_name: platform::PreparedStageName,
    output_name: OsString,
    root: platform::PreparedDiscardInventory<2>,
    source_files: platform::PreparedDiscardInventory<2>,
    published: bool,
}

impl NewProjectAuthority {
    pub fn create(
        parent_path: &Path,
        output_name: &OsStr,
        stage_name: &OsStr,
    ) -> Result<Self, NewProjectAuthorityError> {
        let parent = platform::hold_directory(parent_path).map_err(map_changed)?;
        let output = platform::prepare_child_name(output_name).map_err(map_invalid)?;
        if !platform::child_absent_prepared(&parent, &output).map_err(map_changed)? {
            return Err(NewProjectAuthorityError::Exists);
        }
        let stage_name = platform::prepare_stage_name(stage_name).map_err(map_invalid)?;
        let source_name = platform::prepare_stage_name(OsStr::new("src")).map_err(map_invalid)?;
        let empty = platform::prepare_discard_inventory([]).map_err(map_invalid)?;
        let root =
            platform::prepare_discard_inventory(ROOT_NAMES.map(OsStr::new)).map_err(map_invalid)?;
        let source_files = platform::prepare_discard_inventory(SOURCE_NAMES.map(OsStr::new))
            .map_err(map_invalid)?;
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
            source,
            stage_name,
            source_name,
            output_name: output_name.to_os_string(),
            root,
            source_files,
            published: false,
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
        let (directory, inventory, name) = match relative_path {
            "README.md" => (&self.stage, &mut self.root, "README.md"),
            "semaprax.toml" => (&self.stage, &mut self.root, "semaprax.toml"),
            "src/app.spx" => (&self.source, &mut self.source_files, "app.spx"),
            "src/tests.spx" => (&self.source, &mut self.source_files, "tests.spx"),
            _ => return Err(NewProjectAuthorityError::Invalid),
        };
        platform::write_file_new_prepared(directory, inventory, name, bytes, 0o600)
            .map_err(map_create)
    }

    pub fn authenticate(&self, files: [(&str, &[u8]); 4]) -> Result<(), NewProjectAuthorityError> {
        if files.map(|(name, _)| name)
            != ["README.md", "semaprax.toml", "src/app.spx", "src/tests.spx"]
        {
            return Err(NewProjectAuthorityError::Invalid);
        }
        authenticate_file(
            self.root.file("README.md").map_err(map_changed)?,
            files[0].1,
        )?;
        authenticate_file(
            self.root.file("semaprax.toml").map_err(map_changed)?,
            files[1].1,
        )?;
        authenticate_file(
            self.source_files.file("app.spx").map_err(map_changed)?,
            files[2].1,
        )?;
        authenticate_file(
            self.source_files.file("tests.spx").map_err(map_changed)?,
            files[3].1,
        )?;
        let mut source_scan =
            platform::prepare_inventory_exact(&self.source_files).map_err(map_invalid)?;
        platform::inventory_exact_prepared(&mut source_scan, &self.source, &self.source_files)
            .map_err(map_changed)?;
        let mut root_scan = platform::prepare_inventory_entries_exact(
            [
                OsStr::new("README.md"),
                OsStr::new("semaprax.toml"),
                OsStr::new("src"),
            ],
            2,
        )
        .map_err(map_invalid)?;
        platform::inventory_entries_exact_prepared(
            &mut root_scan,
            &self.stage,
            [
                self.root.file("README.md").map_err(map_changed)?,
                self.root.file("semaprax.toml").map_err(map_changed)?,
            ],
            [&self.source],
        )
        .map_err(map_changed)
    }

    pub fn publish_and_verify(
        &mut self,
        files: [(&str, &[u8]); 4],
    ) -> Result<(), NewProjectAuthorityError> {
        self.authenticate(files)?;
        self.source_files
            .settle_for_publish()
            .map_err(map_changed)?;
        self.root.settle_for_publish().map_err(map_changed)?;
        let mut publish =
            platform::prepare_publish_directory(&self.output_name).map_err(map_invalid)?;
        platform::publish_directory_new_prepared(
            &mut publish,
            &self.parent,
            &self.stage,
            &self.stage_name,
            &self.output_name,
        )
        .map_err(map_create)?;
        self.published = true;
        self.authenticate_published(files)
    }

    fn authenticate_published(
        &self,
        files: [(&str, &[u8]); 4],
    ) -> Result<(), NewProjectAuthorityError> {
        let readme = hold_matching(&self.stage, OsStr::new("README.md"), files[0].1)?;
        let manifest = hold_matching(&self.stage, OsStr::new("semaprax.toml"), files[1].1)?;
        let app = hold_matching(&self.source, OsStr::new("app.spx"), files[2].1)?;
        let tests = hold_matching(&self.source, OsStr::new("tests.spx"), files[3].1)?;
        let mut source_scan = platform::prepare_inventory_entries_exact(
            [OsStr::new("app.spx"), OsStr::new("tests.spx")],
            2,
        )
        .map_err(map_invalid)?;
        platform::inventory_entries_exact_prepared(
            &mut source_scan,
            &self.source,
            [&app, &tests],
            [],
        )
        .map_err(map_changed)?;
        let mut root_scan = platform::prepare_inventory_entries_exact(
            [
                OsStr::new("README.md"),
                OsStr::new("semaprax.toml"),
                OsStr::new("src"),
            ],
            2,
        )
        .map_err(map_invalid)?;
        platform::inventory_entries_exact_prepared(
            &mut root_scan,
            &self.stage,
            [&readme, &manifest],
            [&self.source],
        )
        .map_err(map_changed)?;
        platform::recheck_directory(&self.parent).map_err(map_changed)?;
        platform::recheck_directory(&self.stage).map_err(map_changed)?;
        platform::recheck_directory(&self.source).map_err(map_changed)
    }
}

impl Drop for NewProjectAuthority {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if platform::discard_owned_stage_prepared(
            &self.stage,
            &self.source,
            &self.source_name,
            &self.source_files,
        )
        .is_ok()
        {
            let _ = platform::discard_owned_stage_prepared(
                &self.parent,
                &self.stage,
                &self.stage_name,
                &self.root,
            );
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
