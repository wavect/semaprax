//! Project-bound assurance over admitted source/HIR and explicit architecture claims.
//! This report never grants execution or publication authority.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::architecture_claims::ArchitectureClaimSet;
use crate::diagnostic::Diagnostic;
use crate::project::{with_authenticated_project, ProgramRoot, ProjectRevision, ProjectSnapshot};

use super::{
    AssuranceClass, AssuranceManifestOptions, MethodRecord, Obligation, ObligationKind,
    VerifiedProjectProof,
};

pub const SCHEMA: &str = "semaprax.project-assurance-manifest.v1";
const PAYLOAD_DOMAIN: &[u8] = b"semaprax.project-assurance-manifest.payload.v1\0";
const CLAIM_DOMAIN: &[u8] = b"semaprax.project-assurance-manifest.claims.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Caller-selected claims are checked facts, never ambient policy or external proof records.
#[derive(Clone, Debug)]
pub struct ProjectAssuranceOptions {
    pub max_bytes: usize,
    pub max_obligations: usize,
    claims: Option<ArchitectureClaimSet>,
}

impl Default for ProjectAssuranceOptions {
    fn default() -> Self {
        let defaults = AssuranceManifestOptions::default();
        Self {
            max_bytes: defaults.max_bytes,
            max_obligations: defaults.max_obligations,
            claims: None,
        }
    }
}
impl ProjectAssuranceOptions {
    pub fn new(max_bytes: usize, max_obligations: usize) -> std::result::Result<Self, Diagnostic> {
        AssuranceManifestOptions::new(max_bytes, max_obligations)?;
        Ok(Self {
            max_bytes,
            max_obligations,
            claims: None,
        })
    }
    #[must_use]
    pub fn with_claims(mut self, claims: ArchitectureClaimSet) -> Self {
        self.claims = Some(claims);
        self
    }
    fn validate(&self) -> Result<()> {
        AssuranceManifestOptions::new(self.max_bytes, self.max_obligations)
            .map(|_| ())
            .map_err(|error| vec![error])
    }
}

/// Authenticate all held Project inputs, derive once, and recheck before returning bytes.
pub fn generate(manifest: &Path, options: &ProjectAssuranceOptions) -> Result<String> {
    options.validate()?;
    with_authenticated_project(manifest, |snapshot| {
        derive(&snapshot.retain_revision(), options)
    })
}

/// Detect source/manifest drift before and after a request on an existing snapshot.
pub fn generate_from_snapshot(
    snapshot: &mut ProjectSnapshot,
    options: &ProjectAssuranceOptions,
) -> Result<String> {
    options.validate()?;
    snapshot.with_authenticated_request(|snapshot| derive(&snapshot.retain_revision(), options))
}

/// Generate Project assurance with opaque evidence that a proof backend has
/// already kernel-confirmed against this exact retained Project. The evidence
/// is rechecked against the current authenticated snapshot before it can add a
/// method; it is never accepted through the public single-file options bag.
pub fn generate_from_snapshot_with_verified_proofs(
    snapshot: &mut ProjectSnapshot,
    options: &ProjectAssuranceOptions,
    proofs: &[VerifiedProjectProof],
) -> Result<String> {
    options.validate()?;
    snapshot.with_authenticated_request(|snapshot| {
        derive_with_verified_proofs(&snapshot.retain_revision(), options, proofs)
    })
}

/// Derive from a retained immutable revision. This pure route does not claim current filesystem freshness.
pub fn derive(revision: &ProjectRevision, options: &ProjectAssuranceOptions) -> Result<String> {
    derive_with_verified_proofs(revision, options, &[])
}

