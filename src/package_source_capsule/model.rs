use crate::hir;
use crate::package_lock_v2::{Coordinate, PackageSourceSubject};

pub const SCHEMA: &str = "semaprax.offline-multi-package-source-capsule.v1";
pub const MIN_OUTPUT_BYTES: usize = 4 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_RENDER_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_PACKAGES: usize = 4;
pub const MIN_PACKAGES: usize = 2;
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_IMPORTS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSource {
    pub package: String,
    pub report: String,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCapsuleOptions {
    pub root_package: String,
    pub max_bytes: usize,
}

impl SourceCapsuleOptions {
    pub fn new(
        root_package: String,
        max_bytes: usize,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        let options = Self {
            root_package,
            max_bytes,
        };
        super::admission::validate_options(&options)?;
        Ok(options)
    }
}

impl Default for SourceCapsuleOptions {
    fn default() -> Self {
        Self {
            root_package: "app.main".to_owned(),
            max_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSourceCapsule {
    schema: String,
    digest: String,
    bytes: usize,
    source_set_digest: String,
    link_digest: String,
    root_package: String,
    packages: Vec<Coordinate>,
    source_revisions: Vec<(Coordinate, String)>,
    exports: Vec<String>,
}

impl VerifiedSourceCapsule {
    pub fn schema(&self) -> &str {
        &self.schema
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub const fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn source_set_digest(&self) -> &str {
        &self.source_set_digest
    }
    pub fn link_digest(&self) -> &str {
        &self.link_digest
    }
    pub fn root_package(&self) -> &str {
        &self.root_package
    }
    pub fn packages(&self) -> &[Coordinate] {
        &self.packages
    }
    pub fn source_revisions(&self) -> &[(Coordinate, String)] {
        &self.source_revisions
    }
    pub fn exports(&self) -> &[String] {
        &self.exports
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkedPackageSourceFact {
    pub(crate) coordinate: Coordinate,
    pub(crate) subject_digest: String,
    pub(crate) report_digest: String,
    pub(crate) interface_digest: String,
    pub(crate) interface_source_revision: String,
    pub(crate) source_revision: String,
    pub(crate) source_digest: String,
    pub(crate) source_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkedPackageImportFact {
    pub(crate) dependent: Coordinate,
    pub(crate) dependency: Coordinate,
    pub(crate) target: String,
    pub(crate) alias: String,
    pub(crate) ordinal: usize,
}

#[allow(
    dead_code,
    reason = "frozen crate-private receipt consumed by Offline Package Build v2"
)]
pub(crate) struct VerifiedLinkedSourceCapsule {
    pub(crate) receipt: VerifiedSourceCapsule,
    pub(crate) program: hir::ResolvedProgram,
    pub(crate) selected_subjects: Vec<PackageSourceSubject>,
    pub(crate) package_facts: Vec<LinkedPackageSourceFact>,
    pub(crate) import_facts: Vec<LinkedPackageImportFact>,
}

pub(crate) struct BuiltCapsule {
    pub(crate) json: String,
    pub(crate) receipt: VerifiedSourceCapsule,
    pub(crate) program: hir::ResolvedProgram,
    pub(crate) selected_subjects: Vec<PackageSourceSubject>,
    pub(crate) package_facts: Vec<LinkedPackageSourceFact>,
    pub(crate) import_facts: Vec<LinkedPackageImportFact>,
}

impl VerifiedSourceCapsule {
    pub(crate) fn new(
        digest: String,
        bytes: usize,
        source_set_digest: String,
        link_digest: String,
        root_package: String,
        packages: Vec<Coordinate>,
        source_revisions: Vec<(Coordinate, String)>,
        exports: Vec<String>,
    ) -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            digest,
            bytes,
            source_set_digest,
            link_digest,
            root_package,
            packages,
            source_revisions,
            exports,
        }
    }
}
