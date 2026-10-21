//! Safe text for the webview (SR-26, M2-05): repo, pasted and terminal text crosses IPC only as
//! typed spans, with bidi, zero-width, tag and control characters as visible escapes computed here.

use foreman_core::sanitize::visible_escapes;

use crate::ipc::types::{DocNode, TextSpan};

/// Text as `plain` spans with every invisible or control character as an `escape` span.
#[must_use]
pub fn spans(text: &str) -> Vec<TextSpan> {
    let mut out = Vec::new();
    let mut last = 0;
    for e in visible_escapes(text) {
        if e.start > last {
            out.push(TextSpan::Plain {
                text: text[last..e.start].to_owned(),
            });
        }
        out.push(TextSpan::Escape {
            codepoint: e.codepoint,
            label: e.label,
        });
        last = e.end;
    }
    if last < text.len() || out.is_empty() {
        out.push(TextSpan::Plain {
            text: text[last..].to_owned(),
        });
    }
    out
}

/// Text with every invisible or control character replaced by `‹U+XXXX›` (for native dialogs and
/// error details, which are plain strings).
#[must_use]
pub fn escape_visible(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for e in visible_escapes(text) {
        out.push_str(&text[last..e.start]);
        out.push_str(&format!("‹U+{:04X}›", e.codepoint));
        last = e.end;
    }
    out.push_str(&text[last..]);
    out
}

/// How many lines of text about to be written a native dialog shows.
pub const EXCERPT_LINES: usize = 15;
/// How many characters of each of those lines it shows.
pub const EXCERPT_WIDTH: usize = 100;

/// What a native dialog shows of text Foreman is about to write (security review 2026-10-20,
/// W-2; SR-24a, SR-56): the SHA-256 prefix, the size in bytes and lines, and the first
/// [`EXCERPT_LINES`] lines with every control, bidi, zero-width or invisible character shown as
/// `‹U+XXXX›` (TAB and CR too), each cut at [`EXCERPT_WIDTH`] characters. Billy sees what will
/// be written, not only that something will be.
#[must_use]
pub fn content_summary(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = format!(
        "SHA-256 {} · {} bytes · {} {}",
        &foreman_core::store::sha256_hex(text.as_bytes())[..12],
        text.len(),
        lines.len(),
        if lines.len() == 1 { "line" } else { "lines" }
    );
    if lines.is_empty() {
        out.push_str("\n(empty)");
        return out;
    }
    out.push_str(&format!(
        "\n{}:",
        if lines.len() > EXCERPT_LINES {
            format!("First {EXCERPT_LINES} lines")
        } else {
            "The text".to_owned()
        }
    ));
    for line in lines.iter().take(EXCERPT_LINES) {
        let shown = escape_visible(line)
            .replace('\t', "‹U+0009›")
            .replace('\r', "‹U+000D›");
        let mut cut: String = shown.chars().take(EXCERPT_WIDTH).collect();
        if shown.chars().count() > EXCERPT_WIDTH {
            cut.push('…');
        }
        out.push_str("\n│ ");
        out.push_str(&cut);
    }
    if lines.len() > EXCERPT_LINES {
        out.push_str(&format!(
            "\n… and {} more lines.",
            lines.len() - EXCERPT_LINES
        ));
    }
    out
}

/// A packet field's text as block nodes (SR-26): paragraphs, `- `/`* ` and `1. ` lists, fenced
/// code. Inline Markdown stays literal text; nothing becomes HTML.
#[must_use]
pub fn doc_nodes(text: &str) -> Vec<DocNode> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut items: Vec<Vec<DocNode>> = Vec::new();
    let mut ordered = false;
    let mut fence: Option<(String, Vec<&str>)> = None;
    let flush_para = |para: &mut Vec<&str>, out: &mut Vec<DocNode>| {
        if !para.is_empty() {
            out.push(DocNode::Paragraph {
                spans: spans(&para.join("\n")),
            });
            para.clear();
        }
    };
    let flush_list = |items: &mut Vec<Vec<DocNode>>, ordered: bool, out: &mut Vec<DocNode>| {
        if !items.is_empty() {
            out.push(DocNode::List {
                ordered,
                items: std::mem::take(items),
            });
        }
    };
    for line in text.lines() {
        if let Some((info, body)) = &mut fence {
            if line.trim_start().starts_with("```") {
                out.push(DocNode::CodeBlock {
                    info: std::mem::take(info),
                    text: body.join("\n"),
                });
                fence = None;
            } else {
                body.push(line);
            }
            continue;
        }
        let t = line.trim_start();
        if let Some(info) = t.strip_prefix("```") {
            flush_para(&mut para, &mut out);
            flush_list(&mut items, ordered, &mut out);
            fence = Some((info.trim().to_owned(), Vec::new()));
            continue;
        }
        let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
        let numbered = t
            .split_once(". ")
            .filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
            .map(|(_, rest)| rest);
        if let Some(rest) = bullet.or(numbered) {
            flush_para(&mut para, &mut out);
            let is_ordered = bullet.is_none();
            if !items.is_empty() && is_ordered != ordered {
                flush_list(&mut items, ordered, &mut out);
            }
            ordered = is_ordered;
            items.push(vec![DocNode::Paragraph { spans: spans(rest) }]);
        } else if t.is_empty() {
            flush_para(&mut para, &mut out);
            flush_list(&mut items, ordered, &mut out);
        } else if !items.is_empty() && line.starts_with([' ', '\t']) {
            if let Some(DocNode::Paragraph { spans: sp }) =
                items.last_mut().and_then(|i| i.last_mut())
            {
                sp.push(TextSpan::Plain {
                    text: format!("\n{t}"),
                });
            }
        } else {
            flush_list(&mut items, ordered, &mut out);
            para.push(line);
        }
    }
    if let Some((info, body)) = fence {
        out.push(DocNode::CodeBlock {
            info,
            text: body.join("\n"),
        });
    }
    flush_para(&mut para, &mut out);
    flush_list(&mut items, ordered, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_summary_shows_hash_size_and_visible_lines() {
        let s = content_summary("a\tb\r\n\u{1b}[31mred\n");
        assert!(s.starts_with("SHA-256 "), "{s}");
        assert!(s.contains(" · 14 bytes · 2 lines\nThe text:"), "{s}");
        assert!(s.contains("│ a‹U+0009›b"), "{s}");
        assert!(s.contains("│ ‹U+001B›[31mred"), "{s}");
        assert_eq!(content_summary("").lines().nth(1), Some("(empty)"));
        let long = "x".repeat(150);
        assert!(content_summary(&long).contains(&format!("│ {}…", "x".repeat(100))));
    }

    #[test]
    fn escapes_bidi() {
        let s = spans("a\u{202e}b");
        assert_eq!(s.len(), 3);
        assert!(matches!(
            s[1],
            TextSpan::Escape {
                codepoint: 0x202e,
                ..
            }
        ));
        assert_eq!(escape_visible("a\u{202e}b"), "a‹U+202E›b");
        assert_eq!(
            spans(""),
            vec![TextSpan::Plain {
                text: String::new()
            }]
        );
    }
}
