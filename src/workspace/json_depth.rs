use super::*;

pub(super) fn validate_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for byte in source.bytes() {
        if string {
            if escape {
                escape = false
            } else if byte == b'\\' {
                escape = true
            } else if byte == b'"' {
                string = false
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return Err(format_error("workspace JSON exceeds depth 8"));
                }
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| format_error("workspace JSON is unbalanced"))?;
            }
            _ => {}
        }
    }
    if string || depth != 0 {
        return Err(format_error("workspace JSON is unbalanced"));
    }
    Ok(())
}
