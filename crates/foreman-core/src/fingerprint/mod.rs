//! The Claude Code config fingerprint (SR-06, SR-07, SR-55; ADR-021 §2–4).
//!
//! [`sources`] lists every input Claude Code loads for a step's cwd; `store::config_read` reads
//! them (caps, symlink rules, the streamed `~/.claude.json`); [`derived_sources`] adds what the
//! files themselves name (`@` imports, enabled plugins, hook and helper programs); [`fingerprint`]
//! hashes the set; [`auth_key_hits`] applies SR-07; [`redact`] and [`diff`] build the SR-55
//! snapshot and the acknowledgement diff. Everything here is pure.

pub mod auth_keys;
pub mod sources;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use auth_keys::{AllowlistEntry, AuthKeyHit, allowlist_hash, auth_key_hits};
pub use sources::{
    MANAGED_DIR, Origin, SettingsScope, SourceKind, SourceSpec, derived_sources, imports, sources,
};

use crate::schema::fingerprint::SnapshotEntry;
use crate::store::sha256_hex;

/// One read source, as `store::config_read` returns it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFile {
    /// Which source.
    pub kind: SourceKind,
    /// Settings scope, for JSON settings sources.
    pub scope: Option<SettingsScope>,
    /// The path as listed (absolute).
    pub path: String,
    /// The resolved target when `path` is a followed symlink (outside sources only).
    pub link_target: Option<String>,
    /// SHA-256 of the content (of the target for a link).
    pub sha256: String,
    /// Parsed JSON for settings-like sources (`None` for Markdown and programs).
    pub json: Option<Value>,
    /// Text for Markdown memory files (to find `@` imports).
    pub text: Option<String>,
    /// For a bare program name (`node`): recorded as a name, not a file.
    pub bare_name: bool,
}

/// The fingerprint of a set of files: SHA-256 over sorted `kind\tpath\ttarget\tsha256` lines.
#[must_use]
pub fn fingerprint(files: &[ConfigFile]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}\n",
                f.kind.as_str(),
                f.path,
                f.link_target.as_deref().unwrap_or(""),
                f.sha256
            )
        })
        .collect();
    lines.sort();
    lines.dedup();
    sha256_hex(lines.concat().as_bytes())
}

const REDACTED_PREFIX: &str = "‹redacted sha256:";

fn redacted_marker(v: &Value) -> Value {
    let text = crate::json::value_to_canonical(v);
    let h = sha256_hex(text.as_bytes());
    Value::String(format!("{REDACTED_PREFIX}{}›", &h[..8]))
}

fn sensitive_key(k: &str) -> bool {
    let l = k.to_ascii_lowercase();
    k.starts_with("apiKey") || l.contains("token") || l.contains("secret") || l.contains("password")
}

/// Strings that look like credentials (a subset of the secret scanner's patterns, SR-55).
#[must_use]
pub fn looks_secret(s: &str) -> bool {
    const PREFIXES: [&str; 12] = [
        "sk-",
        "sk_live_",
        "rk_live_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "AIza",
        "-----BEGIN",
    ];
    let t = s.trim();
    PREFIXES.iter().any(|p| t.starts_with(p))
        || (t.len() == 20
            && t.starts_with("AKIA")
            && t.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
        || t.contains("://")
            && t.split("://").nth(1).is_some_and(|r| {
                r.split('/')
                    .next()
                    .is_some_and(|auth| auth.contains('@') && auth.contains(':'))
            })
}

/// SR-55: values under `env` or `headers`, `apiKey*` keys, keys containing `token`, `secret` or
/// `password` at any depth, and secret-looking strings become `‹redacted sha256:8›`.
#[must_use]
pub fn redact(v: &Value) -> Value {
    fn go(v: &Value, under_secret: bool) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, x)| {
                        let r = if under_secret || sensitive_key(k) {
                            redacted_marker(x)
                        } else if k == "env" || k == "headers" {
                            match x {
                                Value::Object(inner) => Value::Object(
                                    inner
                                        .iter()
                                        .map(|(ik, iv)| (ik.clone(), redacted_marker(iv)))
                                        .collect(),
                                ),
                                other => redacted_marker(other),
                            }
                        } else {
                            go(x, false)
                        };
                        (k.clone(), r)
                    })
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(|x| go(x, under_secret)).collect()),
            Value::String(s) if under_secret || looks_secret(s) => redacted_marker(v),
            other => other.clone(),
        }
    }
    go(v, false)
}

