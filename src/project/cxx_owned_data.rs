//! Deterministic, authority-free C/C++17 package for the admitted Project v8 API.

use sha2::{Digest, Sha256};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::digest_hex::LowerHex;

use super::{
    ProjectSnapshot, PublicApiDescriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

mod render;

pub const PROJECT_CXX_OWNED_DATA_PACKAGE_SCHEMA: &str =
    "semaprax.project-cxx-owned-data-package.v1";
pub const MAX_CXX_OWNED_DATA_PACKAGE_BYTES: usize = 4 * 1024 * 1024;
const DIGEST_DOMAIN: &[u8] = b"semaprax.project-cxx-owned-data-package.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CxxOwnedDataPackage {
    canonical: Vec<u8>,
    digest: String,
    descriptor: Vec<u8>,
    descriptor_digest: String,
    c_header: String,
    cxx_header: String,
    provider_c: String,
}

impl CxxOwnedDataPackage {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
    pub fn c_header(&self) -> &str {
        &self.c_header
    }
    pub fn cxx_header(&self) -> &str {
        &self.cxx_header
    }
    pub fn provider_c(&self) -> &str {
        &self.provider_c
    }
}

impl ProjectSnapshot {
    /// Derive a pathless Project-v8 C/C++ package while this Project remains held.
    pub fn cxx_owned_data_package_v1(&mut self) -> Result<CxxOwnedDataPackage, Vec<Diagnostic>> {
        self.recheck()?;
        if self.manifest().schema() != PUBLIC_OWNED_DATA_PROJECT_SCHEMA {
            return Err(vec![package_error(
                "C++ owned-data packaging requires the exact Project v8 profile",
            )]);
        }
        let descriptor = self.public_api_descriptor()?;
        let selected = self.manifest().web_exports();
        let subject = PublicApiSubject {
            project_schema: self.manifest().schema(),
            project_revision: self.project_revision(),
            workspace_revision: self.workspace_revision(),
            project_graph_digest: descriptor.project_graph_digest(),
        };
        let descriptor_bytes = descriptor.canonical_bytes();
        let descriptor_digest = descriptor.digest();
        let (provider, overflowed) =
            crate::bounded_output::with_limit(MAX_CXX_OWNED_DATA_PACKAGE_BYTES, || {
                crate::codegen::emit_project_v8_native_owned_data_provider(
                    self.entry_program(),
                    selected,
                    subject,
                    &descriptor_bytes,
                    &descriptor_digest,
                )
            });
        if overflowed {
            return Err(vec![package_error(
                "native provider exceeded its builder bound",
            )]);
        }
        let provider = provider.map_err(|error| vec![error])?;
        if provider.descriptor() != descriptor_bytes
            || provider.descriptor_digest() != descriptor_digest
        {
            return Err(vec![package_error(
                "native provider disagrees with the replayed descriptor",
            )]);
        }
        let package = build_package(&descriptor, provider.source())?;
        self.recheck()?;
        Ok(package)
    }

    /// Verify by regenerating from the currently held subject, never by trusting hashes.
    pub fn replay_cxx_owned_data_package_v1(
        &mut self,
        submitted: &[u8],
        digest: &str,
    ) -> Result<CxxOwnedDataPackage, Vec<Diagnostic>> {
        let expected = self.cxx_owned_data_package_v1()?;
        exact_replay(expected, submitted, digest).map_err(|error| vec![error])
    }
}

/// Exact replay for a package already derived from authenticated Project data.
pub fn replay_cxx_owned_data_package(
    expected: CxxOwnedDataPackage,
    submitted: &[u8],
    digest: &str,
) -> Result<CxxOwnedDataPackage, Diagnostic> {
    exact_replay(expected, submitted, digest)
}

