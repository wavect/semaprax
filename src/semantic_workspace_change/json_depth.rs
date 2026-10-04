use super::*;

pub(super) fn validate_json_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for byte in source.bytes() {
        if string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| limit("json_depth", semantic_workspace::MAX_JSON_DEPTH))?;
                if depth > semantic_workspace::MAX_JSON_DEPTH {
                    return Err(limit("json_depth", semantic_workspace::MAX_JSON_DEPTH));
                }
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    grammar("Semantic Workspace Change proposal JSON is unbalanced")
                })?;
            }
            _ => {}
        }
    }
    if string || depth != 0 {
        return Err(grammar(
            "Semantic Workspace Change proposal JSON is unbalanced",
        ));
    }
    Ok(())
}
