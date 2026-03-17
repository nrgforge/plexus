//! Input classifier — determines the appropriate input_kind for JSON data.

/// Classify input JSON to determine the appropriate `input_kind` (ADR-028).
///
/// Detection precedence:
/// 1. `{text}` or `{annotation}` → `"content"`
/// 2. `{file_path}` → `"extract-file"`
/// 3. No match → error with guidance
pub fn classify_input(data: &serde_json::Value) -> Result<&'static str, ClassifyError> {
    if data.get("text").is_some() || data.get("annotation").is_some() {
        return Ok("content");
    }
    if data.get("file_path").is_some() {
        return Ok("extract-file");
    }
    Err(ClassifyError)
}

/// Error from input classification.
#[derive(Debug)]
pub struct ClassifyError;

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unrecognized input shape — expected one of: \
             {{\"text\": ...}} for content, \
             {{\"file_path\": ...}} for file extraction"
        )
    }
}

impl std::error::Error for ClassifyError {}

#[cfg(test)]
mod tests {
    use super::*;

    // === Scenario: Classifier detects content-shaped input ===
    #[test]
    fn classify_content_with_text() {
        let json = serde_json::json!({"text": "some thought", "tags": ["idea"], "source": "trellis"});
        assert_eq!(classify_input(&json).unwrap(), "content");
    }

    // === Scenario: Classifier detects content with annotation field ===
    #[test]
    fn classify_content_with_annotation() {
        let json = serde_json::json!({"annotation": "pattern here", "line": 10, "file": "foo.rs"});
        assert_eq!(classify_input(&json).unwrap(), "content");
    }

    // === Scenario: Classifier detects file-extraction-shaped input ===
    #[test]
    fn classify_extract_file() {
        let json = serde_json::json!({"file_path": "/path/to/file.txt"});
        assert_eq!(classify_input(&json).unwrap(), "extract-file");
    }

    // === Scenario: Unrecognized input returns error ===
    #[test]
    fn classify_unrecognized_returns_error() {
        let json = serde_json::json!({"unknown_field": true});
        let result = classify_input(&json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unrecognized input shape"));
        assert!(msg.contains("text"));
        assert!(msg.contains("file_path"));
    }
}