/// Derive Project assurance and append only opaque kernel-confirmed proof
/// evidence whose Project revision, ProgramRoot, source row and certificate
/// association all still match this exact retained revision.
pub fn derive_with_verified_proofs(
    revision: &ProjectRevision,
    options: &ProjectAssuranceOptions,
    proofs: &[VerifiedProjectProof],
) -> Result<String> {
    options.validate()?;
    let workspace = revision.canonical_workspace_revision()?;
    let root = workspace.program_root()?;
    let programs = [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ];
    let selected: BTreeSet<&str> = programs
        .iter()
        .flat_map(|program| {
            program
                .functions
                .iter()
                .map(|function| function.id.as_str())
        })
        .collect();
    let mut owners = BTreeMap::new();
    let mut records = Records::new(options);
    let mut source_rows = Vec::new();
    let mut unselected = BTreeSet::new();
    let mut sources: Vec<_> = revision.sources().iter().collect();
    sources.sort_by(|left, right| left.path().cmp(right.path()));
    for source in sources {
        let mut program =
            crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
        // Class methods are authored functions too. Move their AST bodies into
        // the same derivation view without cloning recursive expression trees.
        for declaration in &mut program.types {
            if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
                program.functions.append(methods);
            }
        }
        // A stable source owner is unique across modules. Never silently coalesce identities.
        for id in program
            .functions
            .iter()
            .map(|function| function.stable_id.as_str())
            .chain(
                program
                    .interfaces
                    .iter()
                    .map(|interface| interface.stable_id.as_str()),
            )
        {
            if owners
                .insert(id.to_owned(), source.path().to_owned())
                .is_some()
            {
                return Err(invalid(
                    "project assurance source declaration identity is duplicated",
                ));
            }
        }
        for function in &program.functions {
            if !selected.contains(function.stable_id.as_str()) {
                unselected.insert(function.stable_id.clone());
            }
        }
        // Only source functions with an admitted ordinary HIR body justify source-level facts.
        // Provider sources need no synthetic main or standalone re-resolution.
        program
            .functions
            .retain(|function| selected.contains(function.stable_id.as_str()));
        // Interfaces were validated by Project admission; source identity binds their exact imports.
        for obligation in super::derive::derive_obligations(&program) {
            records.insert(obligation, Some(source.path()), false)?;
        }
        source_rows.push(json!({
            "path": source.path(), "source_digest": source.source_digest(),
            "source_revision": source.source_revision(),
        }));
    }
    let mut synthetic = BTreeSet::new();
    for program in programs {
        for obligation in
            super::derive::derive_resolved_obligations(program).map_err(|error| vec![error])?
        {
            let Some(path) = owners.get(&obligation.declaration_id) else {
                synthetic.insert(obligation.declaration_id.clone());
                continue;
            };
            // Entry, API and test closures may retain the same source function. Same fact,
            // source owner and method bytes are emitted once; disagreement is a hard error.
            records.insert(obligation, Some(path), true)?;
        }
    }
    records.attach_verified_proofs(proofs, revision, &root)?;
    let claims = match &options.claims {
        None => Value::Null,
        Some(claims) => {
            let result = claims.evaluate(revision)?;
            let result_value: Value = serde_json::from_str(result.to_json())
                .map_err(|_| invalid("checked architecture claim result is malformed"))?;
            let result_digest =
                super::render::domain_digest(CLAIM_DOMAIN, result.to_json().as_bytes());
            for claim in result_value["claims"]
                .as_array()
                .ok_or_else(|| invalid("checked architecture claim result has no claims"))?
            {
                if claim["status"] != "held" {
                    return Err(invalid(
                        "project assurance requires every requested architecture claim to be held",
                    ));
                }
                let id = claim["claim_id"]
                    .as_str()
                    .ok_or_else(|| invalid("architecture claim has no identity"))?;
                let operator = claim["operator"].as_str().unwrap_or("forbid_reaches");
                let from = claim[if operator == "protocol_realizers_bound" {
                    "protocol"
                } else {
                    "from"
                }]
                .as_str()
                .ok_or_else(|| invalid("architecture claim has no source"))?;
                let mut method = MethodRecord::new(
                    AssuranceClass::CompilerProved,
                    "semaprax-architecture-claims",
                    env!("CARGO_PKG_VERSION"),
                );
                method.inputs = vec![
                    revision.project_revision().to_owned(),
                    result_digest.clone(),
                ];
                method.detail = Some(if operator == "protocol_realizers_bound" {
                    "The requested protocol_realizers_bound claim held: every via target of the declared session protocol is a checked function node of the retained Project static call graph. It attests realizer binding only, not message or call order, and grants no execution, capability, or publication authority.".to_owned()
                } else {
                    "The requested forbid_reaches claim held over the retained Project static call graph; dynamic/external uncertainty is refused, and this is not execution or publication authority.".to_owned()
                });
                let obligation = Obligation::new(
                    ObligationKind::ArchitectureLaw,
                    from,
                    &format!("architecture:{operator}:{id}"),
                )
                .with_method(method);
                records.insert(obligation, owners.get(from).map(String::as_str), false)?;
            }
            result_value
        }
    };
    let selected_source_functions: Vec<_> = selected
        .iter()
        .filter_map(|id| {
            owners.get(*id).map(|path| {
                let views: Vec<_> = ["entry", "public_api", "test"]
                    .into_iter()
                    .zip(programs)
                    .filter_map(|(view, program)| {
                        program
                            .functions
                            .iter()
                            .any(|function| function.id.as_str() == *id)
                            .then_some(view)
                    })
                    .collect();
                json!({"declaration_id": id, "source_path": path, "hir_views": views})
            })
        })
        .collect();
    let payload = json!({
        "project_revision": revision.project_revision(),
        "workspace_revision": workspace.workspace_revision(),
        "program_root": root.program_root(),
        "sources": source_rows,
        "obligations": records.rows.into_values().map(|(_, value)| value).collect::<Vec<_>>(),
        "architecture_claims": claims,
        "coverage": {
            "hir_views": ["entry", "public_api", "test"],
            "selected_source_functions": selected_source_functions,
            "unselected_source_function_ids": unselected,
            "synthetic_function_ids_without_source_owner": synthetic,
        },
        "limits": { "max_bytes": options.max_bytes, "max_obligations": options.max_obligations },
        "nonclaims": ["no_target_or_test_execution", "no_external_proof_or_signature_authority", "caller_selected_claims_are_not_ambient_policy", "unselected_and_synthetic_functions_are_not_assured", "retained_revision_is_not_current_filesystem_authority"],
    });
    let encoded = canonical(payload.clone(), options.max_bytes)?;
    let payload_digest = super::render::domain_digest(PAYLOAD_DOMAIN, encoded.as_bytes());
    canonical(
        json!({"schema": SCHEMA, "payload": payload, "payload_digest": payload_digest}),
        options.max_bytes,
    )
}

