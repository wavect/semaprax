use std::ffi::OsString;
use std::path::PathBuf;

use semaprax_doctor_release::{create_release, key_information, ReleaseInputs};

fn main() {
    if let Err(error) = run(std::env::args_os()) {
        eprintln!("semaprax-doctor-release: {error}");
        std::process::exit(2);
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let _program = args.next();
    let mode = utf8(args.next(), "mode")?;
    if mode == "key-info" {
        let key = take_option(&mut args, "--signing-key")?;
        if args.next().is_some() {
            return Err("unknown or repeated argument".into());
        }
        print!("{}", key_information(&key)?);
        return Ok(());
    }
    if mode != "create" {
        return Err("mode must be create or key-info".into());
    }
    let request = take_option(&mut args, "--request")?;
    let bundle = take_option(&mut args, "--bundle")?;
    let launcher = take_option(&mut args, "--launcher")?;
    let worker = take_option(&mut args, "--worker")?;
    let collector = take_option(&mut args, "--collector")?;
    let provisioner = take_option(&mut args, "--provisioner")?;
    let selector = utf8(args.next(), "--selector")?;
    if selector != "--selector" {
        return Err("arguments must use canonical order".into());
    }
    let selector = utf8(args.next(), "selector")?;
    let architecture_flag = utf8(args.next(), "--architecture")?;
    if architecture_flag != "--architecture" {
        return Err("arguments must use canonical order".into());
    }
    let architecture = match utf8(args.next(), "architecture")?.as_str() {
        "x86_64" => 1,
        "aarch64" => 2,
        _ => return Err("architecture must be x86_64 or aarch64".into()),
    };
    let target_flag = utf8(args.next(), "--target")?;
    if target_flag != "--target" {
        return Err("arguments must use canonical order".into());
    }
    let target = match utf8(args.next(), "target")?.as_str() {
        "contributor" => 0,
        "native" => 1,
        "web" => 2,
        "all" => 3,
        _ => return Err("target must be contributor, native, web, or all".into()),
    };
    let release_version = take_text_option(&mut args, "--release-version")?;
    let release_commit = take_text_option(&mut args, "--release-commit")?;
    let target_triple = take_text_option(&mut args, "--target-triple")?;
    let signing_key = take_option(&mut args, "--signing-key")?;
    let output_directory = take_option(&mut args, "--output-directory")?;
    if args.next().is_some() {
        return Err("unknown or repeated argument".into());
    }
    create_release(&ReleaseInputs {
        request,
        bundle,
        launcher,
        worker,
        collector,
        provisioner,
        selector,
        architecture,
        target,
        release_version,
        release_commit,
        target_triple,
        signing_key,
        output_directory,
    })
}

fn take_text_option(
    args: &mut impl Iterator<Item = OsString>,
    expected: &str,
) -> Result<String, String> {
    if utf8(args.next(), expected)? != expected {
        return Err("arguments must use canonical order".into());
    }
    utf8(args.next(), expected)
}

fn take_option(
    args: &mut impl Iterator<Item = OsString>,
    expected: &str,
) -> Result<PathBuf, String> {
    if utf8(args.next(), expected)? != expected {
        return Err("arguments must use canonical order".into());
    }
    let value = args
        .next()
        .ok_or_else(|| format!("{expected} requires one path"))?;
    if value.is_empty() {
        return Err(format!("{expected} requires one nonempty path"));
    }
    Ok(PathBuf::from(value))
}

fn utf8(value: Option<OsString>, name: &str) -> Result<String, String> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .into_string()
        .map_err(|_| format!("{name} must be UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_arguments_are_closed() {
        assert!(run(["release", "unknown"].into_iter().map(OsString::from)).is_err());
        assert!(run(["release", "key-info", "--unknown", "key"]
            .into_iter()
            .map(OsString::from))
        .is_err());
        assert!(
            run(["release", "key-info", "--signing-key", "key", "surplus"]
                .into_iter()
                .map(OsString::from))
            .is_err()
        );
    }
}
