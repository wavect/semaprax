//! Independent replay of a `change`-profile capsule against source, and the
//! emitter that produces one whose identities were re-derived rather than
//! asserted (issue #209).
//!
//! ## The gap this closes
//!
//! Everything else in [`super`] is a *structural* check: it proves a capsule
//! is internally consistent and that its retained objects' bytes still hash
//! to the digests the manifest records. That is necessary and insufficient.
//! A producer can hand-write a perfectly consistent capsule whose `subject`
//! names a `source_digest`, `root_digest`, and `revision` that no source
//! tree ever produced, and every structural check passes, because every
//! structural check only ever compares the capsule against itself.
//!
//! [`verify_change_capsule_against_source`] is the check that does not.
//! It re-parses the current source, re-verifies it, re-renders the canonical
//! projection and the semantic graph, and recomputes every identity from
//! scratch, then refuses when the capsule disagrees -- the same discipline
//! [`crate::assurance_manifest::proof_certificate::verify_certificate_against_source`]
//! applies to a proof certificate, for the same reason: a capsule's
//! self-reported fields are the claim under test, never the evidence for it.
//! Drift fails closed with `SPX-Z909`; nothing is repaired, and no partially
//! matching capsule is accepted "as far as it goes".
//!
//! ## Why the `change` profile
//!
//! Of the three profiles issue #209 names, only `change` has every identity
//! in its subject independently re-derivable here and now from bytes alone:
//! [`crate::parse`], [`crate::verify::verify`], [`crate::format::canonical`],
//! [`crate::graph::revision`], and [`crate::graph::to_json`] are all pure,
//! deterministic functions of the source text. An `agent-run` capsule's
//! subject names a `session_id` that only a live invocation can attest, and a
//! `release` capsule's subject names a `release_tag` and `commit` whose
//! authority lives in Git and a signing identity this repository does not
//! have (issue #168). Replaying those would mean inventing evidence; this
//! module replays the one subject whose evidence already exists.
//!
//! ## Still not authority
//!
//! A successful replay proves the capsule's identities match this exact
//! source. It is not permission to publish, tag, deploy, or sign anything,
//! and this module performs no such action: it opens no file, no socket, and
//! no subprocess. `current_source` arrives as a string the caller already
//! read, so replay cannot be a route to reading a path the caller was never
//! authorized to read.

use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;

use super::{
    nonclaims, parse_capsule, render_capsule, sha256_digest, verify_capsule, AssociationEdge,
    CapsuleVerificationReport, ObjectRef, Profile, SignatureEntry, SignaturePolicyContext,
    TransparencyContext, TransparencyEntry,
};

/// The schema id the canonical source projection object carries. This is
/// `crate::package_report_v2`'s existing canonical-source schema id, not a
/// new one: a capsule must never become a second, incompatible name for a
/// document this repository already names.
pub const SOURCE_PROJECTION_SCHEMA: &str = "semaprax.canonical-source.v1";

/// The object id [`emit_change_capsule`] gives the canonical source
/// projection it derives.
pub const SOURCE_PROJECTION_OBJECT_ID: &str = "derived-a-source-projection";

/// The object id [`emit_change_capsule`] gives the semantic graph document
/// it derives.
pub const PROGRAM_ROOT_OBJECT_ID: &str = "derived-b-program-root";

fn drift_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z909", message)
}

/// Every identity a `change`-profile capsule binds, recomputed from source
/// text alone.
#[derive(Debug, Clone)]
pub struct ChangeIdentities {
    /// The plain SHA-256 of the exact source bytes -- not of the canonical
    /// projection. A capsule is bound to the bytes that were actually
    /// reviewed, comments and all, so a reformatting that preserves meaning
    /// still registers as drift rather than passing silently.
    pub source_digest: String,
    /// The plain SHA-256 of the canonical semantic graph document bytes.
    pub root_digest: String,
    /// The semantic revision [`crate::graph::revision`] derives.
    pub revision: String,
    /// The compiler that derived all of the above.
    pub compiler_version: String,
    /// The canonical source projection text itself.
    pub canonical_source: String,
    /// The canonical semantic graph document text itself.
    pub graph_json: String,
    /// The version-selected `semaprax.graph.vNN` schema id the graph
    /// document declares for this exact program, read back out of the
    /// rendered document rather than assumed.
    pub graph_schema: String,
}

