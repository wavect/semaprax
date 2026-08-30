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
    source: Option<platform::HeldDirectory>,
    stage_name: platform::PreparedStageName,
    source_name: platform::PreparedStageName,
    output_name: OsString,
    parent_path: PathBuf,
    output_path: PathBuf,
    root: platform::PreparedDiscardInventory<2>,
    source_files: platform::PreparedDiscardInventory<2>,
    published: bool,
    #[cfg(test)]
    after_rename: Option<Box<dyn FnOnce()>>,
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
        let root =
            platform::prepare_discard_inventory(ROOT_NAMES.map(OsStr::new)).map_err(map_invalid)?;
        let source_files = platform::prepare_discard_inventory(SOURCE_NAMES.map(OsStr::new))
            .map_err(map_invalid)?;
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
        let (directory, inventory, name) = match relative_path {
            "README.md" => (&self.stage, &mut self.root, "README.md"),
            "semaprax.toml" => (&self.stage, &mut self.root, "semaprax.toml"),
            "src/app.spx" => (source, &mut self.source_files, "app.spx"),
            "src/tests.spx" => (source, &mut self.source_files, "tests.spx"),
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
        let source = self
            .source
            .as_ref()
            .ok_or(NewProjectAuthorityError::Changed)?;
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
        platform::inventory_exact_prepared(&mut source_scan, source, &self.source_files)
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
            [source],
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
        drop(self.source.take());
        let mut publish =
            platform::prepare_publish_directory(&self.output_name).map_err(map_invalid)?;
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
        self.source = Some(
            platform::hold_child_directory(&self.stage, OsStr::new("src")).map_err(map_changed)?,
        );
        published.map_err(map_create)?;
        self.authenticate_published(files)?;
        if !self.ambient_paths_still_bind(&self.parent_path, &self.output_path)? {
            return Err(NewProjectAuthorityError::Changed);
        }
        Ok(())
    }

    fn authenticate_published(
        &self,
        files: [(&str, &[u8]); 4],
    ) -> Result<(), NewProjectAuthorityError> {
        let source = self
            .source
            .as_ref()
            .ok_or(NewProjectAuthorityError::Changed)?;
        let readme = hold_matching(&self.stage, OsStr::new("README.md"), files[0].1)?;
        let manifest = hold_matching(&self.stage, OsStr::new("semaprax.toml"), files[1].1)?;
        let app = hold_matching(source, OsStr::new("app.spx"), files[2].1)?;
        let tests = hold_matching(source, OsStr::new("tests.spx"), files[3].1)?;
        let mut source_scan = platform::prepare_inventory_entries_exact(
            [OsStr::new("app.spx"), OsStr::new("tests.spx")],
            2,
        )
        .map_err(map_invalid)?;
        platform::inventory_entries_exact_prepared(&mut source_scan, source, [&app, &tests], [])
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
            [source],
        )
        .map_err(map_changed)?;
        platform::recheck_directory(&self.parent).map_err(map_changed)?;
        platform::recheck_directory(&self.stage).map_err(map_changed)?;
        platform::recheck_directory(source).map_err(map_changed)
    }
}

impl Drop for NewProjectAuthority {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if self.source.as_ref().is_some_and(|source| {
            platform::discard_owned_stage_prepared(
                &self.stage,
                source,
                &self.source_name,
                &self.source_files,
            )
            .is_ok()
        }) {
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
