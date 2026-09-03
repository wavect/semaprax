//! Additive, authority-free Project Agent Transport v6 methods.
//!
//! The retained manifest selects exactly one admitted Project v8-v11 public
//! API profile. Requests cannot select another profile, path, target, tool,
//! process, output, or publication destination.

use serde_json::{Map, Value};

use super::{codec, reject_unknown, take_optional_usize, RequestId, Session};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{
    FlatOwnedRecordApiDescriptor, NestedOwnedRecordApiDescriptor, ProjectNpmBuild, ProjectProfile,
    ProjectSnapshot, PublicApiDescriptor, FLAT_OWNED_RECORD_API_SCHEMA,
    NESTED_OWNED_RECORD_API_SCHEMA, PROJECT_NPM_BUILD_SCHEMA_V10, PROJECT_NPM_BUILD_SCHEMA_V7,
    PROJECT_NPM_BUILD_SCHEMA_V8, PROJECT_NPM_BUILD_SCHEMA_V9, PUBLIC_OWNED_DATA_API_SCHEMA,
    PUBLIC_OWNED_UTF8_API_SCHEMA,
};

const DEFAULT_INLINE_NPM_BYTES: usize = 8 * 1024 * 1024;

enum Descriptor {
    OwnedData(PublicApiDescriptor),
    FlatOwnedRecord(FlatOwnedRecordApiDescriptor),
    OwnedUtf8(PublicApiDescriptor),
    NestedOwnedRecord(NestedOwnedRecordApiDescriptor),
}

impl Descriptor {
    fn derive(snapshot: &ProjectSnapshot) -> Result<Self, Vec<Diagnostic>> {
        match snapshot.manifest().project_profile() {
            ProjectProfile::OwnedDataApiV1 => snapshot.public_api_descriptor().map(Self::OwnedData),
            ProjectProfile::FlatOwnedRecordApiV1 => snapshot
                .flat_owned_record_api_descriptor()
                .map(Self::FlatOwnedRecord),
            ProjectProfile::OwnedUtf8ApiV1 => {
                snapshot.owned_utf8_api_descriptor().map(Self::OwnedUtf8)
            }
            ProjectProfile::NestedOwnedRecordApiV1 => snapshot
                .nested_owned_record_api_descriptor()
                .map(Self::NestedOwnedRecord),
            _ => Err(super::parameter_diagnostic(
                "retained Project profile has no Agent Transport v6 public API",
            )),
        }
    }

    fn descriptor_schema(&self) -> &'static str {
        match self {
            Self::OwnedData(_) => PUBLIC_OWNED_DATA_API_SCHEMA,
            Self::FlatOwnedRecord(_) => FLAT_OWNED_RECORD_API_SCHEMA,
            Self::OwnedUtf8(_) => PUBLIC_OWNED_UTF8_API_SCHEMA,
            Self::NestedOwnedRecord(_) => NESTED_OWNED_RECORD_API_SCHEMA,
        }
    }

    fn carrier_schema(&self) -> &'static str {
        match self {
            Self::OwnedData(_) => PROJECT_NPM_BUILD_SCHEMA_V7,
            Self::FlatOwnedRecord(_) => PROJECT_NPM_BUILD_SCHEMA_V8,
            Self::OwnedUtf8(_) => PROJECT_NPM_BUILD_SCHEMA_V9,
            Self::NestedOwnedRecord(_) => PROJECT_NPM_BUILD_SCHEMA_V10,
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        match self {
            Self::OwnedData(descriptor) | Self::OwnedUtf8(descriptor) => {
                descriptor.canonical_bytes()
            }
            Self::FlatOwnedRecord(descriptor) => descriptor.canonical_bytes(),
            Self::NestedOwnedRecord(descriptor) => descriptor.canonical_bytes(),
        }
    }

    fn digest(&self) -> String {
        match self {
            Self::OwnedData(descriptor) | Self::OwnedUtf8(descriptor) => descriptor.digest(),
            Self::FlatOwnedRecord(descriptor) => descriptor.digest(),
            Self::NestedOwnedRecord(descriptor) => descriptor.digest(),
        }
    }

    fn verify_carrier(&self, build: &ProjectNpmBuild) -> Result<(), Diagnostic> {
        match self {
            Self::OwnedData(descriptor) => build.verify_public_api_descriptor(descriptor),
            Self::FlatOwnedRecord(descriptor) => {
                build.verify_flat_owned_record_api_descriptor(descriptor)
            }
            Self::OwnedUtf8(descriptor) => build.verify_owned_utf8_api_descriptor(descriptor),
            Self::NestedOwnedRecord(descriptor) => {
                build.verify_nested_owned_record_api_descriptor(descriptor)
            }
        }
    }
}

