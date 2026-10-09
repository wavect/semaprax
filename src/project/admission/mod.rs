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
mod public_generic_wasm;
mod source_local_future;
mod source_local_future_indexed_rust;
mod stdin_stream_command;

#[cfg(test)]
mod tests;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{
    FlatOwnedRecordApiDescriptor, NestedOwnedRecordApiDescriptor, ProjectManifest, ProjectProfile,
    PublicApiDescriptor, PublicApiSubject, ScalarWitInterfaceArtifactV1,
};
use crate::public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1;

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
    UsefulDataV2,
    UsefulDataCommandV1,
    UsefulDataCommandV2,
    LanguageCommandIoV1,
    StdinStreamCommandIoV1,
    StdinStreamCommandIoV2,
    StdinStreamTextCommandIoV1,
    StdinStreamDataCommandIoV1,
    StdinStreamDataCommandIoV2,
    LineCommandIoV1,
    NetworkCommandIoV1,
    HttpsCommandIoV1,
    FilesystemIoV1,
    FilesystemIoV2,
    FilesystemIoV3,
    EnvironmentIoV1,
    ProcessIoV1,
    SourceCommandV1,
    SourceCommandResourceOutputV1,
    /// An authenticated no-export alloc-tier standard package retains its
    /// internal owned closure without constructing a public descriptor.
    OwnedDataNoExports,
    OwnedDataApiV1(Box<PublicApiDescriptor>),
    FlatOwnedRecordApiV1(Box<FlatOwnedRecordApiDescriptor>),
    OwnedUtf8ApiV1(Box<PublicApiDescriptor>),
    NestedOwnedRecordApiV1(Box<NestedOwnedRecordApiDescriptor>),
    PublicGenericWasmProviderV1(Box<AdmittedPublicGenericEndpointV1>),
    SourceLocalFutureV1(Box<crate::resumable_effects::source_signature::SourceEffectSignature>),
    SourceLocalFutureIndexedRustV1(
        Box<crate::resumable_effects::source_signature::SourceEffectSignature>,
    ),
}

