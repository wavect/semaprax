//! Source-locked contracts over the production platform text: allocation,
//! ordering, and frozen link-tail evidence that no build alone can enforce.

use super::*;

#[test]
fn prepared_file_syscall_gate_resolves_name_before_entry() {
    TEST_PREPARED_FILE_SYSCALL_ENTRIES.store(0, Ordering::Relaxed);
    assert_eq!(
        enter_prepared_file_syscalls::<()>(Err(Error::Invalid)),
        Err(Error::Invalid)
    );
    assert_eq!(
        TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
        0
    );

    let resolved = ();
    assert!(enter_prepared_file_syscalls(Ok(&resolved)).is_ok());
    assert_eq!(
        TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
        1
    );
}

#[test]
fn production_source_exposes_no_prepared_file_syscall_observer() {
    let source = production_sources();
    assert!(!source.contains(concat!("pub fn reset_prepared_file_", "syscall_entries")));
    assert!(!source.contains(concat!("pub fn prepared_file_", "syscall_entries")));
    assert!(!source.contains(concat!("static PREPARED_FILE_", "SYSCALL_ENTRIES")));
}

#[test]
fn windows_external_reader_handoff_and_discard_rebound_are_exact() {
    let source = WINDOWS_SOURCE;
    let transition_start = source
        .find("pub fn transition_regular_file_to_external_read_prepared")
        .expect("Windows external-reader transition");
    let transition_end = source[transition_start..]
        .find("pub fn recheck_regular")
        .map(|offset| transition_start + offset)
        .expect("end Windows external-reader transition");
    let transition = &source[transition_start..transition_end];
    for required in [
        "recheck_held_regular(tracked)?",
        "hold_regular_file_name_external_read_prepared(directory, name)?",
        "rebound.identity != tracked.identity",
        "rebound.digest != tracked.digest",
    ] {
        assert!(
            transition.contains(required),
            "missing Windows external-reader contract: {required}"
        );
    }
    let reader_start = source
        .find("fn hold_regular_file_name_external_read_prepared")
        .expect("Windows read-compatible holder");
    let reader = &source[reader_start..transition_start];
    assert!(reader.contains("REGULAR_READ_ACCESS"));
    let production = source;
    assert_eq!(
            production
                .matches("hold_regular_file_name_external_read_prepared")
                .count(),
            5,
            "Windows read-compatible holder must serve inventory handoff, executable images, and exact archive admission"
        );

    let discard_start = source
        .find("pub fn discard_owned_stage_prepared")
        .expect("Windows prepared discard");
    let discard_end = source[discard_start..]
        .find("fn disposition_delete")
        .map(|offset| discard_start + offset)
        .expect("end Windows prepared discard");
    let discard = &source[discard_start..discard_end];
    for required in [
        "let mut deletion_handles: [Option<RegularFile>; N]",
        "let rebound = hold_regular_file_name_prepared(stage, name)",
        "let file = settled[index].expect(\"attached settled prefix\")",
        "rebound.identity != identity || rebound.digest != digest",
        "deletion_handles[index] = Some(rebound)",
        "for (deleted, file) in deletion_handles[..attached]",
        "must_close_deletion_handles(&mut deletion_handles[..attached])",
        "DIRECTORY_OWNED_ACCESS",
        "directory_information(&stage_deletion)",
        "disposition_delete_and_close(stage_deletion)",
    ] {
        assert!(
            discard.contains(required),
            "missing Windows discard rebound contract: {required}"
        );
    }
    assert!(
        discard
            .find("must_close_deletion_handles(&mut deletion_handles[..attached])")
            .unwrap()
            < discard
                .find("disposition_delete_and_close(stage_deletion)")
                .unwrap()
    );
    let executable_start = source
        .find("pub fn hold_executable")
        .expect("Windows held executable");
    let executable_end = source[executable_start..]
        .find("pub fn executable_regular_file")
        .map(|offset| executable_start + offset)
        .expect("end Windows held executable");
    let executable = &source[executable_start..executable_end];
    assert!(executable.contains("hold_regular_file_name_external_read_prepared"));
    assert!(!executable.contains("hold_regular_file(directory, name)"));
    let close_start = source
        .find("fn must_close_deletion_handles")
        .expect("Windows deletion-handle settlement");
    let close_end = source[close_start..]
        .find("fn run_argv")
        .map(|offset| close_start + offset)
        .expect("end Windows deletion-handle settlement");
    let close = &source[close_start..close_end];
    for required in [
        "file.into_raw_handle()",
        "close_failed |= unsafe { CloseHandle(handle.cast()) } == 0",
        "std::process::abort()",
    ] {
        assert!(
            close.contains(required),
            "missing Windows deletion-handle settlement: {required}"
        );
    }
}