fn build_package(
    descriptor: &PublicApiDescriptor,
    provider_c: &str,
) -> Result<CxxOwnedDataPackage, Vec<Diagnostic>> {
    if std::str::from_utf8(descriptor.canonical_bytes().as_slice()).is_err() {
        return Err(vec![package_error("public descriptor is not UTF-8")]);
    }
    let ((c_header, cxx_header), overflowed) =
        crate::bounded_output::with_limit(MAX_CXX_OWNED_DATA_PACKAGE_BYTES, || {
            (render::c_header(descriptor), render::cxx_header(descriptor))
        });
    if overflowed {
        return Err(vec![package_error(
            "C/C++ headers exceed their builder bound",
        )]);
    }
    let descriptor_bytes = descriptor.canonical_bytes();
    let descriptor_digest = descriptor.digest();
    let (text, overflowed) = crate::bounded_output::with_limit(
        MAX_CXX_OWNED_DATA_PACKAGE_BYTES,
        || {
            let mut text = CappedString::new();
            text.push_str("{\"schema\":");
            push_json(&mut text, PROJECT_CXX_OWNED_DATA_PACKAGE_SCHEMA);
            text.push_str(",\"project_schema\":");
            push_json(&mut text, descriptor.project_schema());
            text.push_str(",\"project_revision\":");
            push_json(&mut text, descriptor.project_revision());
            text.push_str(",\"workspace_revision\":");
            push_json(&mut text, descriptor.workspace_revision());
            text.push_str(",\"project_graph_digest\":");
            push_json(&mut text, descriptor.project_graph_digest());
            push_artifact(&mut text, "descriptor", &descriptor_bytes);
            push_artifact(&mut text, "c_header", c_header.as_bytes());
            push_artifact(&mut text, "cxx_header", cxx_header.as_bytes());
            push_artifact(&mut text, "provider_c", provider_c.as_bytes());
            text.push_str(",\"limits\":{\"borrowed_input_bytes\":65536,\"owned_output_bytes\":65536,\"package_bytes\":4194304}");
            text.push_str(",\"settlement\":{\"copy_before_drop\":true,\"drop_before_context_close\":true,\"failure_is_sticky\":true,\"uncertainty\":\"fail-stop\"}}");
            text.into_string()
        },
    );
    if overflowed || text.len() > MAX_CXX_OWNED_DATA_PACKAGE_BYTES {
        return Err(vec![package_error(
            "C++ owned-data package exceeds its exact byte limit",
        )]);
    }
    let canonical = text.into_bytes();
    Ok(CxxOwnedDataPackage {
        digest: domain_digest(&canonical),
        canonical,
        descriptor: descriptor_bytes,
        descriptor_digest,
        c_header,
        cxx_header,
        provider_c: provider_c.to_owned(),
    })
}

fn push_artifact(output: &mut CappedString, name: &str, bytes: &[u8]) {
    output.push_str(",\"");
    output.push_str(name);
    output.push_str("\":{\"length\":");
    use std::fmt::Write as _;
    let _ = write!(output, "{}", bytes.len());
    output.push_str(",\"sha256\":");
    push_json(output, &sha256(bytes));
    output.push_str(",\"text\":");
    match std::str::from_utf8(bytes) {
        Ok(text) => push_json(output, text),
        Err(_) => output.push_str("null"),
    }
    output.push('}');
}

fn exact_replay(
    expected: CxxOwnedDataPackage,
    submitted: &[u8],
    digest: &str,
) -> Result<CxxOwnedDataPackage, Diagnostic> {
    if submitted.len() > MAX_CXX_OWNED_DATA_PACKAGE_BYTES
        || digest != domain_digest(submitted)
        || submitted != expected.canonical_bytes()
        || digest != expected.digest()
    {
        return Err(package_error(
            "C++ owned-data package is not the exact authenticated derivation",
        ));
    }
    Ok(expected)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", LowerHex(Sha256::digest(bytes)))
}
fn domain_digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(DIGEST_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("{:x}", LowerHex(hash.finalize()))
}
fn package_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J141", message)
}

fn push_json(output: &mut CappedString, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output.push('"');
}
