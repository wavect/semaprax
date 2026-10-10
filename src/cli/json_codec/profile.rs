use semaprax::project::JsonCodecProfile;

pub(super) fn parse(
    profile: Option<&str>,
    max_string_bytes: Option<&str>,
    max_array_items: Option<&str>,
) -> Option<JsonCodecProfile> {
    match (profile, max_string_bytes, max_array_items) {
        (None, None, None) => Some(JsonCodecProfile::FlatScalars),
        (Some("identifier-views.v1"), None, None) => Some(JsonCodecProfile::IdentifierViews),
        (Some("request-views.v1"), None, None) => Some(JsonCodecProfile::RequestViews),
        (Some("stream-request-views.v1"), None, None) => Some(JsonCodecProfile::StreamRequestViews),
        (Some("owned-request.v1"), None, None) => Some(JsonCodecProfile::OwnedRequest),
        (Some("stream-owned-request.v1"), None, None) => Some(JsonCodecProfile::StreamOwnedRequest),
        (Some("utf8-owned-request.v1"), Some(raw), None) => {
            let bytes = bound(raw, 64)?;
            Some(JsonCodecProfile::Utf8OwnedRequest {
                max_string_bytes: bytes,
            })
        }
        (Some("stream-utf8-owned-request.v1"), Some(raw), None) => {
            let bytes = bound(raw, 64)?;
            Some(JsonCodecProfile::StreamUtf8OwnedRequest {
                max_string_bytes: bytes,
            })
        }
        (Some("bounded-collection-response.v1"), Some(raw), None) => {
            let bytes = bound(raw, 64)?;
            Some(JsonCodecProfile::CollectionResponse {
                max_string_bytes: bytes,
            })
        }
        (Some("bounded-nested-request.v1"), Some(bytes), Some(items)) => {
            Some(JsonCodecProfile::NestedRequest {
                max_string_bytes: bound(bytes, 64)?,
                max_array_items: bound(items, 256)?,
            })
        }
        _ => None,
    }
}

fn bound(raw: &str, maximum: usize) -> Option<usize> {
    let value = raw.parse::<usize>().ok()?;
    ((1..=maximum).contains(&value) && value.to_string() == raw).then_some(value)
}
