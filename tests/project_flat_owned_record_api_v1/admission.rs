use super::*;

#[test]
fn retained_names_match_the_shared_hand_authored_lower_replay_oracle() {
    let source = include_str!("../fixtures/flat_descriptor_retained_names.spx");
    let expected = include_bytes!("../fixtures/flat_descriptor_retained_names.json");
    let program = resolve(source);
    let selected = vec!["api.value".to_owned()];
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    assert!(descriptor.exports()[0].record_source_name().len() > 128);
    assert_eq!(descriptor.canonical_bytes(), expected.as_slice());
    assert_eq!(
        replay_flat_owned_record_api_descriptor(
            &program,
            &selected,
            subject(),
            expected,
            &descriptor.digest()
        )
        .unwrap(),
        descriptor
    );
}

#[test]
fn complete_inventory_bound_includes_unselected_functions() {
    for count in [256, 257] {
        let mut source = SOURCE.to_owned(); // two existing functions
        for index in 2..count {
            source.push_str(&format!(
                "\n@id(\"extra.{index}\") fn extra_{index}() -> i64 {{ 0 }}\n"
            ));
        }
        let program = resolve(&source);
        assert_eq!(program.functions.len(), count);
        let result = derive_flat_owned_record_api_descriptor(
            &program,
            &["frame.info".to_owned()],
            subject(),
        );
        if count == 256 {
            assert!(result.is_ok());
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "SPX-J113");
            assert!(error.message.contains("1..=256"));
        }
    }
}

#[test]
fn source_parameter_and_field_display_names_have_no_128_byte_limit() {
    let parameter = "parameter".repeat(20);
    let field = "field".repeat(30);
    let source = SOURCE
        .replace("value:", &format!("{parameter}:"))
        .replace("bytes_copy(value)", &format!("bytes_copy({parameter})"))
        .replace("byte_len(value)", &format!("byte_len({parameter})"))
        .replace("payload:", &format!("{field}:"));
    let program = resolve(&source);
    let selected = vec!["frame.info".to_owned()];
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    assert_eq!(descriptor.exports()[0].parameters()[0].1, parameter);
    assert_eq!(descriptor.exports()[0].fields()[0].source_name(), field);
    replay_flat_owned_record_api_descriptor(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
}
