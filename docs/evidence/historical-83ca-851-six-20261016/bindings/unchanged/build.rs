//! Generate the installed diagnostic-token inventory from the exact compiler
//! sources being built. The generator intentionally uses no network, VCS, or
//! filesystem state outside this Cargo package.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=crates");
    println!("cargo:rerun-if-changed=build.rs");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let mut files = Vec::new();
    collect_rust(&root.join("src"), &mut files).expect("scan compiler sources");
    collect_rust(&root.join("crates"), &mut files).expect("scan workspace-member sources");
    files.sort();

    let mut codes = BTreeMap::<String, BTreeSet<(String, u32)>>::new();
    let mut dynamic_sites = BTreeSet::<(String, u32)>::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("source remains below package root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).expect("Rust source is UTF-8");
        for (offset, code) in diagnostic_tokens(&source) {
            codes
                .entry(code.to_owned())
                .or_default()
                .insert((relative.clone(), line_number(&source, offset)));
        }
        for needle in [
            "Diagnostic::error(",
            "Diagnostic::warning(",
            "Diagnostic::io(",
        ] {
            let mut rest = source.as_str();
            let mut base = 0usize;
            while let Some(found) = rest.find(needle) {
                let offset = base + found;
                let argument = &source[offset + needle.len()..];
                if !argument.trim_start().starts_with("\"SPX-") {
                    dynamic_sites.insert((relative.clone(), line_number(&source, offset)));
                }
                let advance = found + needle.len();
                base += advance;
                rest = &rest[advance..];
            }
        }
    }

    let mut generated = String::from("&[\n");
    for (code, occurrences) in &codes {
        generated.push_str("    (");
        generated.push_str(&format!("{code:?}, &["));
        for (path, line) in occurrences {
            generated.push_str(&format!("({path:?}, {line}),"));
        }
        generated.push_str("]),\n");
    }
    generated.push_str("]\n");
    let mut dynamic = String::from("&[\n");
    for (path, line) in &dynamic_sites {
        dynamic.push_str(&format!("    ({path:?}, {line}),\n"));
    }
    dynamic.push_str("]\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(out.join("installed_diagnostic_codes.rs"), generated)
        .expect("write generated diagnostic inventory");
    fs::write(out.join("installed_dynamic_diagnostic_sites.rs"), dynamic)
        .expect("write generated dynamic-site inventory");
}

fn collect_rust(path: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_rust(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
    Ok(())
}

fn diagnostic_tokens(source: &str) -> Vec<(usize, &str)> {
    let bytes = source.as_bytes();
    let mut output = Vec::new();
    let mut cursor = 0usize;
    while cursor + 4 <= bytes.len() {
        let Some(relative) = source[cursor..].find("SPX-") else {
            break;
        };
        let start = cursor + relative;
        let mut end = start + 4;
        while end < bytes.len() && (bytes[end].is_ascii_uppercase() || bytes[end].is_ascii_digit())
        {
            end += 1;
        }
        let token = &source[start..end];
        if valid_code(token)
            && (start == 0
                || !(bytes[start - 1].is_ascii_alphanumeric()
                    || matches!(bytes[start - 1], b'_' | b'-')))
            && (end == bytes.len()
                || !(bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'-')))
        {
            output.push((start, token));
        }
        cursor = end.max(start + 4);
    }
    output
}

fn valid_code(token: &str) -> bool {
    let body = token.strip_prefix("SPX-").unwrap_or_default().as_bytes();
    body.len() >= 4
        && body.len() <= 16
        && body[..body.len() - 3].iter().all(u8::is_ascii_uppercase)
        && body[body.len() - 3..].iter().all(u8::is_ascii_digit)
}

fn line_number(source: &str, offset: usize) -> u32 {
    source.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        .saturating_add(1)
        .try_into()
        .expect("Rust source line count fits u32")
}
