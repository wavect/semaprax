//! Root replay oracle for the lower package's per-export identity guard.
use super::*;
use semaprax::project::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA;

#[test]
fn duplicate_parameter_identities_fail_before_subject_replay_even_with_fresh_digest() {
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
        let parameters = descriptor.exports()[0].parameters();
        assert_eq!(parameters.len(), 4);
        assert_eq!(
            remint_descriptor_digest_with_domain(domain, canonical.as_bytes()),
            descriptor.digest()
        );
        for (original, replacement) in [(1, 0), (3, 0), (2, 3)] {
            let fact = |index: usize| {
                format!(
                    "\"stable_id\":{}",
                    serde_json::to_string(parameters[index].stable_id().as_str()).unwrap()
                )
            };
            let from = fact(original);
            let to = fact(replacement);
            assert_ne!(from, to);
            assert_eq!(canonical.matches(&from).count(), 1);
            // Replace only the identity string: property order, ordinals, types,
            // source names and terminal LF remain the compiler's exact bytes.
            let forged = canonical.replacen(&from, &to, 1);
            assert_eq!(forged.matches(&to).count(), 2);
            let digest = remint_descriptor_digest_with_domain(domain, forged.as_bytes());
            assert_ne!(digest, descriptor.digest());
            let error = replay_public_api_descriptor(
                &program,
                &selected,
                subject,
                forged.as_bytes(),
                &digest,
            )
            .unwrap_err();
            assert_eq!(error.code, "SPX-J113");
            assert_eq!(
                error.message,
                "public API parameter identities must be unique within an export"
            );
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
