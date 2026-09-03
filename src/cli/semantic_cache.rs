//! Explicit derived-cache storage; never canonical source publication.
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::semantic_cache_store;
use serde_json::json;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn absolute(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|root| root.join(path))
        .map_err(|_| {
            vec![Diagnostic::io(
                "SPX-G306",
                "cannot resolve semantic cache manifest",
            )]
        })
}
pub(crate) fn initialize(root: &Path) -> Result<String, Vec<Diagnostic>> {
    // Constant output prepared before a filesystem pivot.
    let output =
        "{\"schema\":\"semaprax.semantic-cache-initialized.v1\",\"source_authority\":false}\n"
            .to_owned();
    semantic_cache_store::initialize(root)?;
    Ok(output)
}
pub(crate) fn persist(manifest: &Path, root: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut session =
        VNextSession::open_with_semantic_cache(&absolute(manifest)?, VNextPolicy::default())?;
    let cache = session.retained_semantic_cache()?;
    session.finish()?;
    // Live input authentication is complete. This is an immutable historical
    // cache subject, not a claim that source remains unchanged during storage.
    let receipt = semantic_cache_store::persist(root, &cache)?;
    let mut value = json!({
        "schema":"semaprax.semantic-cache-receipt.v1",
        "entry_digest":receipt.entry_digest(),
        "compiler_digest":receipt.compiler_digest(),
        "payload_bytes":receipt.payload_bytes(),
        "source_authority":false,
        "current_source_admission":false,
        "commit_approval":false,
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}
pub(crate) fn load(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>> {
    let cache = semantic_cache_store::load(root, expected)?;
    cache.restored_work().map(str::to_owned).ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-G307",
            "semantic cache load did not retain warm replay work",
        )]
    })
}