impl PreparedProjectAdmission {
    pub(super) fn profile(&self) -> ProjectProfile {
        match self {
            Self::ScalarV1(_) | Self::ScalarNativeCallbackV1 => ProjectProfile::ScalarV1,
            Self::UsefulTextConsumerV1 => ProjectProfile::UsefulTextConsumerV1,
            Self::UsefulDataV2 => ProjectProfile::UsefulDataV2,
            Self::UsefulDataV1 => ProjectProfile::UsefulDataV1,
            Self::UsefulDataCommandV1 => ProjectProfile::UsefulDataCommandV1,
            Self::UsefulDataCommandV2 => ProjectProfile::UsefulDataCommandV2,
            Self::LanguageCommandIoV1 => ProjectProfile::LanguageCommandIoV1,
            Self::StdinStreamCommandIoV1 => ProjectProfile::StdinStreamCommandIoV1,
            Self::StdinStreamCommandIoV2 => ProjectProfile::StdinStreamCommandIoV2,
            Self::StdinStreamTextCommandIoV1 => ProjectProfile::StdinStreamTextCommandIoV1,
            Self::StdinStreamDataCommandIoV1 => ProjectProfile::StdinStreamDataCommandIoV1,
            Self::StdinStreamDataCommandIoV2 => ProjectProfile::StdinStreamDataCommandIoV2,
            Self::LineCommandIoV1 => ProjectProfile::LineCommandIoV1,
            Self::NetworkCommandIoV1 => ProjectProfile::NetworkCommandIoV1,
            Self::FilesystemIoV1 => ProjectProfile::FilesystemIoV1,
            Self::EnvironmentIoV1 => ProjectProfile::EnvironmentIoV1,
            Self::ProcessIoV1 => ProjectProfile::ProcessIoV1,
            Self::SourceCommandV1 => ProjectProfile::SourceCommandV1,
            Self::SourceCommandResourceOutputV1 => ProjectProfile::SourceCommandResourceOutputV1,
            Self::FilesystemIoV2 => ProjectProfile::FilesystemIoV2,
            Self::FilesystemIoV3 => ProjectProfile::FilesystemIoV3,
            Self::HttpsCommandIoV1 => ProjectProfile::HttpsCommandIoV1,
            Self::OwnedDataNoExports | Self::OwnedDataApiV1(_) => ProjectProfile::OwnedDataApiV1,
            Self::FlatOwnedRecordApiV1(_descriptor) => ProjectProfile::FlatOwnedRecordApiV1,
            Self::OwnedUtf8ApiV1(_descriptor) => ProjectProfile::OwnedUtf8ApiV1,
            Self::NestedOwnedRecordApiV1(_descriptor) => ProjectProfile::NestedOwnedRecordApiV1,
            Self::PublicGenericWasmProviderV1(_) => ProjectProfile::PublicGenericWasmProviderV1,
            Self::SourceLocalFutureV1(_) => ProjectProfile::SourceLocalFutureV1,
            Self::SourceLocalFutureIndexedRustV1(_) => {
                ProjectProfile::SourceLocalFutureIndexedRustV1
            }
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

    pub(super) fn public_generic_wasm_provider_endpoint(
        &self,
    ) -> Option<&AdmittedPublicGenericEndpointV1> {
        match self {
            Self::PublicGenericWasmProviderV1(endpoint) => Some(endpoint.as_ref()),
            _ => None,
        }
    }

    pub(super) fn source_local_future_signature(
        &self,
    ) -> Option<&crate::resumable_effects::source_signature::SourceEffectSignature> {
        match self {
            Self::SourceLocalFutureV1(signature)
            | Self::SourceLocalFutureIndexedRustV1(signature) => Some(signature),
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
        ProjectProfile::SourceCommandV1 => {
            super::source_command::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::SourceCommandV1)
        }
        ProjectProfile::SourceCommandResourceOutputV1 => {
            super::source_command::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::SourceCommandResourceOutputV1)
        }
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
        ProjectProfile::UsefulDataV2 => {
            crate::hir::validate(program)?;
            if !manifest.web_exports().is_empty() {
                legacy::useful_data(program, manifest.web_exports())?;
            }
            Ok(PreparedProjectAdmission::UsefulDataV2)
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
        ProjectProfile::StdinStreamCommandIoV1 => {
            stdin_stream_command::admit(program, manifest.command().unwrap_or(""), false)?;
            Ok(PreparedProjectAdmission::StdinStreamCommandIoV1)
        }
        ProjectProfile::StdinStreamCommandIoV2 => {
            stdin_stream_command::admit(program, manifest.command().unwrap_or(""), true)?;
            Ok(PreparedProjectAdmission::StdinStreamCommandIoV2)
        }
        ProjectProfile::StdinStreamTextCommandIoV1 => {
            stdin_stream_command::admit(program, manifest.command().unwrap_or(""), true)?;
            Ok(PreparedProjectAdmission::StdinStreamTextCommandIoV1)
        }
        ProjectProfile::StdinStreamDataCommandIoV1 => {
            stdin_stream_command::admit(program, manifest.command().unwrap_or(""), true)?;
            Ok(PreparedProjectAdmission::StdinStreamDataCommandIoV1)
        }
        ProjectProfile::StdinStreamDataCommandIoV2 => {
            crate::hir::validate_stream_record_program(
                program,
                Some(&crate::hir::DeclarationId::new(
                    manifest.command().unwrap_or(""),
                )),
            )?;
            stdin_stream_command::admit(program, manifest.command().unwrap_or(""), true)?;
            Ok(PreparedProjectAdmission::StdinStreamDataCommandIoV2)
        }
        ProjectProfile::LineCommandIoV1 => {
            legacy::line_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::LineCommandIoV1)
        }
        ProjectProfile::NetworkCommandIoV1 => {
            legacy::network_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::NetworkCommandIoV1)
        }
        ProjectProfile::EnvironmentIoV1 => {
            super::environment::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::EnvironmentIoV1)
        }
        ProjectProfile::ProcessIoV1 => {
            super::process::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::ProcessIoV1)
        }
        ProjectProfile::FilesystemIoV2 => {
            super::filesystem::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::FilesystemIoV2)
        }
        ProjectProfile::FilesystemIoV3 => {
            super::filesystem::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::FilesystemIoV3)
        }
        ProjectProfile::FilesystemIoV1 => {
            super::filesystem::admit(program, manifest)?;
            Ok(PreparedProjectAdmission::FilesystemIoV1)
        }
        ProjectProfile::HttpsCommandIoV1 => {
            legacy::https_command(program, manifest.command().unwrap_or(""))?;
            Ok(PreparedProjectAdmission::HttpsCommandIoV1)
        }
        ProjectProfile::OwnedDataApiV1 if manifest.web_exports().is_empty() => {
            Ok(PreparedProjectAdmission::OwnedDataNoExports)
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
        ProjectProfile::PublicGenericWasmProviderV1 => {
            public_generic_wasm::prepare(program, manifest, subject)
                .map(Box::new)
                .map(PreparedProjectAdmission::PublicGenericWasmProviderV1)
        }
        ProjectProfile::SourceLocalFutureV1 => source_local_future::prepare(program, manifest)
            .map(Box::new)
            .map(PreparedProjectAdmission::SourceLocalFutureV1),
        ProjectProfile::SourceLocalFutureIndexedRustV1 => {
            source_local_future_indexed_rust::prepare(program, manifest)
                .map(Box::new)
                .map(PreparedProjectAdmission::SourceLocalFutureIndexedRustV1)
        }
    }
}
