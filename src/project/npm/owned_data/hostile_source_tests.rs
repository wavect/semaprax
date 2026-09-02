use super::*;

#[test]
fn bounded_renderer_fragments_pin_the_reviewed_failure_state_correction() {
    // Intentional v8-v10 JavaScript correction; Wasm and descriptor bytes
    // remain unchanged. DATA-only owned_invocation/static_hashes.rb first
    // calibrates all eight historical pins, then hashes these templates.
    // No replacement package or runtime is executed to mint these pins.
    for (rendered, expected) in [
        (
            render_runtime_prelude_with_admission("digest", true, 16),
            "8b2560ae6bca031f27b326e32fa5240821c7c52627fe2b8546980703578e264f",
        ),
        (
            render_runtime_facade(&[], true),
            "231195f9c27ce4667c90ef4985a76b33972b5b7f8d925e4755d160594d06ee06",
        ),
        (
            render_variant_runtime_facade(&[], true),
            "231195f9c27ce4667c90ef4985a76b33972b5b7f8d925e4755d160594d06ee06",
        ),
        (
            render_mixed_runtime_facade(&[], true),
            "231195f9c27ce4667c90ef4985a76b33972b5b7f8d925e4755d160594d06ee06",
        ),
    ] {
        assert_eq!(hex_sha256(rendered.as_bytes()), expected);
    }
}

#[test]
fn identity_guard_is_the_only_change_to_the_previous_bounded_prelude() {
    let rendered = render_runtime_prelude_with_admission("digest", true, 16);
    let guard = concat!(
            "      // Reject without coercion: an unknown identity must not run caller hooks.\n",
            "      if(typeof id!==\"string\")throw new RangeError(\"SEMAPRAX export identity must be a string\");\n",
        );
    assert_eq!(rendered.matches(guard).count(), 1);
    assert_eq!(
        hex_sha256(rendered.replacen(guard, "", 1).as_bytes()),
        "54984891e42a61f52b66a063a0c92b0ee200057710079a1f998276fd659b6e3f"
    );
}

#[test]
fn v10_capacity_substitution_preserves_every_other_prelude_byte() {
    let old = render_runtime_prelude_with_admission("digest", true, 16);
    for capacity in [1, 19, 0x7fff_ffff] {
        assert_eq!(
            render_runtime_prelude_with_admission("digest", true, capacity),
            old.replace("entries.size>=16||", &format!("entries.size>={capacity}||"))
        );
    }
}

#[test]
fn unmodified_profile_renderer_fragments_keep_their_prechange_bytes() {
    // Pinned from the baseline literal templates, without executing a build.
    // These historical private fragments are no longer selected by v9/v10.
    for (rendered, expected) in [
        (
            render_runtime_prelude("digest"),
            "4d0057aed9591b91ea9ef11f84657ca6be1db45dd6d3d3afdd9b6c2bfe19e61f",
        ),
        (
            render_mixed_runtime_facade(&[], false),
            "9b55a36b641c28dfcd77b1658c1fd610716d30a30f304b1cc9b6660e41aaa457",
        ),
        (
            render_runtime_facade(&[], false),
            "51414bc65b07f7dc7f83bb5ecbc7b8958f61cbd55e5929b3babcf37d537850cc",
        ),
        (
            render_variant_runtime_facade(&[], false),
            "b10112d3d1cb8640d02d7a536626ff12ee975d53e7a181642a9e0ae3e791b20f",
        ),
    ] {
        assert_eq!(hex_sha256(rendered.as_bytes()), expected);
    }
}

#[test]
fn v8_and_v10_input_admission_is_explicit_and_precedes_scratch_and_arena() {
    let v8 = render_runtime("digest", &[], PUBLIC_OWNED_DATA_PROJECT_SCHEMA, 16);
    let v10 = render_runtime("digest", &[], PUBLIC_OWNED_UTF8_PROJECT_SCHEMA, 16);
    assert!(v8.contains("function snapshotArguments("));
    assert!(v10.contains("function snapshotArguments("));
    assert!(!render_runtime_prelude_with_admission("digest", true, 16).contains("arrayBufferSlice"));
    for source in [
        render_runtime_facade(&[], true),
        render_variant_runtime_facade(&[], true),
        render_mixed_runtime_facade(&[], true),
    ] {
        let source = format!(
            "{}{}",
            render_runtime_prelude_with_admission("digest", true, 16),
            source
        );
        let admission = source
            .find("snapshotArguments(values,fact.params)")
            .unwrap();
        assert!(source.find("busy=true").unwrap() < admission);
        assert!(admission < source.find("entered=true").unwrap());
        assert!(admission < source.find("linked.copyInto(").unwrap());
        assert!(admission < source.find("arena.begin()").unwrap());
    }
}

#[test]
fn bounded_facades_share_presence_and_identity_based_failure_selection() {
    let source = render_runtime_prelude_with_admission("digest", true, 16);
    assert!(source.contains("if(!hasPrimary){hasPrimary=true;primary=error}"));
    assert!(source.contains("if(hasPrimary)throw primary;"));
    assert!(source.contains("hasSemantic&&error===semanticError"));
    assert!(!source.contains("error?.semapraxSemantic"));
    assert!(!source.contains("error instanceof TypeError"));
    assert!(!source.contains("error instanceof RangeError"));
    assert!(source.contains("Number.isInteger(status)||status<0||status>10"));
    assert!(source.contains("finally{busy=false}"));
    assert!(source.contains("fatal:true,ignoreBOM:true"));
    let slot_proof = source
        .find("SEMAPRAX failure modified result slot")
        .unwrap();
    let identity = source.find("semanticError=error;hasSemantic=true").unwrap();
    assert!(slot_proof < identity);
    let settle = source
        .find("linked.arena.settle();settled=true;linked.arena.check();")
        .unwrap();
    assert!(settle < source.find("answer=complete()").unwrap());
    assert_eq!(
        render_runtime_facade(&[], true),
        render_variant_runtime_facade(&[], true)
    );
    assert_eq!(
        render_runtime_facade(&[], true),
        render_mixed_runtime_facade(&[], true)
    );
    assert_eq!(
        render_runtime_facade(&[], true),
        render_flat_runtime_facade("")
    );
}

#[test]
fn every_owned_facade_rejects_non_scalar_strings_and_wide_i64_before_effects() {
    let sources = [
        render_mixed_runtime_facade(&[], false),
        render_runtime_facade(&[], false),
        render_variant_runtime_facade(&[], false),
    ];
    for source in sources {
        let snapshot = source.find("const snapshots=").unwrap();
        assert!(source[..snapshot].contains("unit>=0xd800&&unit<=0xdbff"));
        assert!(source[..snapshot].contains("unit>=0xdc00&&unit<=0xdfff"));
        assert!(source[..snapshot].contains("++i>=value.length"));
        assert!(source[..snapshot].contains("return encoder.encode(value)"));
        assert!(source[..snapshot].contains("value<-(1n<<63n)"));
        assert!(source[..snapshot].contains("value>(1n<<63n)-1n"));
        assert!(snapshot < source.find("busy=true").unwrap());
        assert!(snapshot < source.find("arena.begin()").unwrap());
    }
}