/// The redacted snapshot of a file set (files.md §4.9 `acked.snapshot`).
#[must_use]
pub fn snapshot(files: &[ConfigFile]) -> Vec<SnapshotEntry> {
    let mut v: Vec<SnapshotEntry> = files
        .iter()
        .map(|f| SnapshotEntry {
            source: f.kind.as_str().to_owned(),
            path: f
                .link_target
                .as_ref()
                .map_or_else(|| f.path.clone(), |t| format!("{} -> {t}", f.path)),
            sha256: f.sha256.clone(),
            redacted: f.json.as_ref().map_or(Value::Null, redact),
        })
        .collect();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

/// How one key changed (ipc.md `ChangeKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// New.
    Added,
    /// Gone.
    Removed,
    /// Different value.
    ValueChanged,
}

/// One redacted change (ipc.md `RedactedEntry`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedEntry {
    /// `permissions.allow[2]`, `env.FOO`, or empty for a whole file.
    pub key_path: String,
    /// The change.
    pub change: ChangeKind,
    /// Redacted before value (canonical JSON text), if any.
    pub before: Option<String>,
    /// Redacted after value, if any.
    pub after: Option<String>,
}

/// The changes in one source (ipc.md `RedactedDiff`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedDiff {
    /// `<kind> <path>`.
    pub source: String,
    /// Entries.
    pub entries: Vec<RedactedEntry>,
}

fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, x) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, x, out);
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for (i, x) in a.iter().enumerate() {
                flatten(&format!("{prefix}[{i}]"), x, out);
            }
        }
        other => out.push((
            prefix.to_owned(),
            crate::json::value_to_canonical(other).trim_end().to_owned(),
        )),
    }
}

fn short(h: &str) -> String {
    format!("sha256:{}", h.get(..8).unwrap_or(h))
}

/// SR-55: what changed between the acknowledged snapshot and now, per source, with redacted
/// values only ("value changed" for redacted ones, since both sides are markers).
#[must_use]
pub fn diff(prev: &[SnapshotEntry], now: &[SnapshotEntry]) -> Vec<RedactedDiff> {
    use std::collections::BTreeMap;
    let old_map: BTreeMap<&str, &SnapshotEntry> =
        prev.iter().map(|e| (e.path.as_str(), e)).collect();
    let new_map: BTreeMap<&str, &SnapshotEntry> =
        now.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut out = Vec::new();
    let mut keys: Vec<&str> = old_map.keys().chain(new_map.keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    for path in keys {
        let (before, after) = (old_map.get(path), new_map.get(path));
        let source = format!(
            "{} {path}",
            after.or(before).map_or("", |e| e.source.as_str())
        );
        let entries = match (before, after) {
            (None, Some(b)) => vec![RedactedEntry {
                key_path: String::new(),
                change: ChangeKind::Added,
                before: None,
                after: Some(short(&b.sha256)),
            }],
            (Some(a), None) => vec![RedactedEntry {
                key_path: String::new(),
                change: ChangeKind::Removed,
                before: Some(short(&a.sha256)),
                after: None,
            }],
            (Some(a), Some(b)) if a.sha256 != b.sha256 => {
                if a.redacted.is_null() || b.redacted.is_null() {
                    vec![RedactedEntry {
                        key_path: String::new(),
                        change: ChangeKind::ValueChanged,
                        before: Some(short(&a.sha256)),
                        after: Some(short(&b.sha256)),
                    }]
                } else {
                    let mut fa = Vec::new();
                    let mut fb = Vec::new();
                    flatten("", &a.redacted, &mut fa);
                    flatten("", &b.redacted, &mut fb);
                    let ma: BTreeMap<_, _> = fa.into_iter().collect();
                    let mb: BTreeMap<_, _> = fb.into_iter().collect();
                    let mut ks: Vec<&String> = ma.keys().chain(mb.keys()).collect();
                    ks.sort_unstable();
                    ks.dedup();
                    let mut es: Vec<RedactedEntry> = ks
                        .into_iter()
                        .filter_map(|k| match (ma.get(k), mb.get(k)) {
                            (None, Some(y)) => Some(RedactedEntry {
                                key_path: k.clone(),
                                change: ChangeKind::Added,
                                before: None,
                                after: Some(y.clone()),
                            }),
                            (Some(x), None) => Some(RedactedEntry {
                                key_path: k.clone(),
                                change: ChangeKind::Removed,
                                before: Some(x.clone()),
                                after: None,
                            }),
                            (Some(x), Some(y)) if x != y => Some(RedactedEntry {
                                key_path: k.clone(),
                                change: ChangeKind::ValueChanged,
                                before: Some(x.clone()),
                                after: Some(y.clone()),
                            }),
                            _ => None,
                        })
                        .collect();
                    if es.is_empty() {
                        // Same redacted content, different bytes (formatting or a redacted value
                        // whose marker collided): still a change to acknowledge.
                        es.push(RedactedEntry {
                            key_path: String::new(),
                            change: ChangeKind::ValueChanged,
                            before: Some(short(&a.sha256)),
                            after: Some(short(&b.sha256)),
                        });
                    }
                    es
                }
            }
            _ => continue,
        };
        out.push(RedactedDiff { source, entries });
    }
    out
}
