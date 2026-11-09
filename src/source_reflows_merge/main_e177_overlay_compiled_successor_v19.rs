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
pub(super) const SCHEMA_V21: &str = "semaprax.finite-source-overlay-page.v21";
pub(super) const SCHEMA_V22: &str = "semaprax.finite-source-overlay-page.v22";
const V21_OUTPUT_CAP: usize = 524_288;

fn is_compressed(page: &Value) -> bool {
    matches!(page["schema"].as_str(), Some(SCHEMA_V21 | SCHEMA_V22))
}

const BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

fn decode_base85(encoded: &str) -> Result<Vec<u8>, &'static str> {
    const INVALID: &str = "v22 canonical base85";
    let remainder = encoded.len() % 5;
    if remainder == 1 {
        return Err(INVALID);
    }

    let mut digits = [u8::MAX; 256];
    for (digit, symbol) in BASE85_ALPHABET.iter().enumerate() {
        digits[usize::from(*symbol)] = digit as u8;
    }

    let mut decoded = Vec::with_capacity(encoded.len() / 5 * 4 + remainder.saturating_sub(1));
    for chunk in encoded.as_bytes().chunks(5) {
        let mut value = 0u32;
        for symbol in chunk.iter().copied().chain(std::iter::repeat_n(b'~', 5 - chunk.len())) {
            let digit = digits[usize::from(symbol)];
            if digit == u8::MAX {
                return Err(INVALID);
            }
            value = value.checked_mul(85)
                .and_then(|value| value.checked_add(u32::from(digit)))
                .ok_or(INVALID)?;
        }

        let bytes = value.to_be_bytes();
        let width = chunk.len() - 1;
        if width < 4 {
            // A short group encodes the original bytes followed by zero
            // padding. Reject alternate encodings of the same decoded bytes.
            let mut padded = [0; 4];
            padded[..width].copy_from_slice(&bytes[..width]);
            let canonical = base85_digits(u32::from_be_bytes(padded));
            if chunk != &canonical[..chunk.len()] {
                return Err(INVALID);
            }
        }
        decoded.extend_from_slice(&bytes[..width]);
    }
    Ok(decoded)
}

fn base85_digits(mut value: u32) -> [u8; 5] {
    let mut encoded = [0; 5];
    for digit in encoded.iter_mut().rev() {
        *digit = BASE85_ALPHABET[(value % 85) as usize];
        value /= 85;
    }
    encoded
}

pub(super) fn is_successor20(page: &Value) -> bool {
    matches!(page["schema"].as_str(), Some(SCHEMA_V20 | SCHEMA_V21 | SCHEMA_V22))
}

pub(super) fn decoded20(page: &Value) -> Result<(Cow<'_, Value>, usize), &'static str> {
    if !is_compressed(page) {
        return Ok((Cow::Borrowed(page), PAGE_CAP));
    }
    use base64::Engine as _;
    use flate2::{Decompress, FlushDecompress, Status};

    require!(
        keys(page, &["schema", "length", "sha256", "payload"]),
        "v21 closed envelope"
    );
    require!(canonical(page).len() <= PAGE_CAP, "v21 same one-page cap");
    let length = page["length"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=V21_OUTPUT_CAP).contains(n))
        .ok_or("v21 output bound")?;
    let encoded = page["payload"].as_str().ok_or("v21 payload type")?;
    let compressed = if page["schema"] == SCHEMA_V22 {
        decode_base85(encoded)?
    } else {
        let engine = base64::engine::general_purpose::STANDARD;
        let compressed = engine.decode(encoded).map_err(|_| "v21 canonical base64")?;
        require!(
            engine.encode(&compressed) == encoded,
            "v21 canonical base64"
        );
        compressed
    };
    let mut raw = vec![0; length + 1];
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress(&compressed, &mut raw, FlushDecompress::Finish)
        .map_err(|_| "v21 complete single stream")?;
    require!(
        status == Status::StreamEnd && decoder.total_in() == compressed.len() as u64,
        "v21 complete single stream"
    );
    require!(
        decoder.total_out() == length as u64,
        "v21 complete logical bytes"
    );
    raw.truncate(length);
    require!(digest(&raw) == page["sha256"], "v21 complete logical bytes");
    let value = json(std::str::from_utf8(&raw).map_err(|_| "v21 logical UTF-8")?)?;
    require!(value["schema"] == SCHEMA_V20, "v21 complete v20 page");
    Ok((Cow::Owned(value), V21_OUTPUT_CAP))
}

#[cfg(test)]
fn header20(page: &Value) -> Result<Cow<'_, Value>, &'static str> {
    header20_with_bound(page, PAGE_CAP)
}
fn header20_with_bound(page: &Value, transport_cap: usize) -> Result<Cow<'_, Value>, &'static str> {
    require!(
        canonical(page).len() <= transport_cap,
        "v19 same one-page cap"
    );
    require!(
        keys(page, &["schema", "compiled_successor", "page"]) && page["schema"] == SCHEMA_V20,
        "v19 closed envelope"
    );
    require!(
        page["page"]["schema"] == super::fragments_v3::SCHEMA_V20_COMPACT,
        "v19 complete V18 page"
    );
    super::fragments_v3::preflight_compact_with_bound(
        &page["page"],
        super::SOURCE_CAP,
        transport_cap,
    )?;
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
    let (page, transport_cap) = decoded20(page)?;
    let successor = header20_with_bound(&page, transport_cap)?;
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
    if !is_successor20(page) {
        return Ok(());
    }
    authenticate20_with(page, &super::super::compiled, &super::current)
}
pub(super) fn compose20(
    before: Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let (page, transport_cap) = decoded20(page)?;
    let successor = header20_with_bound(&page, transport_cap)?;
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
    let expanded = super::fragments_v3::expand_v20_compact(
        &derived,
        &internal_policy,
        &internal_page,
        transport_cap,
    )?;
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
    if !is_successor20(page) {
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
        )
    )
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