/// Exact replay requires independent retained source and policy, never self-asserted envelope evidence.
pub fn verify_against_revision(
    document: &str,
    revision: &ProjectRevision,
    options: &ProjectAssuranceOptions,
) -> Result<()> {
    options.validate()?;
    if document.len() > options.max_bytes {
        return Err(capacity_detail(format!(
            "the document is {} bytes and max_bytes is {}",
            document.len(),
            options.max_bytes
        )));
    }
    let expected = derive(revision, options)?;
    if document != expected {
        return Err(vec![Diagnostic::io("SPX-Z104", "project assurance envelope differs from independently derived Project, claims or policy")]);
    }
    Ok(())
}

struct Records<'a> {
    options: &'a ProjectAssuranceOptions,
    rows: BTreeMap<String, (Obligation, Value)>,
    encoded_bytes: usize,
}
impl<'a> Records<'a> {
    fn new(options: &'a ProjectAssuranceOptions) -> Self {
        Self {
            options,
            rows: BTreeMap::new(),
            encoded_bytes: 0,
        }
    }
    fn insert(
        &mut self,
        obligation: Obligation,
        source: Option<&str>,
        repeated_view: bool,
    ) -> Result<()> {
        if let Some((previous, value)) = self.rows.get(&obligation.id) {
            if repeated_view && previous == &obligation && value["source_path"].as_str() == source {
                return Ok(());
            }
            return Err(invalid(
                "project assurance obligation identity collides or differs between HIR views",
            ));
        }
        if self.rows.len() >= self.options.max_obligations {
            return Err(capacity_detail(format!(
                "max_obligations is {} and this project reached it",
                self.options.max_obligations
            )));
        }
        let mut counts: Vec<_> = AssuranceClass::ALL
            .into_iter()
            .map(|class| (class, 0))
            .collect();
        let (encoded, overflowed) =
            crate::bounded_output::with_limit(self.options.max_bytes, || {
                super::render::render_obligation(&obligation, &mut counts)
            });
        if overflowed {
            return Err(capacity_detail(format!(
                "one obligation's rendering alone exceeds max_bytes {}",
                self.options.max_bytes
            )));
        }
        let mut value: Value = serde_json::from_str(&encoded)
            .map_err(|_| invalid("project assurance obligation cannot be encoded"))?;
        value
            .as_object_mut()
            .ok_or_else(|| invalid("project assurance obligation is not an object"))?
            .insert("source_path".into(), json!(source));
        self.encoded_bytes = self
            .encoded_bytes
            .checked_add(encoded.len())
            .and_then(|size| size.checked_add(source.map_or(0, str::len)))
            .ok_or_else(capacity)?;
        if self.encoded_bytes > self.options.max_bytes {
            return Err(capacity_detail(format!(
                "encoded obligations reached {} bytes and max_bytes is {}",
                self.encoded_bytes, self.options.max_bytes
            )));
        }
        self.rows.insert(obligation.id.clone(), (obligation, value));
        Ok(())
    }