#[test]
fn linux_prepared_transfer_has_injection_and_allocation_free_copy_fallback_contract() {
    let source = UNIX_SOURCE;
    let link = source
        .find("libc::AT_EMPTY_PATH")
        .expect("Linux prepared transfer uses the held source descriptor");
    let fallback = source[link..]
        .find("fn copy_regular_file_new_prepared")
        .map(|offset| link + offset)
        .expect("Linux fallback is independently authored");
    let linux = &source[link..fallback];
    let linked = linux.find("if result == 0").expect("link success branch");
    let injected = linux
        .find("if fail_before_authentication")
        .expect("debug failure precedes reopen authentication");
    let reopened = linux
        .find("hold_regular_file_name_prepared")
        .expect("prepared destination reopen");
    assert!(linked < injected && injected < reopened);
    for errno in ["libc::EPERM", "libc::EACCES", "libc::EOPNOTSUPP"] {
        assert!(linux.contains(errno));
    }
    assert!(linux.contains("copy_regular_file_new_prepared("));

    let copy = &source[fallback..];
    for required in [
        "libc::O_EXCL",
        "file.write_all(source_bytes)",
        "file.sync_data()",
        "authenticate_regular_file(file)",
    ] {
        assert!(copy.contains(required));
    }
    assert!(!source.contains(concat!("CAP_DAC_", "READ_SEARCH")));
}

#[test]
fn prepared_inventory_exact_source_contract_is_raw_bounded_and_allocation_free() {
    let unix_source = UNIX_SOURCE;
    let linux_start = unix_source
        .find("#[cfg(target_os = \"linux\")]\nfn parse_linux_inventory_records")
        .expect("Linux raw inventory scanner");
    let darwin_start = unix_source[linux_start..]
        .find("#[cfg(target_os = \"macos\")]\nfn parse_darwin_inventory_records")
        .map(|offset| linux_start + offset)
        .expect("Darwin raw inventory scanner");
    let inventory_start = unix_source[darwin_start..]
        .find("pub fn inventory_exact_prepared")
        .map(|offset| darwin_start + offset)
        .expect("Unix prepared inventory entry point");
    let linux = &unix_source[linux_start..darwin_start];
    let darwin = &unix_source[darwin_start..inventory_start];
    for required in [
        "libc::SYS_getdents64",
        "let bytes_limit = libc::c_uint::try_from(capacity)",
        "prepared.storage.fill(u64::MAX)",
        "record < 20",
        "record % std::mem::align_of::<u64>() != 0",
        "next > bytes.len()",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            linux.contains(required),
            "missing Linux contract: {required}"
        );
    }
    for required in [
        "SYS_GETDIRENTRIES64",
        "let bytes_limit: libc::size_t",
        "let mut base: libc::off_t",
        "prepared.storage.fill(u64::MAX)",
        "!record.is_multiple_of(4)",
        "name_length > 1023",
        "name_end >= next",
        "next > bytes.len()",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            darwin.contains(required),
            "missing Darwin contract: {required}"
        );
    }
    for forbidden in ["fdopendir", "readdir", "BTreeSet", "to_vec("] {
        assert!(!linux.contains(forbidden));
        assert!(!darwin.contains(forbidden));
    }

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("pub fn inventory_exact_prepared")
        .expect("Windows prepared inventory entry point");
    let windows_end = windows_source[windows_start..]
        .find("pub fn publish_directory_new")
        .map(|offset| windows_start + offset)
        .expect("end of Windows inventory scanner");
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "FileIdExtdDirectoryRestartInfo",
        "FILE_ID_EXTD_DIR_INFO",
        "prepared.storage.fill(u64::MAX)",
        "entry.FileId.Identifier != tracked.identity.file_id",
        "std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>()",
        "record_header_end > byte_length",
        "next < minimum",
        "next_end > byte_length",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows contract: {required}"
        );
    }
    let full_header_bound = windows
        .find("if record_header_end > byte_length")
        .expect("complete Windows record must fit");
    let entry_reference = windows
        .find("let entry = unsafe")
        .expect("Windows entry reference");
    assert!(full_header_bound < entry_reference);
    assert!(!windows.contains("FILE_ID_BOTH_DIR_INFO"));
    assert!(!windows.contains("String::from_utf16"));
}

