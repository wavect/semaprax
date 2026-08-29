//! Additive read-only Project Agent Transport v5 methods.
//!
//! Requests bind the already retained Project v8 subject and can select only
//! a non-widenable carrier byte ceiling. They cannot select paths, artifacts,
//! targets, tools, processes, or publication destinations.

use serde_json::{Map, Value};

use super::{
    codec, reject_unknown, take_optional_usize, RequestId, ServerProfile, Session, METHOD_NOT_FOUND,
};

const DEFAULT_INLINE_NPM_BYTES: usize = 8 * 1024 * 1024;

impl Session {
    pub(super) fn api_describe(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectOwnedDataV1 {
            return self.error(
                id,
                METHOD_NOT_FOUND,
                "method not found: project/api-describe",
            );
        }
        self.subject(id, params, |snapshot, params| {
            reject_unknown(&params)?;
            let descriptor = snapshot.public_api_descriptor()?;
            let canonical = String::from_utf8(descriptor.canonical_bytes()).map_err(|_| {
                super::parameter_diagnostic("canonical public API descriptor is not UTF-8")
            })?;
            let canonical = canonical.strip_suffix('\n').ok_or_else(|| {
                super::parameter_diagnostic("canonical public API descriptor lacks its terminator")
            })?;
            Ok(format!(
                "{{\"descriptor\":{canonical},\"descriptor_digest\":{}}}",
                crate::diagnostic::quote_json(&descriptor.digest()),
            ))
        })
    }

    pub(super) fn npm_build_inline(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        if self.profile != ServerProfile::ProjectOwnedDataV1 {
            return self.error(
                id,
                METHOD_NOT_FOUND,
                "method not found: project/npm-build-inline",
            );
        }
        let response_bytes = self.limits.response_bytes();
        self.subject(id, params, |snapshot, mut params| {
            // Derive and replay before target generation. The build path also
            // derives and replays the same descriptor, preventing target-local
            // signature rediscovery from becoming semantic authority.
            let descriptor = snapshot.public_api_descriptor()?;
            let canonical = String::from_utf8(descriptor.canonical_bytes()).map_err(|_| {
                super::parameter_diagnostic("canonical public API descriptor is not UTF-8")
            })?;
            let canonical = canonical.strip_suffix('\n').ok_or_else(|| {
                super::parameter_diagnostic("canonical public API descriptor lacks its terminator")
            })?;
            let response_without_carrier = codec::bounded_success_response(
                id,
                &format!(
                    "{{\"descriptor\":{canonical},\"descriptor_digest\":{},\"build\":}}",
                    crate::diagnostic::quote_json(&descriptor.digest()),
                ),
                usize::MAX,
            )
            .len();
            let carrier_response_allowance = response_bytes
                .checked_sub(response_without_carrier.saturating_add(1))
                .ok_or_else(|| {
                    super::parameter_diagnostic(
                        "configured response limit cannot contain the v5 descriptor wrapper",
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
            build
                .verify_public_api_descriptor(&descriptor)
                .map_err(|error| vec![error])?;
            Ok(format!(
                "{{\"descriptor\":{canonical},\"descriptor_digest\":{},\"build\":{}}}",
                crate::diagnostic::quote_json(&descriptor.digest()),
                build.envelope(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v5_success_response_boundary_is_exact_and_never_truncates() {
        let id = RequestId::Number(5);
        let result = "{\"descriptor\":{},\"descriptor_digest\":\"sha256:00\"}";
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
