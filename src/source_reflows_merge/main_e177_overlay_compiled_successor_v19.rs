//! One explicitly witnessed metadata successor; the V18/V2 readers stay intact.
use super::{canonical, digest, keys, pin, Value, PAGE_CAP};
use std::borrow::Cow;

pub(super) const SCHEMA: &str = "semaprax.finite-source-overlay-page.v19";
const SOURCE: &str = "src/workspace_graph/expected_projection/borrowed_imports.rs";
const OWNER: &str = "src/workspace_graph/expected_projection.rs";
const POSITION: usize = 389;
const DECLARATION: &str = "#[path = \"expected_projection/borrowed_imports.rs\"]\npub(in crate::workspace_graph) mod borrowed_imports;\n";

fn current_pin(value: &Value) -> Result<(usize, &str), &'static str> {
    require!(
        keys(value, &["length", "sha256", "archive"]),
        "v19 current pin keys"
    );
    let (length, sha, archive) = pin(value)?;
    require!(
        archive.strip_prefix("e/") == Some(sha),
        "v19 current archive channel"
    );
    Ok((length, sha))
}

fn tuple_entry(value: &Value) -> Result<Value, &'static str> {
    let values = value.as_array().ok_or("v19 successor tuple")?;
    require!(
        values.len() == 7 && values[0] == "m",
        "v19 successor tuple tag/arity"
    );
    for index in [1, 3, 5] {
        let sha = values[index].as_str().ok_or("v19 successor tuple digest")?;
        require!(
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "v19 successor tuple digest"
        );
    }
    let source_length = usize::try_from(values[2].as_u64().ok_or("v19 successor tuple length")?)
        .map_err(|_| "v19 successor tuple length")?;
    let owner_length = usize::try_from(values[4].as_u64().ok_or("v19 successor tuple length")?)
        .map_err(|_| "v19 successor tuple length")?;
    let start = usize::try_from(values[6].as_u64().ok_or("v19 successor tuple start")?)
        .map_err(|_| "v19 successor tuple start")?;
    require!(
        source_length <= super::SOURCE_CAP && owner_length <= super::SOURCE_CAP,
        "v19 successor tuple bound"
    );
    require!(
        start >= 2
            && start
                .checked_add(DECLARATION.len())
                .is_some_and(|end| end <= owner_length),
        "v19 successor tuple span"
    );
    // Every borrowed scalar and complete fixed-size output bound is checked
    // before constructing a source, archive descriptor, or registration object.
    Ok(serde_json::json!({
        "source":SOURCE,"prior_row_sha256":values[1],
        "current":{"length":source_length,"sha256":values[3],"archive":format!("e/{}",values[3].as_str().unwrap())},
        "registration":{"owner":OWNER,"current":{"length":owner_length,"sha256":values[5],"archive":format!("e/{}",values[5].as_str().unwrap())},"start":start,"text":DECLARATION}
    }))
}

