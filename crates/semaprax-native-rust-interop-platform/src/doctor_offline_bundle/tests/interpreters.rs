use super::*;

#[test]
fn dynamic_interpreter_is_an_exact_executable_inventory_entry() {
    let loader = elf(62, None);
    let executable = elf(62, Some("/lib64/ld-linux-x86-64.so.2"));
    let files = [
        ("bin/node", true, executable.as_slice()),
        ("lib64/ld-linux-x86-64.so.2", true, loader.as_slice()),
    ];
    assert!(parse(&node(&files, 0)).is_ok());
    rejects(&node(&files[..1], 0), Error::Invalid);
    rejects(
        &node(
            &[
                ("bin/node", true, &executable),
                ("lib64/ld-linux-x86-64.so.2", false, &loader),
            ],
            0,
        ),
        Error::Invalid,
    );
    for spelling in [
        "/lib64/other",
        "//lib64/ld-linux-x86-64.so.2",
        "/lib64/../lib64/ld-linux-x86-64.so.2",
        "/lib64/./ld-linux-x86-64.so.2",
        "/lib64/ld-linux-x86-64.so.2/",
        "/",
        "/lib64/ld linux",
        "/lib64/é",
    ] {
        let bad = elf(62, Some(spelling));
        rejects(
            &node(&[("bin/node", true, &bad), files[1]], 0),
            Error::Invalid,
        );
    }
}

#[test]
fn self_references_cycles_and_interpreter_chains_are_not_followed() {
    let self_ref = elf(62, Some("/bin/node"));
    rejects(&node(&[("bin/node", true, &self_ref)], 0), Error::Invalid);
    let node_bytes = elf(62, Some("/lib/first"));
    let first = elf(62, Some("/lib/second"));
    let second = elf(62, None);
    rejects(
        &node(
            &[
                ("bin/node", true, &node_bytes),
                ("lib/first", true, &first),
                ("lib/second", true, &second),
            ],
            0,
        ),
        Error::Invalid,
    );
    let cycle = elf(62, Some("/bin/node"));
    rejects(
        &node(
            &[("bin/node", true, &node_bytes), ("lib/first", true, &cycle)],
            0,
        ),
        Error::Invalid,
    );
}

#[test]
fn unselected_executable_interpreters_are_also_closed_and_machine_bound() {
    let executable = elf(62, None);
    let extra = elf(62, Some("/missing"));
    rejects(
        &node(
            &[("bin/node", true, &executable), ("other", true, &extra)],
            0,
        ),
        Error::Invalid,
    );
    let other_arch = elf(183, None);
    rejects(
        &node(
            &[
                ("bin/node", true, &executable),
                ("other", true, &other_arch),
            ],
            0,
        ),
        Error::Invalid,
    );
    let node_bytes = elf(62, Some("/loader"));
    rejects(
        &node(
            &[
                ("bin/node", true, &node_bytes),
                ("loader", true, &other_arch),
            ],
            0,
        ),
        Error::Invalid,
    );
}
