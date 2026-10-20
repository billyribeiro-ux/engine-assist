//! M3-20: packet normalization, splitting a paste and the fix request (DR-131, DR-132, DR-137;
//! AC-F012-05, AC-F012-10, AC-F012-11; T-090). The golden inputs are Foreman's own packets
//! (`tests/fixtures/packets/foreman/`, copies of `docs/handoff/stage-0[1-4].md`) pasted back the
//! way chat output arrives: fenced, CRLF, smart quotes, NBSP, header variants and every unsigned
//! `APPROVED BY` and `OPEN ITEMS` variant.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "test code may use std::fs (design doc §3.2)"
)]

mod common;

use common::{blueprint, parse, read_fixture};
use foreman_core::packet::{
    self as pk, NoteKind, PacketCtx, SplitError, fix_request_text, normalize_packet,
};
use foreman_core::template::Template;
use proptest::prelude::*;

fn tpl() -> Template {
    parse(&blueprint())
}

fn foreman(n: u8) -> String {
    read_fixture(&format!("fixtures/packets/foreman/stage-{n:02}.md"))
}

/// The packet as Billy would get it back unsigned: the real file with `APPROVED BY: <blank>`.
fn unsigned(n: u8) -> String {
    let real = foreman(n);
    let line = real
        .lines()
        .find(|l| l.starts_with("APPROVED BY:"))
        .unwrap()
        .to_owned();
    real.replacen(&line, "APPROVED BY: <blank>", 1)
}

/// Alternating curly double quotes and curly apostrophes for every straight one.
fn smart_quotes(s: &str) -> String {
    let mut open = true;
    s.chars()
        .map(|c| match c {
            '"' => {
                open = !open;
                if open { '”' } else { '“' }
            }
            '\'' => '’',
            c => c,
        })
        .collect()
}

fn kinds(n: &pk::Normalized) -> Vec<NoteKind> {
    n.notes.iter().map(|x| x.kind).collect()
}

/// How each Foreman packet is pasted back (one mangling recipe per packet, so together they cover
/// every row of files.md §5.2).
fn pasted(n: u8) -> (String, Vec<NoteKind>) {
    let base = unsigned(n);
    let mut body: Vec<String> = base.lines().map(str::to_owned).collect();
    let approved = body
        .iter()
        .position(|l| l.starts_with("APPROVED BY:"))
        .unwrap();
    let open = body
        .iter()
        .position(|l| l.starts_with("OPEN ITEMS:"))
        .unwrap();
    match n {
        1 => {
            // Fenced, CRLF, smart quotes, `<leave blank for me to sign>`, `None.`.
            body[approved] = "APPROVED BY: <leave blank for me to sign>".into();
            body[open] = "OPEN ITEMS: None.".into();
            let text = smart_quotes(&body.join("\n"));
            let fenced = format!("```text\n{text}\n```\n").replace('\n', "\r\n");
            (
                fenced,
                vec![
                    NoteKind::CodeFence,
                    NoteKind::SmartQuotes,
                    NoteKind::Unsigned,
                    NoteKind::OpenItemsNone,
                ],
            )
        }
        2 => {
            // NBSP and narrow NBSP, a `->`/`-` header with extra spaces, `<sign here…>`, `NONE`.
            body[0] = "===  HANDOFF PACKET - Stage 2 -> Stage 3  ===".into();
            body[approved] = "APPROVED BY: <Sign here when you've read it>".into();
            body[open] = "OPEN ITEMS: NONE".into();
            let text = body
                .join("\n")
                .replace(". ", ".\u{a0}")
                .replace(": ", ":\u{202f}");
            (
                format!("{text}\n"),
                vec![
                    NoteKind::NonBreakingSpaces,
                    NoteKind::Header,
                    NoteKind::Unsigned,
                    NoteKind::OpenItemsNone,
                ],
            )
        }
        3 => {
            // BOM, CR-only line ends, `[BLANK]`, `none.`, a `•` header, trailing spaces.
            body[0] = "=== HANDOFF PACKET • Stage 3 → Stage 4 ===".into();
            body[approved] = "APPROVED BY: [BLANK]".into();
            body[open] = "OPEN ITEMS: none.".into();
            let text = body
                .iter()
                .map(|l| format!("{l}  \t"))
                .collect::<Vec<_>>()
                .join("\r");
            (
                format!("\u{feff}{text}\r"),
                vec![
                    NoteKind::Header,
                    NoteKind::Unsigned,
                    NoteKind::OpenItemsNone,
                ],
            )
        }
        _ => {
            // An empty APPROVED BY, `None`, a `|` header, a plain fence.
            body[0] = "=== HANDOFF PACKET | Stage 4 → Stage 5 ===".into();
            body[approved] = "APPROVED BY:".into();
            body[open] = "OPEN ITEMS: None".into();
            (
                format!("\n```\n{}\n```\n", body.join("\n")),
                vec![
                    NoteKind::CodeFence,
                    NoteKind::Header,
                    NoteKind::Unsigned,
                    NoteKind::OpenItemsNone,
                ],
            )
        }
    }
}

