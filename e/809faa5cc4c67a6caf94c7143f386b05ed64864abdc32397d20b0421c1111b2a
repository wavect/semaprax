//! Keep the original refused selected domains and genuine ordinary success.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn original_scope(text: &str, id: &str) {
    let original = program(text);
    crate::cleanup_plan::replay::validate_program(&original).unwrap();
    let f = original
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap();
    assert!(!crate::hir::lexical_exit::requires(f).unwrap());
    // The original owning subject is not a Copy-profile success fixture.
    // Retain both full old/current six-field refusals and complete final Drop.
    same_owner(&original, id, false);
    let source = primitive_fixture::Context::new(&original);
    let expected = replay_error(f, "selected scalar helper body contains non-Copy storage");
    for old in [true, false] {
        assert_eq!(owner(&source, f, old), Err(expected.clone()));
    }
    // Also retain the exact old failed forced-lexical acquisition. Genuine
    // lexical subjects are separate authored inputs, not a forged mode flag.
    let mut b = ReplayBudget::with_physical(f, LIMIT).unwrap();
    let error = match selected_context::Context::new_paid(&source, f, &mut b, true) {
        Ok(_) => panic!("an ordinary fixture cannot grant lexical authority"),
        Err(error) => render(error, f),
    };
    assert_eq!(
        error,
        replay_error(
            f,
            "selected retained lexical replay requires an actual lexical function"
        )
    );
}
fn digest(raw: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)))
}
fn unreachable_fixtures() {
    for original in [
        SCALAR_UNREACHABLE,
        super::strings::CONCAT_UNREACHABLE,
        super::strings::LEXICAL_CONCAT_UNREACHABLE,
        super::renewal::SOURCE_UNREACHABLE,
        super::renewal::LEXICAL_SOURCE_UNREACHABLE,
    ] {
        let at = original.rfind("\n 0\n}").unwrap() + 2;
        let line = original[..at].bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = at - original[..at].rfind('\n').unwrap();
        assert_eq!(
            crate::check(original, "selected-path-family.spx").unwrap_err(),
            vec![Diagnostic {
                code: "SPX-P209",
                severity: crate::diagnostic::Severity::Error,
                message: "code after an unconditional return is unreachable".into(),
                path: Some("selected-path-family.spx".to_owned()),
                span: Some(crate::ast::Span {
                    start: at,
                    end: at + 1,
                    line,
                    column
                }),
                help: Some(
                    "remove the unreachable code or put it on a non-returning guard path".into()
                ),
            }],
        );
    }
}
pub(super) fn source_custody() {
    unreachable_fixtures();
    let manifest = include_bytes!("fixture_scope_before/source_manifest.json");
    assert_eq!(manifest.len(), MANIFEST_LENGTH);
    assert_eq!(digest(manifest), MANIFEST_SHA);
    let manifest: serde_json::Value = serde_json::from_slice(manifest).unwrap();
    assert_eq!(manifest.as_array().unwrap().len(), 5);
    let inverse = include_bytes!("fixture_scope_before/inverse.json");
    assert_eq!(inverse.len(), INVERSE_LENGTH);
    assert_eq!(digest(inverse), INVERSE_SHA);
    let inverse: serde_json::Value = serde_json::from_slice(inverse).unwrap();
    assert_eq!(inverse.as_array().unwrap().len(), 5);
    for (source, row) in manifest
        .as_array()
        .unwrap()
        .iter()
        .zip(inverse.as_array().unwrap())
    {
        let path = source["path"].as_str().unwrap();
        assert_eq!(path, row["current"].as_str().unwrap());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let raw = std::fs::read(root.join(path)).unwrap();
        assert_eq!(raw.len(), row["current_length"].as_u64().unwrap() as usize);
        assert_eq!(digest(&raw), row["current_sha256"].as_str().unwrap());
        let mut actual = String::from_utf8(raw).unwrap();
        if let Some(prior) = row.get("exact_prior_current_record") {
            assert_eq!(prior["current"], row["current"]);
            let edits = row["edits"].as_array().unwrap();
            let count = row["current_acquisition_count"].as_u64().unwrap() as usize;
            assert!((1..=2).contains(&count));
            assert_eq!(edits[0]["from"].as_str().unwrap(), actual);
            let immediate = edits[0]["to"].as_str().unwrap();
            assert_eq!(
                immediate.len(),
                row["immediate_before_length"].as_u64().unwrap() as usize
            );
            assert_eq!(
                digest(immediate.as_bytes()),
                row["immediate_before_sha256"].as_str().unwrap()
            );
            if count == 2 {
                assert_eq!(edits[1]["from"].as_str().unwrap(), immediate);
            }
            let prior_current = edits[count - 1]["to"].as_str().unwrap();
            assert_eq!(
                prior_current.len(),
                prior["current_length"].as_u64().unwrap() as usize
            );
            assert_eq!(
                digest(prior_current.as_bytes()),
                prior["current_sha256"].as_str().unwrap()
            );
            assert_eq!(&edits[count..], prior["edits"].as_array().unwrap());
        }
        for edit in row["edits"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            assert_eq!(actual.matches(from).count(), 1);
            actual = actual.replacen(from, edit["to"].as_str().unwrap(), 1);
        }
        let before = std::fs::read(root.join(source["before"].as_str().unwrap())).unwrap();
        assert_eq!(before.len(), source["length"].as_u64().unwrap() as usize);
        assert_eq!(digest(&before), source["sha256"].as_str().unwrap());
        assert_eq!(actual.as_bytes(), before);
    }
}
const MANIFEST_LENGTH: usize = 1426;
const MANIFEST_SHA: &str = "a36d7cb733abcec14387f901e543b56afb2b6e34777a9e1f59415e0129948aef";
const INVERSE_LENGTH: usize = 136427;
const INVERSE_SHA: &str = "f05591a38207b1df416e13eee8d1904225522471b361b1bbfb9ca83eaa03824b";
