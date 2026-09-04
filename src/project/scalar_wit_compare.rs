//! Fine-grained scalar-interface compatibility: compare two Project v1 scalar
//! WIT interface descriptors export by export.
//!
//! [`super::project_lock`]'s `--compare` gives a coarse verdict from the single
//! retained interface digest: it can say the interface changed, not which
//! export changed or how. This comparator reads two full
//! `semaprax.project.scalar-wit-interface.v1` descriptors (each already a
//! deterministic, byte-reproducible projection of a checked project) and
//! reports, per export, whether it was added, removed, or had its parameter or
//! result types change. It never touches the `semaprax.lock` format; the
//! baseline descriptor is emitted and stored separately.

use std::collections::BTreeMap;

use serde_json::Value;

use super::scalar_wit::{MAX_SCALAR_WIT_DESCRIPTOR_BYTES, SCALAR_WIT_INTERFACE_SCHEMA};
use crate::diagnostic::Diagnostic;

/// The schema of the fine-grained comparison report.
pub const SCALAR_WIT_COMPATIBILITY_SCHEMA: &str = "semaprax.project-scalar-wit-compatibility.v1";
const CODE_FOREIGN: &str = "SPX-J124";

/// The classification of one candidate scalar interface against a baseline.
/// `breaking` is true when any export was removed or had a parameter or result
/// type change; a purely additive interface is not breaking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarWitCompatibility {
    breaking: bool,
    report: String,
}

impl ScalarWitCompatibility {
    pub fn breaking(&self) -> bool {
        self.breaking
    }

    pub fn report(&self) -> &str {
        &self.report
    }
}

struct Signature {
    parameters: Vec<String>,
    result: String,
}