fn header(page: &Value) -> Result<Cow<'_, Value>, &'static str> {
    require!(canonical(page).len() <= PAGE_CAP, "v19 same one-page cap");
    require!(
        keys(page, &["schema", "compiled_successor", "page"]) && page["schema"] == SCHEMA,
        "v19 closed envelope"
    );
    require!(
        page["page"]["schema"] == super::fragments_v3::SCHEMA_V18
            || page["page"]["schema"] == super::fragments_v3::SCHEMA_V19_INDEXED,
        "v19 complete V18 page"
    );
    if page["page"]["schema"] == super::fragments_v3::SCHEMA_V19_INDEXED {
        super::fragments_v3::preflight_indexed(&page["page"], super::SOURCE_CAP)?;
    }
    let successor = if page["compiled_successor"].is_array() {
        Cow::Owned(tuple_entry(&page["compiled_successor"])?)
    } else {
        Cow::Borrowed(&page["compiled_successor"])
    };
    require!(
        keys(
            &successor,
            &["source", "prior_row_sha256", "current", "registration"]
        ) && successor["source"] == SOURCE,
        "v19 singleton source"
    );
    let prior = successor["prior_row_sha256"]
        .as_str()
        .ok_or("v19 prior row digest")?;
    require!(
        prior.len() == 64
            && prior
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "v19 prior row digest"
    );
    current_pin(&successor["current"])?;
    let registration = &successor["registration"];
    require!(
        keys(registration, &["owner", "current", "start", "text"])
            && registration["owner"] == OWNER
            && registration["text"] == DECLARATION,
        "v19 registration declaration"
    );
    let (length, _) = current_pin(&registration["current"])?;
    let start = usize::try_from(
        registration["start"]
            .as_u64()
            .ok_or("v19 declaration start")?,
    )
    .map_err(|_| "v19 declaration start")?;
    require!(
        start >= 2
            && start
                .checked_add(DECLARATION.len())
                .is_some_and(|end| end <= length),
        "v19 declaration span"
    );
    Ok(successor)
}

fn current_row(source: &str, current: &Value) -> Value {
    serde_json::json!({"source":source,"compiled":true,"current":current,"views":[]})
}

// The fixed ordinary readers acquire all three source channels. The wire only
// supplies checked descriptors; it cannot choose a source or grant a role.
fn authenticate_with<'a>(
    page: &Value,
    compiled: &impl Fn(&str) -> Option<&'a str>,
    acquire: &impl Fn(&Value) -> Result<String, &'static str>,
) -> Result<(), &'static str> {
    let successor = header(page)?;
    let registration = &successor["registration"];
    // Rebind both fixed sources to the trusted compiled registry before the
    // first physical or archive read; an entry cannot authorize a new channel.
    for (source, descriptor) in [
        (SOURCE, &successor["current"]),
        (OWNER, &registration["current"]),
    ] {
        let actual = compiled(source).ok_or("v19 trusted compiled registration")?;
        let (length, sha) = current_pin(descriptor)?;
        require!(
            actual.len() == length && digest(actual.as_bytes()) == sha,
            "v19 trusted current binding"
        );
    }
    let source = acquire(&current_row(SOURCE, &successor["current"]))?;
    let owner = acquire(&current_row(OWNER, &registration["current"]))?;
    for (raw, descriptor) in [
        (&source, &successor["current"]),
        (&owner, &registration["current"]),
    ] {
        let (length, sha, _) = pin(descriptor)?;
        require!(
            raw.len() == length && digest(raw.as_bytes()) == sha,
            "v19 whole current source"
        );
    }
    let start = usize::try_from(
        registration["start"]
            .as_u64()
            .ok_or("v19 declaration start")?,
    )
    .map_err(|_| "v19 declaration start")?;
    let end = start
        .checked_add(DECLARATION.len())
        .ok_or("v19 declaration span")?;
    require!(
        owner.get(start..end) == Some(DECLARATION)
            && owner.get(start - 2..start) == Some("\n\n")
            && owner.matches(DECLARATION).count() == 1,
        "v19 exact unconditional declaration"
    );
    Ok(())
}

pub(super) fn authenticate(page: &Value) -> Result<(), &'static str> {
    if page["schema"] != SCHEMA {
        return Ok(());
    }
    authenticate_with(page, &super::super::compiled, &super::current)
}

