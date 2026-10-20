//! Bracket-token classification (DR-080–DR-082): every bracket token in prompt text falls into
//! exactly one of six classes, by a closed table of exact texts plus two syntactic rules. An unknown
//! token is a template error (`resolver.unknown-token`), never a guess.

use serde::{Deserialize, Serialize};

/// The six classes (`docs/contracts/ipc.md` `TokenClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenClass {
    /// `[APP_NAME]` …
    ProjectVariable,
    /// `[paste the Stage 1 Handoff Packet]` …
    PacketMarker,
    /// `[app]`, `[N-1]`, `[N+1]`
    PathAlias,
    /// `[X]`, `[changes]`, `[N]` (context) …
    RunInput,
    /// `[optional: …]`
    OptionalPart,
    /// `[M#-NN]`, links, `sonnet[1m]`
    Literal,
}

/// The project variables (DR-082 #1–7).
pub const PROJECT_VARIABLES: [&str; 7] = [
    "APP_NAME",
    "MY_ADVANTAGES",
    "BUDGET_TIME",
    "PREFERRED_STACK",
    "ROUGH_IDEA",
    "TARGET_USERS",
    "QUALITY_BAR",
];

/// The packet markers and the stages each names (DR-082 #8–12; `None` = the inspection list).
pub const PACKET_MARKERS: [(&str, &[u8]); 5] = [
    ("paste the Stage 1 Handoff Packet", &[1]),
    ("paste the Stage 2 Handoff Packet", &[2]),
    ("paste the Stage 2 and Stage 3 Handoff Packets", &[2, 3]),
    ("paste the Stage 2 and 3 packets", &[2, 3]),
    ("paste the stage's inspection list", &[]),
];

/// Path aliases (DR-082 #13–15).
pub const PATH_ALIASES: [&str; 3] = ["app", "N-1", "N+1"];

/// Run inputs known to the table (DR-082 #16–21); `N` is the stage-number context input.
pub const RUN_INPUTS: [&str; 6] = [
    "N",
    "X",
    "changes",
    "paste numbers",
    "jurisdictions",
    "list screens",
];

/// Literal tokens (DR-082 #22).
pub const LITERALS: [&str; 1] = ["M#-NN"];

/// One token found in prompt text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    /// The full text including brackets.
    pub text: String,
    /// The inner text.
    pub inner: String,
    /// The class (`None` = unknown: a template error).
    pub class: Option<TokenClass>,
    /// Byte offset in the prompt.
    pub offset: usize,
    /// Byte length.
    pub len: usize,
    /// For an optional part: its ID (`opt-1`); for a token inside one, the enclosing part's ID.
    pub part: Option<String>,
}

/// A scanning problem (DR-033).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    /// `[optional:` inside an optional part.
    NestedOptional(usize),
    /// An optional part without its closing `]`.
    UnclosedOptional(usize),
}

/// Classifies one inner text (DR-080, DR-081). `declared` are the step's run-input names.
#[must_use]
pub fn classify(
    inner: &str,
    prev: Option<char>,
    next: Option<char>,
    declared: &[String],
) -> Option<TokenClass> {
    if next == Some('(') || prev.is_some_and(char::is_alphanumeric) {
        return Some(TokenClass::Literal);
    }
    if PROJECT_VARIABLES.contains(&inner) {
        return Some(TokenClass::ProjectVariable);
    }
    if PACKET_MARKERS.iter().any(|(m, _)| *m == inner) {
        return Some(TokenClass::PacketMarker);
    }
    if PATH_ALIASES.contains(&inner) {
        return Some(TokenClass::PathAlias);
    }
    if RUN_INPUTS.contains(&inner) || declared.iter().any(|d| *d == input_name(inner)) {
        return Some(TokenClass::RunInput);
    }
    if LITERALS.contains(&inner) {
        return Some(TokenClass::Literal);
    }
    None
}

/// A run-input token's binding name: spaces become `_` (DR-025).
#[must_use]
pub fn input_name(inner: &str) -> String {
    inner.replace(' ', "_")
}

/// Scans prompt text for tokens (DR-081: no line break inside, no nested `[`, except optional
/// parts, which may span lines and contain other tokens).
#[must_use]
pub fn scan(text: &str, declared: &[String]) -> (Vec<Token>, Vec<ScanError>) {
    let b = text.as_bytes();
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut i = 0usize;
    let mut opt_n = 0usize;
    // The optional part currently open: (start offset, id).
    let mut open: Option<(usize, String)> = None;
    while i < b.len() {
        if text[i..].starts_with("[optional:") {
            if open.is_some() {
                errors.push(ScanError::NestedOptional(i));
                i += 1;
                continue;
            }
            opt_n += 1;
            open = Some((i, format!("opt-{opt_n}")));
            i += "[optional:".len();
            continue;
        }
        match b[i] {
            b'[' => {
                // A single-line token: up to the next `]` with no `[` or newline before it.
                let rest = &text[i + 1..];
                let end = rest.find([']', '[', '\n']);
                match end {
                    Some(e) if rest.as_bytes()[e] == b']' => {
                        let inner = &rest[..e];
                        let prev = text[..i].chars().next_back();
                        let next = rest[e + 1..].chars().next();
                        tokens.push(Token {
                            text: format!("[{inner}]"),
                            inner: inner.to_owned(),
                            class: classify(inner, prev, next, declared),
                            offset: i,
                            len: e + 2,
                            part: open.as_ref().map(|(_, id)| id.clone()),
                        });
                        i += e + 2;
                    }
                    _ => i += 1,
                }
            }
            b']' if open.is_some() => {
                if let Some((start, id)) = open.take() {
                    tokens.push(Token {
                        text: text[start..=i].to_owned(),
                        inner: text[start + 1..i].to_owned(),
                        class: Some(TokenClass::OptionalPart),
                        offset: start,
                        len: i + 1 - start,
                        part: Some(id),
                    });
                }
                i += 1;
            }
            _ => i += text[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
    if let Some((start, _)) = open {
        errors.push(ScanError::UnclosedOptional(start));
    }
    tokens.sort_by_key(|t| t.offset);
    (tokens, errors)
}
