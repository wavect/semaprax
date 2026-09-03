//! Mechanical boundary for the root crate's sole OS-process quarantine.

use std::fs;
use std::path::{Path, PathBuf};

const QUARANTINE: &str = "src/project/candidate/git_publication/process/platform.rs";

fn rust_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn char_literal_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }
    let mut index = start + 1;
    if bytes.get(index) == Some(&b'\\') {
        index += 1;
        match bytes.get(index)? {
            b'u' => {
                index += 1;
                if bytes.get(index) != Some(&b'{') {
                    return None;
                }
                index += 1;
                while bytes
                    .get(index)
                    .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'_')
                {
                    index += 1;
                }
                if bytes.get(index) != Some(&b'}') {
                    return None;
                }
                index += 1;
            }
            b'x' => index += 3,
            _ => index += 1,
        }
    } else {
        let character = source.get(index..)?.chars().next()?;
        if matches!(character, '\n' | '\r' | '\'') {
            return None;
        }
        index += character.len_utf8();
    }
    (bytes.get(index) == Some(&b'\'')).then_some(index + 1)
}

fn code_without_comments_or_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            output[index..end].fill(b' ');
            index = end;
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            let start = index;
            index += 2;
            let mut depth = 1_usize;
            while index < bytes.len() && depth != 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            output[start..index].fill(b' ');
            continue;
        }

        let (prefix_len, raw_start) = if bytes[index] == b'r' {
            (1, index + 1)
        } else if bytes[index..].starts_with(b"br") || bytes[index..].starts_with(b"cr") {
            (2, index + 2)
        } else {
            (0, index)
        };
        if prefix_len != 0 {
            let hashes = bytes[raw_start..]
                .iter()
                .take_while(|byte| **byte == b'#')
                .count();
            let quote = raw_start + hashes;
            if bytes.get(quote) == Some(&b'"') {
                let start = index;
                index = quote + 1;
                while index < bytes.len() {
                    if bytes[index] == b'"'
                        && bytes
                            .get(index + 1..index + 1 + hashes)
                            .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
                    {
                        index += 1 + hashes;
                        break;
                    }
                    index += 1;
                }
                output[start..index].fill(b' ');
                continue;
            }
        }

        if let Some(end) = char_literal_end(source, index) {
            output[index..end].fill(b' ');
            index = end;
            continue;
        }

        if bytes[index] == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else {
                    let closing = bytes[index] == b'"';
                    index += 1;
                    if closing {
                        break;
                    }
                }
            }
            output[start..index].fill(b' ');
            continue;
        }
        index += 1;
    }
    String::from_utf8(output).unwrap()
}

fn relaxing_unsafe_attributes(source: &str) -> Vec<String> {
    let code = code_without_comments_or_strings(source);
    let bytes = code.as_bytes();
    let mut attributes = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }
        let mut open = index + 1;
        if bytes.get(open) == Some(&b'!') {
            open += 1;
        }
        if bytes.get(open) != Some(&b'[') {
            index += 1;
            continue;
        }
        let start = index;
        index = open + 1;
        let mut depth = 1_usize;
        while index < bytes.len() && depth != 0 {
            match bytes[index] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            index += 1;
        }
        let compact = code[start..index]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if compact.contains("unsafe_code")
            && (compact.contains("allow(")
                || compact.contains("expect(")
                || compact.contains("warn("))
        {
            attributes.push(compact);
        }
    }
    attributes
}

#[test]
fn root_unsafe_is_confined_to_the_held_git_process_quarantine() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert_eq!(manifest.matches("unsafe_code = \"deny\"").count(), 1);

    let quarantine = fs::read_to_string(root.join(QUARANTINE)).unwrap();
    assert!(quarantine.starts_with("//! Quarantined Unix process boundary"));
    assert_eq!(quarantine.matches("#![allow(unsafe_code)]").count(), 1);
    assert_eq!(quarantine.matches("unsafe_code").count(), 1);
    assert!(!quarantine.contains("pub fn "));

    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    let exceptions = sources
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).unwrap();
            (!relaxing_unsafe_attributes(&source).is_empty())
                .then(|| path.strip_prefix(root).unwrap().to_path_buf())
        })
        .collect::<Vec<_>>();
    assert_eq!(exceptions, [PathBuf::from(QUARANTINE)]);

    assert!(
        relaxing_unsafe_attributes("const QUOTE: char = '\"'; #[allow/* hidden */(unsafe_code)]")
            .len()
            == 1
    );
    assert_eq!(
        relaxing_unsafe_attributes("#[warn(unsafe_code)] fn lowered() {}").len(),
        1
    );
    assert!(relaxing_unsafe_attributes(
        "const GENERATED: &str = r#\"#[allow(unsafe_code)]\"#; #![forbid(unsafe_code)]"
    )
    .is_empty());
}