// Pure metadata composition. Physical authentication belongs to the ordinary
// loader above, before any normalization or historical source selection.
pub(super) fn compose(before: Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    let successor = header(page)?;
    require!(
        keys(
            policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ) && policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "v19 policy fields"
    );
    require!(
        policy["replacements"]
            .as_array()
            .ok_or("v19 replacement paths")?
            .iter()
            .filter(|source| source.as_str() == Some(SOURCE))
            .count()
            == 1,
        "v19 explicit replacement"
    );
    require!(
        page["page"]["predecessor"] == policy["predecessor"]
            && digest(&canonical(&before)) == policy["predecessor"]["logical_sha256"],
        "v19 original whole predecessor"
    );
    let rows = before["rows"].as_array().ok_or("v19 old rows")?;
    require!(
        rows.len() == 519 && policy["predecessor"]["rows"] == 519,
        "v19 original 519 rows"
    );
    let prior = &rows[POSITION];
    require!(
        prior["source"] == SOURCE
            && prior["compiled"] == false
            && rows.iter().filter(|row| row["source"] == SOURCE).count() == 1
            && digest(&canonical(prior)) == successor["prior_row_sha256"],
        "v19 complete false predecessor row"
    );
    let mut derived = before.clone();
    derived["rows"][POSITION]["compiled"] = true.into();
    let mut restored = derived.clone();
    restored["rows"][POSITION]["compiled"] = false.into();
    require!(
        canonical(&restored) == canonical(&before),
        "v19 entire original base inverse"
    );
    let mut internal_policy = policy.clone();
    internal_policy["predecessor"]["logical_sha256"] = digest(&canonical(&derived)).into();
    let mut internal_page = page["page"].clone();
    internal_page["predecessor"] = internal_policy["predecessor"].clone();
    let expanded = if internal_page["schema"] == super::fragments_v3::SCHEMA_V19_INDEXED {
        super::fragments_v3::expand_v19_indexed(&derived, &internal_policy, &internal_page)?
    } else {
        super::fragments_v3::expand_v18(&derived, &internal_policy, &internal_page)?
    };
    let result = super::compose_values_v2(derived, &internal_policy, &expanded)?;
    let rows = result["rows"].as_array().ok_or("v19 result rows")?;
    let current = &rows[POSITION];
    require!(
        current["source"] == SOURCE
            && current["compiled"] == true
            && current["current"] == successor["current"],
        "v19 exact current successor pin"
    );
    let owners: Vec<_> = rows.iter().filter(|row| row["source"] == OWNER).collect();
    require!(
        owners.len() == 1
            && owners[0]["compiled"] == true
            && owners[0]["current"] == successor["registration"]["current"],
        "v19 canonical registration owner"
    );
    let mut inverse = result.clone();
    inverse["rows"] = before["rows"].clone();
    require!(
        canonical(&inverse) == canonical(&before),
        "v19 complete predecessor object inverse"
    );
    Ok(result)
}

#[cfg(test)]
#[path = "main_e177_overlay_compiled_successor_v19/tests.rs"]
mod tests;

// The same trusted pre-I/O binding and complete declaration checks, with only
// the ordinary bounded reader injected for a retained invocation's live guard.
pub(super) fn authenticate_using(
    page: &Value,
    read: &impl Fn(&std::path::Path, usize, &str) -> Result<String, &'static str>,
) -> Result<(), &'static str> {
    if page["schema"] != SCHEMA {
        return Ok(());
    }
    authenticate_with(page, &super::super::compiled, &|row| {
        super::current_tree(
            row,
            &|row| {
                let (length, sha, _) = pin(&row["current"])?;
                read(
                    &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(row["source"].as_str().ok_or("source")?),
                    length,
                    sha,
                )
            },
            &super::super::compiled,
            &|descriptor| {
                let (length, sha, archive) = pin(descriptor)?;
                read(
                    &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(archive),
                    length,
                    sha,
                )
            },
        )
    })
}

