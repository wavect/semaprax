use std::fmt::Write as _;

const INDEX_MAX_BYTES: usize = 256;
const INDEX_MAX_CODES: usize = 12;
const INDEX_FOOTER: &str =
    "Fix: semaprax help diagnostic <code>\nAll: semaprax help language mistakes-index\n";

pub(super) fn response(query: &str, entries: &[serde_json::Value]) -> Result<String, String> {
    if query == "codes" {
        return Ok(shortlist(entries));
    }

    let entry = entries
        .iter()
        .find(|entry| entry["code"].as_str() == Some(query));
    let Some(entry) = entry else {
        return Err(format!("diagnostic help has no exact match for `{query}`"));
    };
    let mut output = format!("{query}\n");
    for (index, row) in entry["rows"]
        .as_array()
        .expect("generated diagnostic-help entry must contain rows")
        .iter()
        .enumerate()
    {
        if index > 0 {
            output.push('\n');
        }
        writeln!(
            output,
            "wrote: {}",
            row["wrote"]
                .as_str()
                .expect("generated diagnostic-help row must describe the attempt")
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "fix: {}",
            row["fix"]
                .as_str()
                .expect("generated diagnostic-help row must describe the fix")
        )
        .expect("writing to a string cannot fail");
    }
    Ok(output)
}

fn shortlist(entries: &[serde_json::Value]) -> String {
    // Prioritize codes with more indexed failed forms, then exact code. This
    // order describes advice coverage, not measured diagnostic frequency.
    let mut ranked: Vec<_> = entries.iter().collect();
    ranked.sort_by_key(|entry| {
        (
            std::cmp::Reverse(entry["rows"].as_array().unwrap().len()),
            entry["code"].as_str().unwrap(),
        )
    });
    let mut output = String::from("Common diagnostic codes:\n ");
    for entry in ranked.into_iter().take(INDEX_MAX_CODES) {
        let code = entry["code"]
            .as_str()
            .expect("generated diagnostic-help entry must have a code");
        if output.len() + 1 + code.len() + 1 + INDEX_FOOTER.len() <= INDEX_MAX_BYTES {
            output.push(' ');
            output.push_str(code);
        }
    }
    output.push('\n');
    output.push_str(INDEX_FOOTER);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(code: &str, count: usize) -> serde_json::Value {
        serde_json::json!({
            "code": code,
            "rows": (0..count).map(|_| serde_json::json!({
                "wrote": "invalid form", "fix": "use the admitted form"
            })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn catalogue_growth_preserves_bounded_navigation_and_exact_new_codes() {
        let mut entries: Vec<_> = (0..80)
            .map(|index| entry(&format!("SPX-X{index:03}"), 1))
            .collect();
        for code in ["SPX-T274", "SPX-T275", "SPX-T309", "SPX-T204", "SPX-T205"] {
            entries.push(entry(code, 1));
            assert_eq!(
                response(code, &entries).unwrap(),
                format!("{code}\nwrote: invalid form\nfix: use the admitted form\n")
            );
        }
        entries.push(entry("SPX-P106", 9));
        entries.push(entry("SPX-T203", 3));
        let output = response("codes", &entries).unwrap();
        assert!(output.starts_with("Common diagnostic codes:\n  SPX-P106 SPX-T203 "));
        assert!(output.ends_with(INDEX_FOOTER));
        assert!(!output.contains("SPX-X079"));
        assert_eq!(
            response("SPX-X079", &entries).unwrap(),
            "SPX-X079\nwrote: invalid form\nfix: use the admitted form\n"
        );
        assert!(output.len() <= INDEX_MAX_BYTES);
        assert!(semaprax::agent_economics::lexical_tokens(&output) <= 100);
        assert_eq!(output.lines().count(), 4);
        assert_eq!(
            output.lines().nth(1).unwrap().split_whitespace().count(),
            12
        );
        entries.reverse();
        assert_eq!(response("codes", &entries).unwrap(), output);
        assert_eq!(
            response("spx-t274", &entries).unwrap_err(),
            "diagnostic help has no exact match for `spx-t274`"
        );
        assert_eq!(
            response("SPX-T27", &entries).unwrap_err(),
            "diagnostic help has no exact match for `SPX-T27`"
        );
    }

    #[test]
    fn shortlist_preserves_whole_codes_and_navigation_at_the_byte_bound() {
        let code_bytes =
            INDEX_MAX_BYTES - "Common diagnostic codes:\n  ".len() - 1 - INDEX_FOOTER.len();
        let fitting_code = format!("SPX-{}", "X".repeat(code_bytes - 4));
        let fitting = response("codes", &[entry(&fitting_code, 1)]).unwrap();
        assert_eq!(fitting.len(), INDEX_MAX_BYTES);
        assert!(fitting.contains(&fitting_code));
        let overflow_code = format!("{fitting_code}X");
        let overflow = response("codes", &[entry(&overflow_code, 1)]).unwrap();
        assert!(!overflow.contains(&fitting_code));
        assert!(overflow.ends_with(INDEX_FOOTER));
        let long_code = format!("SPX-{}", "X".repeat(INDEX_MAX_BYTES));
        let entries = vec![entry(&long_code, 9), entry("SPX-T208", 1)];
        let output = response("codes", &entries).unwrap();
        assert_eq!(
            output,
            format!("Common diagnostic codes:\n  SPX-T208\n{INDEX_FOOTER}")
        );
        assert!(output.len() <= INDEX_MAX_BYTES);
        assert!(response(&long_code, &entries)
            .unwrap()
            .starts_with(&long_code));
    }
}
