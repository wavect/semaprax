//! LAW-09: exact, read-only trust frontier for one selected Rust import.
//!
//! A selected signature and an assumption are never a theorem about Rust code.
//! The report carries conditions; it does not change the LAW-06 solver profile.

use sha2::{Digest as _, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedImport;

use super::{valid_digest, verify_scalar_binding, ScalarBindingPlan};

pub const FOREIGN_LAW_SCHEMA: &str = "semaprax.foreign-law-frontier.v1";
pub const SCALAR_ADAPTER_SEMANTICS: &str = "semaprax.native-rust-scalar-adapter.v1";
const MAX_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignBoundary<'a> {
    /// Derived from the authenticated Project's canonical dependency lock.
    pub project_lock_digest: &'a str,
    /// Selected execution target, checked against the Rust binding plan.
    pub target: &'a str,
    /// Digest of the exact selected generated adapter, supplied by its owner.
    pub adapter_digest: &'a str,
}

/// These booleans are declarations about the foreign implementation, never
/// deductions from its signature, tests, Rust ownership, or a successful build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredForeignSummary {
    pub assumption_id: String,
    pub proposition_digest: String,
    pub assumes_no_effects: bool,
    pub assumes_no_callbacks: bool,
    pub assumes_no_panics: bool,
    pub assumes_no_shared_state: bool,
    /// An i64 return bound is enforced by `guard_i64_return` at a call boundary.
    pub return_i64_range: Option<(i64, i64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignLawRequest {
    pub law_id: String,
    pub permit_assumptions: bool,
    pub require_theorem: bool,
    pub require_no_effects: bool,
    pub require_no_callbacks: bool,
    pub require_no_panics: bool,
    pub require_no_shared_state: bool,
    pub require_return_guard: bool,
}

/// A conditional result is a caller-side proof *obligation*, not a completed
/// caller theorem. `conditions` must be retained transitively by later tools.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignLawFrontier {
    schema: &'static str,
    law_id: String,
    import_id: String,
    physical_symbol: String,
    package_name: String,
    package_version: String,
    package_source_sha256: String,
    index_digest: String,
    feature_digest: String,
    project_lock_digest: String,
    target: String,
    adapter_semantics: &'static str,
    adapter_digest: String,
    assumption_id: String,
    proposition_digest: String,
    summary_digest: String,
    conditions: Vec<String>,
    behavior_assumptions: [bool; 4],
    guarded_i64_range: Option<(i64, i64)>,
}

fn invalid(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-FL300", message)
}

fn framed(hash: &mut Sha256, part: &str) {
    hash.update((part.len() as u64).to_be_bytes());
    hash.update(part.as_bytes());
}

fn checked_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// Derive a read-only, source-bound conditional frontier. The caller supplies
/// an already authenticated selected plan and exact adapter artifact digest;
/// replay must be performed again at the consuming boundary.
pub fn derive(
    import: &ResolvedImport,
    binding: &ScalarBindingPlan,
    boundary: ForeignBoundary<'_>,
    declared: &DeclaredForeignSummary,
    law: &ForeignLawRequest,
) -> Result<ForeignLawFrontier, Diagnostic> {
    verify_scalar_binding(import, binding)?;
    if !valid_digest(boundary.project_lock_digest)
        || !valid_digest(boundary.adapter_digest)
        || boundary.target != binding.target
    {
        return Err(invalid(
            "foreign law lock, adapter, or target identity is invalid",
        ));
    }
    if !checked_id(&declared.assumption_id)
        || !checked_id(&law.law_id)
        || !valid_digest(&declared.proposition_digest)
        || declared
            .return_i64_range
            .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(invalid(
            "foreign law assumption, proposition, guard, or law identity is invalid",
        ));
    }
    if !law.require_theorem
        && !law.require_no_effects
        && !law.require_no_callbacks
        && !law.require_no_panics
        && !law.require_no_shared_state
        && !law.require_return_guard
    {
        return Err(invalid(
            "foreign law request has no checked or conditional property",
        ));
    }
    if law.require_theorem {
        return Err(Diagnostic::io(
            "SPX-FL301",
            "foreign signature, assumption, test, and runtime guard are not checked theorem evidence",
        ));
    }
    let obligations = [
        (
            law.require_no_effects,
            declared.assumes_no_effects,
            "no_effects",
        ),
        (
            law.require_no_callbacks,
            declared.assumes_no_callbacks,
            "no_callbacks",
        ),
        (
            law.require_no_panics,
            declared.assumes_no_panics,
            "no_panics",
        ),
        (
            law.require_no_shared_state,
            declared.assumes_no_shared_state,
            "no_shared_state",
        ),
    ];
    if obligations
        .iter()
        .any(|(required, assumed, _)| *required && !assumed)
        || (law.require_return_guard && declared.return_i64_range.is_none())
    {
        return Err(Diagnostic::io(
            "SPX-FL302",
            "foreign law property is unknown or lacks its exact retained runtime guard",
        ));
    }
    let mut conditions = Vec::new();
    for (required, _, name) in obligations {
        if required {
            conditions.push(format!("{}:{name}", declared.assumption_id));
        }
    }
    if !conditions.is_empty() && !law.permit_assumptions {
        return Err(Diagnostic::io(
            "SPX-FL303",
            "strict foreign law refuses declared behavioral assumptions",
        ));
    }
    // Include all four behavior declarations even when a law uses only one.
    // Changing assumptions stales the identity of every dependent frontier.
    let mut hash = Sha256::new();
    hash.update(b"semaprax.foreign-law-summary.v1\0");
    for part in [
        binding.import_id.as_str(),
        binding.cargo_alias.as_str(),
        binding.package_name.as_str(),
        binding.package_version.as_str(),
        binding.package_source_sha256.as_str(),
        binding.index_digest.as_str(),
        binding.feature_digest.as_str(),
        binding.target.as_str(),
        binding.rust_path.as_str(),
        binding.signature.as_str(),
        binding.receiver.as_str(),
        binding.physical_symbol.as_str(),
        boundary.project_lock_digest,
        boundary.adapter_digest,
        SCALAR_ADAPTER_SEMANTICS,
        &declared.assumption_id,
        &declared.proposition_digest,
        &format!("{:?}", declared.return_i64_range),
    ] {
        framed(&mut hash, part);
    }
    for bit in [
        declared.assumes_no_effects,
        declared.assumes_no_callbacks,
        declared.assumes_no_panics,
        declared.assumes_no_shared_state,
    ] {
        hash.update([u8::from(bit)]);
    }
    let summary_digest = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    Ok(ForeignLawFrontier {
        schema: FOREIGN_LAW_SCHEMA,
        law_id: law.law_id.clone(),
        import_id: binding.import_id.clone(),
        physical_symbol: binding.physical_symbol.clone(),
        package_name: binding.package_name.clone(),
        package_version: binding.package_version.clone(),
        package_source_sha256: binding.package_source_sha256.clone(),
        index_digest: binding.index_digest.clone(),
        feature_digest: binding.feature_digest.clone(),
        project_lock_digest: boundary.project_lock_digest.to_owned(),
        target: binding.target.clone(),
        adapter_semantics: SCALAR_ADAPTER_SEMANTICS,
        adapter_digest: boundary.adapter_digest.to_owned(),
        assumption_id: declared.assumption_id.clone(),
        proposition_digest: declared.proposition_digest.clone(),
        summary_digest,
        conditions,
        behavior_assumptions: [
            declared.assumes_no_effects,
            declared.assumes_no_callbacks,
            declared.assumes_no_panics,
            declared.assumes_no_shared_state,
        ],
        guarded_i64_range: declared
            .return_i64_range
            .filter(|_| law.require_return_guard),
    })
}

