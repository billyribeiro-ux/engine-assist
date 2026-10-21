//! The template cache (ADR-005 §6, ADR-023 §5, files.md §4.10): the parsed `Template` serialized to
//! `templates/<sourceSha256>.p<parserVersion>.json` in app data, loaded instead of parsing when the
//! source hash matches. A cache that fails to load is replaced, never migrated.

use super::{PARSER_VERSION, Template, parse_template, source_sha256};
use crate::pathpolicy::{self, ReadClass};
use crate::store::{ReadStamp, Root, StoreError};

/// The cache path for a source hash.
#[must_use]
pub fn cache_path(source_sha256: &str) -> String {
    format!("templates/{source_sha256}.p{PARSER_VERSION}.json")
}

/// Where a template came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Loaded from the cache.
    Cache,
    /// Parsed (and the cache written).
    Parsed,
}

/// Loads the template for `source` from the cache in app data, or parses it and writes the cache.
///
/// # Errors
/// Only when the cache can't be written (the parse itself never fails).
pub fn load_or_parse(app: &Root, source: &str) -> Result<(Template, Origin), StoreError> {
    let sha = source_sha256(source);
    let rel = pathpolicy::check_app_data(&cache_path(&sha))?;
    let (bytes, stamp) = app.read_opt(&rel, ReadClass::Template)?;
    if let Some(b) = bytes
        && let Ok(t) = crate::json::parse::<Template>(&b)
        && t.source_sha256 == sha
        && t.parser_version == PARSER_VERSION
        && t.schema_version == 1
    {
        return Ok((t, Origin::Cache));
    }
    let t = parse_template(source);
    let text = crate::json::to_canonical(&t).map_err(|e| StoreError::Io {
        name: rel.to_string(),
        message: e.to_string(),
        errno: 0,
    })?;
    match app.write(&rel, text.as_bytes(), &stamp) {
        Ok(_) | Err(StoreError::Conflict { .. }) => {}
        Err(e) => return Err(e),
    }
    let _ = ReadStamp::Absent;
    Ok((t, Origin::Parsed))
}
