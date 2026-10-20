//! M3-12, M3-23: the URL policy (SR-25, SR-29; T-093, T-100; AC-XC-08, AC-F009-17, AC-F012-02).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::urls::{UrlRefusal, chat_url, foreman_url, terminal_url};

mod opener {
    use super::*;

    /// SR-25 / AC-XC-08 / AC-F012-02: Foreman opens only `https://claude.ai/…` and
    /// `https://code.claude.com/docs/…`; chat links only `https://claude.ai/…`.
    #[test]
    fn only_allowed_urls() {
        for ok in [
            "https://claude.ai",
            "https://claude.ai/",
            "https://claude.ai/project/0199d-abc",
            "https://claude.ai/chat/7c1e9a2b-55d0-4c1f-8f04-3b2a1d0e9f04",
            "https://claude.ai/new?q=a%20b#frag",
            "https://code.claude.com/docs/en/settings",
            "https://code.claude.com/docs",
        ] {
            assert_eq!(foreman_url(ok).unwrap().as_str(), ok, "{ok}");
        }
        for (bad, why) in [
            ("http://claude.ai/x", UrlRefusal::Scheme),
            ("HTTPS://claude.ai/x", UrlRefusal::Scheme),
            ("https://claude.ai.evil.com/", UrlRefusal::Host),
            ("https://claude.aix/", UrlRefusal::Host),
            ("https://evil.com/claude.ai/", UrlRefusal::Host),
            ("https://claude.ai@evil.com/", UrlRefusal::Userinfo),
            ("https://user:pw@claude.ai/", UrlRefusal::Userinfo),
            ("https://claude.ai:443/", UrlRefusal::Host),
            ("https://code.claude.com/", UrlRefusal::Host),
            ("https://code.claude.com/docsx", UrlRefusal::Host),
            ("https://claude.ai/a b", UrlRefusal::Characters),
            ("https://claude.ai/\\x", UrlRefusal::Characters),
            ("https://claude.ai/\u{202e}x", UrlRefusal::Characters),
            ("https://claude.ai/x\u{200b}", UrlRefusal::Characters),
            ("https://claude.ai/caf\u{e9}", UrlRefusal::Characters),
            (" https://claude.ai/", UrlRefusal::Characters),
            ("https://claude.ai/\n", UrlRefusal::Characters),
            (
                "javascript:alert(1)//https://claude.ai/",
                UrlRefusal::Scheme,
            ),
            ("file:///Applications/Calculator.app", UrlRefusal::Scheme),
            ("", UrlRefusal::Length),
        ] {
            assert_eq!(foreman_url(bad), Err(why), "{bad:?}");
        }
        assert!(foreman_url(&format!("https://claude.ai/{}", "a".repeat(9000))).is_err());
        // Chat links: claude.ai only (the Doctor docs host is not a chat).
        assert!(chat_url("https://claude.ai/chat/abc").is_ok());
        assert_eq!(
            chat_url("https://code.claude.com/docs/x"),
            Err(UrlRefusal::Host)
        );
    }

    /// SR-29 / AC-F009-17 / T-093: terminal links are `https:` (any host) or `http:` to
    /// localhost/127.0.0.1, never with userinfo; every other scheme is refused.
    #[test]
    fn terminal_link_scheme_policy() {
        for (ok, want) in [
            ("https://example.com/a?b#c", "https://example.com/a?b#c"),
            ("HTTPS://Example.com:8443/x", "https://Example.com:8443/x"),
            ("http://localhost:5173/", "http://localhost:5173/"),
            ("http://127.0.0.1/health", "http://127.0.0.1/health"),
            ("http://LOCALHOST", "http://LOCALHOST"),
            // A lookalike host passes the scheme policy; the native dialog shows it in full.
            (
                "https://claude.ai.evil.example/",
                "https://claude.ai.evil.example/",
            ),
            ("https://[::1]:8080/", "https://[::1]:8080/"),
        ] {
            assert_eq!(terminal_url(ok).unwrap().as_str(), want, "{ok}");
        }
        for (bad, why) in [
            ("javascript:alert(1)", UrlRefusal::Scheme),
            ("file:///Applications/Calculator.app", UrlRefusal::Scheme),
            ("vscode://file/etc/passwd", UrlRefusal::Scheme),
            (
                "x-apple.systempreferences:com.apple.preference.security",
                UrlRefusal::Scheme,
            ),
            ("ssh://host", UrlRefusal::Scheme),
            ("data:text/html,<script>", UrlRefusal::Scheme),
            ("http://example.com/", UrlRefusal::Scheme),
            ("http://127.0.0.2/", UrlRefusal::Scheme),
            ("http://localhost.evil.com/", UrlRefusal::Scheme),
            ("https://user@example.com/", UrlRefusal::Userinfo),
            ("https://a:b@example.com/", UrlRefusal::Userinfo),
            ("http://user@localhost/", UrlRefusal::Userinfo),
            ("https://evil.com\\@claude.ai/", UrlRefusal::Characters),
            ("https://exa mple.com/", UrlRefusal::Characters),
            (
                "https://example.com/\u{202e}moc.lave",
                UrlRefusal::Characters,
            ),
            ("https://example.com/\u{1b}]8;;", UrlRefusal::Characters),
            ("https:example.com", UrlRefusal::Malformed),
            ("https://", UrlRefusal::Malformed),
            ("https://example.com:99999/", UrlRefusal::Malformed),
            ("https://example.com:/", UrlRefusal::Malformed),
            ("example.com", UrlRefusal::Malformed),
            ("", UrlRefusal::Length),
        ] {
            assert_eq!(terminal_url(bad), Err(why), "{bad:?}");
        }
        assert_eq!(
            terminal_url(&format!("https://e.com/{}", "a".repeat(8200))),
            Err(UrlRefusal::Length)
        );
    }
}