/// Replaces a field's whole value (first line and continuation lines), or drops the field.
fn set_field(text: &str, key: &str, value: Option<&str>) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for l in text.lines() {
        if skipping && (l.starts_with([' ', '\t']) || l.trim().is_empty()) {
            continue;
        }
        skipping = false;
        if l.starts_with(&format!("{key}:")) {
            skipping = true;
            if let Some(v) = value {
                out.push_str(&format!(
                    "{key}:{}{v}\n",
                    if v.is_empty() { "" } else { " " }
                ));
            }
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn ctx(stage: u8) -> PacketCtx {
    PacketCtx {
        stage,
        next_stage: Some(stage + 1),
        lock_blueprint_version: Some("2.2".into()),
        has_skips: true,
    }
}

mod packet {
    use super::*;

    /// AC-F012-05 (golden): Foreman's own packets pasted back normalize to exactly the real file,
    /// unsigned. `<blank>`, `<leave blank…>`, `<sign here…>`, `[blank]` and an empty value become
    /// `<blank>` ("unsigned"); `None`, `NONE`, `none.`, `None.` become `none` with a note; smart
    /// quotes, NBSP, fences, CRLF, BOM, trailing spaces and header variants are fixed; every note
    /// is recorded in §5.2's order; nothing else changes.
    #[test]
    fn normalization_golden() {
        let t = tpl();
        let mut golden = Vec::new();
        for n in 1..=4u8 {
            let (paste, want) = pasted(n);
            let out = normalize_packet(&paste);
            let expect = unsigned(n);
            if out.text != expect {
                let at = out
                    .text
                    .lines()
                    .zip(expect.lines())
                    .position(|(a, b)| a != b);
                panic!(
                    "stage {n}: first difference at line {at:?}:\n got {:?}\nwant {:?}",
                    at.and_then(|i| out.text.lines().nth(i)),
                    at.and_then(|i| expect.lines().nth(i))
                );
            }
            assert_eq!(kinds(&out), want, "stage {n}");
            // Idempotent: the normalized text has nothing left to note.
            let again = normalize_packet(&out.text);
            assert_eq!(again.text, out.text, "stage {n}");
            assert!(again.notes.is_empty(), "stage {n}: {:?}", again.notes);
            // The normalized packet parses to the real file's fields, unsigned.
            let fmt = t.packet_format(n).unwrap();
            let keys: Vec<String> = fmt.keys.iter().map(|k| k.key.clone()).collect();
            let real = pk::parse_packet(&foreman(n), &keys);
            let norm = pk::parse_packet(&out.text, &keys);
            assert_eq!(norm.header, real.header, "stage {n}");
            for (a, b) in norm.fields.iter().zip(&real.fields) {
                assert_eq!(a.key, b.key);
                if a.key != "APPROVED BY" {
                    assert_eq!(a.value, b.value, "stage {n} {}", a.key);
                }
            }
            assert_eq!(
                pk::parse_signature(&norm.get("APPROVED BY").unwrap().value),
                pk::Signature::Unsigned
            );
            // Validation sees the same problems as for the real file (unsigned is allowed).
            let rules = |s: &str| -> Vec<String> {
                pk::validate_packet(s, fmt, &ctx(n))
                    .into_iter()
                    .filter(|p| p.rule != "packet.approved-by" && p.rule != "packet.signature-note")
                    .map(|p| format!("{}:{}", p.rule, p.line))
                    .collect()
            };
            assert_eq!(rules(&out.text), rules(&foreman(n)), "stage {n}");
            golden.push(serde_json::json!({
                "stage": n,
                "notes": out.notes,
            }));
        }
        insta::assert_json_snapshot!("normalization_golden_notes", golden);
    }

    /// DR-131: each unsigned and OPEN ITEMS variant, one at a time; a signed value, a value with
    /// continuation lines, and bidi and zero-width characters are left alone (SR-26 shows them).
    #[test]
    fn normalization_variants() {
        let base = unsigned(1);
        for v in [
            "",
            " ",
            "<blank>",
            "<BLANK>",
            "<leave blank>",
            "<leave blank…>",
            "<Leave blank for me to sign>",
            "<sign here…>",
            "<SIGN HERE>",
            "[blank]",
            "[Blank]",
        ] {
            let text = base.replacen("APPROVED BY: <blank>", &format!("APPROVED BY: {v}"), 1);
            let out = normalize_packet(&text);
            assert_eq!(out.text, base, "{v:?}");
            let noted = out.notes.iter().any(|n| n.kind == NoteKind::Unsigned);
            assert_eq!(noted, v != "<blank>", "{v:?}");
            assert_eq!(
                out.notes
                    .iter()
                    .find(|n| n.kind == NoteKind::Unsigned)
                    .map(|n| n.message.as_str()),
                (v != "<blank>").then_some("unsigned")
            );
        }
        for v in ["None", "NONE", "none.", "None.", "NONE."] {
            let text = base.replacen("OPEN ITEMS: none", &format!("OPEN ITEMS: {v}"), 1);
            let out = normalize_packet(&text);
            assert_eq!(out.text, base, "{v:?}");
            assert_eq!(
                out.notes
                    .iter()
                    .map(|n| n.message.as_str())
                    .collect::<Vec<_>>(),
                vec!["OPEN ITEMS normalized to none"]
            );
        }
        // Left alone: a real signature, open items with content, a None with a list under it.
        for keep in [
            base.replacen(
                "APPROVED BY: <blank>",
                "APPROVED BY: Billy Ribeiro on 2026-10-20",
                1,
            ),
            base.replacen("OPEN ITEMS: none", "OPEN ITEMS: none yet, see below", 1),
            base.replacen("OPEN ITEMS: none", "OPEN ITEMS: None\n  - pricing", 1),
            base.replacen("PROJECT: Foreman", "PROJECT: Fore\u{200b}man \u{202e}x", 1),
        ] {
            let out = normalize_packet(&keep);
            assert_eq!(out.text, keep);
            assert!(out.notes.is_empty(), "{:?}", out.notes);
        }
        // A fence that isn't closed isn't a wrapping fence.
        let open_fence = format!("```\n{base}");
        assert_eq!(normalize_packet(&open_fence).text, open_fence);
    }

    /// DR-132, AC-F012-06, AC-F012-10: header through the format's last key's value is the packet
    /// (that value ends at the first blank line), the rest is the attachments; a preamble is kept
    /// apart; two headers or another stage's header refuse, so nothing can be written.
    #[test]
    fn split_paste() {
        let t = tpl();
        let fmt1 = t.packet_format(1).unwrap();
        let last = &fmt1.keys.last().unwrap().key;
        assert_eq!(last, "NEXT");
        let packet = unsigned(1);
        let release = "# Press release\n\nForeman runs the blueprint for you.\n\n## FAQ\n\nQ: Why?\nA: Friction.\n";
        let paste = format!("Here is the packet:\n\n```\n{packet}```\n\n{release}\n\n");
        let norm = normalize_packet(&paste);
        let s = pk::split_paste(&norm.text, fmt1, 1).unwrap();
        assert_eq!(s.before, "Here is the packet:");
        assert_eq!(s.packet, packet, "the packet is exactly the real file");
        assert_eq!(s.attachments.as_deref(), Some(release));
        // The last key's value ends at the first blank line even with more fields after it.
        let with_extra = format!("{packet}\nFILES: a.md\n");
        let s = pk::split_paste(&with_extra, fmt1, 1).unwrap();
        assert_eq!(s.packet, packet);
        assert_eq!(s.attachments.as_deref(), Some("FILES: a.md\n"));
        // Nothing after the packet.
        let s = pk::split_paste(&packet, fmt1, 1).unwrap();
        assert_eq!((s.before.as_str(), s.attachments), ("", None));
        // The last key missing: everything from the header is the packet (validation names it).
        let no_next: String = packet
            .lines()
            .take_while(|l| !l.starts_with("NEXT:"))
            .map(|l| format!("{l}\n"))
            .collect();
        let s = pk::split_paste(&no_next, fmt1, 1).unwrap();
        assert_eq!(s.packet, no_next);
        // Wrong stage, exact words.
        let stage3 = unsigned(3);
        let e = pk::split_paste(&stage3, t.packet_format(2).unwrap(), 2).unwrap_err();
        assert_eq!(
            e,
            SplitError::WrongStage {
                found: 3,
                expected: 2
            }
        );
        assert_eq!(
            e.to_string(),
            "This is the Stage 3 packet; this step expects Stage 2."
        );
        // Two packets (even if both are this stage's).
        let two = format!("{packet}\n{packet}");
        let e = pk::split_paste(&two, fmt1, 1).unwrap_err();
        assert_eq!(e, SplitError::TwoPackets);
        assert_eq!(e.to_string(), "paste one packet at a time");
        let mixed = format!("{packet}\n{}", unsigned(2));
        assert_eq!(
            pk::split_paste(&mixed, fmt1, 1).unwrap_err(),
            SplitError::TwoPackets
        );
        // No header at all.
        assert_eq!(
            pk::split_paste("just some notes\n", fmt1, 1).unwrap_err(),
            SplitError::NoHeader
        );
    }

    /// DR-137, AC-F012-11: the fix request names the actual problems.
    #[test]
    fn fix_request_text() {
        let t = tpl();
        let fmt = t.packet_format(1).unwrap();
        let good = unsigned(1);
        // Missing two fields (one absent, one empty) and OPEN ITEMS not `none`.
        let bad = set_field(&good, "KILL CRITERION", None);
        let bad = set_field(&bad, "WOW MOMENT", Some(""));
        let bad = set_field(&bad, "OPEN ITEMS", Some("pricing still open"));
        let problems = pk::validate_packet(&bad, fmt, &ctx(1));
        let text = super::fix_request_text(&problems);
        assert_eq!(
            text,
            "Your packet is missing: WOW MOMENT, KILL CRITERION, PRESS RELEASE + FAQ. OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block."
        );
        // Only OPEN ITEMS: no "missing" sentence.
        let open_only = good.replacen("OPEN ITEMS: none", "OPEN ITEMS: one thing", 1)
            + "PRESS RELEASE + FAQ: skipped: chat only\n";
        let text = super::fix_request_text(&pk::validate_packet(&open_only, fmt, &ctx(1)));
        assert_eq!(
            text,
            "OPEN ITEMS must be exactly `none`. Re-output the complete packet in one code block."
        );
        // Other problems are listed by their message; warnings are left out.
        let header = good.replacen("Stage 1 → Stage 2", "Stage 1 → Stage 3", 1)
            + "PRESS RELEASE + FAQ: skipped: chat only\n";
        let problems = pk::validate_packet(&header, fmt, &ctx(1));
        assert!(
            problems.iter().any(|p| p.warning),
            "SIGNED VIA is an extra field"
        );
        assert_eq!(
            super::fix_request_text(&problems),
            "Your packet has problems: the header says Stage 3, but the next applicable stage is 2. Re-output the complete packet in one code block."
        );
    }

    proptest! {
        /// Normalization never panics, and is idempotent on any input without a fence or BOM (a
        /// fence inside a removed fence is the next wrapping fence, by design).
        #[test]
        fn normalization_idempotent(s in "(?s).{0,400}") {
            let once = normalize_packet(&s);
            if !s.contains("```") && !s.contains('\u{feff}') {
                let twice = normalize_packet(&once.text);
                prop_assert_eq!(&twice.text, &once.text);
                prop_assert!(twice.notes.is_empty(), "{:?}", twice.notes);
            }
        }
    }
}
