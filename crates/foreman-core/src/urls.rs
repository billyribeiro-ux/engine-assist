//! URLs Foreman opens in the default browser (SR-25, SR-29; T-093, T-100). Pure and lexical: a
//! URL is accepted only in the narrow forms below, and only an [`OpenUrl`] (which only these
//! checks construct) reaches `foreman-sys`'s `open`.
//!
//! - [`foreman_url`] (SR-25, AC-XC-08): links Foreman generates or stores: `https://claude.ai/…`
//!   (Project and chat links) and `https://code.claude.com/docs/…` (Doctor help).
//! - [`chat_url`] (AC-F012-02): what Link this chat and the Project link accept: `https://claude.ai/…`.
//! - [`terminal_url`] (SR-29, AC-F009-17): a link clicked in a terminal: `https:` to any host, or
//!   `http:` to `localhost` / `127.0.0.1`, with no userinfo. Billy still confirms it in a native
//!   dialog showing the full URL with visible escapes.
//!
//! Characters that parsers disagree on are refused outright: whitespace, controls and the
//! backslash (which WHATWG URL parsing treats as `/` in `http(s)` URLs, so `https://a\@b` would
//! name a different host than a strict parser sees).

use serde::{Deserialize, Serialize};

/// The longest URL Foreman opens (`terminal_link_open`'s 8 KB, `docs/contracts/ipc.md` §4.7).
pub const MAX_URL: usize = 8 * 1024;

/// A URL that passed one of the checks in this module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenUrl(String);

impl OpenUrl {
    /// The URL as it is opened.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OpenUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a URL was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum UrlRefusal {
    /// Empty or longer than [`MAX_URL`].
    #[error("the link is empty or too long")]
    Length,
    /// Whitespace, a control character or a backslash.
    #[error("the link contains a space, a control character or a backslash")]
    Characters,
    /// Not `scheme://authority…`.
    #[error("the link isn't a web address")]
    Malformed,
    /// A scheme other than the ones allowed here.
    #[error("only https links open")]
    Scheme,
    /// `user:password@` before the host.
    #[error("the link carries a user name or password")]
    Userinfo,
    /// A host (or port) outside the allowed set.
    #[error("Foreman doesn't open links to that site")]
    Host,
}

/// The allowed prefixes of [`foreman_url`] (each also allowed without its final `/`).
pub const FOREMAN_PREFIXES: [&str; 2] = ["https://claude.ai/", "https://code.claude.com/docs/"];

fn clean(s: &str) -> Result<(), UrlRefusal> {
    if s.is_empty() || s.len() > MAX_URL {
        return Err(UrlRefusal::Length);
    }
    if s.chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\' || is_invisible(c))
    {
        return Err(UrlRefusal::Characters);
    }
    Ok(())
}

/// Bidi controls, zero-width and other format characters (never valid in a URL Foreman makes).
fn is_invisible(c: char) -> bool {
    matches!(c,
        '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{e0000}'..='\u{e007f}')
}

/// RFC 3986 characters (unreserved, reserved and `%`), ASCII only.
fn rfc3986(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&b))
}

fn prefixed(s: &str, prefixes: &[&str]) -> Result<OpenUrl, UrlRefusal> {
    clean(s)?;
    let ok = prefixes
        .iter()
        .any(|p| s.starts_with(p) || p.strip_suffix('/').is_some_and(|bare| s == bare));
    if !ok {
        return Err(match split(s) {
            Ok(_) if !s.starts_with("https://") => UrlRefusal::Scheme,
            Ok((_, authority, _)) if authority.contains('@') => UrlRefusal::Userinfo,
            Ok(_) => UrlRefusal::Host,
            Err(e) => e,
        });
    }
    if !rfc3986(s) {
        return Err(UrlRefusal::Characters);
    }
    Ok(OpenUrl(s.to_owned()))
}

/// SR-25 / AC-XC-08: a URL Foreman generates or stores: `https://claude.ai/…` or
/// `https://code.claude.com/docs/…`, ASCII URL characters only (no userinfo or port can appear,
/// because the authority is fixed by the prefix).
///
/// # Errors
/// The [`UrlRefusal`].
pub fn foreman_url(s: &str) -> Result<OpenUrl, UrlRefusal> {
    prefixed(s, &FOREMAN_PREFIXES)
}

/// AC-F012-02: a chat or Project link: `https://claude.ai/…` only.
///
/// # Errors
/// The [`UrlRefusal`].
pub fn chat_url(s: &str) -> Result<OpenUrl, UrlRefusal> {
    prefixed(s, &FOREMAN_PREFIXES[..1])
}

/// `(scheme lowercased, authority, rest)` of `scheme://authority rest`.
fn split(s: &str) -> Result<(String, &str, &str), UrlRefusal> {
    let (scheme, after) = s.split_once(':').ok_or(UrlRefusal::Malformed)?;
    let scheme_ok = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b));
    if !scheme_ok {
        return Err(UrlRefusal::Malformed);
    }
    let scheme = scheme.to_ascii_lowercase();
    let Some(rest) = after.strip_prefix("//") else {
        // `javascript:…`, `mailto:…`, `file:/…` and other schemes without an authority.
        return Err(if matches!(scheme.as_str(), "http" | "https") {
            UrlRefusal::Malformed
        } else {
            UrlRefusal::Scheme
        });
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    Ok((scheme, &rest[..end], &rest[end..]))
}

/// `host` and the optional port of an authority (an IPv6 literal keeps its brackets).
fn host_port(authority: &str) -> Result<(String, Option<&str>), UrlRefusal> {
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (inner, after) = v6.split_once(']').ok_or(UrlRefusal::Malformed)?;
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':').ok_or(UrlRefusal::Malformed)?),
        };
        (format!("[{inner}]"), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_owned(), Some(p)),
            None => (authority.to_owned(), None),
        }
    };
    if host.is_empty() || (host.contains(':') && !host.starts_with('[')) {
        return Err(UrlRefusal::Malformed);
    }
    if let Some(p) = port
        && (!p.bytes().all(|b| b.is_ascii_digit()) || p.parse::<u16>().is_err())
    {
        return Err(UrlRefusal::Malformed);
    }
    Ok((host.to_ascii_lowercase(), port))
}

/// SR-29 / AC-F009-17: a terminal link: `https:` (any host), or `http:` to `localhost` or
/// `127.0.0.1`; never userinfo. The scheme is lowercased; nothing else is changed.
///
/// # Errors
/// The [`UrlRefusal`]; every scheme but `https`/`http` is [`UrlRefusal::Scheme`].
pub fn terminal_url(s: &str) -> Result<OpenUrl, UrlRefusal> {
    clean(s)?;
    let (scheme, authority, rest) = split(s)?;
    if scheme != "https" && scheme != "http" {
        return Err(UrlRefusal::Scheme);
    }
    if authority.contains('@') {
        return Err(UrlRefusal::Userinfo);
    }
    let (host, _) = host_port(authority)?;
    if scheme == "http" && host != "localhost" && host != "127.0.0.1" {
        return Err(UrlRefusal::Scheme);
    }
    Ok(OpenUrl(format!("{scheme}://{authority}{rest}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_parts() {
        assert_eq!(
            split("HTTPS://Example.com:8443/a?b#c"),
            Ok(("https".to_owned(), "Example.com:8443", "/a?b#c"))
        );
        assert_eq!(split("javascript:alert(1)"), Err(UrlRefusal::Scheme));
        assert_eq!(split("https:/x"), Err(UrlRefusal::Malformed));
    }
}
