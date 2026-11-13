use super::super::{digest, held_4a6eb};
use std::path::Path;

const INVERSE: &str = include_str!("main_e177_borrowed_context_source.json");
const BEFORE_SHA: &str = "641da07da0eacec27e7afb717b5f5ddd75800d2f291b7d264d94a1d39bddb9d8";
const BEFORE: &str =
    include_str!("../../e/641da07da0eacec27e7afb717b5f5ddd75800d2f291b7d264d94a1d39bddb9d8");

pub(super) fn restore(current: &str) -> String {
    assert_eq!(
        digest(INVERSE.as_bytes()),
        "046e6cd09f8b73bbd9d1b44afdc5fa8819a7195aced0e52446129e6febbe6e57"
    );
    let physical = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("e/{BEFORE_SHA}"));
    assert_eq!(held_4a6eb(&physical, 80892, BEFORE_SHA).unwrap(), BEFORE);
    let rows: serde_json::Value = serde_json::from_str(INVERSE).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);

    let mut result = current.to_owned();
    for row in rows {
        let begin = row["begin"].as_str().unwrap();
        let end = row["end"].as_str().unwrap();
        let (head, tail) = result.split_once(begin).unwrap();
        let (section, rest) = tail.split_once(end).unwrap();
        let mut restored = section.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let offset = edit[0].as_u64().unwrap() as usize;
            let after = edit[1].as_str().unwrap();
            let before = edit[2].as_str().unwrap();
            let range = offset..offset + after.len();
            assert_eq!(&restored[range.clone()], after);
            restored.replace_range(range, before);
        }
        let archived = BEFORE
            .split_once(begin)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0;
        assert_eq!(restored, archived, "complete pre-format borrowed sources");
        result = format!("{head}{begin}{restored}{end}{rest}");
    }
    result
}
