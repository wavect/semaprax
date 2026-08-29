//! Closed, authority-neutral Project profile admission over retained HIR.
//!
//! This is the sole Phase-A profile dispatcher. A prepared value records that
//! the schema-selected target surface was derived and independently replayed;
//! it carries no filesystem, process, publication, transport, or reusable
//! evidence authority.

mod flat_record;
mod legacy;
mod owned;

#[cfg(test)]
mod tests;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{
    FlatOwnedRecordApiDescriptor, ProjectManifest, ProjectProfile, PublicApiDescriptor,
    PublicApiSubject,
};

/// One completely admitted schema-selected Project profile.
///
/// The descriptors are retained only as authenticated Phase-A facts. Public
/// consumers must still replay their bytes against the retained HIR before
/// treating them as semantic input.
pub(super) enum PreparedProjectAdmission {
    ScalarV1,
    UsefulTextConsumerV1,
    UsefulDataV1,
    UsefulDataCommandV1,
    UsefulDataCommandV2,
    LanguageCommandIoV1,
    LineCommandIoV1,
    OwnedDataApiV1(Box<PublicApiDescriptor>),
    FlatOwnedRecordApiV1(Box<FlatOwnedRecordApiDescriptor>),
    OwnedUtf8ApiV1(Box<PublicApiDescriptor>),
}

impl PreparedProjectAdmission {
    pub(super) fn profile(&self) -> ProjectProfile {
        match self {
            Self::ScalarV1 => ProjectProfile::ScalarV1,
            Self::UsefulTextConsumerV1 => ProjectProfile::UsefulTextConsumerV1,
            Self::UsefulDataV1 => ProjectProfile::UsefulDataV1,
            Self::UsefulDataCommandV1 => ProjectProfile::UsefulDataCommandV1,
            Self::UsefulDataCommandV2 => ProjectProfile::UsefulDataCommandV2,
            Self::LanguageCommandIoV1 => ProjectProfile::LanguageCommandIoV1,
            Self::LineCommandIoV1 => ProjectProfile::LineCommandIoV1,
            Self::OwnedDataApiV1(_descriptor) => ProjectProfile::OwnedDataApiV1,
            Self::FlatOwnedRecordApiV1(_descriptor) => ProjectProfile::FlatOwnedRecordApiV1,
            Self::OwnedUtf8ApiV1(_descriptor) => ProjectProfile::OwnedUtf8ApiV1,
        }
    }

    pub(super) fn owned_descriptor(&self) -> Option<&PublicApiDescriptor> {
        match self {
            Self::OwnedDataApiV1(descriptor) | Self::OwnedUtf8ApiV1(descriptor) => {
                Some(descriptor.as_ref())
            }
            _ => None,
        }
    }

    pub(super) fn flat_record_descriptor(&self) -> Option<&FlatOwnedRecordApiDescriptor> {
        match self {
            Self::FlatOwnedRecordApiV1(descriptor) => Some(descriptor.as_ref()),
            _ => None,
        }
    }
}

/// Prepare exactly one manifest-selected profile from the authenticated linked
/// entry closure. Successful construction is the Project Phase-A admission
/// boundary; target bytes remain private and are discarded here.
pub(super) fn prepare(
    manifest: &ProjectManifest,
    program: &ResolvedProgram,
    subject: PublicApiSubject<'_>,
) -> Result<PreparedProjectAdmission, Diagnostic> {
    match manifest.project_profile() {
        ProjectProfile::ScalarV1 => {
            legacy::scalar(program, manifest.web_exports())?;
            Ok(PreparedProjectAdmission::ScalarV1)
        }
        ProjectProfile::UsefulTextConsumerV1 => {
            legacy::useful_text(program, manifest.web_exports())?;
            Ok(PreparedProjectAdmission::UsefulTextConsumerV1)
        }
        ProjectProfile::UsefulDataV1 => {
            legacy::useful_data(program, manifest.web_exports())?;
            Ok(PreparedProjectAdmission::UsefulDataV1)
        }
        ProjectProfile::UsefulDataCommandV1 => {
            legacy::useful_data_command_v1(program, manifest.web_exports())?;
            Ok(PreparedProjectAdmission::UsefulDataCommandV1)
        }
        ProjectProfile::UsefulDataCommandV2 => {
            legacy::useful_data_command_v2(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::UsefulDataCommandV2)
        }
        ProjectProfile::LanguageCommandIoV1 => {
            legacy::language_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::LanguageCommandIoV1)
        }
        ProjectProfile::LineCommandIoV1 => {
            legacy::line_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::LineCommandIoV1)
        }
        ProjectProfile::OwnedDataApiV1 => owned::prepare(program, manifest, subject)
            .map(Box::new)
            .map(PreparedProjectAdmission::OwnedDataApiV1),
        ProjectProfile::FlatOwnedRecordApiV1 => flat_record::prepare(program, manifest, subject)
            .map(Box::new)
            .map(PreparedProjectAdmission::FlatOwnedRecordApiV1),
        ProjectProfile::OwnedUtf8ApiV1 => owned::prepare(program, manifest, subject)
            .map(Box::new)
            .map(PreparedProjectAdmission::OwnedUtf8ApiV1),
    }
}