fn cached_predecessor(before: &Value, policy: &Value) -> (Value, Value) {
    let mut derived = before.clone();
    derived["rows"][POSITION]["compiled"] = true.into();
    let mut adjusted = policy.clone();
    adjusted["predecessor"]["logical_sha256"] = digest(&canonical(&derived)).into();
    (derived, adjusted)
}

fn expand_authenticated_successor(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let result = compose_values(before.clone(), policy, page)?;
    let (_, adjusted) = cached_predecessor(before, policy);
    let rows: std::collections::BTreeMap<_, _> = result["rows"]
        .as_array()
        .ok_or("successor rows")?
        .iter()
        .filter_map(|row| Some((row["source"].as_str()?, row)))
        .collect();
    let group = |name: &str| -> Result<Vec<&Value>, &'static str> {
        policy[name]
            .as_array()
            .ok_or("successor policy group")?
            .iter()
            .map(|entry| {
                let source = entry
                    .as_str()
                    .or_else(|| entry["source"].as_str())
                    .ok_or("successor policy source")?;
                rows.get(source).copied().ok_or("successor result source")
            })
            .collect()
    };
    Ok(serde_json::json!({
        "schema":"semaprax.finite-source-overlay-page.v2", "index":1, "total_pages":1,
        "predecessor":adjusted["predecessor"], "replacements":group("replacements")?,
        "additions":group("additions")?
    }))
}

fn replay_cached(
    before: Value,
    policy: &Value,
    expanded: &Value,
    successor: bool,
) -> Result<Value, &'static str> {
    if !successor {
        return compose_values_v2(before, policy, expanded);
    }
    let (derived, adjusted) = cached_predecessor(&before, policy);
    let result = compose_values_v2(derived, &adjusted, expanded)?;
    let mut inverse = result.clone();
    inverse["rows"] = before["rows"].clone();
    require!(inverse == before, "v19 complete predecessor object inverse");
    Ok(result)
}

pub(super) fn compose_with_expansion_cache(
    before: Value,
    cache: &std::sync::OnceLock<(Value, Arc<Value>)>,
) -> Result<Value, &'static str> {
    // Keep the complete current policy/page acquisition on every invocation.
    let (policy, page) = load_current()?;
    let successor = page["schema"] == SCHEMA || is_successor20(&page);
    if !successor && !compiled_successor_v19::cacheable(&page) {
        return compose_values(before, &policy, &page);
    }
    if let Some((authenticated_before, expanded)) = cache.get() {
        // A different predecessor keeps the complete original validation and
        // its first refusal; it cannot replace or poison the cached owner.
        return if &before == authenticated_before {
            replay_cached(before, &policy, expanded, successor)
        } else {
            compose_values(before, &policy, &page)
        };
    }
    let expanded = if successor {
        expand_authenticated_successor(&before, &policy, &page)?
    } else if page["schema"] == fragments_v3::SCHEMA_V13 {
        fragments_v3::expand_v13(&before, &policy, &page)?
    } else if page["schema"] == fragments_v3::SCHEMA_V12 {
        fragments_v3::expand_v12(&before, &policy, &page)?
    } else {
        fragments_v3::expand_v11_compatible(&before, &policy, &page)?
    };
    let result = replay_cached(before.clone(), &policy, &expanded, successor)?;
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

#[cfg(test)]
pub(super) mod source_sections {
    use std::ops::Range;

    pub(in super::super) fn constant(code: &str, name: &str) -> Range<usize> {
        let prefix = format!("const {name}:");
        assert_eq!(code.matches(&prefix).count(), 1);
        let start = code.find(&prefix).unwrap();
        let terminator = if matches!(name, "ADDITIONS" | "REPLACEMENTS") {
            "];"
        } else {
            ";"
        };
        let end = start + code[start..].find(terminator).unwrap() + terminator.len();
        start..end
    }

    pub(in super::super) fn array(code: &str, name: &str) -> Vec<String> {
        let declaration = &code[constant(code, name)];
        let contents = declaration
            .split_once("= &[")
            .unwrap()
            .1
            .strip_suffix("];")
            .unwrap();
        contents
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(line.strip_suffix(',').unwrap()).unwrap())
                }
            })
            .collect()
    }

    pub(in super::super) fn lookup(code: &str) -> &str {
        let prefix = "pub(super) fn compiled(source: &str) -> Option<&'static str> {\n";
        assert_eq!(code.matches(prefix).count(), 1);
        let start = code.find(prefix).unwrap();
        let end = start + code[start..].find("\n}\n").unwrap() + "\n}\n".len();
        &code[start..end]
    }

}