/// Classify the change from a `baseline` scalar WIT descriptor to a `candidate`
/// one. Both are `semaprax.project.scalar-wit-interface.v1` JSON as emitted by
/// `semaprax lock --emit-interface`; a descriptor that is not that schema
/// rejects with `SPX-J124`. Only the export signatures and the interface digest
/// are compared, never the revision fields, so two descriptors of the same
/// interface at different project revisions compare as compatible.
pub fn classify_scalar_wit_change(
    baseline: &str,
    candidate: &str,
) -> Result<ScalarWitCompatibility, Vec<Diagnostic>> {
    let (base_digest, base) = parse_descriptor(baseline, "baseline")?;
    let (head_digest, head) = parse_descriptor(candidate, "candidate")?;

    let mut changes: Vec<(String, &'static str, &'static str, String)> = Vec::new();
    let mut breaking = false;
    let mut note =
        |export: String, kind: &'static str, classification: &'static str, detail: String| {
            if classification == "breaking" {
                breaking = true;
            }
            changes.push((export, kind, classification, detail));
        };

    for (id, base_sig) in &base {
        match head.get(id) {
            None => note(id.clone(), "export-removed", "breaking", String::new()),
            Some(head_sig) => {
                if base_sig.result != head_sig.result {
                    note(
                        id.clone(),
                        "result-type-changed",
                        "breaking",
                        format!("{} became {}", base_sig.result, head_sig.result),
                    );
                } else if base_sig.parameters.len() != head_sig.parameters.len() {
                    note(
                        id.clone(),
                        "parameter-count-changed",
                        "breaking",
                        format!(
                            "{} became {}",
                            base_sig.parameters.len(),
                            head_sig.parameters.len()
                        ),
                    );
                } else if let Some((position, (was, now))) = base_sig
                    .parameters
                    .iter()
                    .zip(&head_sig.parameters)
                    .enumerate()
                    .find(|(_, (was, now))| was != now)
                {
                    note(
                        id.clone(),
                        "parameter-type-changed",
                        "breaking",
                        format!("parameter {position}: {was} became {now}"),
                    );
                }
            }
        }
    }
    for id in head.keys() {
        if !base.contains_key(id) {
            note(id.clone(), "export-added", "nonbreaking", String::new());
        }
    }

    // The change rows are collected in (removed/changed by base order) then
    // (added by candidate order); sort by export id for a stable report.
    changes.sort_by(|left, right| left.0.cmp(&right.0));

    let verdict = if breaking { "breaking" } else { "compatible" };
    let change_rows = changes
        .iter()
        .map(|(export, kind, classification, detail)| {
            format!(
                "{{\"classification\":{},\"detail\":{},\"export\":{},\"kind\":{}}}",
                json_string(classification),
                json_string(detail),
                json_string(export),
                json_string(kind),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let report = format!(
        "{{\"baseline\":{},\"candidate\":{},\"changes\":[{change_rows}],\"schema\":{},\"verdict\":{}}}\n",
        json_string(&base_digest),
        json_string(&head_digest),
        json_string(SCALAR_WIT_COMPATIBILITY_SCHEMA),
        json_string(verdict),
    );
    Ok(ScalarWitCompatibility { breaking, report })
}

fn parse_descriptor(
    descriptor: &str,
    role: &str,
) -> Result<(String, BTreeMap<String, Signature>), Vec<Diagnostic>> {
    if descriptor.len() > MAX_SCALAR_WIT_DESCRIPTOR_BYTES {
        return Err(foreign(format!(
            "{role} scalar interface descriptor exceeds {MAX_SCALAR_WIT_DESCRIPTOR_BYTES} bytes"
        )));
    }
    let value: Value = serde_json::from_str(descriptor).map_err(|_| {
        foreign(format!(
            "{role} scalar interface descriptor is not a JSON object"
        ))
    })?;
    if value.get("schema").and_then(Value::as_str) != Some(SCALAR_WIT_INTERFACE_SCHEMA) {
        return Err(foreign(format!(
            "{role} scalar interface descriptor does not carry schema {SCALAR_WIT_INTERFACE_SCHEMA}"
        )));
    }
    let digest = value
        .get("wit_digest")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let mut exports = BTreeMap::new();
    for export in value
        .get("exports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = export.get("stable_id").and_then(Value::as_str) else {
            return Err(foreign(format!(
                "{role} scalar interface export has no stable id"
            )));
        };
        let parameters = export
            .get("parameters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|parameter| parameter.as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        let result = export
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        exports.insert(id.to_owned(), Signature { parameters, result });
    }
    Ok((digest, exports))
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string is JSON")
}

fn foreign(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io(CODE_FOREIGN, message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(exports: &[(&str, &[&str], &str)]) -> String {
        let rows = exports
            .iter()
            .map(|(id, parameters, result)| {
                let params = parameters
                    .iter()
                    .map(|parameter| format!("\"{parameter}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"stable_id\":\"{id}\",\"wit_name\":\"{id}\",\"parameters\":[{params}],\"result\":\"{result}\"}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"{SCALAR_WIT_INTERFACE_SCHEMA}\",\"project_revision\":\"sha256:rev\",\"exports\":[{rows}],\"wit_digest\":\"sha256:wd\"}}"
        )
    }

    fn classify(base: &str, candidate: &str) -> (bool, Value) {
        let compatibility = classify_scalar_wit_change(base, candidate).unwrap();
        let report: Value = serde_json::from_str(compatibility.report()).unwrap();
        assert_eq!(
            report["verdict"],
            if compatibility.breaking() {
                "breaking"
            } else {
                "compatible"
            }
        );
        (compatibility.breaking(), report)
    }

    #[test]
    fn identical_interfaces_are_compatible_regardless_of_revision() {
        let base = descriptor(&[("pkg.add", &["s64", "s64"], "s64")]);
        // A different project revision, same exports, is still compatible.
        let candidate = base.replace("sha256:rev", "sha256:other");
        let (breaking, report) = classify(&base, &candidate);
        assert!(!breaking);
        assert!(report["changes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn per_export_signature_changes_are_classified() {
        let base = descriptor(&[
            ("pkg.add", &["s64", "s64"], "s64"),
            ("pkg.drop", &["s64"], "bool"),
        ]);

        // Removed export is breaking.
        let (breaking, report) =
            classify(&base, &descriptor(&[("pkg.add", &["s64", "s64"], "s64")]));
        assert!(breaking);
        assert_eq!(report["changes"][0]["kind"], "export-removed");
        assert_eq!(report["changes"][0]["export"], "pkg.drop");

        // Added export is not breaking.
        let (breaking, report) = classify(
            &base,
            &descriptor(&[
                ("pkg.add", &["s64", "s64"], "s64"),
                ("pkg.drop", &["s64"], "bool"),
                ("pkg.mul", &["s64", "s64"], "s64"),
            ]),
        );
        assert!(!breaking);
        assert_eq!(report["changes"][0]["kind"], "export-added");
        assert_eq!(report["changes"][0]["export"], "pkg.mul");

        // Result type change is breaking.
        let (breaking, report) = classify(
            &base,
            &descriptor(&[
                ("pkg.add", &["s64", "s64"], "s64"),
                ("pkg.drop", &["s64"], "s64"),
            ]),
        );
        assert!(breaking);
        let drop = report["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["export"] == "pkg.drop")
            .unwrap();
        assert_eq!(drop["kind"], "result-type-changed");
        assert_eq!(drop["detail"], "bool became s64");

        // Parameter count change is breaking.
        let (breaking, report) = classify(
            &base,
            &descriptor(&[("pkg.add", &["s64"], "s64"), ("pkg.drop", &["s64"], "bool")]),
        );
        assert!(breaking);
        let add = report["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["export"] == "pkg.add")
            .unwrap();
        assert_eq!(add["kind"], "parameter-count-changed");

        // Parameter type change is breaking and names the position.
        let (breaking, report) = classify(
            &base,
            &descriptor(&[
                ("pkg.add", &["s32", "s64"], "s64"),
                ("pkg.drop", &["s64"], "bool"),
            ]),
        );
        assert!(breaking);
        let add = report["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["export"] == "pkg.add")
            .unwrap();
        assert_eq!(add["kind"], "parameter-type-changed");
        assert_eq!(add["detail"], "parameter 0: s64 became s32");
    }

    #[test]
    fn a_foreign_descriptor_is_rejected() {
        let base = descriptor(&[("pkg.add", &["s64"], "s64")]);
        assert_eq!(
            classify_scalar_wit_change("{}", &base).unwrap_err()[0].code,
            "SPX-J124"
        );
        assert!(classify_scalar_wit_change(&base, "not json").is_err());
    }
}
