use semaprax::project::JsonCodecProfile;

pub(super) fn parse(profile: Option<&str>, max_string_bytes: Option<&str>) -> Option<JsonCodecProfile> {
    match (profile, max_string_bytes) {
        (None, None) => Some(JsonCodecProfile::FlatScalars),
        (Some("identifier-views.v1"), None) => Some(JsonCodecProfile::IdentifierViews),
        (Some("request-views.v1"), None) => Some(JsonCodecProfile::RequestViews),
        (Some("stream-request-views.v1"), None) => Some(JsonCodecProfile::StreamRequestViews),
        (Some("owned-request.v1"), None) => Some(JsonCodecProfile::OwnedRequest),
        (Some("stream-owned-request.v1"), None) => Some(JsonCodecProfile::StreamOwnedRequest),
        (Some("utf8-owned-request.v1"), Some(raw)) => {
            let bytes = raw.parse::<usize>().ok()?;
            if !(1..=64).contains(&bytes) || bytes.to_string() != raw {
                return None;
            }
            Some(JsonCodecProfile::Utf8OwnedRequest {
                max_string_bytes: bytes,
            })
        }
        (Some("stream-utf8-owned-request.v1"), Some(raw)) => {
            let bytes = raw.parse::<usize>().ok()?;
            if !(1..=64).contains(&bytes) || bytes.to_string() != raw {
                return None;
            }
            Some(JsonCodecProfile::StreamUtf8OwnedRequest {
                max_string_bytes: bytes,
            })
        }
        _ => None,
    }
}