pub(super) const SCHEMA_V20: &str = "semaprax.finite-source-overlay-page.v20";
fn header20(page: &Value) -> Result<Cow<'_, Value>, &'static str> {
    require!(canonical(page).len() <= PAGE_CAP, "v19 same one-page cap");
    require!(
        keys(page, &["schema", "compiled_successor", "page"]) && page["schema"] == SCHEMA_V20,
        "v19 closed envelope"
    );
    require!(
        page["page"]["schema"] == super::fragments_v3::SCHEMA_V20_COMPACT,
        "v19 complete V18 page"
    );
    super::fragments_v3::preflight_compact(&page["page"], super::SOURCE_CAP)?;
    let successor = if page["compiled_successor"].is_array() {
        Cow::Owned(tuple_entry(&page["compiled_successor"])?)
    } else {
        Cow::Borrowed(&page["compiled_successor"])
    };
    require!(
        keys(
            &successor,
            &["source", "prior_row_sha256", "current", "registration"]
        ) && successor["source"] == SOURCE,
        "v19 singleton source"
    );
    let prior = successor["prior_row_sha256"]
        .as_str()
        .ok_or("v19 prior row digest")?;
    require!(
        prior.len() == 64
            && prior
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "v19 prior row digest"
    );
    current_pin(&successor["current"])?;
    let registration = &successor["registration"];
    require!(
        keys(registration, &["owner", "current", "start", "text"])
            && registration["owner"] == OWNER
            && registration["text"] == DECLARATION,
        "v19 registration declaration"
    );
    let (length, _) = current_pin(&registration["current"])?;
    let start = usize::try_from(
        registration["start"]
            .as_u64()
            .ok_or("v19 declaration start")?,
    )
    .map_err(|_| "v19 declaration start")?;
    require!(
        start >= 2
            && start
                .checked_add(DECLARATION.len())
                .is_some_and(|end| end <= length),
        "v19 declaration span"
    );
    Ok(successor)
}
fn authenticate20_with<'a>(
    page: &Value,
    compiled: &impl Fn(&str) -> Option<&'a str>,
    acquire: &impl Fn(&Value) -> Result<String, &'static str>,
) -> Result<(), &'static str> {
    let successor = header20(page)?;
    let registration = &successor["registration"];
    // Rebind both fixed sources to the trusted compiled registry before the
    // first physical or archive read; an entry cannot authorize a new channel.
    for (source, descriptor) in [
        (SOURCE, &successor["current"]),
        (OWNER, &registration["current"]),
    ] {
        let actual = compiled(source).ok_or("v19 trusted compiled registration")?;
        let (length, sha) = current_pin(descriptor)?;
        require!(
            actual.len() == length && digest(actual.as_bytes()) == sha,
            "v19 trusted current binding"
        );
    }
    let source = acquire(&current_row(SOURCE, &successor["current"]))?;
    let owner = acquire(&current_row(OWNER, &registration["current"]))?;
    for (raw, descriptor) in [
        (&source, &successor["current"]),
        (&owner, &registration["current"]),
    ] {
        let (length, sha, _) = pin(descriptor)?;
        require!(
            raw.len() == length && digest(raw.as_bytes()) == sha,
            "v19 whole current source"
        );
    }
    let start = usize::try_from(
        registration["start"]
            .as_u64()
            .ok_or("v19 declaration start")?,
    )
    .map_err(|_| "v19 declaration start")?;
    let end = start
        .checked_add(DECLARATION.len())
        .ok_or("v19 declaration span")?;
    require!(
        owner.get(start..end) == Some(DECLARATION)
            && owner.get(start - 2..start) == Some("\n\n")
            && owner.matches(DECLARATION).count() == 1,
        "v19 exact unconditional declaration"
    );
    Ok(())
}
pub(super) fn authenticate20(page: &Value) -> Result<(), &'static str> {
    if page["schema"] != SCHEMA_V20 {
        return Ok(());
    }
    authenticate20_with(page, &super::super::compiled, &super::current)
}
pub(super) fn compose20(
    before: Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let successor = header20(page)?;
    require!(
        keys(
            policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ) && policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "v19 policy fields"
    );
    require!(
        policy["replacements"]
            .as_array()
            .ok_or("v19 replacement paths")?
            .iter()
            .filter(|source| source.as_str() == Some(SOURCE))
            .count()
            == 1,
        "v19 explicit replacement"
    );
    require!(
        page["page"]["predecessor"] == policy["predecessor"]
            && digest(&canonical(&before)) == policy["predecessor"]["logical_sha256"],
        "v19 original whole predecessor"
    );
    let rows = before["rows"].as_array().ok_or("v19 old rows")?;
    require!(
        rows.len() == 519 && policy["predecessor"]["rows"] == 519,
        "v19 original 519 rows"
    );
    let prior = &rows[POSITION];
    require!(
        prior["source"] == SOURCE
            && prior["compiled"] == false
            && rows.iter().filter(|row| row["source"] == SOURCE).count() == 1
            && digest(&canonical(prior)) == successor["prior_row_sha256"],
        "v19 complete false predecessor row"
    );
    let mut derived = before.clone();
    derived["rows"][POSITION]["compiled"] = true.into();
    let mut restored = derived.clone();
    restored["rows"][POSITION]["compiled"] = false.into();
    require!(
        canonical(&restored) == canonical(&before),
        "v19 entire original base inverse"
    );
    let mut internal_policy = policy.clone();
    internal_policy["predecessor"]["logical_sha256"] = digest(&canonical(&derived)).into();
    let mut internal_page = page["page"].clone();
    internal_page["predecessor"] = internal_policy["predecessor"].clone();
    let expanded =
        super::fragments_v3::expand_v20_compact(&derived, &internal_policy, &internal_page)?;
    let result = super::compose_values_v2(derived, &internal_policy, &expanded)?;
    let rows = result["rows"].as_array().ok_or("v19 result rows")?;
    let current = &rows[POSITION];
    require!(
        current["source"] == SOURCE
            && current["compiled"] == true
            && current["current"] == successor["current"],
        "v19 exact current successor pin"
    );
    let owners: Vec<_> = rows.iter().filter(|row| row["source"] == OWNER).collect();
    require!(
        owners.len() == 1
            && owners[0]["compiled"] == true
            && owners[0]["current"] == successor["registration"]["current"],
        "v19 canonical registration owner"
    );
    let mut inverse = result.clone();
    inverse["rows"] = before["rows"].clone();
    require!(
        canonical(&inverse) == canonical(&before),
        "v19 complete predecessor object inverse"
    );
    Ok(result)
}