pub(crate) fn evict(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>> {
    let receipt = semantic_cache_store::evict(root, expected)?;
    let mut value = json!({
        "schema":"semaprax.semantic-cache-eviction.v1",
        "entry_digest":receipt.entry_digest(),
        "envelope_bytes":receipt.envelope_bytes(),
        "entries_remaining":receipt.entries_remaining(),
        "source_authority":false,
        "canonical_source_mutation":false,
        "publication_authority":false,
        "cache_management_effect":"selected_entry_removed",
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}

const MAX_LIFECYCLE_REPORT_BYTES: usize = 512 * 1024;

/// Exercise one complete derived-cache lifecycle while leaving canonical
/// source untouched. The caller supplies an existing empty private store root;
/// failure after a store pivot retains the ordinary operation's uncertainty.
pub(crate) fn lifecycle(manifest: &Path, root: &Path) -> Result<String, Vec<Diagnostic>> {
    let manifest = absolute(manifest)?;
    semantic_cache_store::initialize(root)?;

    let mut cold = VNextSession::open_with_semantic_cache(&manifest, VNextPolicy::default())?;
    let cold_image = cold.image_revision().to_owned();
    let cold_work = initial_work(&cold)?;
    let project_revision = text(&cold_work, "project_revision")?.to_owned();
    let cache = cold.retained_semantic_cache()?;
    cold.finish()?;
    let persisted = semantic_cache_store::persist(root, &cache)?;
    let entry_digest = persisted.entry_digest().to_owned();
    let compiler_digest = persisted.compiler_digest().to_owned();
    let payload_bytes = persisted.payload_bytes();

    let restored = semantic_cache_store::load(root, &entry_digest)?;
    let mut warm = VNextSession::open_with_retained_semantic_cache(
        &manifest,
        VNextPolicy::default(),
        restored,
    )?;
    let warm_image = warm.image_revision().to_owned();
    let warm_work = initial_work(&warm)?;
    let refresh = same_source_refresh(&mut warm, &project_revision)?;
    let refresh_work = refresh
        .get("frontend_work")
        .cloned()
        .ok_or_else(|| lifecycle_error("same-source refresh omitted compiler-work telemetry"))?;
    require_work_profile(&cold_work, &warm_work, &refresh_work)?;
    warm.finish()?;

    let evicted = semantic_cache_store::evict(root, &entry_digest)?;
    let envelope_bytes = evicted.envelope_bytes();
    let entries_remaining = evicted.entries_remaining();

    let mut rebuilt = VNextSession::open_with_semantic_cache(&manifest, VNextPolicy::default())?;
    let rebuilt_image = rebuilt.image_revision().to_owned();
    let rebuilt_work = initial_work(&rebuilt)?;
    rebuilt.finish()?;

    if cold_image != warm_image
        || cold_image != rebuilt_image
        || text(&warm_work, "project_revision")? != project_revision
        || text(&rebuilt_work, "project_revision")? != project_revision
        || cold_work != rebuilt_work
        || refresh.get("project_revision").and_then(Value::as_str) != Some(&project_revision)
    {
        return Err(lifecycle_error(
            "semantic cache lifecycle changed admitted semantic identity or cold reconstruction work",
        ));
    }

    let mut value = json!({
        "schema":"semaprax.semantic-cache-lifecycle.v1",
        "project_revision":project_revision,
        "image_revision":cold_image,
        "entry_digest":entry_digest,
        "compiler_digest":compiler_digest,
        "payload_bytes":payload_bytes,
        "envelope_bytes":envelope_bytes,
        "stages":[
            {"stage":"cold_open","frontend_work":cold_work},
            {"stage":"authenticated_store_restore","frontend_work":warm_work},
            {"stage":"same_revision_refresh","frontend_work":refresh_work},
            {"stage":"exact_eviction","entries_remaining":entries_remaining},
            {"stage":"cold_rebuild_after_eviction","frontend_work":rebuilt_work},
        ],
        "equivalence":{
            "project_revision_preserved":true,
            "image_revision_preserved":true,
            "cold_rebuild_work_identical":true,
        },
        "source_authority":false,
        "canonical_source_mutation":false,
        "publication_authority":false,
        "execution":false,
        "store_effect":"initialized_persisted_loaded_then_exact_entry_evicted",
        "nonclaims":[
            "not_wall_clock_or_RSS_measurement",
            "not_cross_process_execution_evidence",
            "not_crash_or_power_loss_recovery_evidence",
            "no_automatic_cleanup_after_failure",
        ],
    });
    value.sort_all_objects();
    let output = format!("{value}\n");
    if output.len() > MAX_LIFECYCLE_REPORT_BYTES {
        return Err(lifecycle_error(
            "semantic cache lifecycle report exceeds its fixed byte bound",
        ));
    }
    Ok(output)
}

fn initial_work(session: &VNextSession) -> Result<Value, Vec<Diagnostic>> {
    session
        .initial_frontend_work()
        .cloned()
        .ok_or_else(|| lifecycle_error("semantic cache session omitted initial compiler work"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| lifecycle_error("semantic cache lifecycle work report is malformed"))
}

fn work_count(value: &Value, key: &str) -> Result<u64, Vec<Diagnostic>> {
    value
        .get("work")
        .and_then(|work| work.get(key))
        .and_then(Value::as_u64)
        .ok_or_else(|| lifecycle_error("semantic cache lifecycle work count is malformed"))
}

fn require_work_profile(
    cold: &Value,
    restored: &Value,
    refreshed: &Value,
) -> Result<(), Vec<Diagnostic>> {
    let modules = work_count(cold, "modules_resolved")?;
    if modules == 0
        || work_count(cold, "checked_HIR_reused")? != 0
        || work_count(restored, "modules_resolved")? != 0
        || work_count(restored, "checked_HIR_reused")? != modules
        || work_count(refreshed, "modules_resolved")? != 0
        || work_count(refreshed, "checked_HIR_reused")? != modules
    {
        return Err(lifecycle_error(
            "semantic cache lifecycle did not observe cold resolution and complete warm checked-HIR reuse",
        ));
    }
    Ok(())
}

fn same_source_refresh(
    session: &mut VNextSession,
    expected_project_revision: &str,
) -> Result<Value, Vec<Diagnostic>> {
    let frame = json!({
        "jsonrpc":"2.0",
        "id":"semantic-cache-lifecycle",
        "method":"workspace/refresh",
        "params":{
            "image_revision":session.image_revision(),
            "expected_new_project_revision":expected_project_revision,
        },
    })
    .to_string();
    let response = session
        .handle_frame(frame.as_bytes())
        .ok_or_else(|| lifecycle_error("semantic cache lifecycle refresh returned no response"))?;
    let value: Value = serde_json::from_slice(&response)
        .map_err(|_| lifecycle_error("semantic cache lifecycle refresh response is malformed"))?;
    if value.get("error").is_some() {
        return Err(lifecycle_error(
            "semantic cache lifecycle same-source refresh was rejected",
        ));
    }
    value
        .get("result")
        .and_then(|result| result.get("payload"))
        .cloned()
        .ok_or_else(|| lifecycle_error("semantic cache lifecycle refresh payload is absent"))
}

fn lifecycle_error(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G307", message)]
}
