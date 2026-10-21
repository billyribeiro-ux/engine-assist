//! M1-23: paste stripping and visible escapes (DR-095, SR-26, SR-28, T-088).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use foreman_core::sanitize::{for_paste, visible_escapes};

mod pty {
    use super::*;

    /// AC-F009-15/-16 (`for_paste` unit part): ESC, C0 (except TAB, LF, CR), DEL and C1 go.
    #[test]
    fn paste_strips_escape_sequences() {
        let (out, r) = for_paste("\x1b[201~\x03!touch /tmp/pwn\r");
        assert_eq!(out, "[201~!touch /tmp/pwn\r");
        assert_eq!(r.removed, 2);
        assert_eq!(r.message().unwrap(), "removed 2 control characters");
        let (out, r) = for_paste("line1\nline2\ttab\r\n\u{7f}\u{9b}\u{85}x");
        assert_eq!(out, "line1\nline2\ttab\r\nx");
        assert_eq!(r.removed, 3);
        let (out, r) = for_paste("plain");
        assert_eq!((out.as_str(), r.removed, r.message()), ("plain", 0, None));
        // OSC 52 / bracketed-paste terminators lose their ESC and BEL.
        let (out, _) = for_paste("\x1b]52;c;ZWNobyBwd24=\x07");
        assert!(!out.contains('\x1b') && !out.contains('\x07'));
    }
}

mod render {
    use super::*;

    /// SR-26 / T-088 (unit part): bidi, zero-width, tag characters and every
    /// Default_Ignorable_Code_Point come back as spans.
    #[test]
    fn default_ignorables_visible() {
        let s = "exe\u{202e}dm.md a\u{200b}b \u{e0041}\u{e0042} soft\u{ad}hyphen \u{2066}x\u{2069} \u{fe0f} \u{180e} \u{1d173} \u{3164} c\u{1b}d";
        let spans = visible_escapes(s);
        let cps: Vec<u32> = spans.iter().map(|x| x.codepoint).collect();
        assert_eq!(
            cps,
            [
                0x202e, 0x200b, 0xe0041, 0xe0042, 0xad, 0x2066, 0x2069, 0xfe0f, 0x180e, 0x1d173,
                0x3164, 0x1b
            ]
        );
        assert_eq!(spans[0].label, "U+202E bidi control");
        assert_eq!(spans[1].label, "U+200B zero-width");
        assert_eq!(spans[2].label, "U+E0041 tag character");
        assert_eq!(spans.last().unwrap().label, "U+001B control");
        assert_eq!(&s[spans[0].start..spans[0].end], "\u{202e}");
        assert!(visible_escapes("plain text\twith tabs\nand lines").is_empty());
        // Every range of the Default_Ignorable table is covered.
        for (a, b) in foreman_core::sanitize::DEFAULT_IGNORABLE {
            for cp in [a, b] {
                let c = char::from_u32(cp).unwrap();
                assert_eq!(visible_escapes(&c.to_string()).len(), 1, "U+{cp:04X}");
            }
        }
    }
}