/// Recomputes every `change`-profile identity from `source`.
///
/// Refuses source that does not parse, or that no longer passes
/// [`crate::verify::verify`]: a capsule about source that does not compile
/// would be evidence of nothing, and accepting one would let a capsule
/// outlive the property it was assembled to attest.
///
/// This is a pure function of `source`. `source_path_label` is used only for
/// diagnostic rendering and is never opened.
pub fn derive_change_identities(
    source: &str,
    source_path_label: &str,
) -> Result<ChangeIdentities, Diagnostic> {
    let program = crate::parse(source, source_path_label)
        .map_err(|error| drift_error(format!("source does not parse: {}", error.message)))?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(drift_error(format!(
            "source does not pass verification ({} error diagnostic(s)); a capsule cannot be \
             bound to source that does not compile",
            diagnostics
                .iter()
                .filter(|item| item.severity.is_error())
                .count()
        )));
    }
    let canonical_source = crate::format::canonical(&program);
    let revision = crate::graph::revision(&program);
    let graph_json = crate::graph::to_json(&program).map_err(|diagnostics| {
        drift_error(format!(
            "source no longer projects to a semantic graph ({} diagnostic(s))",
            diagnostics.len()
        ))
    })?;
    let graph_value: serde_json::Value = serde_json::from_str(&graph_json)
        .map_err(|error| drift_error(format!("semantic graph is not valid JSON: {error}")))?;
    let graph_schema = graph_value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| drift_error("semantic graph declares no schema".to_owned()))?
        .to_owned();

    Ok(ChangeIdentities {
        source_digest: sha256_digest(source.as_bytes()),
        root_digest: sha256_digest(graph_json.as_bytes()),
        revision,
        compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
        canonical_source,
        graph_json,
        graph_schema,
    })
}

/// A rendered capsule: canonical manifest bytes plus the content-addressed
/// object set they reference, ready to be written side by side and verified
/// anywhere.
#[derive(Debug, Clone)]
pub struct ChangeCapsule {
    pub manifest_bytes: Vec<u8>,
    pub object_bytes: BTreeMap<String, Vec<u8>>,
}

