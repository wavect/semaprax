//! Full held-input retention and actual repeated derivation, without execution authority.
use super::{derive_json_codec_plan, refusal, JsonCodecBundle, JsonCodecPlanEntry};
use crate::diagnostic::Diagnostic;
use crate::project::{ProjectSnapshot, MANIFEST_FILE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecRetainedInput {
    pub role: String,
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub source: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecRepeat {
    pub ordinal: usize,
    pub project_revision: String,
    pub bundle_bytes: usize,
    pub bundle_sha256: String,
}
/// Authority-neutral evidence. Version is compatibility, never binary provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonCodecRetainedReceipt {
    pub schema: String,
    pub compiler_package_version: String,
    pub compiler_binary_sha256: Option<String>,
    pub repeat_kind: String,
    pub independent_execution: bool,
    pub publication_policy: String,
    pub held_inputs: Vec<JsonCodecRetainedInput>,
    pub derivations: Vec<JsonCodecRepeat>,
    pub bundle: JsonCodecBundle,
}
fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn retain(snapshot: &mut ProjectSnapshot) -> Result<Vec<JsonCodecRetainedInput>, Vec<Diagnostic>> {
    if snapshot.held_sources.len() != snapshot.revision.manifest().sources().len()
        || snapshot.held_dependency_sources.len()
            != snapshot.revision.manifest().dependency_sources().len()
    {
        return Err(crate::project::authority::authentication(
            "codec receipt held inventory differs from manifest",
        ));
    }
    let mut total = 0usize;
    let mut inputs = Vec::new();
    let mut append = |role: &str, path: &str, held: &mut crate::project::authority::HeldFile| {
        let source = held.retain_exact_utf8(MAX_INPUT_BYTES - total)?;
        total += source.len();
        inputs.push(JsonCodecRetainedInput {
            role: role.to_owned(),
            path: path.to_owned(),
            bytes: source.len(),
            sha256: digest(source.as_bytes()),
            source,
        });
        Ok::<_, Vec<Diagnostic>>(())
    };
    append("manifest", MANIFEST_FILE, &mut snapshot.held_manifest)?;
    for (path, held) in snapshot
        .revision
        .manifest()
        .sources()
        .iter()
        .zip(&mut snapshot.held_sources)
    {
        append("declared_source", path, held)?;
    }
    for (dependency, held) in snapshot
        .revision
        .manifest()
        .dependency_sources()
        .iter()
        .zip(&mut snapshot.held_dependency_sources)
    {
        append("dependency_subject", dependency.path(), held)?;
    }
    Ok(inputs)
}

fn repeat_bundle(
    mut derive: impl FnMut() -> Result<JsonCodecBundle, Vec<Diagnostic>>,
) -> Result<(JsonCodecBundle, Vec<JsonCodecRepeat>), Vec<Diagnostic>> {
    let bundle = derive()?;
    let first =
        serde_json::to_vec(&bundle).map_err(|_| refusal("cannot encode codec repeat subject"))?;
    let mut derivations = vec![JsonCodecRepeat {
        ordinal: 1,
        project_revision: bundle.authored_project_revision.clone(),
        bundle_bytes: first.len(),
        bundle_sha256: digest(&first),
    }];
    // A real second ordinary derivation; no copied digest stands in for it.
    let repeated = derive()?;
    let second = serde_json::to_vec(&repeated)
        .map_err(|_| refusal("cannot encode repeated codec subject"))?;
    if first != second || bundle != repeated {
        return Err(refusal(
            "codec repeated derivation differs for the same retained revision",
        ));
    }
    derivations.push(JsonCodecRepeat {
        ordinal: 2,
        project_revision: bundle.authored_project_revision.clone(),
        bundle_bytes: second.len(),
        bundle_sha256: digest(&second),
    });
    Ok((bundle, derivations))
}

/// Acquires exactly the ordinary Project authority, retains original bytes, then
/// derives twice from one immutable revision. No subprocess or new byte-read authority is used.
pub fn with_authenticated_json_codec_receipt(
    manifest: &Path,
    entries: &[JsonCodecPlanEntry],
) -> Result<JsonCodecRetainedReceipt, Vec<Diagnostic>> {
    crate::project::with_authenticated_project(manifest, |snapshot| {
        let held_inputs = retain(snapshot)?;
        snapshot.with_authenticated_request(|snapshot| {
            let revision = snapshot.retain_revision();
            let (bundle, derivations) =
                repeat_bundle(|| derive_json_codec_plan(&revision, entries))?;
            let receipt = JsonCodecRetainedReceipt {
                schema: "semaprax.json-codec-retained-receipt.v1".to_owned(),
                compiler_package_version: env!("CARGO_PKG_VERSION").to_owned(),
                compiler_binary_sha256: None,
                repeat_kind: "same-process.same-retained-revision.v1".to_owned(),
                independent_execution: false,
                publication_policy: "explicit-new-artifact.no-overwrite.v1".to_owned(),
                held_inputs,
                derivations,
                bundle,
            };
            require_size(&receipt)?;
            Ok(receipt)
        })
    })
}
/// Replay always reacquires ordinary authority and rederives both complete
/// subjects; reminting digests or changing any field cannot authenticate evidence.
pub fn verify_authenticated_json_codec_receipt(
    manifest: &Path,
    entries: &[JsonCodecPlanEntry],
    receipt: &JsonCodecRetainedReceipt,
) -> Result<(), Vec<Diagnostic>> {
    require_size(receipt)?;
    if &with_authenticated_json_codec_receipt(manifest, entries)? != receipt {
        return Err(refusal(
            "retained codec receipt differs from complete authenticated replay",
        ));
    }
    Ok(())
}
fn require_size(receipt: &JsonCodecRetainedReceipt) -> Result<(), Vec<Diagnostic>> {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_RECEIPT_BYTES)
                .ok_or_else(|| std::io::Error::other("codec receipt exceeds bound"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Count(0), receipt)
        .map_err(|_| refusal("codec retained receipt exceeds 64 MiB"))
}
#[cfg(test)]
#[path = "retained_receipt/tests.rs"]
mod tests;