pub(super) fn authenticate20_using(
    page: &Value,
    read: &impl Fn(&std::path::Path, usize, &str) -> Result<String, &'static str>,
) -> Result<(), &'static str> {
    if page["schema"] != SCHEMA_V20 {
        return Ok(());
    }
    authenticate20_with(page, &super::super::compiled, &|row| {
        super::current_tree(
            row,
            &|row| {
                let (length, sha, _) = pin(&row["current"])?;
                read(
                    &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(row["source"].as_str().ok_or("source")?),
                    length,
                    sha,
                )
            },
            &super::super::compiled,
            &|descriptor| {
                let (length, sha, archive) = pin(descriptor)?;
                read(
                    &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(archive),
                    length,
                    sha,
                )
            },
        )
    })
}

// Retain the complete V20 composition when building the cache's logical V2
// page. Every cache hit still runs the original full V2 composition, inverse,
// and fresh policy/page acquisition. No row or unknown field is projected away.
pub(super) fn expanded_for_cache(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let complete = super::compose_values(before.clone(), policy, page)?;
    let rows: std::collections::BTreeMap<_, _> = complete["rows"]
        .as_array()
        .ok_or("cache rows")?
        .iter()
        .map(|row| Ok((row["source"].as_str().ok_or("cache source")?, row)))
        .collect::<Result<_, &'static str>>()?;
    let group = |name: &str| -> Result<Vec<Value>, &'static str> {
        policy[name]
            .as_array()
            .ok_or("cache scope")?
            .iter()
            .map(|entry| {
                let source = if name == "replacements" {
                    entry.as_str()
                } else {
                    entry["source"].as_str()
                }
                .ok_or("cache scope source")?;
                Ok((*rows.get(source).ok_or("cache scoped row")?).clone())
            })
            .collect()
    };
    Ok(serde_json::json!({
        "schema":"semaprax.finite-source-overlay-page.v2", "index":1, "total_pages":1,
        "predecessor":policy["predecessor"], "replacements":group("replacements")?,
        "additions":group("additions")?
    }))
}

