//! Canonical JSON (FILE-R1, ADR-010 §7, ADR-023): UTF-8, object keys sorted by their UTF-8 bytes,
//! two-space indent, LF, one trailing newline. A parse–write round trip is byte-identical.
//! Parsing stops at nesting depth 64 (SR-31, FILE-R9).

use std::fmt::Write as _;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Maximum nesting depth of JSON Foreman parses (SR-31).
pub const MAX_DEPTH: usize = 64;

/// A JSON problem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JsonError {
    /// Not valid JSON (or not UTF-8, or has a BOM).
    #[error("invalid JSON: {0}")]
    Syntax(String),
    /// Nested deeper than [`MAX_DEPTH`].
    #[error("JSON nested deeper than {MAX_DEPTH} levels")]
    TooDeep,
    /// Didn't fit the typed shape.
    #[error("unexpected shape: {0}")]
    Shape(String),
}

/// Checks the nesting depth of JSON text without building it (strings are skipped correctly).
fn depth_ok(text: &str) -> bool {
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    for b in text.bytes() {
        if in_str {
            match (esc, b) {
                (true, _) => esc = false,
                (false, b'\\') => esc = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return false;
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// Parses JSON bytes into a `Value`, refusing a BOM, invalid UTF-8 and deep nesting.
///
/// # Errors
/// [`JsonError::Syntax`] or [`JsonError::TooDeep`].
pub fn parse_value(bytes: &[u8]) -> Result<Value, JsonError> {
    let text = std::str::from_utf8(bytes).map_err(|e| JsonError::Syntax(e.to_string()))?;
    if text.starts_with('\u{feff}') {
        return Err(JsonError::Syntax("byte-order mark".into()));
    }
    if !depth_ok(text) {
        return Err(JsonError::TooDeep);
    }
    serde_json::from_str(text).map_err(|e| JsonError::Syntax(e.to_string()))
}

/// Parses JSON bytes into `T` (with the same limits as [`parse_value`]).
///
/// # Errors
/// As [`parse_value`], or [`JsonError::Shape`] when the value doesn't fit `T`.
pub fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, JsonError> {
    let v = parse_value(bytes)?;
    serde_json::from_value(v).map_err(|e| JsonError::Shape(e.to_string()))
}

/// Serializes `v` canonically.
///
/// # Errors
/// [`JsonError::Shape`] if `v` can't be represented as JSON (non-string map keys, NaN).
pub fn to_canonical<T: Serialize>(v: &T) -> Result<String, JsonError> {
    let value = serde_json::to_value(v).map_err(|e| JsonError::Shape(e.to_string()))?;
    Ok(value_to_canonical(&value))
}

/// Writes a `Value` canonically (sorted keys, two-space indent, trailing newline).
#[must_use]
pub fn value_to_canonical(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v, 0);
    out.push('\n');
    out
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_str(out: &mut String, s: &str) {
    // serde_json's string escaping is the canonical one; serializing a &str can't fail.
    match serde_json::to_string(s) {
        Ok(q) => out.push_str(&q),
        Err(_) => out.push_str("\"\""),
    }
}

fn write_value(out: &mut String, v: &Value, level: usize) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let _ = write!(out, "{n}");
        }
        Value::String(s) => write_str(out, s),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in a.iter().enumerate() {
                indent(out, level + 1);
                write_value(out, item, level + 1);
                if i + 1 < a.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            indent(out, level);
            out.push(']');
        }
        Value::Object(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push_str("{\n");
            for (i, k) in keys.iter().enumerate() {
                indent(out, level + 1);
                write_str(out, k);
                out.push_str(": ");
                if let Some(val) = m.get(*k) {
                    write_value(out, val, level + 1);
                }
                if i + 1 < keys.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            indent(out, level);
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorted_indented_trailing_newline() {
        let v = json!({"b": 1, "a": [true, null, {"z": "x", "y": []}], "c": {}});
        assert_eq!(
            value_to_canonical(&v),
            "{\n  \"a\": [\n    true,\n    null,\n    {\n      \"y\": [],\n      \"z\": \"x\"\n    }\n  ],\n  \"b\": 1,\n  \"c\": {}\n}\n"
        );
    }

    #[test]
    fn depth_limit() {
        let deep = "[".repeat(65) + &"]".repeat(65);
        assert_eq!(parse_value(deep.as_bytes()), Err(JsonError::TooDeep));
        let ok = "[".repeat(64) + &"]".repeat(64);
        assert!(parse_value(ok.as_bytes()).is_ok());
        assert!(
            parse_value(
                br#"{"a":"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[["}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn bom_refused() {
        assert!(parse_value("\u{feff}{}".as_bytes()).is_err());
    }
}