    fn attach_verified_proofs(
        &mut self,
        proofs: &[VerifiedProjectProof],
        revision: &ProjectRevision,
        root: &ProgramRoot,
    ) -> Result<()> {
        let mut seen_certificates = BTreeSet::new();
        let mut seen_obligations = BTreeSet::new();
        for proof in proofs {
            if proof.project_revision != revision.project_revision()
                || proof.program_root != root.program_root()
            {
                return Err(invalid(
                    "kernel-confirmed proof belongs to a different retained Project revision or ProgramRoot",
                ));
            }
            let source = revision
                .sources()
                .iter()
                .find(|source| source.path() == proof.source_path)
                .ok_or_else(|| {
                    invalid("kernel-confirmed proof source is absent from the retained Project")
                })?;
            if source.source_revision() != proof.source_revision
                || source.source_digest() != proof.source_digest
            {
                return Err(invalid(
                    "kernel-confirmed proof source row differs from the retained Project",
                ));
            }
            if !seen_certificates.insert(proof.certificate_digest.as_str())
                || !seen_obligations.insert(proof.obligation_id.as_str())
            {
                return Err(invalid(
                    "kernel-confirmed proof repeats a certificate or postcondition obligation",
                ));
            }
            let (obligation, value) = self.rows.get_mut(&proof.obligation_id).ok_or_else(|| {
                invalid("kernel-confirmed proof targets no derived Project assurance obligation")
            })?;
            if obligation.declaration_id != proof.declaration_id
                || obligation.kind != ObligationKind::Postcondition
                || value["source_path"].as_str() != Some(proof.source_path.as_str())
            {
                return Err(invalid(
                    "kernel-confirmed proof does not match the exact Project source postcondition",
                ));
            }
            obligation.methods.push(proof.method.clone());
        }
        self.rerender_rows()
    }

    fn rerender_rows(&mut self) -> Result<()> {
        let rows: Vec<_> = self
            .rows
            .values()
            .map(|(obligation, value)| {
                (
                    obligation.clone(),
                    value["source_path"].as_str().map(str::to_owned),
                )
            })
            .collect();
        self.rows.clear();
        self.encoded_bytes = 0;
        for (obligation, source) in rows {
            self.insert(obligation, source.as_deref(), false)?;
        }
        Ok(())
    }
}

fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-Z101", message)]
}
/// Issue #271 left this half open: the refusal named neither which budget was
/// exceeded, nor by how much, nor the flag that raises it -- so
/// `semaprax new --template service` followed by
/// `semaprax project-assurance-manifest semaprax.toml` failed with nothing a
/// caller could act on. Its `SPX-Z101` sibling already states its valid range
/// ("max_bytes must be between ..."), and there is no reason a runtime
/// exhaustion should say less than an option rejection.
///
/// The code is unchanged, so anything matching on `SPX-Z102` keeps working.
fn capacity_detail(detail: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-Z102",
        format!("project assurance exceeds its obligation or output byte budget: {detail}"),
    )
    .with_help(
        "raise the budget with `--max-bytes N` or `--max-obligations N`, \
         or narrow the project",
    )]
}

fn capacity() -> Vec<Diagnostic> {
    capacity_detail("budget exhausted while encoding".to_owned())
}

fn canonical(mut value: Value, limit: usize) -> Result<String> {
    value.sort_all_objects();
    struct Capped {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Capped {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other(
                    "project assurance output byte budget",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Capped {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut output, &value).map_err(|_| capacity())?;
    output.write_all(b"\n").map_err(|_| capacity())?;
    String::from_utf8(output.bytes).map_err(|_| invalid("project assurance encoding is not UTF-8"))
}
