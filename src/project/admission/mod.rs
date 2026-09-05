//! Closed, authority-neutral Project profile admission over retained HIR.
//!
//! This is the sole Phase-A profile dispatcher. A prepared value records that
//! the schema-selected target surface was derived and independently replayed;
//! it carries no filesystem, process, publication, transport, or reusable
//! evidence authority.

mod flat_record;
mod legacy;
mod native_callback;
mod nested_record;
mod owned;

#[cfg(test)]
mod tests;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{
    FlatOwnedRecordApiDescriptor, NestedOwnedRecordApiDescriptor, ProjectManifest, ProjectProfile,
    PublicApiDescriptor, PublicApiSubject, ScalarWitInterfaceArtifactV1,
};

/// One completely admitted schema-selected Project profile.
///
/// The descriptors are retained only as authenticated Phase-A facts. Public
/// consumers must still replay their bytes against the retained HIR before
/// treating them as semantic input.
pub(super) enum PreparedProjectAdmission {
    ScalarV1(Box<ScalarWitInterfaceArtifactV1>),
    /// A Project v1 closure that declares a Native Rust callback. It has no
    /// Web target and therefore no scalar WIT descriptor.
    ScalarNativeCallbackV1,
    UsefulTextConsumerV1,
    UsefulDataV1,
    UsefulDataCommandV1,
    UsefulDataCommandV2,
    LanguageCommandIoV1,
    LineCommandIoV1,
    NetworkCommandIoV1,
    HttpsCommandIoV1,
    OwnedDataApiV1(Box<PublicApiDescriptor>),
    FlatOwnedRecordApiV1(Box<FlatOwnedRecordApiDescriptor>),
    OwnedUtf8ApiV1(Box<PublicApiDescriptor>),
    NestedOwnedRecordApiV1(Box<NestedOwnedRecordApiDescriptor>),
}

impl PreparedProjectAdmission {
    pub(super) fn profile(&self) -> ProjectProfile {
        match self {
            Self::ScalarV1(_) | Self::ScalarNativeCallbackV1 => ProjectProfile::ScalarV1,
            Self::UsefulTextConsumerV1 => ProjectProfile::UsefulTextConsumerV1,
            Self::UsefulDataV1 => ProjectProfile::UsefulDataV1,
            Self::UsefulDataCommandV1 => ProjectProfile::UsefulDataCommandV1,
            Self::UsefulDataCommandV2 => ProjectProfile::UsefulDataCommandV2,
            Self::LanguageCommandIoV1 => ProjectProfile::LanguageCommandIoV1,
            Self::LineCommandIoV1 => ProjectProfile::LineCommandIoV1,
            Self::NetworkCommandIoV1 => ProjectProfile::NetworkCommandIoV1,
            Self::HttpsCommandIoV1 => ProjectProfile::HttpsCommandIoV1,
            Self::OwnedDataApiV1(_descriptor) => ProjectProfile::OwnedDataApiV1,
            Self::FlatOwnedRecordApiV1(_descriptor) => ProjectProfile::FlatOwnedRecordApiV1,
            Self::OwnedUtf8ApiV1(_descriptor) => ProjectProfile::OwnedUtf8ApiV1,
            Self::NestedOwnedRecordApiV1(_descriptor) => ProjectProfile::NestedOwnedRecordApiV1,
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

    pub(super) fn nested_record_descriptor(&self) -> Option<&NestedOwnedRecordApiDescriptor> {
        match self {
            Self::NestedOwnedRecordApiV1(descriptor) => Some(descriptor.as_ref()),
            _ => None,
        }
    }

    pub(super) fn scalar_wit_descriptor(&self) -> Option<&ScalarWitInterfaceArtifactV1> {
        match self {
            Self::ScalarV1(descriptor) => Some(descriptor.as_ref()),
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
        ProjectProfile::ScalarV1 if native_callback::declares_callback(program) => {
            native_callback::prepare(program, manifest.web_exports())?;
            Ok(PreparedProjectAdmission::ScalarNativeCallbackV1)
        }
        ProjectProfile::ScalarV1 => {
            legacy::scalar(program, manifest.web_exports())?;
            let scalar_subject = super::scalar_wit::ScalarWitSubject {
                project_name: manifest.name(),
                project_revision: subject.project_revision,
                workspace_revision: subject.workspace_revision,
                project_graph_digest: subject.project_graph_digest,
            };
            let descriptor = super::scalar_wit::derive_scalar_wit_interface_v1(
                program,
                manifest.web_exports(),
                scalar_subject,
            )?;
            super::scalar_wit::replay_scalar_wit_interface_v1(
                program,
                manifest.web_exports(),
                scalar_subject,
                &descriptor.canonical_bytes(),
                &descriptor.digest(),
            )
            .map(Box::new)
            .map(PreparedProjectAdmission::ScalarV1)
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
        ProjectProfile::NetworkCommandIoV1 => {
            legacy::network_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::NetworkCommandIoV1)
        }
        ProjectProfile::HttpsCommandIoV1 => {
            legacy::https_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::HttpsCommandIoV1)
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
        ProjectProfile::NestedOwnedRecordApiV1 => {
            nested_record::prepare(program, manifest, subject)
                .map(Box::new)
                .map(PreparedProjectAdmission::NestedOwnedRecordApiV1)
        }
    }
}
