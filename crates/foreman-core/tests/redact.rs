//! M2-23: redacted config and decision snapshots (SR-55, AC-F008-23, T-070): every class SR-55
//! names is replaced by `‹redacted sha256:8›`, at any depth; a redacted value that changes shows
//! as "value changed" with no value on either side; plain values stay readable.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::fingerprint::{self, ChangeKind, looks_secret, redact};
use foreman_core::schema::fingerprint::SnapshotEntry;
use serde_json::{Value, json};

fn is_marker(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.strip_prefix("‹redacted sha256:")
            .and_then(|r| r.strip_suffix('›'))
            .is_some_and(|h| {
                h.len() == 8
                    && h.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
    })
}

fn entry(redacted: Value, sha: &str) -> SnapshotEntry {
    SnapshotEntry {
        source: "settings".into(),
        path: "/home/b/.claude/settings.json".into(),
        sha256: sha.repeat(64),
        redacted,
    }
}

mod preflight {
    use super::*;

    /// AC-F008-23, one assertion per class of SR-55, including nesting and arrays.
    #[test]
    fn config_snapshot_redacts_every_class() {
        let v = json!({
            "env": { "A": "x", "B": 1, "C": true, "D": { "deep": "y" }, "E": ["z"] },
            "headers": { "Authorization": "plain-looking" },
            "servers": [{ "name": "s", "env": { "TOKEN_FILE": "/p" }, "headers": { "X": "1" } }],
            "apiKeyHelper": "/bin/h",
            "apiKey2": "v",
            "AccessToken": "t1",
            "clientSECRET": "t2",
            "db": { "pass": { "PassWord": "t3" } },
            "list": [{ "refresh_token": "t4" }, "ok"],
            "model": "opus",
            "n": 3,
            "ok": true,
            "permissions": { "allow": ["Bash(ls)"], "deny": [] }
        });
        let r = redact(&v);
        for p in ["A", "B", "C", "D", "E"] {
            let inner = &r["env"][p];
            let all_markers = match inner {
                Value::Object(m) => m.values().all(is_marker),
                Value::Array(a) => a.iter().all(is_marker),
                other => is_marker(other),
            };
            assert!(all_markers, "env.{p}: {inner}");
        }
        assert!(is_marker(&r["headers"]["Authorization"]));
        assert!(is_marker(&r["servers"][0]["env"]["TOKEN_FILE"]));
        assert!(is_marker(&r["servers"][0]["headers"]["X"]));
        for k in ["apiKeyHelper", "apiKey2", "AccessToken", "clientSECRET"] {
            assert!(is_marker(&r[k]), "{k}: {}", r[k]);
        }
        assert!(is_marker(&r["db"]["pass"]["PassWord"]));
        assert!(is_marker(&r["list"][0]["refresh_token"]));
        // Plain values stay readable so a diff is useful.
        assert_eq!(r["list"][1], "ok");
        assert_eq!(r["model"], "opus");
        assert_eq!(r["n"], 3);
        assert_eq!(r["ok"], true);
        assert_eq!(r["permissions"], v["permissions"]);
        assert_eq!(r["servers"][0]["name"], "s");
        // Nothing of any redacted value survives anywhere in the text.
        let text = r.to_string();
        for leaked in [
            "plain-looking",
            "/bin/h",
            "\"t1\"",
            "\"t2\"",
            "\"t3\"",
            "\"t4\"",
            "/p\"",
        ] {
            assert!(!text.contains(leaked), "{leaked} in {text}");
        }
    }

