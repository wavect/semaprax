//! Fixed wire limits are facts, not caller-selected policy or runtime capacity.
use super::*;
use semaprax::project::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA;

#[test]
fn freshly_digested_limit_changes_reject_in_both_descriptor_profiles() {
    let program = resolve(ADMITTED);
    let selected = vec!["api.mixed".to_owned()];
    for (project_schema, domain) in [
        (
            PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            b"semaprax.public-owned-data-api.digest.v1\0".as_slice(),
        ),
        (
            PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
            b"semaprax.public-owned-utf8-api.digest.v1\0".as_slice(),
        ),
    ] {
        let subject = PublicApiSubject {
            project_schema,
            ..subject()
        };
        let descriptor = derive_public_api_descriptor(&program, &selected, subject).unwrap();
        let canonical = String::from_utf8(descriptor.canonical_bytes()).unwrap();
        let digest = remint_descriptor_digest_with_domain(domain, canonical.as_bytes());
        assert_eq!(digest, descriptor.digest());
        assert_eq!(
            replay_public_api_descriptor(
                &program,
                &selected,
                subject,
                canonical.as_bytes(),
                &digest,
            )
            .unwrap(),
            descriptor
        );

        // Literal contract values, independent of production limit constants.
        for (field, expected) in [
            ("max_exports", 32usize),
            ("max_parameters", 8),
            ("max_closure_functions", 256),
            ("max_borrowed_input_bytes", 65_536),
            ("max_owned_output_bytes", 65_536),
            ("max_descriptor_bytes", 1_048_576),
        ] {
            let original = format!("\"{field}\":{expected}");
            assert_eq!(canonical.matches(&original).count(), 1);
            for changed in [expected - 1, expected + 1] {
                let replacement = format!("\"{field}\":{changed}");
                assert!(!canonical.contains(&replacement));
                // Change only the selected number, preserving every other
                // fact, member order, spelling and terminal LF.
                let submitted = canonical.replacen(&original, &replacement, 1);
                assert_ne!(submitted, canonical);
                assert_eq!(submitted.matches(&replacement).count(), 1);
                let digest = remint_descriptor_digest_with_domain(domain, submitted.as_bytes());
                assert_ne!(digest, descriptor.digest());
                let error = replay_public_api_descriptor(
                    &program,
                    &selected,
                    subject,
                    submitted.as_bytes(),
                    &digest,
                )
                .unwrap_err();
                assert_eq!(error.code, "SPX-J113", "{field}={changed}: {error}");
                assert_eq!(error.message, "public API descriptor limits are invalid");
            }
        }
        assert_eq!(
            replay_public_api_descriptor(
                &program,
                &selected,
                subject,
                canonical.as_bytes(),
                &descriptor.digest(),
            )
            .unwrap(),
            descriptor
        );
    }
}