#[test]
fn prepared_publish_source_contract_has_no_late_name_or_handle_allocation() {
    let unix_source = UNIX_SOURCE;
    let unix_start = unix_source
        .find("fn observe_publish_rebound")
        .expect("Unix prepared publish");
    let unix_end = unix_source[unix_start..]
        .find("pub fn discard_owned_stage_prepared")
        .map(|offset| unix_start + offset)
        .expect("end Unix prepared publish");
    let unix = &unix_source[unix_start..unix_end];
    for required in [
        "prepared.remaining != 1",
        "prepared.exact_capacity",
        "relative_name_arena_cstr",
        "observe_publish_rebound",
        "prepared_directory_identity(stage)",
        "libc::SYS_renameat2",
        "renameatx_np",
    ] {
        assert!(
            unix.contains(required),
            "missing Unix publish contract: {required}"
        );
    }
    for forbidden in ["c_name(", "try_clone", "CString::new", "Vec::"] {
        assert!(
            !unix.contains(forbidden),
            "late Unix publish operation: {forbidden}"
        );
    }

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("fn observe_publish_rebound")
        .expect("Windows prepared publish");
    let windows_end = windows_source[windows_start..]
        .find("pub fn discard_owned_stage_prepared")
        .map(|offset| windows_start + offset)
        .expect("end Windows prepared publish");
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "prepared.remaining != 1",
        "prepared.exact_capacity",
        "relative_file_arena",
        "observe_publish_rebound",
        "NtSetInformationFile",
        "FileRenameInformation",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows publish contract: {required}"
        );
    }
    for forbidden in [
        "prepare_relative_name(",
        "named_information(",
        "try_clone",
        "collect::<Vec",
        "SetFileInformationByHandle(",
        "FileRenameInfoEx",
    ] {
        assert!(
            !windows.contains(forbidden),
            "late Windows publish operation: {forbidden}"
        );
    }
}

#[test]
fn settlement_failure_injection_is_test_local_and_has_no_ambient_control() {
    let source = production_sources();
    let ambient_name = ["SEMAPRAX_NATIVE_RUST", "_INTEROP_TEST_SETTLEMENT_FAILURE"].concat();
    assert!(!source.contains(&ambient_name));
    assert!(source.contains("#[cfg(test)]\nstatic TEST_SETTLEMENT_FAILURES"));
    assert!(source.contains("#[cfg(not(test))]\n        {\n            false\n        }"));
    let obsolete_function = ["fn injected_settlement_", "failure(point: &str)"].concat();
    assert!(!source.contains(&obsolete_function));
}

#[test]
fn prepared_process_arena_is_exact_and_consumes_twelve_without_growth() {
    let plan = super::platform::prepare_process_arena_plan(12).unwrap();
    let required = super::platform::prepared_process_arena_plan_capacity(&plan);
    let mut arena = super::platform::materialize_process_arena(plan).unwrap();
    let capacity = super::platform::prepared_process_arena_owned_capacity(&arena);
    assert_eq!(capacity, required);
    #[cfg(windows)]
    assert!((131_080 + 8..=1_245_188).contains(&capacity));
    for remaining in (0..12).rev() {
        super::platform::consume_process_arena(&mut arena).unwrap();
        assert_eq!(
            super::platform::prepared_process_arena_remaining(&arena),
            remaining
        );
        assert_eq!(
            super::platform::prepared_process_arena_owned_capacity(&arena),
            capacity
        );
    }
    assert_eq!(
        super::platform::consume_process_arena(&mut arena),
        Err(Error::OutputLimit)
    );
}

