//! SR-07: settings keys that change how a session is authenticated or billed block the launch.
//!
//! Besides SR-07's named keys and `env` patterns, `forceLoginMethod` set to anything other than
//! `"claudeai"` and any `forceLoginGatewayUrl` block too (S-7′ rehearsal 2026-10-20: a project
//! `forceLoginMethod: "console"` made `auth status` report `forcedLoginMethod: "console"` while
//! `authMethod` stayed `oauth_token`, so the SR-08 check alone would have passed it). Values per
//! code.claude.com/docs/en/authentication ("Restrict login to your organization", 2026-10-20):
//! `"claudeai"`, `"console"`, `"gateway"`; the key's scope is "Any file".

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ConfigFile, SettingsScope};
use crate::store::sha256_hex;

/// Top-level keys that run a credential helper or change auth (SR-07).
pub const BLOCKED_KEYS: [&str; 5] = [
    "apiKeyHelper",
    "awsAuthRefresh",
    "awsCredentialExport",
    "gcpAuthRefresh",
    "otelHeadersHelper",
];

/// The only `forceLoginMethod` value that keeps the subscription login (Foreman's requirement).
pub const SUBSCRIPTION_LOGIN_METHOD: &str = "claudeai";

/// SR-07's `env` patterns.
pub const BLOCKED_ENV: [crate::launch::env::Pattern; 15] = {
    use crate::launch::env::Pattern::{Exact, Prefix, Suffix};
    [
        Prefix("ANTHROPIC_"),
        Prefix("CLAUDE_CODE_"),
        Exact("CLAUDE_CONFIG_DIR"),
        Prefix("AWS_"),
        Prefix("GOOGLE_"),
        Prefix("CLOUDSDK_"),
        Prefix("AZURE_"),
        Suffix("_PROXY"),
        Suffix("_proxy"),
        Exact("NODE_EXTRA_CA_CERTS"),
        Exact("NODE_TLS_REJECT_UNAUTHORIZED"),
        Exact("SSL_CERT_FILE"),
        Exact("SSL_CERT_DIR"),
        Exact("NODE_OPTIONS"),
        Prefix("DYLD_"),
    ]
};

/// An allowlisted user-settings key (settings.json `authKeyAllowlist`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowlistEntry {
    /// The key (`apiKeyHelper`, `env.AWS_PROFILE`).
    pub key: String,
    /// `sha256("<key>=<canonical JSON value>")`.
    pub sha256: String,
}

/// One SR-07 hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthKeyHit {
    /// The file.
    pub file: String,
    /// The key.
    pub key: String,
    /// Stable ID: SHA-256 of file, key and value hash.
    pub id: String,
    /// `allowlist_hash(key, value)` (what `auth_key_allowlist_add` stores).
    pub value_hash: String,
    /// User settings only.
    pub allowlistable: bool,
}

/// `sha256("<key>=<canonical JSON value>")` (files.md §4.1).
#[must_use]
pub fn allowlist_hash(key: &str, value: &Value) -> String {
    let canon = crate::json::value_to_canonical(value);
    sha256_hex(format!("{key}={}", canon.trim_end()).as_bytes())
}

fn env_blocked(name: &str) -> bool {
    BLOCKED_ENV.iter().any(|p| p.matches(name))
}

fn scan_settings(v: &Value, out: &mut Vec<(String, Value)>) {
    let Some(m) = v.as_object() else { return };
    for k in BLOCKED_KEYS {
        if let Some(x) = m.get(k)
            && !x.is_null()
        {
            out.push((k.to_owned(), x.clone()));
        }
    }
    if let Some(x) = m.get("forceLoginMethod")
        && x.as_str() != Some(SUBSCRIPTION_LOGIN_METHOD)
        && !x.is_null()
    {
        out.push(("forceLoginMethod".into(), x.clone()));
    }
    if let Some(x) = m.get("forceLoginGatewayUrl")
        && !x.is_null()
    {
        out.push(("forceLoginGatewayUrl".into(), x.clone()));
    }
    if let Some(env) = m.get("env").and_then(Value::as_object) {
        for (name, x) in env {
            if env_blocked(name) {
                out.push((format!("env.{name}"), x.clone()));
            }
        }
    }
}

/// SR-07 over every settings source (user, managed, project, local, and the `env` maps of
/// `~/.claude.json` project entries). Allowlisted user keys pass.
#[must_use]
pub fn auth_key_hits(files: &[ConfigFile], allow: &[AllowlistEntry]) -> Vec<AuthKeyHit> {
    let mut hits = Vec::new();
    for f in files {
        let (Some(scope), Some(json)) = (f.scope, f.json.as_ref()) else {
            continue;
        };
        let mut found = Vec::new();
        match scope {
            SettingsScope::ClaudeJson => {
                // Project entries' env maps (names only were kept by the streaming reader).
                if let Some(projects) = json.get("projects").and_then(Value::as_object) {
                    for p in projects.values() {
                        if let Some(env) = p.get("env").and_then(Value::as_object) {
                            for (name, x) in env {
                                if env_blocked(name) {
                                    found.push((format!("env.{name}"), x.clone()));
                                }
                            }
                        }
                    }
                }
            }
            _ => scan_settings(json, &mut found),
        }
        for (key, value) in found {
            let value_hash = allowlist_hash(&key, &value);
            let allowlistable = scope == SettingsScope::User;
            if allowlistable && allow.iter().any(|a| a.key == key && a.sha256 == value_hash) {
                continue;
            }
            let id = sha256_hex(format!("{}\n{key}\n{value_hash}", f.path).as_bytes());
            hits.push(AuthKeyHit {
                file: f.path.clone(),
                key,
                id,
                value_hash,
                allowlistable,
            });
        }
    }
    hits
}