/// Builds a `change`-profile capsule whose subject identities were
/// **re-derived from `source`** rather than accepted from a caller.
///
/// The caller supplies the evidence objects only this change knows about --
/// its semantic transaction, its assurance manifest, any test or build
/// evidence -- and their bytes; this function derives the canonical source
/// projection and the semantic graph document, derives the subject, and
/// records the derived-from edge between them. A caller therefore cannot
/// produce a capsule from this entry point whose `source_digest`,
/// `root_digest`, `revision`, or `compiler_version` disagrees with the
/// source it named, which is the property
/// [`verify_change_capsule_against_source`] later re-checks independently.
///
/// [`nonclaims::ALWAYS_REQUIRED_NONCLAIMS`] are always declared;
/// `extra_nonclaims` adds further ones from [`nonclaims::KNOWN_NONCLAIMS`].
/// `"identities-not-replayed-against-source"` is deliberately **not** added
/// here -- this emitter did re-derive them -- so a hand-assembled capsule
/// that declares it stays distinguishable from one this function produced.
pub fn emit_change_capsule(
    source: &str,
    source_path_label: &str,
    supplied_objects: &[ObjectRef],
    supplied_object_bytes: &BTreeMap<String, Vec<u8>>,
    associations: &[AssociationEdge],
    extra_nonclaims: &[String],
) -> Result<ChangeCapsule, Diagnostic> {
    let identities = derive_change_identities(source, source_path_label)?;

    for reserved in [SOURCE_PROJECTION_OBJECT_ID, PROGRAM_ROOT_OBJECT_ID] {
        if supplied_objects
            .iter()
            .any(|candidate| candidate.id == reserved)
        {
            return Err(drift_error(format!(
                "object id `{reserved}` is reserved for the identity this emitter derives; a \
                 supplied object may not occupy it and shadow the derived one"
            )));
        }
    }

    let mut source_binds = BTreeMap::new();
    source_binds.insert("source_digest".to_owned(), identities.source_digest.clone());
    let mut root_binds = BTreeMap::new();
    root_binds.insert("root_digest".to_owned(), identities.root_digest.clone());
    root_binds.insert("revision".to_owned(), identities.revision.clone());

    let mut objects = vec![
        ObjectRef {
            id: SOURCE_PROJECTION_OBJECT_ID.to_owned(),
            object_type: "source-projection".to_owned(),
            schema: SOURCE_PROJECTION_SCHEMA.to_owned(),
            digest: sha256_digest(identities.canonical_source.as_bytes()),
            redacted: false,
            redaction_reason: None,
            binds: source_binds,
        },
        ObjectRef {
            id: PROGRAM_ROOT_OBJECT_ID.to_owned(),
            object_type: "program-root".to_owned(),
            schema: identities.graph_schema.clone(),
            digest: identities.root_digest.clone(),
            redacted: false,
            redaction_reason: None,
            binds: root_binds,
        },
    ];
    objects.extend(supplied_objects.iter().cloned());

    let mut edges = vec![AssociationEdge {
        from_id: PROGRAM_ROOT_OBJECT_ID.to_owned(),
        relation: "derived_from".to_owned(),
        to_id: SOURCE_PROJECTION_OBJECT_ID.to_owned(),
    }];
    edges.extend(associations.iter().cloned());

    let mut subject = BTreeMap::new();
    subject.insert("source_digest".to_owned(), identities.source_digest.clone());
    subject.insert("root_digest".to_owned(), identities.root_digest.clone());
    subject.insert("revision".to_owned(), identities.revision.clone());
    subject.insert(
        "compiler_version".to_owned(),
        identities.compiler_version.clone(),
    );

    let mut declared: Vec<String> = nonclaims::ALWAYS_REQUIRED_NONCLAIMS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    declared.extend(extra_nonclaims.iter().cloned());
    if objects.iter().any(|candidate| candidate.redacted) {
        declared.push("redacted-objects-withhold-facts".to_owned());
    }

    let manifest_bytes = render_capsule(
        Profile::Change,
        &subject,
        &objects,
        &edges,
        &[] as &[SignatureEntry],
        None::<&TransparencyEntry>,
        &nonclaims::canonical_nonclaims(&declared),
    )?;

    let mut object_bytes = supplied_object_bytes.clone();
    object_bytes.insert(
        SOURCE_PROJECTION_OBJECT_ID.to_owned(),
        identities.canonical_source.into_bytes(),
    );
    object_bytes.insert(
        PROGRAM_ROOT_OBJECT_ID.to_owned(),
        identities.graph_json.into_bytes(),
    );

    Ok(ChangeCapsule {
        manifest_bytes,
        object_bytes,
    })
}