#[cfg(windows)]
#[test]
fn windows_process_arena_attribute_plan_is_exact_aligned_and_bounded() {
    const MAX_ATTRIBUTE_BYTES: usize = 1_048_576;
    for attribute_bytes in [1, 8, 9, 65_537, MAX_ATTRIBUTE_BYTES] {
        let plan = super::platform::process_arena_plan(12, attribute_bytes, 2).unwrap();
        let aligned =
            attribute_bytes.div_ceil(std::mem::size_of::<u64>()) * std::mem::size_of::<u64>();
        assert_eq!(
            super::platform::prepared_process_arena_plan_capacity(&plan),
            131_080 + aligned
        );
        let arena = super::platform::materialize_process_arena(plan).unwrap();
        assert_eq!(
            super::platform::prepared_process_arena_owned_capacity(&arena),
            131_080 + aligned
        );
    }
    assert!(matches!(
        super::platform::process_arena_plan(12, 0, 2),
        Err(Error::Unsupported)
    ));
    assert!(matches!(
        super::platform::process_arena_plan(12, MAX_ATTRIBUTE_BYTES + 1, 2),
        Err(Error::OutputLimit)
    ));

    let include = std::ffi::OsStr::new(r"C:\sdk\include;C:\msvc\include");
    let libraries = std::ffi::OsStr::new(r"C:\sdk\lib;C:\msvc\lib");
    let plan = super::platform::prepare_process_arena_plan_with_environment(
        12,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    let required = super::platform::prepared_process_arena_plan_capacity(&plan);
    let arena = super::platform::materialize_process_arena_with_environment(
        plan,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    assert_eq!(
        super::platform::prepared_process_arena_owned_capacity(&arena),
        required
    );
    let mut expected = "CPATH=C:\\sdk\\include;C:\\msvc\\include\0INCLUDE=C:\\sdk\\include;C:\\msvc\\include\0LIB=C:\\sdk\\lib;C:\\msvc\\lib\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    expected.push(0);
    assert_eq!(
        super::platform::test_prepared_process_environment(&arena),
        expected
    );
}

#[cfg(unix)]
#[test]
fn sysroot_output_is_one_nonempty_absolute_utf8_line() {
    assert_eq!(
        super::platform::one_sysroot_line(b"/toolchain\n"),
        Ok(&b"/toolchain"[..])
    );
    assert_eq!(
        super::platform::one_sysroot_line(b"/toolchain\r\n"),
        Ok(&b"/toolchain"[..])
    );
    for invalid in [
        &b""[..],
        &b"/toolchain"[..],
        &b"\n"[..],
        &b"/one\n/two\n"[..],
        &b"/one\0two\n"[..],
        &[0xff, b'\n'],
    ] {
        assert_eq!(
            super::platform::one_sysroot_line(invalid),
            Err(Error::Invalid)
        );
    }
    let resolver = super::platform::prepare_tool_resolver("rustc", 32_768).unwrap();
    assert!(matches!(
        super::platform::hold_rustc_discovery_prepared(
            resolver,
            std::ffi::OsStr::new("relative-rustc")
        ),
        Err(Error::Invalid)
    ));
}

#[test]
fn direct_rustc_and_windows_process_source_contract_is_closed() {
    let source = [UNIX_SOURCE, WINDOWS_SOURCE].concat();
    let discovery_symbol = ["pub fn hold_rustc_", "discovery_prepared"].concat();
    let direct_compile_symbol = ["pub fn compile_direct_", "rustc_prepared"].concat();
    let generic_worker = ["fn compile_rust_", "prepared_inner"].concat();
    assert_eq!(source.matches(&discovery_symbol).count(), 2);
    assert_eq!(source.matches(&direct_compile_symbol).count(), 2);
    assert_eq!(source.matches(&generic_worker).count(), 2);
    let generic_public = ["pub fn compile_rust_", "prepared("].concat();
    let legacy_public = ["pub fn compile_rust_", "staticlib("].concat();
    let misplaced = ["misplaced_windows_", "direct_rustc"].concat();
    assert!(!source.contains(&generic_public));
    assert!(!source.contains(&legacy_public));
    assert!(!source.contains(&misplaced));

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("fn run_argv(\n    executable: &Executable,\n    cwd: &Directory,\n    arguments: &[String]")
        .unwrap();
    let windows_end = windows_source[windows_start..]
        .find("fn terminate_unassigned")
        .map(|offset| windows_start + offset)
        .unwrap();
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "final_path_prepared(&executable.file.file, &mut process_arena.application)",
        "final_path_prepared(&cwd.file, &mut process_arena.cwd)",
        "process_arena.application.resize(PROCESS_PATH_UNITS, 0)",
        "process_arena.environment.as_ptr().cast()",
        "let mut attribute_bytes = process_arena.attribute_bytes",
        "process_arena.attributes.resize(attribute_words, 0)",
        "let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0]",
        "must_terminate_unassigned(process_handle.raw())",
        "failed |= thread_handle.close().is_err()",
        "failed |= job.close().is_err()",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows process contract: {required}"
        );
    }
    for forbidden in [
        "final_path(&executable.file.file)",
        "OpenOptions",
        "vec![0_u8; attribute_bytes]",
        "String::from_utf16",
        "PathBuf::from",
        "InitializeProcThreadAttributeList(std::ptr::null_mut()",
        "let empty_environment",
    ] {
        assert!(
            !windows.contains(forbidden),
            "late Windows process allocation: {forbidden}"
        );
    }
    let obsolete_attribute_words = ["PROCESS_ATTRIBUTE_", "WORDS"].concat();
    assert!(!source.contains(&obsolete_attribute_words));
    assert!(source.contains("pub fn prepare_process_arena_plan(uses: usize)"));
    assert!(source.contains(
        "InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)"
    ));

    let compile_start = windows_source
        .find("pub fn prepare_c_compile_invocation(")
        .expect("Windows prepared C compile plan");
    let compile_end = windows_source[compile_start..]
        .find("pub fn prepared_c_compile_owned_capacity")
        .map(|offset| compile_start + offset)
        .expect("end Windows prepared C compile plan");
    let compile = &windows_source[compile_start..compile_end];
    assert_eq!(
        compile
            .matches("-mno-incremental-linker-compatible")
            .count(),
        1
    );
    assert!(
        compile.find("target,").unwrap()
            < compile
                .find("\"-mno-incremental-linker-compatible\"")
                .unwrap()
    );
}