use super::{
    held_4a6eb, json, page_bytes, page_data, ADDITIONS, EXTENSION_LENGTH, EXTENSION_PATH,
    EXTENSION_SHA256, PAGE_PATH, POLICY_LENGTH, POLICY_PATH, POLICY_SHA256, REPLACEMENTS,
};
use std::path::Path;
use std::sync::Arc;

pub(super) fn cacheable(page: &Value) -> bool {
    matches!(
        page["schema"].as_str(),
        Some(
            "semaprax.finite-source-overlay-page.v11"
                | "semaprax.finite-source-overlay-page.v12"
                | "semaprax.finite-source-overlay-page.v13"
                | "semaprax.finite-source-overlay-page.v19"
                | "semaprax.finite-source-overlay-page.v20"
        )
    )
}
pub(super) fn successor(page: &Value) -> bool {
    page["schema"] == SCHEMA || page["schema"] == SCHEMA_V20
}

// The controller now owns both physical source files. Its child is acquired
// with the same bounded no-follow reader before ordinary composition begins.
pub(super) fn authenticate_source() -> Result<(), &'static str> {
    let compiled = include_str!("main_e177_overlay_compiled_successor_v19.rs");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/source_reflows_merge/main_e177_overlay_compiled_successor_v19.rs");
    let physical = super::held_4a6eb(&path, compiled.len(), &digest(compiled.as_bytes()))?;
    require!(
        physical == compiled,
        "whole controller child physical/compiled"
    );
    Ok(())
}