    /// The scanner-pattern class: credentials by shape, wherever they sit, even under innocent keys.
    #[test]
    fn config_snapshot_redacts_secret_shapes() {
        let secrets = [
            "AKIAIOSFODNN7EXAMPLE",
            "ghp_0123456789abcdefghijklmnopqrstuvwxyzAB",
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz",
            concat!("glpat", "-abcdefghijklmnopqrst"),
            "sk-ant-api03-abcdefghijklmnop",
            "xoxb-123456789012-abcdefghij",
            "AIzaSyA-abcdefghijklmnopqrstuvwxyz01234",
            "ya29.a0AfH6SMBabcdefghijklmnop",
            "SG.abcdefghijklmnopqrstuvwxyz.ABCDEFGHIJ",
            concat!(
                "eyJhbGciOiJIUzI1NiJ9",
                ".eyJzdWIiOiIxMjM0NTY3ODkwIn0",
                ".abcdefghijklmnop"
            ),
            "Bearer abcdefghijklmnop",
            "Basic dXNlcjpwYXNzd29yZA==",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "postgres://user:hunter2@db.example.com/app",
            "npm_abcdefghijklmnopqrstuvwxyz0123456789",
        ];
        for s in secrets {
            assert!(looks_secret(s), "{s}");
            let r = redact(&json!({ "description": s, "list": [s], "a": { "b": { "note": s } } }));
            assert!(is_marker(&r["description"]), "{s}");
            assert!(is_marker(&r["list"][0]), "{s}");
            assert!(is_marker(&r["a"]["b"]["note"]), "{s}");
            assert!(!r.to_string().contains(s), "{s}");
        }
        for plain in [
            "opus",
            "Bash(ls)",
            "Basic setup",
            "Bearer",
            "https://example.com/docs",
            "sudo",
            "git@github.com:org/repo.git",
            "SG",
        ] {
            assert!(!looks_secret(plain), "{plain}");
        }
    }

    /// The marker is the same for the same value (so an unchanged secret is no change) and
    /// different for a different one; it never contains the value.
    #[test]
    fn redaction_markers_are_stable() {
        let a1 = redact(&json!({ "env": { "K": "same" } }));
        let a2 = redact(&json!({ "env": { "K": "same" } }));
        let b = redact(&json!({ "env": { "K": "other" } }));
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert!(!a1.to_string().contains("same"));
    }

    /// AC-F008-23: a changed secret is "value changed" and neither side carries it; an unchanged
    /// secret next to a plain change produces only the plain change.
    #[test]
    fn diff_shows_redacted_change_only() {
        let old = vec![entry(
            redact(
                &json!({ "env": { "KEY": "old-secret" }, "token": "t-old", "model": "opus", "ok": 1 }),
            ),
            "1",
        )];
        let new = vec![entry(
            redact(
                &json!({ "env": { "KEY": "new-secret" }, "token": "t-old", "model": "sonnet", "ok": 1 }),
            ),
            "2",
        )];
        let d = fingerprint::diff(&old, &new);
        assert_eq!(d.len(), 1);
        let by: std::collections::BTreeMap<&str, &fingerprint::RedactedEntry> = d[0]
            .entries
            .iter()
            .map(|e| (e.key_path.as_str(), e))
            .collect();
        assert_eq!(by["env.KEY"].change, ChangeKind::ValueChanged);
        assert_eq!(by["model"].change, ChangeKind::ValueChanged);
        assert_eq!(by["model"].before.as_deref(), Some("\"opus\""));
        assert!(
            !by.contains_key("token"),
            "an unchanged secret is not a change: {by:?}"
        );
        let text = serde_json::to_string(&d).unwrap();
        for leaked in ["old-secret", "new-secret", "t-old"] {
            assert!(!text.contains(leaked), "{leaked} in {text}");
        }
        for side in [&by["env.KEY"].before, &by["env.KEY"].after] {
            assert!(
                side.as_deref()
                    .is_some_and(|s| s.contains("‹redacted sha256:")),
                "{side:?}"
            );
        }
    }

    /// Unreadable-as-JSON sources (a markdown file, a script) are compared by hash only.
    #[test]
    fn non_json_sources_diff_by_hash() {
        let a = SnapshotEntry {
            redacted: Value::Null,
            ..entry(Value::Null, "1")
        };
        let b = SnapshotEntry {
            redacted: Value::Null,
            ..entry(Value::Null, "2")
        };
        let d = fingerprint::diff(&[a], &[b]);
        assert_eq!(d[0].entries[0].change, ChangeKind::ValueChanged);
        assert!(
            d[0].entries[0]
                .before
                .as_deref()
                .unwrap()
                .starts_with("sha256:")
        );
    }
}