#[test]
fn windows_directory_identity_source_excludes_mutable_length_and_binds_all_rechecks() {
    let source = WINDOWS_SOURCE;
    let start = source.find("struct DirectoryIdentity").unwrap();
    let end = source[start..]
        .find("pub struct Directory")
        .map(|offset| start + offset)
        .unwrap();
    let identity = &source[start..end];
    for required in ["volume: u64", "file_id: [u8; 16]", "stable_attributes: u32"] {
        assert!(identity.contains(required));
    }
    assert!(!identity.contains("length:"));

    let windows = source;
    for required in [
        "identity: DirectoryIdentity",
        "directory_identity: Option<DirectoryIdentity>",
        "FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT",
        "stable_attributes != FILE_ATTRIBUTE_DIRECTORY",
        "directory_information(&directory.file)? != directory.identity",
        "directory_information(&rebound)? == directory.identity",
        "directory_information(&rebound)? != stage.identity",
        "Result<DirectoryIdentity, Error>",
    ] {
        assert!(
            windows.contains(required),
            "missing stable Windows directory identity contract: {required}",
        );
    }
}

#[test]
fn linux_rust_staticlib_link_tail_is_frozen_for_prepared_and_legacy_paths() {
    let unix = UNIX_SOURCE;
    let native_start = unix
        .find("const LINUX_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
        .unwrap();
    let native_end = unix[native_start..]
        .find("\n    ];")
        .map(|offset| native_start + offset + "\n    ];".len())
        .unwrap();
    let native = &unix[native_start..native_end];
    let mut previous = 0usize;
    for required in [
        "-lgcc_s",
        "-lutil",
        "-lrt",
        "-lpthread",
        "-lm",
        "-ldl",
        "-lc",
    ] {
        let offset = native.find(required).unwrap();
        assert!(
            offset >= previous,
            "Linux native-static library order changed"
        );
        previous = offset;
    }
    assert_eq!(
            unix.matches("LINUX_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            3,
            "the frozen Linux native-static library tail must have one definition and exactly two link consumers",
        );
    assert_eq!(
            unix.matches("LINUX_LINKER_ARGUMENT").count(),
            3,
            "the absolute Linux linker argument must have one definition and exactly two link consumers",
        );

    let prepared_start = unix.find("pub fn prepare_link_invocation(").unwrap();
    let prepared_end = unix[prepared_start..]
        .find("pub fn prepared_link_owned_capacity(")
        .map(|offset| prepared_start + offset)
        .unwrap();
    let prepared = &unix[prepared_start..prepared_end];
    let prepared_linker = prepared
        .find("values[count] = LINUX_LINKER_ARGUMENT")
        .unwrap();
    let prepared_archive = prepared.find("rust_archive.to_str()").unwrap();
    let prepared_output = prepared.find("output.to_str()").unwrap();
    let prepared_tail = prepared
        .find("for value in LINUX_RUST_STATICLIB_NATIVE_LIBS")
        .unwrap();
    assert!(
        prepared_linker < prepared_archive
            && prepared_archive < prepared_output
            && prepared_output < prepared_tail
    );

    let legacy_start = unix.find("pub fn link_harness(").unwrap();
    let legacy_end = unix[legacy_start..]
        .find("let mut process_arena = prepare_process_arena(1)?")
        .map(|offset| legacy_start + offset)
        .unwrap();
    let legacy = &unix[legacy_start..legacy_end];
    assert!(legacy.contains("arguments.insert(2, argument(LINUX_LINKER_ARGUMENT)?)"));
    let legacy_archive = legacy.find("rust_archive.to_str()").unwrap();
    let legacy_output = legacy.find("output.to_str()").unwrap();
    let legacy_tail = legacy.find("LINUX_RUST_STATICLIB_NATIVE_LIBS").unwrap();
    assert!(legacy_archive < legacy_output && legacy_output < legacy_tail);
}

#[test]
fn windows_rust_staticlib_link_tail_is_frozen_after_the_archive() {
    let windows = WINDOWS_SOURCE;
    let crt_start = windows
        .find("const WINDOWS_DYNAMIC_CRT_LINK_ARGS: [&str; 2]")
        .unwrap();
    let crt_end = windows[crt_start..]
        .find("];")
        .map(|offset| crt_start + offset + 2)
        .unwrap();
    let crt = &windows[crt_start..crt_end];
    assert!(crt.contains("\"-Xlinker\", \"/NODEFAULTLIB:libcmt\""));
    assert_eq!(windows.matches("WINDOWS_DYNAMIC_CRT_LINK_ARGS").count(), 4);
    let native_start = windows
        .find("const WINDOWS_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
        .unwrap();
    let native_end = windows[native_start..]
        .find("\n    ];")
        .map(|offset| native_start + offset + "\n    ];".len())
        .unwrap();
    let native = &windows[native_start..native_end];
    let mut previous = 0usize;
    for required in [
        "-lkernel32",
        "-ladvapi32",
        "-ldbghelp",
        "-lntdll",
        "-luserenv",
        "-lws2_32",
        "-lmsvcrt",
    ] {
        let offset = native.find(required).unwrap();
        assert!(
            offset >= previous,
            "Windows native-static library order changed"
        );
        previous = offset;
    }
    assert_eq!(
            windows
                .matches("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            9,
            "the frozen Windows native-static library tail must have one definition, seven indexed prepared entries, and one legacy consumer",
        );

    let prepared_start = windows.find("pub fn prepare_link_invocation(").unwrap();
    let prepared_end = windows[prepared_start..]
        .find("pub fn prepared_link_owned_capacity(")
        .map(|offset| prepared_start + offset)
        .unwrap();
    let prepared = &windows[prepared_start..prepared_end];
    let arguments_start = prepared.find("let argument_parts:").unwrap();
    let arguments_end = prepared[arguments_start..]
        .find("preflight_windows_command_line(&argument_parts)?")
        .map(|offset| arguments_start + offset)
        .unwrap();
    let arguments = &prepared[arguments_start..arguments_end];
    let prepared_linker = arguments.find("&[\"-fuse-ld=link\"]").unwrap();
    let prepared_vctools = arguments
        .find("&[\"-Xmicrosoft-visualc-tools-root\"]")
        .unwrap();
    let prepared_crt = arguments.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
    let prepared_archive = arguments.find("&[rust_archive]").unwrap();
    let prepared_tail = arguments
        .find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
        .unwrap();
    let prepared_output = arguments.find("&[\"-o\"]").unwrap();
    assert!(
        prepared_vctools < prepared_linker
            && prepared_linker < prepared_crt
            && prepared_crt < prepared_archive
            && prepared_archive < prepared_tail
            && prepared_tail < prepared_output
    );
    assert!(
        prepared.find("linker_units > MAX_TOOL_PATH_UNITS").unwrap()
            < prepared
                .find("Vec::with_capacity(argument_parts.len())")
                .unwrap()
    );

    let legacy_start = windows.find("pub fn link_harness(").unwrap();
    let legacy_end = windows[legacy_start..]
        .find("let command_line = windows_command_line(&arguments)?")
        .map(|offset| legacy_start + offset)
        .unwrap();
    let legacy = &windows[legacy_start..legacy_end];
    let legacy_linker = legacy.find("\"-fuse-ld=link\".to_owned()").unwrap();
    let legacy_vctools = legacy
        .find("\"-Xmicrosoft-visualc-tools-root\".to_owned()")
        .unwrap();
    let legacy_crt = legacy.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
    let legacy_archive = legacy.find("rust_archive.to_string_lossy()").unwrap();
    let legacy_tail = legacy.find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS").unwrap();
    let legacy_output = legacy.find("arguments.extend([\"-o\"").unwrap();
    assert!(
        legacy_vctools < legacy_linker
            && legacy_linker < legacy_crt
            && legacy_crt < legacy_archive
            && legacy_archive < legacy_tail
            && legacy_tail < legacy_output
    );
}

#[cfg(windows)]
#[test]
fn windows_prepared_link_owns_the_exact_native_static_tail() {
    for linker in [None, Some(std::ffi::OsStr::new("relative\\link.exe"))] {
        assert!(matches!(
            super::platform::prepare_link_invocation(
                "x86_64-pc-windows-msvc",
                linker,
                Some(std::ffi::OsStr::new(
                    r"C:\Program Files\Microsoft Visual Studio\Lïnk",
                )),
                std::ffi::OsStr::new("main.obj"),
                std::ffi::OsStr::new("module.obj"),
                std::ffi::OsStr::new("bridge.lib"),
                std::ffi::OsStr::new("output.exe"),
                false,
            ),
            Err(Error::Invalid)
        ));
    }
    let vctools = r"C:\Program Files\Microsoft Visual Studio\Lïnk";
    let linker = r"C:\Program Files\Microsoft Visual Studio\Lïnk\bin\Hostx64\x64\link.exe";
    let prepared = super::platform::prepare_link_invocation(
        "x86_64-pc-windows-msvc",
        Some(std::ffi::OsStr::new(linker)),
        Some(std::ffi::OsStr::new(vctools)),
        std::ffi::OsStr::new("main.obj"),
        std::ffi::OsStr::new("module.obj"),
        std::ffi::OsStr::new("bridge.lib"),
        std::ffi::OsStr::new("output.exe"),
        false,
    )
    .unwrap();
    let owned = super::platform::prepared_link_owned_capacity(&prepared);
    let expected = [
        "-target",
        "x86_64-pc-windows-msvc",
        "-Xmicrosoft-visualc-tools-root",
        r"C:\Program Files\Microsoft Visual Studio\Lïnk",
        "-fuse-ld=link",
        "-Xlinker",
        "/NODEFAULTLIB:libcmt",
        "main.obj",
        "module.obj",
        "bridge.lib",
        "-lkernel32",
        "-ladvapi32",
        "-ldbghelp",
        "-lntdll",
        "-luserenv",
        "-lws2_32",
        "-lmsvcrt",
        "-o",
        "output.exe",
    ];
    let (arguments, capacity) = super::platform::test_prepared_link_arguments(&prepared);
    assert!(arguments.iter().map(String::as_str).eq(expected));
    assert_eq!(capacity, expected.len());
    assert_eq!(
        super::platform::prepared_link_owned_capacity(&prepared),
        owned,
    );
}

#[test]
fn linux_runner_uses_the_held_executable_path_as_argv0_before_fexecve() {
    let source = UNIX_SOURCE;
    let start = source
        .find("#[cfg(target_os = \"linux\")]\nfn run_argv_mode(")
        .unwrap();
    let end = source[start..]
        .find("#[cfg(target_os = \"macos\")]\nfn run_argv(")
        .map(|offset| start + offset)
        .unwrap();
    let runner = &source[start..end];
    assert!(!runner.contains("semaprax-native-rust-interop-tool"));
    for required in [
        "let executable_fd_format = b\"/proc/self/fd/%d\\0\"",
        "libc::snprintf(",
        "libc::readlink(",
        "argv[0] = argv0.as_ptr().cast()",
        "fexecve(executable_fd, argv.as_ptr(), env.as_ptr())",
    ] {
        assert!(
            runner.contains(required),
            "missing Linux argv0 contract: {required}"
        );
    }
    let duplicated = runner.find("libc::F_DUPFD").unwrap();
    let readlink = runner.find("libc::readlink(").unwrap();
    let argv0 = runner.find("argv[0] = argv0.as_ptr().cast()").unwrap();
    let execute = runner
        .find("fexecve(executable_fd, argv.as_ptr(), env.as_ptr())")
        .unwrap();
    assert!(duplicated < readlink && readlink < argv0 && argv0 < execute);
}

#[test]
fn linux_archive_runner_closes_after_leader_and_settles_only_its_held_output() {
    let source = UNIX_SOURCE;
    for required in [
        "fn run_archive_argv(",
        "if stdout_limit != 0",
        "close_pipe_after_leader: bool",
        "if close_pipe_after_leader",
        "if quiesce_group(pid).is_err()",
        "Some(libc::EAGAIN) => break Ok((output, child_status))",
        "fn create_owned_archive_seed(",
        "libc::O_CREAT",
        "libc::O_EXCL",
        "Err(_) => std::process::abort()",
        "fn discard_created_archive_identity(",
        "if discard_created_archive_identity(directory, name, created_identity).is_err()",
        "let metadata = rebound.metadata().map_err(|_| Error::Changed)?",
        "if !metadata.is_file() || identity(&metadata) != created_identity",
        "libc::unlinkat(directory.file.as_raw_fd(), name.0.as_ptr(), 0)",
        "let owned_output = create_owned_archive_seed(cwd, &prepared.output_name)?",
        "discard_created_archive_identity(",
    ] {
        assert!(
            source.contains(required),
            "missing exact Linux archive settlement contract: {required}"
        );
    }
    let general = source
        .find("fn run_argv(\n")
        .expect("general Linux runner wrapper");
    let archive = source
        .find("fn run_archive_argv(\n")
        .expect("Linux archive runner wrapper");
    assert!(source[general..archive].contains("false,"));
    assert!(source[archive..].contains("true,"));
    let create = source
        .split("fn create_owned_archive_seed(")
        .nth(1)
        .and_then(|tail| tail.split("fn discard_created_archive_identity(").next())
        .expect("bounded Linux archive seed creation");
    assert!(create.contains("let metadata = match file.metadata()"));
    assert!(create.contains("Err(_) => std::process::abort()"));
    assert!(!create.contains("file.metadata().map_err"));
}

#[cfg(windows)]
#[test]
fn windows_directory_identity_survives_full_inventory_and_rejects_foreign_or_substituted_path() {
    use std::ffi::OsStr;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-windows-directory-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let stage_path = root.join("stage");
    let displaced_path = root.join("displaced");
    let foreign_path = root.join("foreign");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&stage_path).unwrap();
    std::fs::create_dir(&foreign_path).unwrap();
    let stage = super::platform::hold_directory(&stage_path).unwrap();
    let names = super::platform::prepare_discard_names([
        OsStr::new("a"),
        OsStr::new("b"),
        OsStr::new("c"),
        OsStr::new("d"),
        OsStr::new("e"),
        OsStr::new("f"),
        OsStr::new("g"),
    ])
    .unwrap();
    let files = (0..7)
        .map(|index| {
            super::platform::write_file_new_prepared(
                &stage,
                &names,
                index,
                &[u8::try_from(index).unwrap()],
                0,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    super::platform::recheck_directory(&stage).unwrap();
    let mut inventory = super::platform::prepare_inventory_exact(&names).unwrap();
    let attached = std::array::from_fn(|index| Some(&files[index]));
    super::platform::inventory_exact_prepared(&mut inventory, &stage, &names, attached).unwrap();
    assert!(!super::platform::same_directory_path(&stage, &foreign_path).unwrap());

    drop((inventory, files, names));
    std::fs::rename(&stage_path, &displaced_path).unwrap();
    std::fs::create_dir(&stage_path).unwrap();
    super::platform::recheck_directory(&stage).unwrap();
    assert!(!super::platform::same_directory_path(&stage, &stage_path).unwrap());
    drop(stage);
    std::fs::remove_dir_all(&root).unwrap();
}