pub(super) fn invocation_load(
    read: &impl Fn(&Path, usize, &str) -> Result<String, &'static str>,
) -> Result<(Value, Arc<Value>), &'static str> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled =
        include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/policy.json");
    require!(POLICY_LENGTH <= PAGE_CAP, "policy bound");
    let raw = read(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;
    require!(raw == compiled, "policy compiled/physical");
    let policy = json(&raw)?;
    require!(
        keys(
            &policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ),
        "policy keys"
    );
    require!(
        policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "policy schema"
    );
    require!(
        keys(&policy["page"], &["path", "length", "sha256"]) && policy["page"]["path"] == PAGE_PATH,
        "one literal page"
    );
    require!(
        keys(
            &policy["predecessor"],
            &["extension", "rows", "logical_sha256"]
        ),
        "predecessor keys"
    );
    require!(policy["predecessor"]["rows"] == 519, "predecessor rows");
    require!(
        policy["predecessor"]["extension"]
            == serde_json::json!({"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256}),
        "predecessor extension"
    );
    require!(
        policy["replacements"] == serde_json::json!(REPLACEMENTS),
        "literal replacement scope"
    );
    require!(
        policy["additions"]
            .as_array()
            .ok_or("additions")?
            .iter()
            .map(|r| r["source"].as_str())
            .collect::<Vec<_>>()
            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),
        "literal addition scope"
    );
    // Original metadata is read as metadata. It is never substituted for a
    // changed physical Rust source, and it retains its original individual cap.
    let extension = include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    require!(
        read(
            &root.join(EXTENSION_PATH),
            EXTENSION_LENGTH,
            EXTENSION_SHA256
        )? == extension,
        "predecessor physical/compiled"
    );
    let length = usize::try_from(policy["page"]["length"].as_u64().ok_or("page length")?)
        .map_err(|_| "page length")?;
    let sha = policy["page"]["sha256"].as_str().ok_or("page digest")?;
    require!(length <= PAGE_CAP, "page bound");
    let physical = read(&root.join(PAGE_PATH), length, sha)?;
    let compiled = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/page.json");
    page_bytes(compiled, &physical, length, sha)?;
    Ok((policy, page_data(&physical)?))
}

use super::{
    compiled_successor_v19, compose_values, compose_values_v2, fragments_v3, load_current,
};

pub(super) fn compose_with_expansion_cache(
    before: Value,
    cache: &std::sync::OnceLock<(Value, Arc<Value>)>,
) -> Result<Value, &'static str> {
    // Keep the complete current policy/page acquisition on every invocation.
    let (policy, page) = load_current()?;
    if !compiled_successor_v19::cacheable(&page) {
        return compose_values(before, &policy, &page);
    }
    if let Some((authenticated_before, expanded)) = cache.get() {
        // A different predecessor keeps the complete original validation and
        // its first refusal; it cannot replace or poison the cached owner.
        return if &before == authenticated_before {
            compose_values_v2(before, &policy, expanded)
        } else {
            compose_values(before, &policy, &page)
        };
    }
    let expanded = if compiled_successor_v19::successor(&page) {
        compiled_successor_v19::expanded_for_cache(&before, &policy, &page)?
    } else if page["schema"] == fragments_v3::SCHEMA_V13 {
        fragments_v3::expand_v13(&before, &policy, &page)?
    } else if page["schema"] == fragments_v3::SCHEMA_V12 {
        fragments_v3::expand_v12(&before, &policy, &page)?
    } else {
        fragments_v3::expand_v11(&before, &policy, &page)?
    };
    let result = compose_values_v2(before.clone(), &policy, &expanded)?;
    // Both complete metadata objects remain owned. No row or unknown field is
    // projected away, and a refused predecessor never initializes the cache.
    let _ = cache.set((before, Arc::new(expanded)));
    Ok(result)
}
// Complete independent reference parser retained verbatim, except visibility.
#[cfg(test)]
pub(super) fn load_before_page_data() -> Result<(Value, Value), &'static str> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled =
        include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/policy.json");
    require!(POLICY_LENGTH <= PAGE_CAP, "policy bound");
    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;
    require!(raw == compiled, "policy compiled/physical");
    let policy = json(&raw)?;
    require!(
        keys(
            &policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ),
        "policy keys"
    );
    require!(
        policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "policy schema"
    );
    require!(
        keys(&policy["page"], &["path", "length", "sha256"]) && policy["page"]["path"] == PAGE_PATH,
        "one literal page"
    );
    require!(
        keys(
            &policy["predecessor"],
            &["extension", "rows", "logical_sha256"]
        ),
        "predecessor keys"
    );
    require!(policy["predecessor"]["rows"] == 519, "predecessor rows");
    require!(
        policy["predecessor"]["extension"]
            == serde_json::json!({"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256}),
        "predecessor extension"
    );
    require!(
        policy["replacements"] == serde_json::json!(REPLACEMENTS),
        "literal replacement scope"
    );
    require!(
        policy["additions"]
            .as_array()
            .ok_or("additions")?
            .iter()
            .map(|r| r["source"].as_str())
            .collect::<Vec<_>>()
            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),
        "literal addition scope"
    );
    // Original metadata is read as metadata. It is never substituted for a
    // changed physical Rust source, and it retains its original individual cap.
    let extension = include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    require!(
        held_4a6eb(
            &root.join(EXTENSION_PATH),
            EXTENSION_LENGTH,
            EXTENSION_SHA256
        )? == extension,
        "predecessor physical/compiled"
    );
    let length = usize::try_from(policy["page"]["length"].as_u64().ok_or("page length")?)
        .map_err(|_| "page length")?;
    let sha = policy["page"]["sha256"].as_str().ok_or("page digest")?;
    require!(length <= PAGE_CAP, "page bound");
    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;
    let compiled = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/page.json");
    page_bytes(compiled, &physical, length, sha)?;
    Ok((policy, json(&physical)?))
}