/// Verifies a `change`-profile capsule structurally **and** against the
/// source it claims to describe, failing closed on any drift (`SPX-Z909`).
///
/// Order matters: [`verify_capsule`] runs first, so a capsule that is not
/// even internally consistent is rejected by its own structural diagnostic
/// rather than by a confusing drift message. Only then is every subject
/// identity recomputed from `current_source` and compared. The capsule's own
/// fields are never used as the source of truth for anything they assert
/// about themselves.
///
/// Returns the same [`CapsuleVerificationReport`] [`verify_capsule`] does --
/// a report, never a capability. Nothing about a successful return
/// authorizes publishing, tagging, deploying, or signing the change this
/// capsule describes.
pub fn verify_change_capsule_against_source(
    manifest_bytes: &[u8],
    object_bytes: &BTreeMap<String, Vec<u8>>,
    current_source: &str,
    source_path_label: &str,
    signature_ctx: &SignaturePolicyContext,
    transparency_ctx: &TransparencyContext,
) -> Result<CapsuleVerificationReport, Diagnostic> {
    let report = verify_capsule(
        manifest_bytes,
        object_bytes,
        signature_ctx,
        transparency_ctx,
    )?;
    let capsule = parse_capsule(manifest_bytes)?;
    if capsule.profile != Profile::Change {
        return Err(drift_error(format!(
            "only a `change` capsule can be replayed against source; this capsule declares \
             profile `{}`",
            capsule.profile.as_str()
        )));
    }

    let claimed = |key: &str| -> String {
        capsule
            .subject
            .get(key)
            .cloned()
            .unwrap_or_else(|| "<absent>".to_owned())
    };

    // Toolchain drift first: a different compiler can legitimately derive a
    // different revision and graph, so comparing identities across versions
    // would report a misleading source-drift failure for what is really a
    // toolchain change (the same ordering
    // `verify_certificate_against_source` uses).
    let claimed_compiler = claimed("compiler_version");
    if claimed_compiler != env!("CARGO_PKG_VERSION") {
        return Err(drift_error(format!(
            "capsule was emitted by compiler version `{claimed_compiler}`, but this replay is \
             running compiler version `{}`; a capsule must be re-emitted rather than trusted \
             across a toolchain change",
            env!("CARGO_PKG_VERSION"),
        )));
    }

    let identities = derive_change_identities(current_source, source_path_label)?;

    for (key, recomputed) in [
        ("source_digest", &identities.source_digest),
        ("root_digest", &identities.root_digest),
        ("revision", &identities.revision),
    ] {
        let claimed_value = claimed(key);
        if &claimed_value != recomputed {
            return Err(drift_error(format!(
                "capsule subject claims `{key}` = `{claimed_value}`, but replaying the current \
                 source independently derives `{recomputed}`; the source drifted after the \
                 capsule was emitted, or the capsule's subject was never derived from this \
                 source at all"
            )));
        }
    }

    // Bind replay to the capsule's required source-derived object *types*,
    // never to the emitter's conventional ids (issue #583): an object id is
    // an arbitrary caller-chosen label, so looking up only
    // `SOURCE_PROJECTION_OBJECT_ID`/`PROGRAM_ROOT_OBJECT_ID` let a renamed,
    // substituted object escape this comparison entirely. `verify_capsule`
    // already proved each required type occurs exactly once and that every
    // retained object has integrity-checked bytes; a redacted one stays
    // explicitly unavailable in the report rather than replayed.
    for (object_type, expected_schema, expected, description) in [
        (
            "source-projection",
            SOURCE_PROJECTION_SCHEMA,
            identities.canonical_source.as_bytes(),
            "canonical source projection",
        ),
        (
            "program-root",
            identities.graph_schema.as_str(),
            identities.graph_json.as_bytes(),
            "semantic graph document",
        ),
    ] {
        let Some(object) = capsule
            .objects
            .iter()
            .find(|candidate| candidate.object_type == object_type)
        else {
            return Err(drift_error(format!(
                "a `change` capsule must carry a `{object_type}` object for replay to compare \
                 against the current source, but none is present"
            )));
        };
        if object.redacted {
            continue;
        }
        let object_id = object.id.as_str();
        if object.schema != expected_schema {
            return Err(drift_error(format!(
                "object `{object_id}` of type `{object_type}` declares schema `{}`, but the \
                 {description} the current source renders uses schema `{expected_schema}`",
                object.schema
            )));
        }
        let Some(retained) = object_bytes.get(object_id) else {
            return Err(drift_error(format!(
                "object `{object_id}` of type `{object_type}` is retained, but no bytes were \
                 supplied for replay to compare against the {description}"
            )));
        };
        if retained.as_slice() != expected {
            return Err(drift_error(format!(
                "object `{object_id}` does not contain the {description} the current source \
                 deterministically renders; its bytes hash consistently with the manifest but \
                 describe a different program"
            )));
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests;