/// Exact replay stales every identity input, including lock, feature set,
/// target, symbol, adapter, declaration, and each behavioral assumption.
pub fn replay(
    recorded: &ForeignLawFrontier,
    import: &ResolvedImport,
    binding: &ScalarBindingPlan,
    boundary: ForeignBoundary<'_>,
    declared: &DeclaredForeignSummary,
    law: &ForeignLawRequest,
) -> Result<(), Diagnostic> {
    let expected = derive(import, binding, boundary, declared, law)?;
    if &expected != recorded {
        return Err(Diagnostic::io(
            "SPX-FL304",
            "foreign law frontier is stale for the exact selected import and assumptions",
        ));
    }
    Ok(())
}

/// Call this after the physical foreign return and before publishing that i64
/// to checked Semaprax code. A guard checks this value only; it says nothing
/// about prior side effects, callbacks, panics, or shared-state mutation.
pub fn guard_i64_return(frontier: &ForeignLawFrontier, value: i64) -> Result<i64, Diagnostic> {
    let Some((minimum, maximum)) = frontier.guarded_i64_range else {
        return Err(Diagnostic::io(
            "SPX-FL305",
            "foreign return guard was not retained",
        ));
    };
    if value < minimum || value > maximum {
        return Err(Diagnostic::io(
            "SPX-FL306",
            "foreign i64 return violated its retained runtime guard",
        ));
    }
    Ok(value)
}

impl ForeignLawFrontier {
    pub fn target(&self) -> &str {
        &self.target
    }
    /// Minimal canonical diagnostic projection. Every unknown remains visible.
    pub fn physical_symbol(&self) -> &str {
        &self.physical_symbol
    }

    pub fn conditions(&self) -> &[String] {
        &self.conditions
    }

    pub fn summary_digest(&self) -> &str {
        &self.summary_digest
    }

    pub fn public_view(&self) -> String {
        let status = if self.conditions.is_empty() {
            "runtime_guard_only"
        } else {
            "conditional_on_foreign_assumptions"
        };
        let behavior = |assumed| if assumed { "assumed_absent" } else { "unknown" };
        let value = serde_json::json!({
            "schema": self.schema,
            "status": status,
            "law_id": self.law_id,
            "import_id": self.import_id,
            "physical_symbol": self.physical_symbol,
            "package_name": self.package_name,
            "package_version": self.package_version,
            "package_source_sha256": self.package_source_sha256,
            "index_digest": self.index_digest,
            "feature_digest": self.feature_digest,
            "project_lock_digest": self.project_lock_digest,
            "target": self.target,
            "adapter_semantics": self.adapter_semantics,
            "adapter_digest": self.adapter_digest,
            "assumption_id": self.assumption_id,
            "proposition_digest": self.proposition_digest,
            "summary_digest": self.summary_digest,
            "conditions": self.conditions,
            "guarded_i64_range": self.guarded_i64_range,
            "foreign_internals_proved": false,
            "foreign_behavior": {
                "effects": behavior(self.behavior_assumptions[0]),
                "callbacks": behavior(self.behavior_assumptions[1]),
                "panics": behavior(self.behavior_assumptions[2]),
                "shared_state": behavior(self.behavior_assumptions[3]),
            }
        });
        format!(
            "{}\n",
            serde_json::to_string(&value).expect("closed frontier JSON")
        )
    }
}