struct Description {
    project_schema: &'static str,
    descriptor: Descriptor,
    canonical: String,
    digest: String,
}

impl Description {
    fn derive(snapshot: &ProjectSnapshot) -> Result<Self, Vec<Diagnostic>> {
        let descriptor = Descriptor::derive(snapshot)?;
        let canonical = String::from_utf8(descriptor.canonical_bytes()).map_err(|_| {
            super::parameter_diagnostic("canonical public API descriptor is not UTF-8")
        })?;
        let canonical = canonical.strip_suffix('\n').ok_or_else(|| {
            super::parameter_diagnostic("canonical public API descriptor lacks its terminator")
        })?;
        let canonical = canonical.to_owned();
        let digest = descriptor.digest();
        Ok(Self {
            project_schema: snapshot.manifest().schema(),
            descriptor,
            canonical,
            digest,
        })
    }

    fn render(&self) -> String {
        render_fields(self, None)
    }
}

fn render_prefix(description: &Description) -> String {
    format!(
        "{{\"project_schema\":{},\"descriptor_schema\":{},\"carrier_schema\":{},\"descriptor\":{},\"descriptor_digest\":{}",
        quote_json(description.project_schema),
        quote_json(description.descriptor.descriptor_schema()),
        quote_json(description.descriptor.carrier_schema()),
        description.canonical,
        quote_json(&description.digest),
    )
}

fn render_fields(description: &Description, build: Option<&str>) -> String {
    let prefix = render_prefix(description);
    match build {
        Some(carrier) => format!("{prefix},\"build\":{carrier}}}"),
        None => format!("{prefix}}}"),
    }
}

impl Session {
    pub(super) fn public_api_describe(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        self.subject(id, params, |snapshot, params| {
            reject_unknown(&params)?;
            Ok(Description::derive(snapshot)?.render())
        })
    }

    pub(super) fn public_npm_build_inline(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        let response_bytes = self.limits.response_bytes();
        self.subject(id, params, |snapshot, mut params| {
            let description = Description::derive(snapshot)?;
            let response_without_carrier = codec::bounded_success_response(
                id,
                &format!("{},\"build\":}}", render_prefix(&description)),
                usize::MAX,
            )
            .len();
            let carrier_response_allowance = response_bytes
                .checked_sub(response_without_carrier.saturating_add(1))
                .ok_or_else(|| {
                    super::parameter_diagnostic(
                        "configured response limit cannot contain the v6 public API wrapper",
                    )
                })?;
            let requested = take_optional_usize(&mut params, "max_bytes")?;
            reject_unknown(&params)?;
            let max_bytes = requested
                .unwrap_or_else(|| DEFAULT_INLINE_NPM_BYTES.min(carrier_response_allowance));
            if max_bytes == 0
                || max_bytes > crate::project::MAX_PROJECT_NPM_BUILD_BYTES
                || max_bytes > carrier_response_allowance
            {
                return Err(super::parameter_diagnostic(
                    "max_bytes exceeds the fixed carrier or effective response limit",
                ));
            }
            let build = snapshot.build_npm_inline(max_bytes)?;
            build.verify().map_err(|error| vec![error])?;
            description
                .descriptor
                .verify_carrier(&build)
                .map_err(|error| vec![error])?;
            Ok(render_fields(&description, Some(build.envelope())))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v6_success_wrapper_budget_is_exact_and_never_truncates() {
        let id = RequestId::Number(6);
        let result = "{\"project_schema\":\"semaprax.project.v11\",\"descriptor_schema\":\"semaprax.public-nested-owned-record-api.v1\",\"carrier_schema\":\"semaprax.project-npm-build.v10\",\"descriptor\":{},\"descriptor_digest\":\"sha256:00\"}";
        let response = codec::bounded_success_response(&id, result, usize::MAX);
        assert_eq!(
            codec::bounded_success_response(&id, result, response.len() + 1),
            response
        );
        assert!(codec::is_overflow_response(
            &codec::bounded_success_response(&id, result, response.len())
        ));
    }
}
