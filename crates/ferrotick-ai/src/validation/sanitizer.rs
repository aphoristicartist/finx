use regex::Regex;

use crate::error::{AIError, AIResult};

/// Sanitizes and validates LLM outputs.
pub struct OutputSanitizer;

impl OutputSanitizer {
    /// Extract and clean JSON from LLM output.
    pub fn sanitize_json(input: &str) -> AIResult<String> {
        // Remove markdown code blocks
        let re = Regex::new(r"```(?:json)?\s*(.*?)\s*```").unwrap();
        let cleaned = re.replace(input, "$1");

        // Find JSON object boundaries
        let start = cleaned
            .find('{')
            .ok_or_else(|| AIError::Parsing("No JSON object found".into()))?;
        let end = cleaned
            .rfind('}')
            .ok_or_else(|| AIError::Parsing("No JSON object found".into()))?;

        Ok(cleaned[start..=end].to_string())
    }

    /// Extract and clean YAML from LLM output.
    pub fn sanitize_yaml(input: &str) -> AIResult<String> {
        // Remove markdown code blocks
        let re = Regex::new(r"```(?:yaml|yml)?\s*(.*?)\s*```").unwrap();
        let mut cleaned = re.replace(input, "$1").to_string();

        // If no code blocks, use the raw input
        if cleaned.is_empty() {
            cleaned = input.to_string();
        }

        // Trim whitespace
        cleaned = cleaned.trim().to_string();

        if cleaned.is_empty() {
            return Err(AIError::Parsing("No YAML content found".into()));
        }

        Ok(cleaned)
    }

    /// Validate that a string is valid JSON and contains expected structure.
    pub fn validate_json_structure(json: &str, required_keys: &[&str]) -> AIResult<()> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| AIError::Parsing(e.to_string()))?;

        if !value.is_object() {
            return Err(AIError::Validation("JSON must be an object".into()));
        }

        for key in required_keys {
            if !value.as_object().map_or(false, |obj| obj.contains_key(*key)) {
                return Err(AIError::Validation(format!(
                    "Missing required key: {}",
                    key
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_json_with_code_block() {
        let input = r#"Here's the JSON:
```json
{"name": "test", "value": 42}
```
That's it!"#;

        let cleaned = OutputSanitizer::sanitize_json(input).unwrap();
        assert!(cleaned.starts_with('{'));
        assert!(cleaned.ends_with('}'));
        assert!(cleaned.contains("\"name\""));
    }

    #[test]
    fn test_sanitize_json_without_code_block() {
        let input = r#"Some text {"name": "test"} more text"#;

        let cleaned = OutputSanitizer::sanitize_json(input).unwrap();
        assert_eq!(cleaned, r#"{"name": "test"}"#);
    }

    #[test]
    fn test_sanitize_yaml_with_code_block() {
        let input = r#"```yaml
name: test
value: 42
```"#;

        let cleaned = OutputSanitizer::sanitize_yaml(input).unwrap();
        assert!(cleaned.contains("name: test"));
    }

    #[test]
    fn test_validate_json_structure() {
        let json = r#"{"name": "test", "value": 42}"#;
        assert!(OutputSanitizer::validate_json_structure(json, &["name", "value"]).is_ok());
        assert!(OutputSanitizer::validate_json_structure(json, &["missing"]).is_err());
    }
}
