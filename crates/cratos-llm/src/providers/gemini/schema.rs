//! JSON Schema sanitization for Gemini API

/// Fields not supported by Gemini's OpenAPI Schema subset.
/// See: https://ai.google.dev/api/caching#Schema
const UNSUPPORTED_SCHEMA_FIELDS: &[&str] = &["default", "additionalProperties", "$schema"];

/// Recursively strip JSON Schema fields that Gemini API does not support.
///
/// Gemini accepts only a limited subset of OpenAPI Schema:
/// `type`, `format`, `description`, `nullable`, `enum`, `items`,
/// `properties`, `required`.
/// Sending unsupported fields like `default` or `additionalProperties`
/// causes INVALID_ARGUMENT 400 errors.
pub(crate) fn strip_unsupported_schema_fields(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        for field in UNSUPPORTED_SCHEMA_FIELDS {
            obj.remove(*field);
        }
        // Recurse into nested schemas
        for (_, v) in obj.iter_mut() {
            strip_unsupported_schema_fields(v);
        }
    } else if let Some(arr) = value.as_array_mut() {
        for v in arr.iter_mut() {
            strip_unsupported_schema_fields(v);
        }
    }
}
